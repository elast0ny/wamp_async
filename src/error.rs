use quick_error::*;
use url::ParseError;

use crate::common::*;
use crate::transport::TransportError;
use crate::serializer::SerializerError;

quick_error! {
    #[derive(Debug)]
    pub enum WampError {
        UnknownError(e: String) {
            from()
            description("Unhandled error")
            display(_self) -> ("{} : {}", _self, e)
        }
        ConnectionError(e: TransportError) {
            from()
            cause(e)
            description("An error occured with the connection")
            display(_self) -> ("{} : ({})", _self, e)
        }
        SerializationError(e: SerializerError) {
            from()
            cause(e)
            description("An error occured while [de]serializing a message")
            display(_self) -> ("{} : ({})", _self, e)
        }
        InvalidUri(e: ParseError) {
            cause(e)
            description("The uri provided could not be parsed")
            display(_self) -> ("{} : {}", _self, e)
        }
        NoHostInUri {
            description("The uri provided did not contain a host address")
        }
        ProtocolError(e: String) {
            description("An unexpected WAMP message was received")
            display(_self) -> ("{} : {}", _self, e)
        }
        ClientDied {
            description("The client has exited without sending Shutdown")
        }
        RequestIdCollision {
            description("There was a collision with a unique request id")
        }
        ServerError(uri: String, details: WampDict) {
            context(uri: String, details: WampDict) -> (uri, details)
            description("The server returned an error")
            display(_self) -> ("{} : {} {:?}", _self, uri, details)
        }
    }
}
