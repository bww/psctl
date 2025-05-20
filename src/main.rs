use std::fs;
use std::path::Path;
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
mod colorwheel;

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
    procs.append(&mut read_procs(file)?);
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

#[derive(Serialize, Deserialize)]
struct SpecFile {
  version: u32,
  tasks: Vec<runner::Process>,
}

fn read_procs(path: &str) -> Result<Vec<runner::Process>, error::Error> {
  let data = fs::read_to_string(path)?;
  let specs = serde_yaml::from_str::<SpecFile>(&data)?.tasks;
  let origin = origin_from_path(path)?;
  Ok(specs.iter().map(|e| e.with_origin(&origin)).collect())
}

fn read_specs(specs: &Vec<String>) -> Result<Vec<runner::Process>, error::Error> {
  let mut procs = Vec::new();
  for e in specs {
    procs.push(runner::Process::parse(Some("STDIN"), e)?);
  }
  Ok(procs)
}

const INVALID_PATH: &str = "<invalid>";

fn origin_from_path(path: &str) -> Result<String, error::Error> {
  let path = match Path::new(path).file_name() {
    Some(path) => path.to_str().unwrap_or(INVALID_PATH),
    None       => INVALID_PATH,
  };
  Ok(path.to_owned())
}
