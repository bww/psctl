use std::fs;
use std::process;

use tokio::sync::mpsc;
use futures::executor;

use clap::Parser;
use serde::{Serialize, Deserialize};
use colored::Colorize;

mod waiter;
mod runner;
mod error;
mod config;

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
  let opts = config::Options::parse();
  let (tx, mut rx) = mpsc::channel(1);

  ctrlc::set_handler(move || {
    executor::block_on(async {
      tx.send(()).await
    }).expect("Failed to propagate signal")
  }).expect("Failed to set Ctrl-C handler");

  let procs = if let Some(file) = &opts.file {
    read_procs(file)?.tasks
  }else{
    let mut procs = Vec::new();
    for e in &opts.specs {
      procs.push(runner::Process::parse(e)?);
    }
    procs
  };

  if opts.debug {
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    eprintln!("{}", &format!("====> {} {}, at your service", name, version).bold().cyan());
  }

  if procs.is_empty() {
    Ok(0) // nothing to do
  }else{
    Ok(runner::Pod::new(opts, procs).exec(&mut rx).await?)
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
