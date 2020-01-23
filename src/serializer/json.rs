use std::error::Error;

use serde_json::{to_vec, from_slice};
use crate::message::*;
use crate::serializer::SerializerImpl;

pub struct JsonSerializer {}
impl SerializerImpl for JsonSerializer {
    fn pack(&self, value: &Msg) -> Result<Vec<u8>, Box<dyn Error>> {
        match to_vec(value) {
            Ok(v) => Ok(v),
            Err(e) => Err(From::from(format!("Json serialize failed : {}", e))),
        }
    }
    fn unpack<'a>(&self, v: &'a [u8]) -> Result<Msg, Box<dyn Error>> {
        match from_slice(v) {
            Ok(v) => Ok(v),
            Err(e) => Err(From::from(format!("Json deserialize failed : {}", e))),
        }
    }
}
