use std::io;
use std::fmt;

use crate::waiter;

#[derive(Debug, PartialEq, Eq)]
pub struct ExecError {
  message: String,
}

impl ExecError {
  pub fn new(msg: &str) -> ExecError {
    ExecError{
      message: msg.to_owned(),
    }
  }
}

impl fmt::Display for ExecError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DependencyError {
  Cycle(String),
}

impl fmt::Display for DependencyError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Cycle(msg) => write!(f, "Cycle: {}", msg),
    }
  }
}

#[derive(Debug)]
pub enum Error {
  IOError(io::Error),
  WaiterError(waiter::error::Error),
  ExecError(ExecError),
  DependencyError(DependencyError),
  CanceledError,
  NeverInitializedError(String),
}

impl From<io::Error> for Error {
  fn from(err: io::Error) -> Self {
    Self::IOError(err)
  }
}

impl From<waiter::error::Error> for Error {
  fn from(err: waiter::error::Error) -> Self {
    Self::WaiterError(err)
  }
}

impl From<ExecError> for Error {
  fn from(err: ExecError) -> Self {
    Self::ExecError(err)
  }
}

impl From<DependencyError> for Error {
  fn from(err: DependencyError) -> Self {
    Self::DependencyError(err)
  }
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::IOError(err) => err.fmt(f),
      Self::WaiterError(err) => err.fmt(f),
      Self::ExecError(err) => err.fmt(f),
      Self::DependencyError(err) => err.fmt(f),
      Self::CanceledError => write!(f, "Canceled"),
      Self::NeverInitializedError(key) => write!(f, "{}: exited before it became available", key),
    }
  }
}
