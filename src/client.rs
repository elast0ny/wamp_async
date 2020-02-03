use std::future::Future;
use std::collections::HashSet;

use log::*;
use url::*;
use tokio::sync::{mpsc, mpsc::UnboundedSender};
use tokio::sync::oneshot;

use crate::error::*;
use crate::serializer::SerializerType;
use crate::common::*;
use crate::core::*;

pub struct ClientConfig {
    agent: String,
    roles: HashSet<ClientRole>,
    serializers: Vec<SerializerType>,
    max_msg_size: u32,
}

impl ClientConfig {
    pub fn new() -> Self {
        // Config with default values
        ClientConfig {
            agent: String::from(DEFAULT_AGENT_STR),
            roles: [
                ClientRole::Caller,
                ClientRole::Callee,
                ClientRole::Publisher,
                ClientRole::Subscriber
            ].iter().cloned().collect(),
            serializers: vec![SerializerType::Json, SerializerType::MsgPack],
            max_msg_size: 0,
        }
    }

    /// Sets the maximum payload size which can be sent over the transport
    /// Set to 0 to use default
    pub fn set_max_msg_size(mut self, msg_size: u32) -> Self {
        self.max_msg_size = msg_size;
        self
    }
    /// Returns the maximum message size for the transport
    pub fn get_max_msg_size(&self) -> Option<u32> {
        if self.max_msg_size == 0 {
            None
        } else {
            Some(self.max_msg_size)
        }
    }

    /// Sets the serializers that will be used in order of preference (serializers[0] will be attempted first)
    pub fn set_serializer_list(mut self, serializers: Vec<SerializerType>) -> Self {
        self.serializers = serializers;
        self
    }
    /// Returns the list of prefered serializers
    pub fn get_serializers(&self) -> &Vec<SerializerType> {
        &self.serializers
    }

    /// Sets the roles that are intended to be used by the client
    pub fn set_roles(mut self, roles: Vec<ClientRole>) -> Self {
        self.roles.drain();
        for role in roles {
            self.roles.insert(role);
        }
        self
    }
}

pub struct Client {
    /// Configuration struct used to customize the client
    config: ClientConfig,
    /// Generic transport
    conn: Option<Connection>,
    /// Roles supported by the server
    server_roles: HashSet<String>,
    /// Current Session ID
    session_id: Option<WampId>,
    /// Channel to send requests to the event loop
    ctl_channel: UnboundedSender<Request>,
}

impl Client {

    pub async fn connect<T: AsRef<str>>(uri: T, cfg: Option<ClientConfig>) -> Result<Self, WampError> {
        
        let uri = match Url::parse(uri.as_ref()) {
            Ok(u) => u,
            Err(e) => return Err(WampError::InvalidUri(e)),
        };
        
        let config = match cfg {
            Some(c) => c,
            // Set defaults
            None => ClientConfig::new(),
        };

        let (ctl_channel, ctl_receiver) = mpsc::unbounded_channel();

        // Establish a connection
        let conn = Connection::connect(&uri, &config, ctl_receiver).await?;

        Ok(Client {
            config,
            conn: Some(conn),
            server_roles: HashSet::new(),
            session_id: None,
            ctl_channel,
        })
    }
    
    /// Returns a future which will run the event loop. The caller is responsible for running it.
    pub fn event_loop(&mut self) -> Result<impl Future<Output = Result<(), WampError>> + Send + 'static, WampError> {
        let conn = self.conn.take();
        match conn {
            Some(c) => return Ok(c.event_loop()),
            None => return Err(From::from("Event loop already running".to_string())),
        };
    }

    /// Sends a join realm request to the core event loop
    pub async fn join_realm<T: AsRef<str>>(&mut self, realm: T) -> Result<(), WampError> {

        if self.session_id.is_some() {
            return Err(From::from(format!("join_realm('{}') : Client already joined to a realm", realm.as_ref())));
        }

        let (res_sender, res) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Join {
            uri: realm.as_ref().to_string(),
            roles: self.config.roles.clone(),
            agent_str: if self.config.agent.len() > 0 {
                Some(self.config.agent.clone())
            } else {
                None
            },
            res: res_sender,
        }) {
            return Err(From::from(format!("Core never received our request : {}", e)));
        }

        let (session_id, mut server_roles) = match res.await {
            Ok(r) => r?,
            Err(e) => return Err(From::from(format!("Core never returned a response : {}", e))),
        };

        // Add the server roles
        self.server_roles.drain();
        for (role,_) in server_roles.drain().take(1) {
            self.server_roles.insert(role);
        }

        // Set the current session
        self.session_id = Some(session_id);

        debug!("Connected with session_id {:?} !", session_id);

        Ok(())
    }

    /// Sends a leave realm request to the core event loop
    pub async fn leave_realm(&mut self) -> Result<(), WampError> {

        // Nothing to do if not currently in a session
        if self.session_id.take().is_none() {
            return Ok(());
        }

        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Leave {res}) {
            return Err(From::from(format!("Core never received our request : {}", e)));
        }

        // Wait for the result
        match result.await {
            Ok(r) => r?,
            Err(e) => return Err(From::from(format!("Core never returned a response : {}", e))),
        };

        Ok(())
    } 

    /// Subscribes to event for the specifiec topic
    pub async fn subscribe<T: AsRef<str>>(&self, topic: T) -> Result<(WampId, SubscriptionQueue), WampError> {
        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Subscribe {
            uri: topic.as_ref().to_string(),
            res,
        }) {
            return Err(From::from(format!("Core never received our request : {}", e)));
        }

        // Wait for the result
        let (sub_id, evt_queue) = match result.await {
            Ok(r) => r?,
            Err(e) => return Err(From::from(format!("Core never returned a response : {}", e))),
        };

        Ok((sub_id, evt_queue))
    }

    /// Unsubscribes to the subscription ID
    pub async fn unsubscribe(&self, sub_id: WampId) -> Result<(), WampError> {
        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Unsubscribe {
            sub_id,
            res,
        }) {
            return Err(From::from(format!("Core never received our request : {}", e)));
        }

        // Wait for the result
        match result.await {
            Ok(r) => r?,
            Err(e) => return Err(From::from(format!("Core never returned a response : {}", e))),
        };

        Ok(())
    }

    /// Publishes an event on a specifiec topic
    pub async fn publish<T: AsRef<str>>(&self, topic: T, arguments: WampArgs, arguments_kw: WampKwArgs, acknowledge: bool) -> Result<WampId, WampError> {
        let mut options = WampDict::new();

        if acknowledge {
            options.insert("acknowledge".to_string(), Arg::Bool(true));
        }
        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Publish {
            uri: topic.as_ref().to_string(),
            options,
            arguments,
            arguments_kw,
            res,
        }) {
            return Err(From::from(format!("Core never received our request : {}", e)));
        }

        // Wait for the result
        let pub_id = match result.await {
            Ok(r) => r?,
            Err(e) => return Err(From::from(format!("Core never returned a response : {}", e))),
        };

        Ok(pub_id.unwrap())
    }

    /// Cleanly closes a connection with the server
    pub async fn disconnect(mut self) {
        // Cleanly leave realm
        let _ = self.leave_realm().await;
        // Stop the eventloop and disconnect from server
        let _ = self.ctl_channel.send(Request::Shutdown);
    }
}