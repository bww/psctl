use std::fs;
use std::process;

use tokio;
use clap::Parser;
use serde::{Serialize, Deserialize};

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
  #[clap(long, help="Use process specification file")]
  pub file: Option<String>,
  #[clap(help="Processes to manage")]
  pub specs: Vec<String>,
}

#[tokio::main]
async fn main() {
  match cmd().await {
    Ok(_)     => {},
    Err(err)  => {
      eprintln!("* * * {}", err);
      process::exit(1);
    },
  };
}

async fn cmd() -> Result<(), error::Error> {
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
    Ok(()) // nothing to do
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
