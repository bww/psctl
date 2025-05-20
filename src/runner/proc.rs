use core::time;

use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::collections::HashSet;
use std::collections::HashMap;
use std::os::unix::process::CommandExt;

use tokio::process;
use serde::{Serialize, Deserialize};
use colored::Colorize;

use crate::runner;
use crate::runner::error;
use crate::runner::util;

use super::util::join_labels;

fn wait_default() -> time::Duration {
  return time::Duration::from_secs(30)
}

const INVALID_PATH: &str = "<invalid>";

#[derive(Serialize, Deserialize)]
pub struct Taskfile {
  version: u32,
  tasks: Vec<Process>,
}

impl Taskfile {
  pub fn read_from(path: &str) -> runner::Result<Vec<Process>> {
    let data = fs::read_to_string(path)?;
    let specs = serde_yaml::from_str::<Taskfile>(&data)?.tasks;
    let origin = origin_from_path(path)?;
    Ok(specs.iter().map(|e| e.with_origin(&origin)).collect())
  }
}

fn origin_from_path(path: &str) -> Result<String, error::Error> {
  let path = match Path::new(path).file_name() {
    Some(path) => path.to_str().unwrap_or(INVALID_PATH),
    None       => INVALID_PATH,
  };
  Ok(path.to_owned())
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
  pub fn parse(origin: Option<&str>, text: &str) -> runner::Result<Process> {
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
    dup.qualified_label = join_labels(vec![Some(origin), self.label()], util::LABEL_SEP);
    dup
  }

  pub fn key(&self) -> &str {
    if let Some(key) = self.qualified_label() {
      key
    } else if let Some(key) = self.label() {
      key
    } else{
      self.command()
    }
  }

  pub fn key_with_padding(&self, nchar: usize) -> String {
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

  pub fn wait(&self) -> time::Duration {
    return self.wait
  }

  pub fn checks(&self) -> &Vec<String> {
    &self.checks
  }

  pub async fn _exec(&self) -> runner::Result<()> {
    match self._proc()?.wait().await {
      Ok(_stat) => Ok(()),
      Err(err)  => Err(error::ExecError::new(&format!("Could not exec process: {}", err)).into()),
    }
  }

  fn _proc(&self) -> runner::Result<process::Child> {
    match self.task()?.spawn() {
      Ok(proc) => Ok(proc),
      Err(err) => return Err(error::ExecError::new(&format!("Could not spawn process: {}", err)).into()),
    }
  }

  pub fn task(&self) -> runner::Result<process::Command> {
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
    if let Some(l) = self.qualified_label() {
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

pub fn order_procs<'a>(procs: Vec<&'a Process>) -> runner::Result<Vec<&'a Process>> {
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

fn order_procs_sub<'a>(proc: &'a Process, set: &HashMap<String, &'a Process>, run: &mut HashSet<String>, vis: &mut HashSet<String>, path: &mut Vec<&'a Process>) -> runner::Result<Vec<&'a Process>> {
  let key = proc.key();

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
