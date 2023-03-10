use tokio;

mod awaiter;

#[tokio::main]
async fn main() {
  let a = awaiter::Awaiter::new(vec![
    "http://www.google.com".to_owned(),
    "file:///Users/brian/Development/Products/psctl/Cargo.lock".to_owned(),
  ]);
  match a.check().await {
    Ok(())   => println!("Ok"),
    Err(err) => panic!("Failed: {}", err),
  }
}
