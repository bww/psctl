pub mod error;

use core::time;

use std::io;
use std::fmt;
use std::result;
use std::pin::Pin;
use std::collections::HashSet;
use std::collections::HashMap;
use std::os::unix::process::CommandExt;

use tokio::process;
use tokio::sync::mpsc;
use futures::stream;
use futures::stream::TryStreamExt;
use futures::future::FutureExt;
use serde::{Serialize, Deserialize};
use colored::Colorize;

use nix::unistd::Pid;
use nix::sys::signal;
use nix::sys::signal::Signal;

use crate::waiter;

type Result<T> = result::Result<T, error::Error>;

pub struct Pod {
  procs: Vec<Process>,
}

impl Pod {
  pub fn new(procs: Vec<Process>) -> Pod {
    Pod{
      procs: procs,
    }
  }

  pub async fn exec(&self, rx: &mut mpsc::Receiver<()>) -> Result<i32> {
    let ord: Vec<&Process> = order_procs(self.procs.iter().map(|e| e).collect())?;
    let mut tset: Vec<(&Process, process::Command)> = Vec::new();
    let mut pset: Vec<(&Process, process::Child)> = Vec::new();
    for spec in &ord {
      tset.push((spec, spec.task()?));
    }

    // run processes
    let res = match self._exec(&ord, &mut tset, &mut pset, rx).await {
      Ok(code) => code,
      Err(err) => {
        eprintln!("Error: {}", err);
        255
      },
    };

    // explicitly clean up after processes
    Self::cleanup(&mut pset).await?;

    Ok(res)
  }

  pub async fn _exec<'a>(&self, ord: &Vec<&Process>, tset: &mut Vec<(&'a Process, process::Command)>, pset: &mut Vec<(&'a Process, process::Child)>, rx: &mut mpsc::Receiver<()>) -> Result<i32> {
    eprintln!("{}", &format!("====> {}", ord.iter().map(|e| e.key()).collect::<Vec<&str>>().join(", ")).bold());
    for (spec, task) in tset {
      let proc = match task.spawn() {
        Ok(proc) => proc,
        Err(err) => return Err(error::ExecError::new(&format!("Could not run process: {}; because: {}", spec, err)).into()),
      };
      eprintln!("{}", &format!("----> {}", spec).bold());
      pset.push((spec, proc));
      let checks = spec.checks();
      if checks.len() > 0 {
        tokio::select! {
          _ = rx.recv() =>  return Err(error::Error::CanceledError),
          res = waiter::wait(checks, time::Duration::from_secs(10)) =>  match res {
            Ok(_)    => eprintln!("{}", &format!("----> {}: available", spec.key()).bold()),
            Err(err) => return Err(err.into()),
          }
        };
      }
    }

    let code = {
      let mut jobs: Vec<Pin<Box<dyn futures::Future<Output = Result<i32>>>>> = Vec::new();
      for (_, proc) in pset {
        jobs.push(Box::pin(proc.wait().map(|f| match f?.code() {
          Some(code) => Ok(code),
          None => Ok(0),
        })));
      }

      let mut jobs = stream::FuturesUnordered::from_iter(jobs);
      tokio::select! {
        _ = rx.recv() =>  return Err(error::Error::CanceledError),
        res = jobs.try_next() => match res? {
          Some(code) => code,
          None => 0,
        }
      }
    };

    eprintln!("{}", "====> finished".bold());
    Ok(code)
  }

  async fn cleanup(pset: &mut Vec<(&Process, process::Child)>) -> Result<()> {
    // explicitly clean up after remaining processes
    for (spec, proc) in pset {
      if let Some(pid) = proc.id() { // negative-pid addresses the process group
        if let Err(err) = signal::kill(Pid::from_raw(-(pid as i32)), Signal::SIGTERM) {
          eprintln!("{}", &format!("~~~~> {} [failed] {}", spec, err).bold());
          continue; // could not kill this one; move on
        }
        match proc.wait().await {
          Ok(_) => {
            eprintln!("{}", &format!("~~~~> {} [{} killed]", spec, pid).bold());
          },
          Err(err) => match err.kind() {
            io::ErrorKind::InvalidInput => {
              eprintln!("{}", &format!("~~~~> {} [{} ended]", spec, pid).bold());
            },
            _ => return Err(error::Error::IOError(err)),
          },
        };
      }
    }
    Ok(())
  }
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
pub struct Process {
  #[serde(rename(serialize="run", deserialize="run"))]
  command: String,
  #[serde(rename(serialize="name", deserialize="name"))]
  label: Option<String>,
  #[serde(default="Vec::new")]
  deps: Vec<String>,
  #[serde(default="Vec::new")]
  checks: Vec<String>,
}

impl Process {
  pub fn new(label: Option<&str>, cmd: &str, deps: Vec<&str>, url: Option<&str>) -> Process {
    Process{
      command: cmd.to_owned(),
      label: match label {
        Some(label) => Some(label.to_owned()),
        None => None,
      },
      deps: deps.iter().map(|e| e.to_string()).collect(),
      checks: match url {
        Some(url) => vec![url.to_owned()],
        None => vec![],
      },
    }
  }

  // <label> [+ <dep1> [, ...]]: <command>=<check url>
  pub fn parse(text: &str) -> Result<Process> {
    let split: Vec<&str> = text.splitn(2, ":").collect();
    let (label, text) = match split.len() {
      2 => (Some(split[0].trim()), split[1].trim()),
      1 => (None, text),
      _ => return Err(error::ExecError::new(&format!("Invalid process format: {}", text)).into()),
    };

    let (label, deps) = match label {
      Some(label) => {
        let split: Vec<&str> = label.splitn(2, "+").collect();
        match split.len() {
          2 => (Some(split[0].trim()), split[1].trim().split(",").map(|e| e.trim()).collect()),
          1 => (Some(label), Vec::new()),
          _ => return Err(error::ExecError::new(&format!("Invalid process format: {}", text)).into()),
        }
      },
      None => (label, Vec::new()),
    };

    let split: Vec<&str> = text.splitn(2, "=").collect();
    let (cmd, check) = match split.len() {
      2 => (split[0].trim(), Some(split[1].trim())),
      1 => (split[0], None),
      _ => return Err(error::ExecError::new(&format!("Invalid process format: {}", text)).into()),
    };

    Ok(Self::new(label, cmd, deps, check))
  }

  fn key<'a>(&'a self) -> &'a str {
    match self.label() {
      Some(label) => label,
      None => self.command(),
    }
  }

  pub fn label<'a>(&'a self) -> Option<&'a str> {
    match &self.label {
      Some(label) => Some(label),
      None => None,
    }
  }

  pub fn deps<'a>(&'a self) -> &'a Vec<String> {
    &self.deps
  }

  pub fn command<'a>(&'a self) -> &'a str {
    &self.command
  }

  pub fn checks<'a>(&'a self) -> &'a Vec<String> {
    &self.checks
  }

  pub async fn _exec(&self) -> Result<()> {
    match self._proc()?.wait().await {
      Ok(_stat) => Ok(()),
      Err(err)  => Err(error::ExecError::new(&format!("Could not exec process: {}", err)).into()),
    }
  }

  fn _proc(&self) -> Result<process::Child> {
    match self.task()?.spawn() {
      Ok(proc) => Ok(proc),
      Err(err) => return Err(error::ExecError::new(&format!("Could not spawn process: {}", err)).into()),
    }
  }

  fn task(&self) -> Result<process::Command> {
    let mut cmd = std::process::Command::new("sh");
    cmd.process_group(0); // use a process group to clean up children; providing '0' uses this process' id for the group
    cmd.arg("-c").arg(self.command());
    Ok(cmd.into())
  }
}

impl fmt::Display for Process {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut d = String::new();
    if let Some(l) = self.label() {
      d.push_str(&format!("{}: ", l));
    }
    d.push_str(self.command());
    if self.checks.len() > 0 {
      d.push_str(&format!(" ({})", self.checks.join("; ")));
    }
    write!(f, "{}", &d)
  }
}

impl fmt::Debug for Process {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(self, f)
  }
}

fn order_procs<'a>(procs: Vec<&'a Process>) -> Result<Vec<&'a Process>> {
  let mut ord: Vec<&'a Process> = Vec::new();
  let mut vis: HashSet<String> = HashSet::new();
  let mut set: HashMap<String, &'a Process> = HashMap::new();

  for proc in &procs {
    set.insert(proc.key().to_string(), proc);
  }
  for proc in &procs {
    ord.append(&mut order_procs_sub(proc, &set, &mut HashSet::new(), &mut vis)?);
  }

  Ok(ord)
}

fn order_procs_sub<'a>(proc: &'a Process, set: &HashMap<String, &'a Process>, run: &mut HashSet<String>, vis: &mut HashSet<String>) -> Result<Vec<&'a Process>> {
  let key = match proc.label() {
    Some(label) => label,
    None => proc.command(),
  };

  let mut ord: Vec<&'a Process> = Vec::new();
  if !vis.contains(key) {
    for dep in proc.deps() {
      if run.contains(dep) {
        return Err(error::DependencyError::Cycle(format!("{} in {:?}", dep, run)).into());
      }
      run.insert(dep.to_owned());
      match set.get(dep) {
        Some(dep) => ord.append(&mut order_procs_sub(dep, set, run, vis)?),
        None => return Err(error::ExecError::new(&format!("Unknown dependency: {}", dep)).into()),
      };
      run.remove(dep);
    }
    vis.insert(key.to_owned());
    ord.push(proc);
  }

  Ok(ord)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_resolve_deps() {
    let p1 = Process::new(Some("p1"), "proc 1", vec![], None);
    let p2 = Process::new(Some("p2"), "proc 2", vec!["p1"], None);
    let p3 = Process::new(Some("p3"), "proc 3", vec!["p2", "p1"], None);
    let p4 = Process::new(Some("p4"), "proc 4", vec!["p1"], None);

    match order_procs(vec![&p2, &p3, &p1]) {
      Ok(res)  => assert_eq!(vec![&p1, &p2, &p3], res),
      Err(err) => panic!("{}", err),
    };
    match order_procs(vec![&p2, &p4, &p3, &p1]) {
      Ok(res)  => assert_eq!(vec![&p1, &p2, &p4, &p3], res),
      Err(err) => panic!("{}", err),
    };

    // circular
    let p5 = Process::new(Some("p5"), "proc 5", vec!["p6"], None);
    let p6 = Process::new(Some("p6"), "proc 6", vec!["p5"], None);

    match order_procs(vec![&p5, &p6]) {
      Ok(res)  => panic!("Cannot succeed!"),
      Err(err) => match err {
        error::Error::DependencyError(error::DependencyError::Cycle(msg)) => {}, // expected error
        _ => panic!("Unexpected error: {}", err),
      },
    };
  }

}
