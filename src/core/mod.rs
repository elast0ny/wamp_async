use std::collections::{HashMap, HashSet};

use log::*;
use tokio::select;
use tokio::sync::oneshot::Sender;
use tokio::sync::{mpsc, mpsc::UnboundedReceiver, mpsc::UnboundedSender};

use crate::common::*;
use crate::error::*;
use crate::serializer::*;
use crate::transport::*;

mod recv;
mod send;

use crate::client;
use crate::message::*;
pub use send::Request;

pub enum Status {
    /// Returned when the event loop should shutdown
    Shutdown,
    Ok,
}

pub type JoinResult = Sender<
    Result<
        (
            WampId,                   // Session ID
            HashMap<WampString, Arg>, // Server roles
        ),
        WampError,
    >,
>;
pub type SubscriptionQueue = UnboundedReceiver<(
    WampId,           // Publish event ID
    Option<WampArgs>, // Publish args
    Option<WampKwArgs>,
)>; // publish kwargs
pub type PendingSubResult = Sender<
    Result<
        (
            WampId,            //Subcription ID
            SubscriptionQueue, // Queue for incoming events
        ),
        WampError,
    >,
>;
pub type PendingRegisterResult = Sender<
    Result<
        WampId, // Registration ID
        WampError,
    >,
>;
pub type PendingCallResult = Sender<
    Result<
        (
            Option<WampArgs>,   // Return args
            Option<WampKwArgs>, // Return kwargs
        ),
        WampError,
    >,
>;

pub struct Core<'a> {
    /// Generic transport
    sock: Box<dyn Transport + Send>,
    valid_session: bool,
    core_res: UnboundedSender<Result<(), WampError>>,
    /// Generic serializer
    serializer: Box<dyn SerializerImpl + Send>,
    /// Holds the request_id queues waiting for messages
    ctl_sender: UnboundedSender<Request<'a>>,
    /// Channel for receiving client requests
    ctl_channel: Option<UnboundedReceiver<Request<'a>>>, //Wrapped in option so we can give ownership to eventloop

    /// Holds set of pending requests
    pending_requests: HashSet<WampId>,
    /// Holds generic transactions that can succeed/fail
    pending_transactions: HashMap<WampId, Sender<Result<Option<WampId>, WampError>>>,

    /// Pending subscription requests sent to the server
    pending_sub: HashMap<WampId, PendingSubResult>,
    /// Current subscriptions
    subscriptions: HashMap<WampId, UnboundedSender<(WampId, Option<WampArgs>, Option<WampKwArgs>)>>,

    /// Pending RPC registration requests sent to the server
    pending_register: HashMap<WampId, (RpcFunc<'a>, PendingRegisterResult)>,
    /// Currently registered RPC endpoints
    rpc_endpoints: HashMap<WampId, RpcFunc<'a>>,
    /// Queue passed back to the client caller to handle rpc events
    pub rpc_event_queue_r: Option<UnboundedReceiver<GenericFuture<'a>>>,
    rpc_event_queue_w: UnboundedSender<GenericFuture<'a>>,

    pending_call: HashMap<WampId, PendingCallResult>,
}

impl<'a> Core<'a> {
    /// Establishes a connection with a WAMP server
    pub async fn connect(
        uri: &url::Url,
        cfg: &client::ClientConfig,
        ctl_channel: (UnboundedSender<Request<'a>>, UnboundedReceiver<Request<'a>>),
        core_res: UnboundedSender<Result<(), WampError>>,
    ) -> Result<Core<'a>, WampError> {
        // Connect to the router using the requested transport
        let (sock, serializer_type) = match uri.scheme() {
            "ws" | "wss" => ws::connect(uri, &cfg).await?,
            "tcp" | "tcps" => {
                let host_port = match uri.port() {
                    Some(p) => p,
                    None => {
                        return Err(From::from("No port specified for tcp host".to_string()));
                    }
                };

                // Perform the TCP connection
                tcp::connect(
                    uri.host_str().unwrap(),
                    host_port,
                    uri.scheme() != "tcp",
                    &cfg,
                )
                .await?
            }
            s => return Err(From::from(format!("Unknown uri scheme : {}", s))),
        };

        debug!("Connected with serializer : {:?}", serializer_type);

        let serializer: Box<dyn SerializerImpl + Send> = match serializer_type {
            SerializerType::Cbor => Box::new(cbor::CborSerializer {}),
            SerializerType::Json => Box::new(json::JsonSerializer {}),
            SerializerType::MsgPack => Box::new(msgpack::MsgPackSerializer {}),
        };

        //let (rpc_result_w, rpc_result_r) = mpsc::unbounded_channel();
        let (rpc_event_queue_w, rpc_event_queue_r) = mpsc::unbounded_channel();

        Ok(Core {
            sock,
            core_res,
            valid_session: false,
            serializer,
            ctl_sender: ctl_channel.0,
            ctl_channel: Some(ctl_channel.1),
            pending_requests: HashSet::new(),
            pending_transactions: HashMap::new(),

            pending_sub: HashMap::new(),
            subscriptions: HashMap::new(),

            pending_register: HashMap::new(),
            rpc_endpoints: HashMap::new(),
            rpc_event_queue_r: Some(rpc_event_queue_r),
            rpc_event_queue_w,
            pending_call: HashMap::new(),
        })
    }

    /// Event loop that handles outbound/inboud events
    pub async fn event_loop(mut self) -> Result<(), WampError> {
        let mut ctl_channel = self.ctl_channel.take().unwrap();

        // Notify the client that we are now running the event loop
        let _ = self.core_res.send(Ok(()));
        loop {
            match select! {
                // Peer sent us a message
                msg = self.recv() => {
                    match msg {
                        Err(e) => {
                            /* The WAMP spec leaves it up to the server implementation
                            to decide whether to close a connection or not after a
                            GOODBYE message (leaving the realm). If we have left the realm,
                            treat a recv() error as expected */
                            if self.valid_session {
                                error!("Failed to recv : {:?}", e);
                                let _ = self.core_res.send(Err(e));
                            }

                            break;
                        },
                        Ok(m) => self.handle_peer_msg(m).await,
                    }
                },
                // client wants to send a message
                req = ctl_channel.recv() => {
                    let req = match req {
                        Some(r) => r,
                        None => {
                            let _ = self.core_res.send(Err(WampError::ClientDied));
                            break;
                        }
                    };
                    self.handle_local_request(req).await
                }
            } {
                Status::Shutdown => {
                    let _ = self.core_res.send(Ok(()));
                    break;
                }
                Status::Ok => {}
            }
        }
        debug!("Event loop shutting down !");

        self.shutdown().await;

        Ok(())
    }

    /// Handles unsolicited messages from the peer (events, rpc calls, etc...)
    async fn handle_peer_msg<'b>(&'b mut self, msg: Msg) -> Status
    where
        'a: 'b,
    {
        // Make sure we were expecting this message if it has a request ID
        if let Some(ref request) = msg.request_id() {
            if !self.pending_requests.remove(request) {
                warn!("Peer sent a response to an unknown request : {}", request);
                return Status::Ok;
            }
        }
        match msg {
            Msg::Subscribed {
                request,
                subscription,
            } => recv::subscribed(self, request, subscription).await,
            Msg::Unsubscribed { request } => recv::unsubscribed(self, request).await,
            Msg::Published {
                request,
                publication,
            } => recv::published(self, request, publication).await,
            Msg::Event {
                subscription,
                publication,
                details,
                arguments,
                arguments_kw,
            } => {
                recv::event(
                    self,
                    subscription,
                    publication,
                    details,
                    arguments,
                    arguments_kw,
                )
                .await
            }
            Msg::Registered {
                request,
                registration,
            } => recv::registered(self, request, registration).await,
            Msg::Unregistered { request } => recv::unregisterd(self, request).await,
            Msg::Invocation {
                request,
                registration,
                details,
                arguments,
                arguments_kw,
            } => {
                recv::invocation(
                    self,
                    request,
                    registration,
                    details,
                    arguments,
                    arguments_kw,
                )
                .await
            }
            Msg::Result {
                request,
                details,
                arguments,
                arguments_kw,
            } => recv::call_result(self, request, details, arguments, arguments_kw).await,
            Msg::Goodbye { details, reason } => recv::goodbye(self, details, reason).await,
            Msg::Abort { details, reason } => recv::abort(self, details, reason).await,
            Msg::Error {
                typ,
                request,
                details,
                error,
                arguments,
                arguments_kw,
            } => recv::error(self, typ, request, details, error, arguments, arguments_kw).await,
            _ => {
                warn!("Recevied unhandled message {:?}", msg);
                Status::Ok
            }
        }
    }

    /// Handles the basic ways one can interact with the peer
    async fn handle_local_request(&mut self, req: Request<'a>) -> Status {
        // Forward the request the the implementor
        match req {
            Request::Shutdown => Status::Shutdown,
            Request::Join {
                uri,
                roles,
                agent_str,
                authentication_methods,
                authentication_id,
                authextra,
                on_challenge_handler,
                res,
            } => {
                send::join_realm(
                    self,
                    uri,
                    roles,
                    agent_str,
                    authentication_methods,
                    authextra,
                    authentication_id,
                    on_challenge_handler,
                    res,
                )
                .await
            }
            Request::Leave { res } => send::leave_realm(self, res).await,
            Request::Subscribe { uri, res } => send::subscribe(self, uri, res).await,
            Request::Unsubscribe { sub_id, res } => send::unsubscribe(self, sub_id, res).await,
            Request::Publish {
                uri,
                options,
                arguments,
                arguments_kw,
                res,
            } => send::publish(self, uri, options, arguments, arguments_kw, res).await,
            Request::Register { uri, res, func_ptr } => {
                send::register(self, uri, res, func_ptr).await
            }
            Request::Unregister { rpc_id, res } => send::unregister(self, rpc_id, res).await,
            Request::InvocationResult { request, res } => {
                send::invoke_yield(self, request, res).await
            }
            Request::Call {
                uri,
                options,
                arguments,
                arguments_kw,
                res,
            } => send::call(self, uri, options, arguments, arguments_kw, res).await,
        }
    }

    /// Serializes a message and sends it on the transport
    pub async fn send(&mut self, msg: &Msg) -> Result<(), WampError> {
        // Serialize the data
        let payload = self.serializer.pack(msg)?;

        match std::str::from_utf8(&payload) {
            Ok(v) => debug!("Send : {}", v),
            Err(_) => debug!("Send : {:?}", msg),
        };

        // Send to host
        self.sock.send(&payload).await?;

        Ok(())
    }

    /// Receives a message and deserializes it
    pub async fn recv<'b>(&'b mut self) -> Result<Msg, WampError>
    where
        'a: 'b,
    {
        // Receive a full message from the host
        let payload = self.sock.recv().await?;

        // Deserialize into a Msg
        let msg = self.serializer.unpack(&payload);

        match std::str::from_utf8(&payload) {
            Ok(v) => debug!("Recv : {}", v),
            Err(_) => debug!("Recv : {:?}", msg),
        };

        Ok(msg?)
    }

    /// Closes the transport
    pub async fn shutdown(mut self) {
        // Close the transport
        self.sock.close().await;
    }

    /// Generates a new request_id and inserts it into the pending_requests
    fn create_request(&mut self) -> WampId {
        let mut request = WampId::generate();
        // Pick a unique request_id
        while !self.pending_requests.insert(request) {
            request = WampId::generate();
        }
        request
    }
}
