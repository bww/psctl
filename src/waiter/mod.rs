pub mod error;

use core::time;
use std::path;
use std::pin::Pin;
use std::time::SystemTime;
use std::result;

use futures::Future;
use colored::Colorize;
use futures::future::try_join_all;
use tokio::time::sleep;
use humantime::format_duration;

use crate::config;

type Result<T> = result::Result<T, error::Error>;

pub struct Config {
  pub key: Option<String>,
  pub verbose: bool,
}

impl Config {
  pub fn from_options(key: String, opts: &config::Options) -> Self {
    Self{
      key: Some(key),
      verbose: opts.verbose(),
    }
  }
}

pub async fn wait_config(conf: &Config, urls: &Vec<String>, timeout: time::Duration) -> Result<()> {
  if conf.verbose {
    for u in urls {
      match &conf.key {
        Some(key) => eprintln!("{}", &format!("----> {}: ... {}", key, u).italic()),
        None      => eprintln!("{}", &format!("----> ... {}", u).italic()),
      }
    }
  }
  try_join_all(wait_jobs(urls, timeout)?).await?;
  Ok(())
}

pub fn wait_jobs<'a>(urls: &'a Vec<String>, timeout: time::Duration) -> Result<Vec<Pin<Box<dyn futures::Future<Output = Result<()>> + 'a>>>> {
  let deadline = SystemTime::now() + timeout;
  let mut jobs: Vec<Pin<Box<dyn futures::Future<Output = Result<()>>>>> = Vec::new();
  for base in urls {
    let url = url::Url::parse(base)?;
    let scheme = url.scheme();
    match scheme {
      "http" | "https" => jobs.push(Box::pin(wait_http(base, deadline))),
      "file"           => jobs.push(Box::pin(wait_file(base, deadline))),
      "shell"          => jobs.push(Box::pin(wait_shell(base, deadline))),
      _                => return Err(error::AwaitError::new(&format!("Scheme '{}' not supported: {}", scheme, base)).into())
    }
  }
  Ok(jobs)
}

async fn wait_fn<F>(url: &str, deadline: SystemTime, func: F) -> Result<()>
where
  F: Fn(String, time::Duration) -> Pin<Box<dyn Future<Output = Result<bool>>>>
{
  let wait = time::Duration::from_secs(1);
  loop {
    let before = SystemTime::now();
    if (func(url.to_string(), deadline.duration_since(SystemTime::now())?).await).unwrap_or_default() {
      return Ok(()); // success
    }
    let after = SystemTime::now();
    if after + wait >= deadline {
      return Err(error::AwaitError::new(&format!("Deadline exceeded ({} elapsed): {}", format_duration(after.duration_since(before)?), url)).into());
    } else {
      let elapsed = after.duration_since(before)?;
      if elapsed < wait {
        sleep(wait - elapsed).await;
      }
    }
  }
}

async fn wait_http(url: &str, deadline: SystemTime) -> Result<()> {
  wait_fn(url, deadline, |u, t| {
    Box::pin(async move {
      match reqwest::Client::new().get(u).timeout(t).send().await {
        Ok(rsp)  => Ok(rsp.status().is_success()),
        Err(err) => Err(err.into()),
      }
    })
  }).await
}

async fn wait_file(url: &str, deadline: SystemTime) -> Result<()> {
  wait_fn(url, deadline, |u, _| {
    Box::pin(async move {
      match url::Url::parse(&u) {
        Ok(u)    => Ok(path::Path::new(u.path()).exists()),
        Err(err) => Err(err.into()),
      }
    })
  }).await
}

async fn wait_shell(url: &str, deadline: SystemTime) -> Result<()> {
  wait_fn(url, deadline, |u, _| {
    Box::pin(async move {
      let cmd = match url::Url::parse(&u) {
        Ok(u) => {
          let s = u.as_str();
          match s.find(":") {
            Some(n) => s[n+1..].to_owned(),
            None    => s.to_owned(),
          }
        },
        Err(err) => return Err(err.into()),
      };
      let status = std::process::Command::new("sh")
        .arg("-c").arg(cmd)
        .spawn()?
        .wait()?;
      match status.success() {
        true  => Ok(true),
        _     => Err(error::Error::CommandError("Exited with error".to_string())),
      }
    })
  }).await
}
