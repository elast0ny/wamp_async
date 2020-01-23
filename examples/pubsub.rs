use std::error::Error;

use wamp_async::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let client = WampClient::connect("tcp://localhost:8081", Some("realm1"), None).await?;

    println!("Connected !!");

    Ok(())
}