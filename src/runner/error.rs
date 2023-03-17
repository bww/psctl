use std::io;
use std::fmt;

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

#[derive(Debug)]
pub enum Error {
  IOError(io::Error),
  ExecError(ExecError),
}

impl From<io::Error> for Error {
  fn from(err: io::Error) -> Self {
    Self::IOError(err)
  }
}

impl From<ExecError> for Error {
  fn from(err: ExecError) -> Self {
    Self::ExecError(err)
  }
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::IOError(err) => err.fmt(f),
      Self::ExecError(err) => err.fmt(f),
    }
  }
}
