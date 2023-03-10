pub mod error;

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
  
  pub async fn check(&self) -> Result<(), error::Error> {
    for base in &self.urls {
      let url = url::Url::parse(base)?;
      match url.scheme() {
        "http" | "https" => self.wait_http(base).await?,
        // "file"           => self.wait_file(url).await?,
        _                => return Err(error::AwaitError::new(&format!("Scheme '{}' not supported: {}", url.scheme(), base)).into())
      }
    }
    Ok(())
  }
  
  async fn wait_http(&self, url: &str) -> Result<(), error::Error> {
    let rsp = reqwest::get(url).await?;
    let status = rsp.status();
    if status.is_success() {
      Ok(())
    }else{
      Err(error::AwaitError::new(&format!("Invalid status code: {}", status)).into())
    }
  }
  
  async fn wait_file(&self, url: &str) -> Result<(), error::Error> {
    let rsp = reqwest::get(url).await?;
    let status = rsp.status();
    if status.is_success() {
      Ok(())
    }else{
      Err(error::AwaitError::new(&format!("Invalid status code: {}", status)).into())
    }
  }
}
