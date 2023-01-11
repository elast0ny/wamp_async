use crate::message::*;
use crate::serializer::*;
use serde_cbor::{from_slice, to_vec};

pub struct CborSerializer {}

impl SerializerImpl for CborSerializer {
    fn pack(&self, value: &Msg) -> Result<Vec<u8>, SerializerError> {
        match to_vec(value) {
            Ok(v) => Ok(v),
            Err(e) => Err(SerializerError::Serialization(e.to_string())),
        }
    }
    fn unpack<'a>(&self, v: &'a [u8]) -> Result<Msg, SerializerError> {
        match from_slice(v) {
            Ok(v) => Ok(v),
            Err(e) => Err(SerializerError::Deserialization(e.to_string())),
        }
    }
}