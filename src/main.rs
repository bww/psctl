use std::process;

use tokio::sync::mpsc;
use futures::executor;

use clap::Parser;
use colored::Colorize;

mod waiter;
mod runner;
mod error;
mod config;
mod colorwheel;

use crate::runner::proc;

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

  let mut procs = Vec::new();
  // load taskfiles in the order they are specified...
  for file in &opts.files {
    procs.append(&mut proc::Taskfile::read_from(file)?);
  }
  // ...then load specs, if any, provided on the command line
  procs.append(&mut read_specs(&opts.specs)?);

  if opts.debug() {
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

fn read_specs(specs: &Vec<String>) -> Result<Vec<proc::Process>, error::Error> {
  let mut procs = Vec::new();
  for e in specs {
    procs.push(proc::Process::parse(Some("STDIN"), e)?);
  }
  Ok(procs)
}
