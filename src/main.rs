use core::time;

use tokio;

mod awaiter;

#[tokio::main]
async fn main() {
  let u = vec![
    "http://www.google.com",
    "http://www.google.com",
    "http://localhost/foobar",
    "file:///Users/brian/Development/Products/psctl/Cargo.lock",
  ];
  match awaiter::wait(u, time::Duration::from_secs(10)).await {
    Ok(())   => println!("Ok"),
    Err(err) => panic!("Failed: {}", err),
  }
}
