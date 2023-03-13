use core::time;
use std::process;

use tokio;
use clap::Parser;

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
  
  let mut procs  = Vec::new();
  let mut checks: Vec<String> = Vec::new();
  for e in &opts.specs {
    let proc = runner::Process::parse(e)?;
    match &proc.check() {
      Some(url) => checks.push(url.to_string()),
      None => {},
    };
    println!("----> {}", &proc);
    procs.push(proc);
  }
  
  waiter::wait(&checks, time::Duration::from_secs(10)).await?;
  
  Ok(())
}
