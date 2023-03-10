pub mod error;

use core::time;
use std::time::SystemTime;
use tokio::time::sleep;

use url;

pub struct Awaiter {
  urls: Vec<String>,
}

impl Awaiter {
  pub fn new(urls: Vec<String>) -> Awaiter {
    Awaiter{
      urls: urls,
    }
  }
  
  pub async fn check(&self, timeout: time::Duration) -> Result<(), error::Error> {
    let deadline = SystemTime::now() + timeout;
    for base in &self.urls {
      let url = url::Url::parse(base)?;
      match url.scheme() {
        "http" | "https" => self.wait_http(base, deadline).await?,
        "file"           => self.wait_file(base, deadline).await?,
        _                => return Err(error::AwaitError::new(&format!("Scheme '{}' not supported: {}", url.scheme(), base)).into())
      }
    }
    Ok(())
  }
  
  async fn wait_http(&self, url: &str, deadline: SystemTime) -> Result<(), error::Error> {
    let wait = time::Duration::from_secs(1);
    loop {
      let before = SystemTime::now();
      println!(">>> Polling: {}", url);
      if match reqwest::Client::new().get(url).timeout(deadline.duration_since(SystemTime::now())?).send().await {
        Ok(rsp)  => rsp.status().is_success(),
        Err(err) => false,
      } {
        return Ok(()); // success
      }
      let after = SystemTime::now();
      if after + wait >= deadline {
        return Err(error::AwaitError::new(&format!("Deadline exceeded: {}", url)).into());
      } else {
        let elapsed = after.duration_since(before)?;
        if elapsed < wait {
          println!("WAIT: {:?}", wait - elapsed);
          sleep(wait - elapsed).await;
        }
      }
    }
  }
  
  async fn wait_file(&self, url: &str, deadline: SystemTime) -> Result<(), error::Error> {
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
