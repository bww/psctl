use core::time;
use std::process;

use tokio;
use clap::Parser;

mod awaiter;

use crate::awaiter::error;

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
  awaiter::wait(&opts.specs, time::Duration::from_secs(10)).await
}
