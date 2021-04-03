use quick_error::*;
use url::ParseError;

use crate::common::*;
use crate::serializer::SerializerError;
use crate::transport::TransportError;

quick_error! {
    /// Types of errors a WAMP client can encounter
    #[derive(Debug)]
    pub enum WampError {
        UnknownError(e: String) {
            from()
            display("Unhandled error : {}", e)
        }
        /// Error with the connection
        ConnectionError(e: TransportError) {
            from()
            source(e)
            display("An error occured with the connection: ({})", e)
        }
        /// Error with serialization
        SerializationError(e: SerializerError) {
            from()
            source(e)
            display("An error occured while [de]serializing a message: ({})", e)
        }
        /// WAMP uri is invalid
        InvalidUri(e: ParseError) {
            source(e)
            display("The uri provided could not be parsed: {}", e)
        }
        /// Server uri is invalid
        NoHostInUri {
            display("The uri provided did not contain a host address")
        }
        /// The WAMP protocol was not respected by the peer
        ProtocolError(e: String) {
            display("An unexpected WAMP message was received: {}", e)
        }
        /// The client has been dropped while the event loop was running
        ClientDied {
            display("The client has exited without sending Shutdown")
        }
        /// A randomly generated ID was not unique
        RequestIdCollision {
            display("There was a collision with a unique request id")
        }
        /// The server sent us an Error message
        ServerError(uri: String, details: WampDict) {
            context(uri: String, details: WampDict) -> (uri, details)
            display("The server returned an error: {} {:?}", uri, details)
        }
    }
}
