use std::collections::HashMap;

use serde::{Serialize, Deserialize};

pub type WampUri = String;
pub type WampId = u64;
pub type WampInteger = usize;
pub type WampString = String;
pub type WampBool = bool;
pub type WampDict = HashMap<String, WampMsgVal>;
pub type WampList = Vec<WampMsgVal>;

#[derive(Serialize, Deserialize, Debug)]
enum WampMsgVal {
    ///uri: a string URI as defined in URIs
    Uri(WampUri),
    ///id: an integer ID as defined in IDs
    Id(WampId),
    ///integer: a non-negative integer
    Integer(WampInteger),
    ///string: a Unicode string, including the empty string
    String(WampString),
    ///bool: a boolean value (true or false) - integers MUST NOT be used instead of boolean value
    Bool(WampBool),
    ///dict: a dictionary (map) where keys MUST be strings, keys MUST be unique and serialization order is undefined (left to the serializer being used)
    Dict(WampDict),
    ///list: a list (array) where items can be again any of this enumeration
    List(WampList),
}

pub WampGenericMsg {
    code: WampInteger,
    payload: Option<WampMsg
}

#[derive(Serialize, Deserialize, Debug)]
pub enum WampMsg {

    /// Sent by a Client to initiate opening of a WAMP session to a Router attaching to a Realm.
    Hello {
        Realm : WampUri,
        Details: WampDict,
    },

    /// Sent by a Router to accept a Client. The WAMP session is now open.
    Welcome {
        Session: WampId,
        Details: WampDict,
    },

    /// Sent by a Peer*to abort the opening of a WAMP session. No response is expected.
    Abort {
        Details: WampDict,
        Reason: WampUri,
    },

    /// Sent by a Peer to close a previously opened WAMP session. Must be echo'ed by the receiving Peer.
    Goodbye {
        Details: WampDict,
        Reason: WampUri,
    },

    /// Error reply sent by a Peer as an error response to different kinds of requests.
    Error {
        Type: WampInteger,
        Request: WampId,
        Details: WampDict,
        Error: WampUri,
        Arguments: Option<WampList>,
        ArgumentsKw: Option<WampDict>,
    },
}