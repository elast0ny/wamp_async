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
    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError>;
    /// Receives a whole wamp message from the transport
    async fn recv(&mut self) -> Result<Vec<u8>, TransportError>;
    /// Closes the transport connection with the host
    async fn close(&mut self);
}

quick_error! {
    #[derive(Debug)]
    pub enum TransportError {
        NotImplemented {
            description("Transport not implemented")
        }
        FaildOperation(e: std::io::Error) {
            from()
            description("An operation on the transport failed")
            display(_self) -> ("{} : {}", _self, e)
        }
        InvalidWampMsgHeader {
            description("Invalid WAMP message header received from the server")
        }
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
        ConnectionFailed {
            description("Failed to negotiate connection with the server")
        }
    }
}
