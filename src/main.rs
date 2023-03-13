use core::time;

use tokio;

mod awaiter;

#[tokio::main]
async fn main() {
  let u = vec![
    "http://www.google.com".to_owned(),
    "http://www.google.com".to_owned(),
    "http://localhost/foobar".to_owned(),
    "file:///Users/brian/Development/Products/psctl/Cargo.lock".to_owned(),
  ];
  match awaiter::check(&u, time::Duration::from_secs(10)).await {
    Ok(())   => println!("Ok"),
    Err(err) => panic!("Failed: {}", err),
  }
}
