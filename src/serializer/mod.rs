use std::error::Error;

use crate::message::Msg;

pub mod json;

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum SerializerType {
    Invalid = 0,
    Json = 1,
    MsgPack = 2,
    // 3 - 15 reserved
}

pub trait SerializerImpl {
    fn pack(&self, value: &Msg) -> Result<Vec<u8>, Box<dyn Error>>;
    fn unpack<'a>(&self, v: &'a [u8]) -> Result<Msg, Box<dyn Error>>;
}