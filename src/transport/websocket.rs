use log::*;
use url::*;

use crate::transport::{Transport, TransportError};
use crate::serializer::SerializerType;

pub async fn connect(_uri: &Url) -> Result<(Box<dyn Transport + Send + Sync>, SerializerType), TransportError> {
    warn!("Websocket not implemented yet !");
    return Err(TransportError::NotImplemented);
}