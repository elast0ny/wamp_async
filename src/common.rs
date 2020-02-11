use std::collections::HashMap;
use std::hash::Hash;
use std::error::Error;
use std::pin::Pin;
use std::future::Future;

use crate::error::*;

use serde::{Serialize, Deserialize};
use log::*;

pub const DEFAULT_AGENT_STR: &str = concat!(env!("CARGO_PKG_NAME"), "_rs-", env!("CARGO_PKG_VERSION"));

//Basic WAMP data types
pub type WampUri = String;
pub type WampId = u64;
pub type WampInteger = usize;
pub type WampString = String;
pub type WampBool = bool;
pub type WampDict = HashMap<String, Arg>;
pub type WampList = Vec<Arg>;
pub type WampArgs = Option<WampList>;
pub type WampKwArgs = Option<WampDict>;
pub type WampFuncArgs = (WampArgs, WampKwArgs);
pub type WampRetVal = Result<WampFuncArgs, Box<dyn Error>>;
pub type WampFunc = fn(WampArgs, WampKwArgs) -> WampRetVal;

/// Generic enum that can represent all types
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
    /// bool: a boolean value (true or false) - integers MUST NOT be used instead of boolean value
    Bool(WampBool),
    /// dict: a dictionary (map) where keys MUST be strings, keys MUST be unique and serialization order is undefined (left to the serializer being used)
    Dict(WampDict),
    /// list: a list (array) where items can be again any of this enumeration
    List(WampList),
    None,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
/// All the supported roles a client can have
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
    pub fn to_string(&self) -> String {
        String::from(
            match self {
                &ClientRole::Caller => "caller",
                &ClientRole::Callee => "callee",
                &ClientRole::Publisher => "publisher",
                &ClientRole::Subscriber => "subscriber",
            }
        )
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
    pub fn to_string(&self) -> String {
        String::from(
            match self {
                &ServerRole::Router => "router",
                &ServerRole::Broker => "broker",
            }
        )
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
                warn!("URI '{}' contains a zero length token ending @ index {}", uri, i);
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
            warn!("URI '{}' contains a non lower case character @ index {}", uri, i);
            return false;
        }
        if !c.is_alphanumeric() {
            warn!("URI '{}' contains an invalid character @ index {}", uri, i);
            return false;
        }
    }
    
    return true;
}


pub type GenericFuture = Pin<Box<dyn Future<Output = Result<(), WampError>> + Send>>;
pub type RpcFuture = Pin<Box<dyn Future<Output = Result<(WampArgs, WampKwArgs), WampError>> + Send>>;
pub type RpcFunc = Box<dyn Fn(WampArgs, WampKwArgs) -> RpcFuture + Send + Sync>;