use log::*;
use url::*;
use std::error::Error;

use tokio::net::TcpStream;

pub async fn connect(_uri: &Url) -> Result<TcpStream, Box<dyn Error>> {
    warn!("Websocket not implemented yet !");
    return Err(From::from(format!("Websocket transport not implemented yet...")));
}