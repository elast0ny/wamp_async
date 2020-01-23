use std::error::Error;
use async_trait::async_trait;
use quick_error::*;

pub mod tcp;
pub use tcp::*;

pub mod websocket;
pub use websocket as ws;
pub use ws::*;

use crate::serializer::SerializerType;

#[async_trait]
pub trait Transport {
    /// Sends a whole wamp message over the transport
    async fn send(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>>;
    /// Receives a whole wamp message from the transport
    async fn recv(&mut self) -> Result<Vec<u8>, Box<dyn Error>>;
    /// Closes the transport connection with the host
    async fn close(self);
}

quick_error! {
    #[derive(Debug)]
    pub enum TransportError {
        MaximumServerConn {
            description("Server hit the maximum connection count")
        }
        UnexpectedResponse {
            description("Server responded with unexpected data")
        }
        SerializerNotSupported(e: SerializerType) {
            description("The current serializer is not supported by the server")
            display(self_) -> ("{} (Requested : {:?})", self_, e)
        }
        InvalidMaximumMsgSize(e: u32) {
            description("The server did not accept the maximum payload size")
            display(self_) -> ("{} (Requested : {})", self_, e)
        }
        UnknownState(e: String) {
            description("Agent is in an unexpected state")
            display(self_) -> ("{} : {}", self_, e)
        }
        ConnectionFailed(err: std::io::Error) {
            cause(err)
            description("Failed to connect")
            display(self_) -> ("{} : {}", self_.description(), err)
        }
    }
}
