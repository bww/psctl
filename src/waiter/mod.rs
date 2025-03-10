pub mod error;

use core::time;
use std::path;
use std::pin::Pin;
use std::time::SystemTime;
use std::result;

use url;
use futures::Future;
use futures::future::try_join_all;
use tokio::time::sleep;
use humantime::format_duration;

type Result<T> = result::Result<T, error::Error>;

pub async fn wait(urls: &Vec<String>, timeout: time::Duration) -> Result<()> {
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
    if match func(url.to_string(), deadline.duration_since(SystemTime::now())?).await {
      Ok(res) => res,
      Err(_)  => false, // maybe log this?
    } {
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
