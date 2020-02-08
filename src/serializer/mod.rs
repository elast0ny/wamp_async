use quick_error::*;

use crate::message::Msg;

pub mod json;
pub mod msgpack;

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum SerializerType {
    Invalid = 0,
    Json = 1,
    MsgPack = 2,
    // 3 - 15 reserved
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