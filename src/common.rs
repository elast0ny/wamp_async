use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::hash::Hash;
use std::num::NonZeroU64;
use std::pin::Pin;

use log::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::error::*;

pub(crate) const DEFAULT_AGENT_STR: &str =
    concat!(env!("CARGO_PKG_NAME"), "_rs-", env!("CARGO_PKG_VERSION"));

/// uri: a string URI as defined in URIs
pub type WampUri = String;

/// id: an integer ID as defined in IDs
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct WampId(NonZeroU64);

impl fmt::Display for WampId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<WampId> for NonZeroU64 {
    fn from(id: WampId) -> Self {
        id.0
    }
}

impl WampId {
    /// IDs in the global scope MUST be drawn randomly from a uniform distribution over the complete
    /// range [1, 2^53]
    pub(crate) fn generate() -> Self {
        let random_id = rand::random::<u64>() & ((1 << 53) - 1);
        // Safety: since random_id is in range of [0, 2**53) and we add 1, the value is always in
        // range [1, 2^53].
        Self(unsafe { NonZeroU64::new_unchecked(random_id + 1) })
    }
}

/// integer: a non-negative integer
pub type WampInteger = usize;
/// string: a Unicode string, including the empty string
pub type WampString = String;
/// bool: a boolean value (true or false)
pub type WampBool = bool;
/// dict: a dictionary (map) where keys MUST be strings
pub type WampDict = HashMap<String, Arg>;
/// list: a list (array) where items can be of any type
pub type WampList = Vec<Arg>;
/// Arbitrary values supported by the serialization format in the payload
///
/// Implementation note: we currently use `serde_json::Value`, which is
/// suboptimal when you want to use MsgPack and pass binary data.
pub type WampPayloadValue = serde_json::Value;
/// Unnamed WAMP argument list
pub type WampArgs = Vec<WampPayloadValue>;
/// Named WAMP argument map
pub type WampKwArgs = serde_json::Map<String, WampPayloadValue>;

/// Generic enum that can hold any concrete WAMP value
#[serde(untagged)]
#[derive(Serialize, Deserialize, Debug)]
pub enum Arg {
    /// uri: a string URI as defined in URIs
    Uri(WampUri),
    /// id: an integer ID as defined in IDs
    Id(WampId),
    /// integer: a non-negative integer
    Integer(WampInteger),
    /// string: a Unicode string, including the empty string
    String(WampString),
    /// bool: a boolean value (true or false)
    Bool(WampBool),
    /// dict: a dictionary (map) where keys MUST be strings
    Dict(WampDict),
    /// list: a list (array) where items can be again any of this enumeration
    List(WampList),
    None,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
/// All roles a client can be
pub enum ClientRole {
    /// Client can call RPC endpoints
    Caller,
    /// Client can register RPC endpoints
    Callee,
    /// Client can publish events to topics
    Publisher,
    /// Client can register for events on topics
    Subscriber,
}
impl ClientRole {
    /// Returns the string repesentation of the role
    pub fn to_str(&self) -> &'static str {
        match self {
            ClientRole::Caller => "caller",
            ClientRole::Callee => "callee",
            ClientRole::Publisher => "publisher",
            ClientRole::Subscriber => "subscriber",
        }
    }
}

/// All the supported roles a server can have
pub enum ServerRole {
    /// Server supports RPC calls
    Router,
    /// Server supports pub/sub
    Broker,
}
impl ServerRole {
    /// Returns the string repesentation of the role
    pub fn to_str(&self) -> &'static str {
        match self {
            ServerRole::Router => "router",
            ServerRole::Broker => "broker",
        }
    }
}

/// Convert WampPayloadValue into any serde-deserializable object
pub fn try_from_any_value<'a, T: DeserializeOwned>(
    value: WampPayloadValue,
) -> Result<T, WampError> {
    serde_json::from_value(value).map_err(|e| {
        WampError::SerializationError(crate::serializer::SerializerError::Deserialization(
            e.to_string(),
        ))
    })
}

/// Convert WampArgs into any serde-deserializable object
pub fn try_from_args<'a, T: DeserializeOwned>(value: WampArgs) -> Result<T, WampError> {
    try_from_any_value(value.into())
}

/// Convert WampArgs into any serde-deserializable object
pub fn try_from_kwargs<'a, T: DeserializeOwned>(value: WampKwArgs) -> Result<T, WampError> {
    try_from_any_value(value.into())
}

/// Convert any serde-serializable object into WampPayloadValue
pub fn try_into_any_value<T: Serialize>(value: T) -> Result<WampPayloadValue, WampError> {
    serde_json::to_value(value).map_err(|e| {
        WampError::SerializationError(crate::serializer::SerializerError::Serialization(
            e.to_string(),
        ))
    })
}

/// Convert any serde-serializable object into WampArgs
pub fn try_into_args<T: Serialize>(value: T) -> Result<WampArgs, WampError> {
    match serde_json::to_value(value).unwrap() {
        serde_json::value::Value::Array(array) => Ok(array),
        value => Err(WampError::SerializationError(
            crate::serializer::SerializerError::Serialization(format!(
                "failed to serialize {:?} into positional arguments",
                value
            )),
        )),
    }
}

/// Convert any serde-serializable object into WampKwArgs
pub fn try_into_kwargs<T: Serialize>(value: T) -> Result<WampKwArgs, WampError> {
    match serde_json::to_value(value).unwrap() {
        serde_json::value::Value::Object(object) => Ok(object),
        value => Err(WampError::SerializationError(
            crate::serializer::SerializerError::Serialization(format!(
                "failed to serialize {:?} into keyword arguments",
                value
            )),
        )),
    }
}

/// Returns whether a uri is valid or not (using strict rules)
pub fn is_valid_strict_uri<T: AsRef<str>>(in_uri: T) -> bool {
    let uri: &str = in_uri.as_ref();
    let mut num_chars_token: usize = 0;
    if uri.starts_with("wamp.") {
        warn!("URI '{}' cannot start with 'wamp'", uri);
        return false;
    }

    for (i, c) in uri.chars().enumerate() {
        if c == '.' {
            if num_chars_token == 0 {
                warn!(
                    "URI '{}' contains a zero length token ending @ index {}",
                    uri, i
                );
                return false;
            }
            num_chars_token = 0;
        } else {
            num_chars_token += 1;
        }

        if c == '_' {
            continue;
        }

        if !c.is_lowercase() {
            warn!(
                "URI '{}' contains a non lower case character @ index {}",
                uri, i
            );
            return false;
        }
        if !c.is_alphanumeric() {
            warn!("URI '{}' contains an invalid character @ index {}", uri, i);
            return false;
        }
    }

    true
}

/// Future that can return success or an error
pub type GenericFuture = Pin<Box<dyn Future<Output = Result<(), WampError>> + Send>>;
/// Type returned by RPC functions
pub type RpcFuture =
    Pin<Box<dyn Future<Output = Result<(Option<WampArgs>, Option<WampKwArgs>), WampError>> + Send>>;
/// Generic function that can receive RPC calls
pub type RpcFunc = Box<dyn Fn(Option<WampArgs>, Option<WampKwArgs>) -> RpcFuture + Send + Sync>;
