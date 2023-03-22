use std::fs;
use std::process;

use tokio;
use clap::Parser;
use serde::{Serialize, Deserialize};
use colored::Colorize;

mod waiter;
mod runner;
mod error;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Options {
  #[clap(long, help="Enable debugging mode")]
  pub debug: bool,
  #[clap(long, help="Enable verbose output")]
  pub verbose: bool,
  #[clap(long, help="Load process specifiers from a taskfile")]
  pub file: Option<String>,
  #[clap(help="Process specifiers to manage. When a taskfile is provided, it is preferred:

specs   := <spec1> [... <specN>]
spec    := <label> [<deps>]: <command>[=<check>]
labels  := <label> | <label>, <labels>
label   := /[a-zA-Z0-9]+/
deps    := + <labels>
command := /[^=]+/
check   := any file:// or http(s):// url

$ psctl 'a: echo A' 'b: echo B=file:///tmp/file' 'c +a,b: echo C'
")]
  pub specs: Vec<String>,
}

#[tokio::main]
async fn main() {
  match cmd().await {
    Ok(code)  => process::exit(code),
    Err(err)  => {
      eprintln!("{}", &format!("* * * {}", err).yellow().bold());
      process::exit(1);
    },
  };
}

async fn cmd() -> Result<i32, error::Error> {
  let opts = Options::parse();
  
  let procs = if let Some(file) = &opts.file {
    read_procs(file)?.tasks
  }else{
    let mut procs = Vec::new();
    for e in &opts.specs {
      procs.push(runner::Process::parse(e)?);
    }
    procs
  };
  
  if procs.len() < 1 {
    Ok(0) // nothing to do
  }else{
    Ok(runner::Pod::new(procs).exec().await?)
  }
}

#[derive(Serialize, Deserialize)]
struct SpecFile {
  version: u32,
  tasks: Vec<runner::Process>,
}

fn read_procs(path: &str) -> Result<SpecFile, error::Error> {
  let data = fs::read_to_string(path)?;
  Ok(serde_yaml::from_str(&data)?)
}
