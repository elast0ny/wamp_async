use std::future::Future;
use std::collections::HashSet;

use log::*;
use url::*;
use tokio::sync::{mpsc, mpsc::UnboundedSender, mpsc::UnboundedReceiver, mpsc::error::TryRecvError};
use tokio::sync::{oneshot};

use crate::error::*;
use crate::serializer::SerializerType;
pub use crate::common::*;
use crate::core::*;

pub struct ClientConfig {
    agent: String,
    roles: HashSet<ClientRole>,
    serializers: Vec<SerializerType>,
    max_msg_size: u32,
    ssl_verify: bool
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
            ssl_verify: true,
        }
    }

    /// Sets the agent string
    pub fn set_agent<T: AsRef<str>>(mut self, agent: T) -> Self {
        self.agent = String::from(agent.as_ref());
        self
    }
    /// Returns the currently set agent string
    pub fn get_agent(&self) -> &str {
        &self.agent
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
    pub fn set_serializers(mut self, serializers: Vec<SerializerType>) -> Self {
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

    pub fn set_ssl_verify(mut self, val: bool) -> Self {
        self.ssl_verify = val;
        self
    }
    pub fn get_ssl_verify(&self) -> bool {
        self.ssl_verify
    }
}

pub struct Client {
    /// Configuration struct used to customize the client
    config: ClientConfig,
    /// Generic transport
    conn: Option<Core>,
    core_res: UnboundedReceiver<Result<(), WampError>>,
    core_status: ClientState,
    /// Roles supported by the server
    server_roles: HashSet<String>,
    /// Current Session ID
    session_id: Option<WampId>,
    /// Channel to send requests to the event loop
    ctl_channel: UnboundedSender<Request>,
}

pub enum ClientState {
    NoEventLoop,
    Running,
    Disconnected(Result<(), WampError>),
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
        let (core_res_w, core_res) = mpsc::unbounded_channel();

        let ctl_sender = ctl_channel.clone();
        // Establish a connection
        let conn = Core::connect(&uri, &config, (ctl_sender, ctl_receiver), core_res_w).await?;

        Ok(Client {
            config,
            conn: Some(conn),
            server_roles: HashSet::new(),
            session_id: None,
            ctl_channel,
            core_res,
            core_status: ClientState::NoEventLoop,
        })
    }
    
    /// Returns a future which will run the event loop. The caller is responsible for running it.
    pub fn event_loop(&mut self) -> Result<
        (
            std::pin::Pin<Box<dyn Future<Output = ()> + Send>>, 
            Option<UnboundedReceiver<GenericFuture>>
        ), WampError> {
        
        let mut core = match self.conn.take() {
            Some(c) => c,
            None => return Err(From::from("Event loop already running".to_string())),
        };

        let rpc_evt_queue = if self.config.roles.contains(&ClientRole::Callee) {
            core.rpc_event_queue_r.take()
        } else {
            None
        };

        Ok((Box::pin(core.event_loop()), rpc_evt_queue))
    }

    /// Sends a join realm request to the core event loop
    pub async fn join_realm<T: AsRef<str>>(&mut self, realm: T) -> Result<(), WampError> {

        // Make sure we are still connected to a server
        if !self.is_connected() {
            return Err(From::from("The client is currently not connected".to_string()));
        }

        // Make sure we arent already part of a realm
        if self.session_id.is_some() {
            return Err(From::from(format!("join_realm('{}') : Client already joined to a realm", realm.as_ref())));
        }

        // Send a request for the core to perform the action
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

        // Wait for the request results
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

         // Make sure we are still connected to a server
         if !self.is_connected() {
            return Err(From::from("The client is currently not connected".to_string()));
        }

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

    /// Register an RPC endpoint
    pub async fn register<T, F, Fut>(&self, uri: T, func_ptr: F) -> Result<WampId, WampError>
    where
        T: AsRef<str>,
        F: Fn(WampArgs, WampKwArgs) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(WampArgs, WampKwArgs), WampError>> + Send + 'static,
    {
        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Register {
            uri: uri.as_ref().to_string(),
            res,
            func_ptr: Box::new(move |a,k| Box::pin(func_ptr(a,k))),
        }) {
            return Err(From::from(format!("Core never received our request : {}", e)));
        }

        // Wait for the result
        let rpc_id = match result.await {
            Ok(r) => r?,
            Err(e) => return Err(From::from(format!("Core never returned a response : {}", e))),
        };

        Ok(rpc_id)
    }

    /// Unregisters the RPC ID
    pub async fn unregister(&self, rpc_id: WampId) -> Result<(), WampError> {
        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Unregister {
            rpc_id,
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

    pub async fn call<T: AsRef<str>>(&self, uri: T, arguments: WampArgs, arguments_kw: WampKwArgs) -> Result<(WampArgs, WampKwArgs), WampError> {

        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Call {
            uri: uri.as_ref().to_string(),
            options: WampDict::new(),
            arguments,
            arguments_kw,
            res,
        }) {
            return Err(From::from(format!("Core never received our request : {}", e)));
        }

        // Wait for the result
        let res = match result.await {
            Ok(r) => r,
            Err(e) => return Err(From::from(format!("Core never returned a response : {}", e))),
        };

        res
    }

    pub fn get_status(&mut self) -> &ClientState {
        let new_status = self.core_res.try_recv();

        match new_status {
            Ok(state) => {
                if let Err(e) = state {
                    // Error occured
                    self.core_status = ClientState::Disconnected(Err(e));
                } else {
                    //Transition to running or disconnected depending on previous state
                    self.core_status = match self.core_status {
                        ClientState::NoEventLoop => ClientState::Running,
                        _ => ClientState::Disconnected(Ok(())),
                    };
                }
            },
            Err(TryRecvError::Closed) => {
                self.core_status = ClientState::Disconnected(Err(From::from(format!("Core has exited unexpectedly..."))));
            },
            Err(TryRecvError::Empty) => {},
        }

        &self.core_status
    }

    pub fn is_connected(&mut self) -> bool {
        match self.get_status() {
            ClientState::Running => true,
            _ => false,
        }
    }

    /// Cleanly closes a connection with the server
    pub async fn disconnect(mut self) {
        if self.is_connected() {
            // Cleanly leave realm
            let _ = self.leave_realm().await;
            // Stop the eventloop and disconnect from server
            let _ = self.ctl_channel.send(Request::Shutdown);

            // Wait for return status from core
            match self.core_res.recv().await {
                Some(Err(e)) => error!("Error while shutting down : {:?}", e),
                None => error!("Core never sent a status after shutting down..."),
                _ => {},
            }
        }
    }
}