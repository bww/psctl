pub mod error;
pub mod util;

use core::time;

use std::io;
use std::fmt;
use std::cmp::min;
use std::result;
use std::pin::Pin;
use std::process::Stdio;
use std::collections::HashSet;
use std::collections::HashMap;
use std::os::unix::process::CommandExt;

use tokio::io::{BufReader, AsyncBufReadExt};
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
use crate::config;
use crate::colorwheel;

type Result<T> = result::Result<T, error::Error>;

pub struct Pod {
  opts:  config::Options,
  procs: Vec<Process>,
  wheel: colorwheel::Wheel,
}

impl Pod {
  pub fn new(opts: config::Options, procs: Vec<Process>) -> Pod {
    Pod{
      opts: opts,
      procs: procs,
      wheel: colorwheel::Wheel::default(),
    }
  }

  pub fn overlay(&self, procs: Vec<Process>) -> Pod {
    Pod{
      opts: self.opts.clone(),
      procs: [self.procs.clone(), procs].concat(),
      wheel: self.wheel.clone(),
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
    let res = self._exec(&ord, &mut tset, &mut pset, rx).await;
    // explicitly clean up after processes
    Self::cleanup(&self.opts, &mut pset).await?;
    // return the result
    res
  }

  pub async fn _exec<'a>(&self, ord: &Vec<&Process>, tset: &mut Vec<(&'a Process, process::Command)>, pset: &mut Vec<(&'a Process, process::Child)>, rx: &mut mpsc::Receiver<()>) -> Result<i32> {
    if !self.opts.quiet() {
      eprintln!("{}", &format!("====> {}", ord.iter().map(|e| e.key()).collect::<Vec<&str>>().join(", ")).bold());
    }
    let maxkey: usize = min(32, tset.iter().map(|(spec, _)| spec.key().len()).max().unwrap_or(0));

    let mut i: usize = 0;
    for (spec, task) in tset {
      let mut proc = match task.spawn() {
        Ok(proc) => proc,
        Err(err) => return Err(error::ExecError::new(&format!("Could not run process: {}; because: {}", spec, err)).into()),
      };

      let mut stdout = match proc.stdout.take() {
        Some(stdout) => BufReader::new(stdout).lines(),
        None         => return Err(error::ExecError::new(&format!("Could not configure process STDOUT: {}", spec)).into()),
      };
      let mut stderr = match proc.stderr.take() {
        Some(stderr) => BufReader::new(stderr).lines(),
        None         => return Err(error::ExecError::new(&format!("Could not configure process STDERR: {}", spec)).into()),
      };

      let key_stdout = match self.opts.prefix() {
        true  => Some(format!("[ {} ]", self.wheel.colorize(i, spec.key_with_padding(maxkey)))),
        false => None,
      };
      tokio::spawn(async move {
        while let Some(line) = stdout.next_line().await.expect("Could not read from STDOUT") {
          if let Some(pfx) = &key_stdout {
            println!("{} {}", pfx, line);
          }
        }
      });

      let key_stderr = match self.opts.prefix() {
        true  => Some(format!("[ {} ]", self.wheel.colorize(i, spec.key_with_padding(maxkey)))),
        false => None,
      };
      tokio::spawn(async move {
        while let Some(line) = stderr.next_line().await.expect("Could not read from STDERR") {
          if let Some(pfx) = &key_stderr {
            println!("{} {}", pfx, line);
          }
        }
      });

      if !self.opts.quiet() {
        eprintln!("{}", &format!("----> {}", spec).bold());
      }
      let checks = spec.checks();
      let res = if !checks.is_empty(){
        let waitconf = waiter::Config::from_options(spec.key().to_owned(), &self.opts);
        tokio::select! {
          _   = rx.recv()   => Err(error::Error::CanceledError),
          _   = proc.wait() => Err(error::Error::NeverInitializedError(spec.key().to_owned())),
          res = waiter::wait_config(&waitconf, checks, spec.wait) =>  match res {
            Ok(_)    => Ok((spec.key(), false)),
            Err(err) => Err(err.into()),
          }
        }
      } else {
        Ok((spec.key(), true)) // immediately available if we have no checks
      };
      pset.push((spec, proc));
      match res {
        Ok((key, dflt))  => if (!dflt && !self.opts.quiet()) || self.opts.verbose() {
          eprintln!("{}", &format!("----> {}: available", key).bold());
        },
        Err(err) => return Err(err),
      }

      i = i + 1;
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
        res = jobs.try_next() => (res?).unwrap_or_default(),
      }
    };

    if !self.opts.quiet() {
      eprintln!("{}", "====> finished".bold());
    }
    Ok(code)
  }

  async fn cleanup(opts: &config::Options, pset: &mut Vec<(&Process, process::Child)>) -> Result<()> {
    // explicitly clean up after remaining processes
    for (spec, proc) in pset {
      if let Some(pid) = proc.id() { // negative-pid addresses the process group
        if let Err(err) = signal::kill(Pid::from_raw(-(pid as i32)), Signal::SIGTERM) {
          eprintln!("{}", &format!("~~~~> {} [failed] {}", spec, err).bold());
          continue; // could not kill this one, it has possibly already exited; move on
        }
        match proc.wait().await {
          Ok(_) => {
            if !opts.quiet() {
              eprintln!("{}", &format!("~~~~> {} [{} killed]", spec, pid).bold());
            }
          },
          Err(err) => match err.kind() {
            io::ErrorKind::InvalidInput => {
              if !opts.quiet() {
                eprintln!("{}", &format!("~~~~> {} [{} ended]", spec, pid).bold());
              }
            },
            _ => return Err(error::Error::IOError(err)),
          },
        };
      }
    }
    Ok(())
  }
}

fn wait_default() -> time::Duration {
  return time::Duration::from_secs(30)
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct Process {
  #[serde(rename(serialize="run", deserialize="run"))]
  command: String,
  #[serde()]
  origin: Option<String>,
  #[serde(rename(serialize="name", deserialize="name"))]
  label: Option<String>,
  qualified_label: Option<String>,
  #[serde(default="Vec::new")]
  deps: Vec<String>,
  #[serde(default="Vec::new")]
  checks: Vec<String>,
  #[serde(with = "humantime_serde", default="wait_default")]
  wait: time::Duration,
  #[serde(default="HashMap::new")]
  env: HashMap<String, String>,
}

impl Process {
  pub fn new(origin: Option<&str>, label: Option<&str>, cmd: &str, deps: Vec<&str>, url: Option<&str>) -> Process {
    Process{
      origin: origin.map(|origin| origin.to_owned()),
      command: cmd.to_owned(),
      label: label.map(|label| label.to_owned()),
      deps: deps.iter().map(|e| e.to_string()).collect(),
      checks: match url {
        Some(url) => vec![url.to_owned()],
        None => vec![],
      },
      wait: wait_default(),
      env: HashMap::new(),
      qualified_label: util::join_labels(vec![origin, label], '/'),
    }
  }

  // <label> [+ <dep1> [, ...]]: <command>=<check url>
  pub fn parse(origin: Option<&str>, text: &str) -> Result<Process> {
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

    Ok(Self::new(origin, label, cmd, deps, check))
  }

  pub fn with_origin(&self, origin: &str) -> Process {
    let mut dup = self.clone();
    dup.origin = Some(origin.to_owned());
    dup
  }

  fn key(&self) -> &str {
    if let Some(qlabel) = self.qualified_label() {
      qlabel
    } else if let Some(label) = self.label() {
      label
    } else{
      self.command()
    }
  }

  fn key_with_padding(&self, nchar: usize) -> String {
    let mut key = self.key().to_owned();
    let l = key.len();
    if l > nchar {
      key.truncate(nchar);
    } else {
      key.push_str(&(" ".repeat(nchar - l)));
    }
    key
  }

  pub fn label(&self) -> Option<&str> {
    match &self.label {
      Some(label) => Some(label),
      None => None,
    }
  }

  pub fn qualified_label(&self) -> Option<&str> {
    match &self.qualified_label {
      Some(label) => Some(label),
      None => None,
    }
  }

  pub fn deps(&self) -> &Vec<String> {
    &self.deps
  }

  pub fn command(&self) -> &str {
    &self.command
  }

  pub fn checks(&self) -> &Vec<String> {
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
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.process_group(0); // use a process group to clean up children; providing '0' uses this process' id for the group
    for (key, val) in self.env.iter() {
      cmd.env(key, val);
    }
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
    match self.checks.len() {
      0 => {},
      1 => d.push_str(&format!(" ({})", self.checks[0])),
      n => d.push_str(&format!(" ({})", &format!("{} checks", n).italic())),
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
  let mut path: Vec<&'a Process> = Vec::new();

  for proc in &procs {
    set.insert(proc.key().to_string(), proc);
  }
  for proc in &procs {
    ord.append(&mut order_procs_sub(proc, &set, &mut HashSet::new(), &mut vis, &mut path)?);
  }

  Ok(ord)
}

fn order_procs_sub<'a>(proc: &'a Process, set: &HashMap<String, &'a Process>, run: &mut HashSet<String>, vis: &mut HashSet<String>, path: &mut Vec<&'a Process>) -> Result<Vec<&'a Process>> {
  let key = match proc.label() {
    Some(label) => label,
    None => proc.command(),
  };

  let mut ord: Vec<&'a Process> = Vec::new();
  if !vis.contains(key) {
    for dep in proc.deps() {
      path.push(proc);
      if run.contains(dep) {
        return Err(error::DependencyError::Cycle(format!("{}", path.iter().map(|e| e.key()).collect::<Vec<&str>>().join(" → "))).into());
      }
      run.insert(dep.to_owned());
      match set.get(dep) {
        Some(dep) => ord.append(&mut order_procs_sub(dep, set, run, vis, path)?),
        None => return Err(error::ExecError::new(&format!("Unknown dependency: {}", dep)).into()),
      };
      run.remove(dep);
      path.pop();
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
    let p1 = Process::new(None, Some("p1"), "proc 1", vec![], None);
    let p2 = Process::new(None, Some("p2"), "proc 2", vec!["p1"], None);
    let p3 = Process::new(None, Some("p3"), "proc 3", vec!["p2", "p1"], None);
    let p4 = Process::new(None, Some("p4"), "proc 4", vec!["p1"], None);

    match order_procs(vec![&p2, &p3, &p1]) {
      Ok(res)  => assert_eq!(vec![&p1, &p2, &p3], res),
      Err(err) => panic!("{}", err),
    };
    match order_procs(vec![&p2, &p4, &p3, &p1]) {
      Ok(res)  => assert_eq!(vec![&p1, &p2, &p4, &p3], res),
      Err(err) => panic!("{}", err),
    };

    // circular
    let p5 = Process::new(None, Some("p5"), "proc 5", vec!["p6"], None);
    let p6 = Process::new(None, Some("p6"), "proc 6", vec!["p5"], None);

    match order_procs(vec![&p5, &p6]) {
      Ok(_)    => panic!("Cannot succeed!"),
      Err(err) => match err {
        error::Error::DependencyError(error::DependencyError::Cycle(_)) => {}, // expected error
        _ => panic!("Unexpected error: {}", err),
      },
    };
  }

}
