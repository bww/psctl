use std::io;
use std::fmt;

use serde_yaml;

use crate::waiter;
use crate::runner;

#[derive(Debug)]
pub enum Error {
  IOError(io::Error),
  YamlError(serde_yaml::Error),
  WaiterError(waiter::error::Error),
  RunnerError(runner::error::Error),
}

impl From<io::Error> for Error {
  fn from(err: io::Error) -> Self {
    Self::IOError(err)
  }
}

impl From<serde_yaml::Error> for Error {
  fn from(err: serde_yaml::Error) -> Self {
    Self::YamlError(err)
  }
}

impl From<waiter::error::Error> for Error {
  fn from(err: waiter::error::Error) -> Self {
    Self::WaiterError(err)
  }
}

impl From<runner::error::Error> for Error {
  fn from(err: runner::error::Error) -> Self {
    Self::RunnerError(err)
  }
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::IOError(err) => err.fmt(f),
      Self::YamlError(err) => err.fmt(f),
      Self::WaiterError(err) => err.fmt(f),
      Self::RunnerError(err) => err.fmt(f),
    }
  }
}
