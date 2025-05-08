use std::io;
use std::fmt;

use url;
use reqwest;

#[derive(Debug)]
pub struct AwaitError {
  message: String,
}

impl AwaitError {
  pub fn new(msg: &str) -> AwaitError {
    AwaitError{
      message: msg.to_owned(),
    }
  }
}

impl fmt::Display for AwaitError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

#[derive(Debug)]
pub enum Error {
  IOError(io::Error),
  AwaitError(AwaitError),
  ParseURLError(url::ParseError),
  ReqwestError(reqwest::Error),
  CommandError(String),
  SystemTimeError(std::time::SystemTimeError),
}

impl From<io::Error> for Error {
  fn from(err: io::Error) -> Self {
    Self::IOError(err)
  }
}

impl From<AwaitError> for Error {
  fn from(err: AwaitError) -> Self {
    Self::AwaitError(err)
  }
}

impl From<url::ParseError> for Error {
  fn from(err: url::ParseError) -> Self {
    Self::ParseURLError(err)
  }
}

impl From<reqwest::Error> for Error {
  fn from(err: reqwest::Error) -> Self {
    Self::ReqwestError(err)
  }
}

impl From<std::time::SystemTimeError> for Error {
  fn from(err: std::time::SystemTimeError) -> Self {
    Self::SystemTimeError(err)
  }
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::IOError(err) => err.fmt(f),
      Self::AwaitError(err) => err.fmt(f),
      Self::ParseURLError(err) => err.fmt(f),
      Self::ReqwestError(err) => err.fmt(f),
      Self::CommandError(msg) => write!(f, "{}", msg),
      Self::SystemTimeError(err) => err.fmt(f),
    }
  }
}
