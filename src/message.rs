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

/// Generic enum that can represent all types
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

/// WAMP message
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
    /// Sent by a Publisher to a Broker to publish an event.
    Publish {
        request: WampId,
        options: WampDict,
        topic: WampUri,
        arguments: Option<WampList>,
        arguments_kw: Option<WampDict>,
    },
    /// Acknowledge sent by a Broker to a Publisher for acknowledged publications.
    Published {
        request: WampId,
        publication: WampId,
    },
    /// Subscribe request sent by a Subscriber to a Broker to subscribe to a topic.
    Subscribe {
        request: WampId,
        options: WampDict,
        topic: WampUri,
    },
    /// Acknowledge sent by a Broker to a Subscriber to acknowledge a subscription.
    Subscribed {
        request: WampId,
        subscription: WampId,
    },
    /// Unsubscribe request sent by a Subscriber to a Broker to unsubscribe a subscription.
    Unsubscribe {
        request: WampId,
        subscription: WampId,
    },
    /// Acknowledge sent by a Broker to a Subscriber to acknowledge unsubscription.
    Unsubscribed {
        request: WampId,
    },
    /// Event dispatched by Broker to Subscribers for subscriptions the event was matching.
    Event {
        subscription: WampId,
        publication: WampId,
        details: WampDict,
        arguments: Option<WampList>,
        arguments_kw: Option<WampDict>,
    },
    /// Call as originally issued by the Caller to the Dealer.
    Call {
        request: WampId,
        options: WampDict,
        procedure: WampUri,
        arguments: Option<WampList>,
        arguments_kw: Option<WampDict>,
    },
    /// Result of a call as returned by Dealer to Caller.
    Result {
        request: WampId,
        details: WampDict,
        arguments: Option<WampList>,
        arguments_kw: Option<WampDict>,
    },
    /// A Callees request to register an endpoint at a Dealer.
    Register {
        request: WampId,
        options: WampDict,
        procedure: WampUri,
    },
    /// Acknowledge sent by a Dealer to a Callee for successful registration.
    Registered {
        request: WampId,
        registration: WampId,
    },
    /// A Callees request to unregister a previously established registration.
    Unregister {
        request: WampId,
        registration: WampId,
    },
    /// Acknowledge sent by a Dealer to a Callee for successful unregistration.
    Unregistered {
        request: WampId,
    },
    /// Actual invocation of an endpoint sent by Dealer to a Callee.
    Invocation {
        request: WampId,
        registration: WampId,
        arguments: Option<WampList>,
        arguments_kw: Option<WampDict>,
    },
    /// Actual yield from an endpoint sent by a Callee to Dealer.
    Yield {
        request: WampId,
        options: WampDict,
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
        // Converts the enum struct to a tuple representation
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
            &Msg::Publish{ref request, ref options, ref topic, ref arguments, ref arguments_kw} => {
                if arguments_kw.is_some() {
                    (request, options, topic, arguments, arguments_kw).serialize(serializer)
                } else if arguments.is_some() {
                    (request, options, topic, arguments).serialize(serializer)
                } else {
                    (request, options, topic).serialize(serializer)
                }                
            },
            &Msg::Published{ref request, ref publication} => (request, publication).serialize(serializer),
            &Msg::Subscribe{ref request, ref options, ref topic} => (request, options, topic).serialize(serializer),
            &Msg::Subscribed{ref request, ref subscription} => (request, subscription).serialize(serializer),
            &Msg::Unsubscribe{ref request, ref subscription} => (request, subscription).serialize(serializer),
            &Msg::Unsubscribed{ref request} => (request).serialize(serializer),
            &Msg::Event{ref subscription, ref publication, ref details, ref arguments, ref arguments_kw} => {
                if arguments_kw.is_some() {
                    (subscription, publication, details, arguments, arguments_kw).serialize(serializer)
                } else if arguments.is_some() {
                    (subscription, publication, details, arguments).serialize(serializer)
                } else {
                    (subscription, publication, details).serialize(serializer)
                }                
            },
            &Msg::Call{ref request, ref options, ref procedure, ref arguments, ref arguments_kw} => {
                if arguments_kw.is_some() {
                    (request, options, procedure, arguments, arguments_kw).serialize(serializer)
                } else if arguments.is_some() {
                    (request, options, procedure, arguments).serialize(serializer)
                } else {
                    (request, options, procedure).serialize(serializer)
                }                
            },
            &Msg::Result{ref request, ref details, ref arguments, ref arguments_kw} => {
                if arguments_kw.is_some() {
                    (request, details, arguments, arguments_kw).serialize(serializer)
                } else if arguments.is_some() {
                    (request, details, arguments).serialize(serializer)
                } else {
                    (request, details).serialize(serializer)
                }                
            },
            &Msg::Register{ref request, ref options, ref procedure} => (request, options, procedure).serialize(serializer),
            &Msg::Registered{ref request, ref registration} => (request, registration).serialize(serializer),
            &Msg::Unregister{ref request, ref registration} => (request, registration).serialize(serializer),
            &Msg::Unregistered{ref request} => (request).serialize(serializer),
            &Msg::Invocation{ref request, ref registration, ref arguments, ref arguments_kw} => {
                if arguments_kw.is_some() {
                    (request, registration, arguments, arguments_kw).serialize(serializer)
                } else if arguments.is_some() {
                    (request, registration, arguments).serialize(serializer)
                } else {
                    (request, registration).serialize(serializer)
                }                
            },
            &Msg::Yield{ref request, ref options, ref arguments, ref arguments_kw} => {
                if arguments_kw.is_some() {
                    (request, options, arguments, arguments_kw).serialize(serializer)
                } else if arguments.is_some() {
                    (request, options, arguments).serialize(serializer)
                } else {
                    (request, options).serialize(serializer)
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
                Ok(Msg::Publish{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    options: v.next_element()?.ok_or(Error::missing_field("options"))?,
                    topic: v.next_element()?.ok_or(Error::missing_field("topic"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
            }
            fn de_published<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Published{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    publication: v.next_element()?.ok_or(Error::missing_field("publication"))?,
                })
            }
            fn de_subscribe<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Subscribe{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    options: v.next_element()?.ok_or(Error::missing_field("options"))?,
                    topic: v.next_element()?.ok_or(Error::missing_field("topic"))?,
                })
            }
            fn de_subscribed<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Subscribed{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    subscription: v.next_element()?.ok_or(Error::missing_field("subscription"))?,
                })
            }
            fn de_unsubscribe<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Unsubscribe{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    subscription: v.next_element()?.ok_or(Error::missing_field("subscription"))?,
                })
            }
            fn de_unsubscribed<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Unsubscribed{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                })
            }
            fn de_event<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Event{
                    subscription: v.next_element()?.ok_or(Error::missing_field("subscription"))?,
                    publication: v.next_element()?.ok_or(Error::missing_field("publication"))?,
                    details: v.next_element()?.ok_or(Error::missing_field("details"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
            }
            fn de_call<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Call{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    options: v.next_element()?.ok_or(Error::missing_field("options"))?,
                    procedure: v.next_element()?.ok_or(Error::missing_field("procedure"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
            }
            fn de_result<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Result{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    details: v.next_element()?.ok_or(Error::missing_field("details"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
            }
            fn de_register<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Register{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    options: v.next_element()?.ok_or(Error::missing_field("options"))?,
                    procedure: v.next_element()?.ok_or(Error::missing_field("procedure"))?,
                })
            }
            fn de_registered<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Registered{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    registration: v.next_element()?.ok_or(Error::missing_field("registration"))?,
                })
            }
            fn de_unregister<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Unregister{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    registration: v.next_element()?.ok_or(Error::missing_field("registration"))?,
                })
            }
            fn de_unregistered<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Unregistered{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                })
            }
            fn de_invocation<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Invocation{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    registration: v.next_element()?.ok_or(Error::missing_field("registration"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
            }
            fn de_yield<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Yield{
                    request: v.next_element()?.ok_or(Error::missing_field("request"))?,
                    options: v.next_element()?.ok_or(Error::missing_field("options"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
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