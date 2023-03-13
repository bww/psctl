pub mod error;

use std::fmt;
use std::result;

use tokio::process;

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
  
  pub fn exec() -> Result<()> {
    Ok(())
  }
}

pub struct Process {
  command: String,
  check: Option<String>,
}

impl Process {
  pub fn new(cmd: &str, url: Option<&str>) -> Process {
    Process{
      command: cmd.to_owned(),
      check: match url {
        Some(url) => Some(url.to_owned()),
        None => None,
      },
    }
  }
  
  pub fn parse(text: &str) -> Result<Process> {
    let split: Vec<&str> = text.splitn(2, "=").collect();
    match split.len() {
      2 => Ok(Self::new(split[0], Some(split[1]))),
      1 => Ok(Self::new(split[0], None)),
      _ => Err(error::ExecError::new(&format!("Invalid process format: {}", text)).into()),
    }
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
    let mut proc = match process::Command::new("echo")
      .arg("hello")
      .arg("world")
      .spawn() {
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
    match self.check() {
      Some(url) => write!(f, "{} ({})", self.command(), url),
      None      => write!(f, "{}", self.command()),
    }
  }
}

impl fmt::Debug for Process {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.check() {
      Some(url) => write!(f, "{} ({})", self.command(), url),
      None      => write!(f, "{}", self.command()),
    }
  }
}
