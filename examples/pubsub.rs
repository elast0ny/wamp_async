use std::error::Error;

use wamp_async::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let mut client = WampClient::connect("tcp://localhost:8081", "realm1", None).await?;

    println!("Connected !!");

    Ok(())
}