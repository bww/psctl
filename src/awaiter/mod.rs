pub mod error;

use core::time;
use std::pin::Pin;
use std::time::SystemTime;
use std::result;

use futures::Future;
use futures::future::try_join_all;
use tokio::time::sleep;

use url;

type Result<T> = result::Result<T, error::Error>;

pub struct Awaiter {
  urls: Vec<String>,
}

impl Awaiter {
  pub fn new(urls: Vec<String>) -> Awaiter {
    Awaiter{
      urls: urls,
    }
  }
  
  pub async fn check(&self, timeout: time::Duration) -> Result<()> {
    let deadline = SystemTime::now() + timeout;
    let mut jobs: Vec<Pin<Box<dyn futures::Future<Output = Result<()>>>>> = Vec::new();
    for base in &self.urls {
      let url = url::Url::parse(base)?;
      match url.scheme() {
        "http" | "https" => jobs.push(Box::pin(self.wait_http(base, deadline))),
        "file"           => jobs.push(Box::pin(self.wait_file(base, deadline))),
        _                => return Err(error::AwaitError::new(&format!("Scheme '{}' not supported: {}", url.scheme(), base)).into())
      }
    }
    try_join_all(jobs).await?;
    Ok(())
  }
  
  async fn wait_http(&self, url: &str, deadline: SystemTime) -> Result<()> {
    let wait = time::Duration::from_secs(1);
    loop {
      let before = SystemTime::now();
      println!(">>> Polling: {}", url);
      if match reqwest::Client::new().get(url).timeout(deadline.duration_since(SystemTime::now())?).send().await {
        Ok(rsp)  => rsp.status().is_success(),
        Err(err) => false,
      } {
        println!("... OK! {}", url);
        return Ok(()); // success
      }
      let after = SystemTime::now();
      if after + wait >= deadline {
        return Err(error::AwaitError::new(&format!("Deadline exceeded: {}", url)).into());
      } else {
        let elapsed = after.duration_since(before)?;
        if elapsed < wait {
          println!("... Waiting: {:?}", wait - elapsed);
          sleep(wait - elapsed).await;
        }
      }
    }
  }
  
  async fn wait_file(&self, url: &str, deadline: SystemTime) -> Result<()> {
    println!(">>> Starting: {}", url);
    let rsp = reqwest::get(url).await?;
    let status = rsp.status();
    if status.is_success() {
      Ok(())
    }else{
      Err(error::AwaitError::new(&format!("Invalid status code: {}", status)).into())
    }
  }
}
