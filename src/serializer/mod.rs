use quick_error::*;

use crate::message::Msg;

pub mod json;
pub mod msgpack;

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
/// Message serialization algorithms
pub enum SerializerType {
    Invalid = 0,
    Json = 1,
    MsgPack = 2,
    // 3 - 15 reserved
}

impl SerializerType {
    /// Returns the WAMP string representation of the serializer
    pub fn to_str(&self) -> &'static str {
        match self {
            &SerializerType::Json => "wamp.2.json",
            &SerializerType::MsgPack => "wamp.2.msgpack",
            _ => "wamp.2.invalid",
        }
    }

    /// Converts the WAMP serializer string to its enum variant
    pub fn from_str<T: AsRef<str>>(in_str: T) -> Self {
        let s = in_str.as_ref();

        if s == SerializerType::Json.to_str() {
            SerializerType::Json
        } else if s == SerializerType::MsgPack.to_str() {
            SerializerType::MsgPack 
        } else {
            SerializerType::Invalid
        }
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum SerializerError {
        Serialization(e: String) {
            description("Failed to serialize message")
            display(_self) -> ("{} : {}", _self, e)
        }
        Deserialization(e: String) {
            description("Failed to deserialize message")
            display(_self) -> ("{} : {}", _self, e)
        }
    }
}


pub trait SerializerImpl {
    fn pack(&self, value: &Msg) -> Result<Vec<u8>, SerializerError>;
    fn unpack<'a>(&self, v: &'a [u8]) -> Result<Msg, SerializerError>;
}