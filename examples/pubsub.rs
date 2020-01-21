use std::error::Error;

use wamp_client::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let mut client = WampClient::new();

    client.connect("tcp://localhost:8081", "realm1").await?;
    println!("Connected !!");

    client.send(WampTcpMsg::Ping, b"T").await?;

    let (msg_type, payload) = client.recv().await?;

    Ok(())
}