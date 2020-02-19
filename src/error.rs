use quick_error::*;
use url::ParseError;

use crate::common::*;
use crate::transport::TransportError;
use crate::serializer::SerializerError;

quick_error! {
    /// Types of errors a WAMP client can encounter
    #[derive(Debug)]
    pub enum WampError {
        UnknownError(e: String) {
            from()
            description("Unhandled error")
            display(_self) -> ("{} : {}", _self, e)
        }
        /// Error with the connection
        ConnectionError(e: TransportError) {
            from()
            cause(e)
            description("An error occured with the connection")
            display(_self) -> ("{} : ({})", _self, e)
        }
        /// Error with serialization
        SerializationError(e: SerializerError) {
            from()
            cause(e)
            description("An error occured while [de]serializing a message")
            display(_self) -> ("{} : ({})", _self, e)
        }
        /// WAMP uri is invalid
        InvalidUri(e: ParseError) {
            cause(e)
            description("The uri provided could not be parsed")
            display(_self) -> ("{} : {}", _self, e)
        }
        /// Server uri is invalid
        NoHostInUri {
            description("The uri provided did not contain a host address")
        }
        /// The WAMP protocol was not respected by the peer
        ProtocolError(e: String) {
            description("An unexpected WAMP message was received")
            display(_self) -> ("{} : {}", _self, e)
        }
        /// The client has been dropped while the event loop was running
        ClientDied {
            description("The client has exited without sending Shutdown")
        }
        /// A randomly generated ID was not unique
        RequestIdCollision {
            description("There was a collision with a unique request id")
        }
        /// The server sent us an Error message
        ServerError(uri: String, details: WampDict) {
            context(uri: String, details: WampDict) -> (uri, details)
            description("The server returned an error")
            display(_self) -> ("{} : {} {:?}", _self, uri, details)
        }
    }
}
