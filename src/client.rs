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

/// Options one can set when connecting to a WAMP server
pub struct ClientConfig {
    /// Replaces the default user agent string
    agent: String,
    /// A Set of all the roles the client will support
    roles: HashSet<ClientRole>,
    /// A priority list of which serializer to use when talking to the server
    serializers: Vec<SerializerType>,
    /// Sets the maximum message to be sent over the transport
    max_msg_size: u32,
    /// When using a secure transport, this option disables certificate validation
    ssl_verify: bool
}

impl ClientConfig {
    /// Creates a client config with reasonnable defaults
    /// 
    /// Roles :
    /// - [ClientRole::Caller](enum.ClientRole.html#variant.Caller)
    /// - [ClientRole::Callee](enum.ClientRole.html#variant.Callee)
    /// - [ClientRole::Publisher](enum.ClientRole.html#variant.Publisher)
    /// - [ClientRole::Subscriber](enum.ClientRole.html#variant.Subscriber)
    /// 
    /// Serializers :
    /// 1. [SerializerType::Json](enum.SerializerType.html#variant.Json)
    /// 2. [SerializerType::MsgPack](enum.SerializerType.html#variant.MsgPack)
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

    /// Replaces the default user agent string. Set to a zero length string to disable
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
    /// Returns the priority list of serializers
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

    /// Enables (default) or disables TLS certificate validation
    pub fn set_ssl_verify(mut self, val: bool) -> Self {
        self.ssl_verify = val;
        self
    }
    /// Returns whether certificate validation is enabled
    pub fn get_ssl_verify(&self) -> bool {
        self.ssl_verify
    }
}

/// Allows interaction as a client with a WAMP server
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

/// All the states a client can be in
pub enum ClientState {
    /// The event loop hasnt been spawned yet
    NoEventLoop,
    /// Currently running and connected to a server
    Running,
    /// Disconnected from a server
    Disconnected(Result<(), WampError>),
}

impl Client {
    /// Connects to a WAMP server using the specified protocol
    /// 
    /// Currently supported protocols are :
    /// - WebSocket : `ws://some.site.com/wamp` | (Secure) `wss://localhost:8080`
    /// - RawSocket : `tcp://some.site.com:80` | (Secure) `tcps://localhost:443`
    /// 
    /// Extra customization can be specified through the ClientConfig struct
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
    
    /// This function must be called by the client after a succesful connection.
    /// It returns a future for the event loop which MUST be executed by the caller.
    /// It also returns the receiving end of a channel (if the client has the 'callee' role) which is reponsible for receiving RPC call futures. The caller
    /// is also responsible for executing the RPC call futures in whatever way they wish.async_trait
    /// 
    /// This allows the caller to use whatever runtime & task execution method they wish
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

    /// Attempts to join a realm and start a session with the server
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

    /// Leaves the current realm and terminates the session with the server
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

    /// Subscribes to events for the specifiec topic
    /// 
    /// This function returns a subscription ID (required to unsubscribe) and
    /// the receive end of a channel for events published on the topic.
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

    /// Unsubscribes to a previously subscribed topic
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
    /// 
    /// The caller can set `acknowledge` to true to receive unique IDs from the server
    /// for each published event.
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

        let pub_id = if acknowledge {
            // Wait for the acknowledgement
            match result.await {
                Ok(Ok(r)) => r.unwrap(),
                Ok(Err(e)) => return Err(From::from(format!("Failed to send publish : {}", e))),
                Err(e) => return Err(From::from(format!("Core never returned a response : {}", e))),
            }
        } else {
            0
        };
        Ok(pub_id)
    }

    /// Register an RPC endpoint. Upon succesful registration, a registration ID is returned (used to unregister)
    /// and calls received from the server will generate a future which will be sent on the rpc event channel
    /// returned by the call to [event_loop()](struct.Client.html#method.event_loop)
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

    /// Unregisters an RPC endpoint
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

    /// Calls a registered RPC endpoint on the server
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

    /// Returns the current client status
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

    /// Returns whether we are connected to the server or not
    pub fn is_connected(&mut self) -> bool {
        match self.get_status() {
            ClientState::Running => true,
            _ => false,
        }
    }

    /// Blocks the caller until the connection with the server is terminated
    pub async fn block_until_disconnect(&mut self) -> &ClientState {

        // Wait until the event loop is running
        match self.get_status() {
            &ClientState::NoEventLoop => {
                match self.core_res.recv().await {
                    Some(Ok(_)) => self.core_status = ClientState::Running,
                    Some(Err(e)) => self.core_status = ClientState::Disconnected(Err(e)),
                    None => {},
                };
            },
            &ClientState::Disconnected(_) => return &self.core_status,
            _ => {},
        };

        // Wait until the event loop stops
        match self.core_res.recv().await {
            Some(s) => self.core_status = ClientState::Disconnected(s),
            None => {},
        };

        &self.core_status
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