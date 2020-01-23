use log::*;
use url::*;
use std::error::Error;

use crate::transport::Transport;
use crate::serializer::SerializerType;

pub async fn connect(_uri: &Url) -> Result<(Box<dyn Transport>, SerializerType), Box<dyn Error>> {
    warn!("Websocket not implemented yet !");
    return Err(From::from(format!("Websocket transport not implemented yet...")));
}