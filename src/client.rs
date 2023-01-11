use std::collections::{HashMap, HashSet};
use std::future::Future;
use futures::FutureExt;

use log::*;
use tokio::sync::oneshot;
use tokio::sync::{
    mpsc, mpsc::UnboundedReceiver, mpsc::UnboundedSender,
};
use url::*;

pub use crate::common::*;
use crate::core::*;
use crate::error::*;
use crate::serializer::SerializerType;

/// Options one can set when connecting to a WAMP server
pub struct ClientConfig {
    /// Replaces the default user agent string
    agent: String,
    /// A Set of all the roles the client will support
    roles: HashSet<ClientRole>,
    /// A priority list of which serializer to use when talking to the server
    serializers: Vec<SerializerType>,

    authextra: HashMap<String, String>,
    /// Sets the maximum message to be sent over the transport
    max_msg_size: u32,
    /// When using a secure transport, this option disables certificate validation
    ssl_verify: bool,
    /// Additional WebSocket headers on establish connection
    websocket_headers: HashMap<String, String>,
}

impl Default for ClientConfig {
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
    fn default() -> Self {
        // Config with default values
        ClientConfig {
            agent: String::from(DEFAULT_AGENT_STR),
            roles: [
                ClientRole::Caller,
                ClientRole::Callee,
                ClientRole::Publisher,
                ClientRole::Subscriber,
            ]
            .iter()
            .cloned()
            .collect(),
            serializers: vec![SerializerType::Json, SerializerType::MsgPack, SerializerType::Cbor],
            max_msg_size: 0,
            ssl_verify: true,
            websocket_headers: HashMap::new(),
            authextra: HashMap::new(),
        }
    }
}

impl ClientConfig {
    /// Replaces the default user agent string. Set to a zero length string to disable
    pub fn set_agent<T: AsRef<str>>(mut self, agent: T) -> Self {
        self.agent = String::from(agent.as_ref());
        self
    }
    pub fn set_authextra(&mut self, pkey: String) {
        let m = HashMap::from([
            ("pubkey".to_owned(), pkey),
        ]);
        self.authextra = m;
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

    pub fn add_websocket_header(mut self, key: String, val: String) -> Self {
        self.websocket_headers.insert(key, val);
        self
    }
    pub fn get_websocket_headers(&self) -> &HashMap<String, String> {
        &self.websocket_headers
    }
}

/// Allows interaction as a client with a WAMP server
pub struct Client<'a> {
    /// Configuration struct used to customize the client
    config: ClientConfig,
    /// Generic transport
    core_res: UnboundedReceiver<Result<(), WampError>>,
    core_status: ClientState,
    /// Roles supported by the server
    server_roles: HashSet<String>,
    /// Current Session ID
    session_id: Option<WampId>,
    /// Channel to send requests to the event loop
    ctl_channel: UnboundedSender<Request<'a>>,
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

impl<'a> Client<'a> {
    /// Connects to a WAMP server using the specified protocol
    ///
    /// __Note__
    ///
    /// On success, this function returns :
    /// -  Client : Used to interact with the server
    /// -  Main event loop Future : __This MUST be spawned by the caller__ (e.g using tokio::spawn())
    /// -  RPC event queue : If you register RPC endpoints, you MUST spawn a seperate task to also handle these events
    ///
    /// To customize parmeters used for the connection, see the [ClientConfig](struct.ClientConfig.html) struct
    pub async fn connect<T: AsRef<str>>(
        uri: T,
        cfg: Option<ClientConfig>,
    ) -> Result<
        (
            Client<'a>,
            (
                GenericFuture<'a>,
                Option<UnboundedReceiver<GenericFuture<'a>>>,
            ),
        ),
        WampError,
    > {
        let uri = match Url::parse(uri.as_ref()) {
            Ok(u) => u,
            Err(e) => return Err(WampError::InvalidUri(e)),
        };

        let config = match cfg {
            Some(c) => c,
            // Set defaults
            None => ClientConfig::default(),
        };

        let (ctl_channel, ctl_receiver) = mpsc::unbounded_channel();
        let (core_res_w, core_res) = mpsc::unbounded_channel();

        let ctl_sender = ctl_channel.clone();
        // Establish a connection
        let mut conn = Core::connect(&uri, &config, (ctl_sender, ctl_receiver), core_res_w).await?;

        let rpc_evt_queue = if config.roles.contains(&ClientRole::Callee) {
            conn.rpc_event_queue_r.take()
        } else {
            None
        };

        Ok((
            Client {
                config,
                server_roles: HashSet::new(),
                session_id: None,
                ctl_channel,
                core_res,
                core_status: ClientState::NoEventLoop,
            },
            (Box::pin(conn.event_loop()), rpc_evt_queue),
        ))
    }

    /// Attempts to join a realm and start a session with the server.
    ///
    /// See [`join_realm_with_authentication`] method for more details.
    async fn inner_join_realm(
        &mut self,
        realm: String,
        authentication_methods: Vec<AuthenticationMethod>,
        authentication_id: Option<String>,
        on_challenge_handler: Option<AuthenticationChallengeHandler<'a>>,
    ) -> Result<(), WampError> {
        // Make sure the event loop is ready to process requests
        if let ClientState::NoEventLoop = self.get_cur_status() {
            debug!("Called join_realm() before th event loop is ready... Waiting...");
            self.wait_for_status_change().await;
        }

        // Make sure we are still connected to a server
        if !self.is_connected() {
            return Err(From::from(
                "The client is currently not connected".to_string(),
            ));
        }

        // Make sure we arent already part of a realm
        if self.session_id.is_some() {
            return Err(From::from(format!(
                "join_realm('{}') : Client already joined to a realm",
                realm
            )));
        }

        // Send a request for the core to perform the action
        let (res_sender, res) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Join {
            uri: realm,
            roles: self.config.roles.clone(),
            agent_str: if self.config.agent.is_empty() {
                Some(self.config.agent.clone())
            } else {
                None
            },
            authentication_methods,
            authentication_id,
            authextra: if !self.config.authextra.is_empty() {
                Some(self.config.authextra.clone())
            } else {
                None
            },
            on_challenge_handler,
            res: res_sender,
        }) {
            return Err(From::from(format!(
                "Core never received our request : {}",
                e
            )));
        }

        // Wait for the request results
        let (session_id, mut server_roles) = match res.await {
            Ok(r) => r?,
            Err(e) => {
                return Err(From::from(format!(
                    "Core never returned a response : {}",
                    e
                )))
            }
        };

        // Add the server roles
        self.server_roles.drain();
        for (role, _) in server_roles.drain().take(1) {
            self.server_roles.insert(role);
        }

        // Set the current session
        self.session_id = Some(session_id);
        debug!("Connected with session_id {} !", session_id);

        Ok(())
    }

    /// Attempts to join a realm and start a session with the server.
    ///
    /// * `realm` - A name of the WAMP realm
    pub async fn join_realm<T: Into<String>>(&mut self, realm: T) -> Result<(), WampError> {
        self.inner_join_realm(realm.into(), vec![], None, None)
            .await
    }

    /// Attempts to join a realm and start a session with the server.
    ///
    /// * `realm` - A name of the WAMP realm
    /// * `authentication_methods` - A set of all the authentication methods the client will support
    /// * `authentication_id` - An authentication ID (e.g. username) the client wishes to authenticate as.
    ///   It is required for non-anynomous authentication methods.
    /// * `on_challenge_handler` - An authentication handler function
    ///
    /// ```ignore
    /// client
    ///     .join_realm_with_authentication(
    ///         "realm1",
    ///         vec![wamp_async::AuthenticationMethod::Ticket],
    ///         "username",
    ///         |_authentication_method, _extra| async {
    ///             Ok(wamp_async::AuthenticationChallengeResponse::with_signature(
    ///                 "password".into(),
    ///             ))
    ///         },
    ///     )
    ///     .await?;
    /// ```
    pub async fn join_realm_with_authentication<
        Realm,
        AuthenticationId,
        AuthenticationChallengeHandler,
        AuthenticationChallengeHandlerResponse,
    >(
        &mut self,
        realm: Realm,
        authentication_methods: Vec<AuthenticationMethod>,
        authentication_id: AuthenticationId,
        on_challenge_handler: AuthenticationChallengeHandler,
    ) -> Result<(), WampError>
    where
        Realm: Into<String>,
        AuthenticationId: Into<String>,
        AuthenticationChallengeHandler: Fn(AuthenticationMethod, WampDict) -> AuthenticationChallengeHandlerResponse
            + Send
            + Sync
            + 'a,
        AuthenticationChallengeHandlerResponse: std::future::Future<Output = Result<AuthenticationChallengeResponse, WampError>>
            + Send
            + 'a,
    {
        self.inner_join_realm(
            realm.into(),
            authentication_methods,
            Some(authentication_id.into()),
            Some(Box::new(move |authentication_method, extra| {
                Box::pin(on_challenge_handler(authentication_method, extra))
            })),
        )
        .await
    }

    pub async fn join_realm_with_cryptosign<
    Realm,
    AuthenticationId,
    >(
        &mut self,
        realm: Realm,
        authentication_id: AuthenticationId,
        public_key: String,
        secret_key: String
    ) -> Result<(), WampError>
    where
        Realm: Into<String>,
        AuthenticationId: Into<String>,
    {
        self.config.set_authextra(public_key);
        let cs = CryptoSign::new(secret_key);
        self.join_realm_with_authentication(
            realm,
            vec![AuthenticationMethod::CryptoSign],
            authentication_id,
            move |_authentication_method, _extra| async move {
                let f = nacl::sign::generate_keypair(&cs.sk);

                let data = _extra.get("challenge").unwrap();
                let challenge = match data {
                    Arg::Uri(c) => c,
                    _ => panic!("ERROR"),
                };

                let signature = CryptoSign::vec_array96(nacl::sign::sign(&CryptoSign::hex2bytes(challenge), &f.skey).ok().unwrap());
                let sig = CryptoSign::bytes2hex96(signature);
                Ok(AuthenticationChallengeResponse::with_signature(sig))
            },
        ).await
    }

    /// Leaves the current realm and terminates the session with the server
    pub async fn leave_realm(&mut self) -> Result<(), WampError> {
        // Make sure we are still connected to a server
        if !self.is_connected() {
            return Err(From::from(
                "The client is currently not connected".to_string(),
            ));
        }

        // Nothing to do if not currently in a session
        if self.session_id.take().is_none() {
            return Ok(());
        }

        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Leave { res }) {
            return Err(From::from(format!(
                "Core never received our request : {}",
                e
            )));
        }

        // Wait for the result
        match result.await {
            Ok(r) => r?,
            Err(e) => {
                return Err(From::from(format!(
                    "Core never returned a response : {}",
                    e
                )))
            }
        };

        Ok(())
    }

    /// Subscribes to events for the specifiec topic
    ///
    /// This function returns a subscription ID (required to unsubscribe) and
    /// the receive end of a channel for events published on the topic.
    pub async fn subscribe<T: AsRef<str>>(
        &self,
        topic: T,
    ) -> Result<(WampId, SubscriptionQueue), WampError> {
        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Subscribe {
            uri: topic.as_ref().to_string(),
            res,
        }) {
            return Err(From::from(format!(
                "Core never received our request : {}",
                e
            )));
        }

        // Wait for the result
        let (sub_id, evt_queue) = match result.await {
            Ok(r) => r?,
            Err(e) => {
                return Err(From::from(format!(
                    "Core never returned a response : {}",
                    e
                )))
            }
        };

        Ok((sub_id, evt_queue))
    }

    /// Unsubscribes to a previously subscribed topic
    pub async fn unsubscribe(&self, sub_id: WampId) -> Result<(), WampError> {
        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Unsubscribe { sub_id, res }) {
            return Err(From::from(format!(
                "Core never received our request : {}",
                e
            )));
        }

        // Wait for the result
        match result.await {
            Ok(r) => r?,
            Err(e) => {
                return Err(From::from(format!(
                    "Core never returned a response : {}",
                    e
                )))
            }
        };

        Ok(())
    }

    /// Publishes an event on a specific topic
    ///
    /// The caller can set `acknowledge` to true to receive unique IDs from the server
    /// for each published event.
    pub async fn publish<T: AsRef<str>>(
        &self,
        topic: T,
        arguments: Option<WampArgs>,
        arguments_kw: Option<WampKwArgs>,
        acknowledge: bool,
    ) -> Result<Option<WampId>, WampError> {
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
            return Err(From::from(format!(
                "Core never received our request : {}",
                e
            )));
        }

        let pub_id = if acknowledge {
            // Wait for the acknowledgement
            Some(match result.await {
                Ok(Ok(r)) => r.unwrap(),
                Ok(Err(e)) => return Err(From::from(format!("Failed to send publish : {}", e))),
                Err(e) => {
                    return Err(From::from(format!(
                        "Core never returned a response : {}",
                        e
                    )))
                }
            })
        } else {
            None
        };
        Ok(pub_id)
    }

    /// Register an RPC endpoint. Upon succesful registration, a registration ID is returned (used to unregister)
    /// and calls received from the server will generate a future which will be sent on the rpc event channel
    /// returned by the call to [event_loop()](struct.Client.html#method.event_loop)
    pub async fn register<T, F, Fut>(&self, uri: T, func_ptr: F) -> Result<WampId, WampError>
    where
        T: AsRef<str>,
        F: Fn(Option<WampArgs>, Option<WampKwArgs>) -> Fut + Send + Sync + 'a,
        Fut: Future<Output = Result<(Option<WampArgs>, Option<WampKwArgs>), WampError>> + Send + 'a,
    {
        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Register {
            uri: uri.as_ref().to_string(),
            res,
            func_ptr: Box::new(move |a, k| Box::pin(func_ptr(a, k))),
        }) {
            return Err(From::from(format!(
                "Core never received our request : {}",
                e
            )));
        }

        // Wait for the result
        let rpc_id = match result.await {
            Ok(r) => r?,
            Err(e) => {
                return Err(From::from(format!(
                    "Core never returned a response : {}",
                    e
                )))
            }
        };

        Ok(rpc_id)
    }

    /// Unregisters an RPC endpoint
    pub async fn unregister(&self, rpc_id: WampId) -> Result<(), WampError> {
        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Unregister { rpc_id, res }) {
            return Err(From::from(format!(
                "Core never received our request : {}",
                e
            )));
        }

        // Wait for the result
        match result.await {
            Ok(r) => r?,
            Err(e) => {
                return Err(From::from(format!(
                    "Core never returned a response : {}",
                    e
                )))
            }
        };

        Ok(())
    }

    /// Calls a registered RPC endpoint on the server
    pub async fn call<T: AsRef<str>>(
        &self,
        uri: T,
        arguments: Option<WampArgs>,
        arguments_kw: Option<WampKwArgs>,
    ) -> Result<(Option<WampArgs>, Option<WampKwArgs>), WampError> {
        // Send the request
        let (res, result) = oneshot::channel();
        if let Err(e) = self.ctl_channel.send(Request::Call {
            uri: uri.as_ref().to_string(),
            options: WampDict::new(),
            arguments,
            arguments_kw,
            res,
        }) {
            return Err(From::from(format!(
                "Core never received our request : {}",
                e
            )));
        }

        // Wait for the result
        match result.await {
            Ok(r) => r,
            Err(e) => Err(From::from(format!(
                "Core never returned a response : {}",
                e
            ))),
        }
    }

    /// Returns the current client status
    pub fn get_cur_status(&mut self) -> &ClientState {
        // Check to see if the status changed
        let new_status = self.core_res.recv().now_or_never();
        #[allow(clippy::match_wild_err_arm)]
        match new_status {
            Some(Some(state)) => self.set_next_status(state),
            None => &self.core_status,
            Some(None) => panic!("The event loop died without sending a new status"),
        }
    }

    /// Returns whether we are connected to the server or not
    pub fn is_connected(&mut self) -> bool {
        match self.get_cur_status() {
            ClientState::Running => true,
            _ => false,
        }
    }

    fn set_next_status(&mut self, new_status: Result<(), WampError>) -> &ClientState {
        // Error means disconnection
        if new_status.is_err() {
            self.core_status = ClientState::Disconnected(new_status);
            return &self.core_status;
        }

        // Progress to next state
        match self.core_status {
            ClientState::NoEventLoop => {
                self.core_status = ClientState::Running;
            }
            ClientState::Running => {
                self.core_status = ClientState::Disconnected(new_status);
            }
            ClientState::Disconnected(_) => {
                panic!("Got new core status after already being disconnected");
            }
        }

        &self.core_status
    }

    // Waits until the event loop sends a status change event
    // This will update the current core_status field
    async fn wait_for_status_change(&mut self) -> &ClientState {
        // State cant change if disconnected
        if let ClientState::Disconnected(ref _r) = self.core_status {
            return &self.core_status;
        }

        // Yield until we receive something
        let new_status = match self.core_res.recv().await {
            Some(v) => v,
            None => {
                panic!("The event loop died without sending a new status");
            }
        };

        // Save the new status
        self.set_next_status(new_status)
    }

    /// Blocks the caller until the connection with the server is terminated
    pub async fn block_until_disconnect(&mut self) -> &ClientState {
        let mut cur_status = self.get_cur_status();
        loop {
            match cur_status {
                ClientState::Disconnected(_) => break,
                _ => {
                    // Wait until status changes
                    cur_status = self.wait_for_status_change().await;
                }
            }
        }

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
                _ => {}
            }
        }
    }
}
