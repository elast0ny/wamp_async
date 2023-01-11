use quick_error::*;

use crate::message::Msg;

pub mod json;
pub mod msgpack;
pub mod cbor;

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
/// Message serialization algorithms
pub enum SerializerType {
    Json = 1,
    MsgPack = 2,
    Cbor = 0,
    // 3 - 15 reserved
}

impl std::str::FromStr for SerializerType {
    type Err = crate::serializer::SerializerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == SerializerType::Cbor.to_str() {
            Ok(SerializerType::Cbor)
        } else if s == SerializerType::Json.to_str() {
            Ok(SerializerType::Json)
        } else if s == SerializerType::MsgPack.to_str() {
            Ok(SerializerType::MsgPack)
        } else {
            Err(crate::serializer::SerializerError::UnknownSerializer(
                s.to_string(),
            ))
        }
    }
}

impl SerializerType {
    /// Returns the WAMP string representation of the serializer
    pub fn to_str(self) -> &'static str {
        match self {
            SerializerType::Json => "wamp.2.json",
            SerializerType::MsgPack => "wamp.2.msgpack",
            SerializerType::Cbor => "wamp.2.cbor",
        }
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum SerializerError {
        Serialization(e: String) {
            display("Failed to serialize message: {}", e)
        }
        Deserialization(e: String) {
            display("Failed to deserialize message: {}", e)
        }
        UnknownSerializer(e: String) {
            display("Unknown serializer specified: {}", e)
        }
    }
}

pub trait SerializerImpl {
    fn pack(&self, value: &Msg) -> Result<Vec<u8>, SerializerError>;
    fn unpack<'a>(&self, v: &'a [u8]) -> Result<Msg, SerializerError>;
}
