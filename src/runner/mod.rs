pub mod error;

use std::fmt;
use std::result;
use std::pin::Pin;
use std::collections::HashSet;

use tokio::process;
use tokio::sync::mpsc;
use futures::Future;
use futures::stream;
use futures::stream::TryStreamExt;

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
  
  pub async fn exec(&self) -> Result<()> {
    let mut jobs: Vec<Pin<Box<dyn futures::Future<Output = Result<()>>>>> = Vec::new();
    for proc in &self.procs {
      println!("----> {}", proc);
      jobs.push(Box::pin(proc.exec()));
    }
    
    let mut jobs = stream::FuturesUnordered::from_iter(jobs);
    let _ = jobs.try_next().await?;
    
    Ok(())
  }
}

pub struct Process {
  command: String,
  label: Option<String>,
  deps: Vec<String>,
  check: Option<String>,
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
      check: match url {
        Some(url) => Some(url.to_owned()),
        None => None,
      },
    }
  }
  
  // <label>: <command>=<check url> (<dep1>,<dep2>)
  pub fn parse(text: &str) -> Result<Process> {
    let split: Vec<&str> = text.splitn(2, ":").collect();
    let (label, text) = match split.len() {
      2 => (Some(split[0].trim()), split[1].trim()),
      1 => (None, text),
      _ => return Err(error::ExecError::new(&format!("Invalid process format: {}", text)).into()),
    };
    
    let split: Vec<&str> = text.splitn(2, "=").collect();
    let (cmd, check) = match split.len() {
      2 => (split[0].trim(), Some(split[1].trim())),
      1 => (split[0], None),
      _ => return Err(error::ExecError::new(&format!("Invalid process format: {}", text)).into()),
    };
    
    Ok(Self::new(label, cmd, Vec::new(), check))
    // let (cmd, check) = match split.len() {
    //   2 => Ok(Self::new(split[0], Some(split[1]))),
    //   1 => Ok(Self::new(split[0], None)),
    //   _ => Err(error::ExecError::new(&format!("Invalid process format: {}", text)).into()),
    // }
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
  
  pub fn check<'a>(&'a self) -> Option<&'a str> {
    match &self.check {
      Some(url) => Some(url),
      None => None,
    }
  }
  
  pub async fn exec(&self) -> Result<()> {
    let mut proc = match process::Command::new("sh").arg("-c").arg(&self.command).spawn() {
      Ok(proc) => proc,
      Err(err) => return Err(error::ExecError::new(&format!("Could not spawn process: {}", err)).into()),
    };
    
    let stat = match proc.wait().await {
      Ok(stat) => stat,
      Err(err) => return Err(error::ExecError::new(&format!("Could not exec process: {}", err)).into()),
    };
    
    Ok(())
  }
}

impl fmt::Display for Process {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut d = String::new();
    match self.label() {
      Some(l) => d.push_str(&format!("{}: ", l)),
      None    => {},
    }
    d.push_str(self.command());
    match self.check() {
      Some(u) => d.push_str(&format!(" ({})", u)),
      None    => {},
    }
    write!(f, "{}", &d)
  }
}

impl fmt::Debug for Process {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(self, f)
  }
}

fn order_procs(procs: Vec<Process>) -> Result<Vec<Process>> {
  let mut ord: Vec<Process> = Vec::new();
  let mut sub: HashSet<String> = HashSet::new();
  
  for proc in procs {
    ord.append(order_procs_sub(&proc, &sub)?);
  }
  
  Ok(ord)
}

fn order_procs_sub(proc: &Process, sub: &mut HashSet<String>) -> Result<Vec<Process>> {
  let key = match proc.label() {
    Some(label) => label,
    None => proc.command(),
  };
  
  if sub.contains(key) {
    return Ok(Vec::new());
  }
  
  let mut ord: Vec<Process> = Vec::new();
  for dep in proc.deps() {
    ord.append(order_procs_sub(dep
  }
  
}

#[cfg(test)]
mod tests {
  use super::*;
  
  #[test]
  fn test_resolve_deps() {
    // assert_eq!(Ok(chrono::Duration::seconds(1)), parse_duration("1s"));
  }
  
}
