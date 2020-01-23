use std::collections::HashMap;
use std::fmt;

use serde::{Serialize, Deserialize};
use serde::ser::{Serializer};
use serde::de::{Deserializer, Error, Visitor, SeqAccess};

//Basic types
pub type WampUri = String;
pub type WampId = u64;
pub type WampInteger = usize;
pub type WampString = String;
pub type WampBool = bool;
pub type WampDict = HashMap<String, MsgVal>;
pub type WampList = Vec<MsgVal>;

// Generic enum that can represent all types
#[serde(untagged)]
#[derive(Serialize, Deserialize, Debug)]
pub enum MsgVal {
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

// Message IDs
pub const HELLO_ID: WampId = 1;
pub const WELCOME_ID: WampId = 2;
pub const ABORT_ID: WampId = 3;
pub const GOODBYE_ID: WampId = 6;
pub const ERROR_ID: WampId = 8;
pub const PUBLISH_ID: WampId = 16;
pub const PUBLISHED_ID: WampId = 17;
pub const SUBSCRIBE_ID: WampId = 32;
pub const SUBSCRIBED_ID: WampId = 33;
pub const UNSUBSCRIBE_ID: WampId = 34;
pub const UNSUBSCRIBED_ID: WampId = 35;
pub const EVENT_ID: WampId = 36;
pub const CALL_ID: WampId = 48;
pub const RESULT_ID: WampId = 50;
pub const REGISTER_ID: WampId = 64;
pub const REGISTERED_ID: WampId = 65;
pub const UNREGISTER_ID: WampId = 66;
pub const UNREGISTERED_ID: WampId = 67;
pub const INVOCATION_ID: WampId = 68;
pub const YIELD_ID: WampId = 70;

#[derive(Debug)]
pub enum Msg {
    /// Sent by a Client to initiate opening of a WAMP session to a Router attaching to a Realm.
    Hello {
        realm: WampUri,
        details: WampDict,
    },

    /// Sent by a Router to accept a Client. The WAMP session is now open.
    Welcome {
        session: WampId,
        details: WampDict,
    },

    /// Sent by a Peer*to abort the opening of a WAMP session. No response is expected.
    Abort {
        details: WampDict,
        reason: WampUri,
    },

    /// Sent by a Peer to close a previously opened WAMP session. Must be echo'ed by the receiving Peer.
    Goodbye {
        details: WampDict,
        reason: WampUri,
    },

    /// Error reply sent by a Peer as an error response to different kinds of requests.
    Error {
        typ: WampInteger,
        request: WampId,
        details: WampDict,
        error: WampUri,
        arguments: Option<WampList>,
        arguments_kw: Option<WampDict>,
    },
}

/// Serialization from the struct to the WAMP tuple
impl Serialize for Msg
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            &Msg::Hello{ref realm, ref details} => (HELLO_ID, realm, details).serialize(serializer),
            &Msg::Welcome{ref session, ref details} => (WELCOME_ID, session, details).serialize(serializer),
            &Msg::Abort{ref details, ref reason} => (ABORT_ID, details, reason).serialize(serializer),
            &Msg::Goodbye{ref details, ref reason} => (GOODBYE_ID, details, reason).serialize(serializer),
            &Msg::Error{ref typ, ref request, ref details, ref error, ref arguments, ref arguments_kw} => {
                if arguments_kw.is_some() {
                    (ERROR_ID, typ, request, details, error, arguments, arguments_kw).serialize(serializer)
                } else if arguments.is_some() {
                    (ERROR_ID, typ, request, details, error, arguments).serialize(serializer)
                } else {
                    (ERROR_ID, typ, request, details, error).serialize(serializer)
                }
            },
        }
    }
}

/// Deserialization from the WAMP tuple to the struct
impl<'de> Deserialize<'de> for Msg {    
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MsgVisitor;
        impl MsgVisitor {
            fn de_hello<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Hello{
                    realm: v.next_element()?.ok_or(Error::missing_field("realm"))?,
                    details: v.next_element()?.ok_or(Error::missing_field("details"))?,
                })
            }
            fn de_welcome<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Welcome{
                    session: v.next_element()?.ok_or(Error::missing_field("session"))?,
                    details: v.next_element()?.ok_or(Error::missing_field("details"))?,
                })
            }
            fn de_abort<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Abort{
                    details: v.next_element()?.ok_or(Error::missing_field("details"))?,
                    reason: v.next_element()?.ok_or(Error::missing_field("reason"))?,
                })
            }
            fn de_goodbye<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Goodbye{
                    details: v.next_element()?.ok_or(Error::missing_field("details"))?,
                    reason: v.next_element()?.ok_or(Error::missing_field("reason"))?,
                })
            }
            fn de_error<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Error{
                    typ: v.next_element()?.ok_or(Error::missing_field("type"))?,
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    details: v.next_element()?.ok_or(Error::missing_field("details"))?,
                    error: v.next_element()?.ok_or(Error::missing_field("error"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
            }
            fn de_publish<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_published<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_subscribe<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_subscribed<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_unsubscribe<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_unsubscribed<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_event<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_call<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_result<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_register<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_registered<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_unregister<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_unregistered<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_invocation<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
            fn de_yield<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Err(Error::custom("Not implemented yet"))
            }
        }
        impl<'de> Visitor<'de> for MsgVisitor {
            type Value = Msg;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("WAMP message")
            }

            fn visit_seq<V>(self, mut v: V) -> Result<Msg, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let msg_id: WampId = v.next_element()?.ok_or_else(|| Error::invalid_length(0, &self))?;

                match msg_id {
                    HELLO_ID => self.de_hello(v),
                    WELCOME_ID => self.de_welcome(v),
                    ABORT_ID => self.de_abort(v),
                    GOODBYE_ID => self.de_goodbye(v),
                    ERROR_ID => self.de_error(v),
                    PUBLISH_ID => self.de_publish(v),
                    PUBLISHED_ID => self.de_published(v),
                    SUBSCRIBE_ID => self.de_subscribe(v),
                    SUBSCRIBED_ID => self.de_subscribed(v),
                    UNSUBSCRIBE_ID => self.de_unsubscribe(v),
                    UNSUBSCRIBED_ID => self.de_unsubscribed(v),
                    EVENT_ID => self.de_event(v),
                    CALL_ID => self.de_call(v),
                    RESULT_ID => self.de_result(v),
                    REGISTER_ID => self.de_register(v),
                    REGISTERED_ID => self.de_registered(v),
                    UNREGISTER_ID => self.de_unregister(v),
                    UNREGISTERED_ID => self.de_unregistered(v),
                    INVOCATION_ID => self.de_invocation(v),
                    YIELD_ID => self.de_yield(v),
                    id => Err(Error::custom(format!("Unknown message id : {}", id))),
                }
            }
        }

        deserializer.deserialize_seq(MsgVisitor)
    }

}