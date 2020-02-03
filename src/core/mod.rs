use std::collections::{HashSet, HashMap};

use tokio::select;
use tokio::sync::oneshot::{Sender};
use tokio::sync::{mpsc, mpsc::UnboundedReceiver, mpsc::UnboundedSender};
use log::*;

use crate::error::*;
use crate::common::*;
use crate::transport::*;
use crate::serializer::*;

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

pub type JoinResult = Sender<Result<(WampId, HashMap<WampString, Arg>), WampError>>;
pub type SubscriptionQueue = UnboundedReceiver<(WampId, WampArgs, WampKwArgs)>;
pub type SubscribeResult = Sender<Result<(WampId, SubscriptionQueue), WampError>>;

pub struct Connection {
    /// Generic transport
    sock: Box<dyn Transport + Send + Sync>,
    /// Generic serializer
    serializer: Box<dyn SerializerImpl + Send + Sync>,
    /// Holds the request_id queues waiting for messages
    ctl_channel: Option<UnboundedReceiver<Request>>,

    /// Holds set of pending requests
    pending_requests: HashSet<WampId>,
    
    /// Holds generic transactions that can succeed/fail
    pending_transactions: HashMap<WampId, Sender<Result<Option<WampId>, WampError>>>,

    /// Holds the subscription requests sent to the server
    pending_sub: HashMap<WampId, SubscribeResult>,
    subscriptions: HashMap<WampId, UnboundedSender<(WampId, WampArgs, WampKwArgs)>>,
}

impl Connection {

    /// Establishes a connection with a WAMP server
    pub async fn connect(uri: &url::Url, cfg: &client::ClientConfig, ctl_channel: UnboundedReceiver<Request>/*, peer_handler: PeerMsgHandler*/) -> Result<Self, WampError> {        
        // Connect to the router using the requested transport
        let (sock, serializer_type) = match uri.scheme() {
            "ws" | "wss" => {
                ws::connect(&uri).await?
            },
            "tcp" => {
                let host_port = match uri.port() {
                    Some(p) => p,
                    None => {
                        return Err(From::from(format!("No port specified for tcp host")));
                    },
                };

                // Perform the TCP connection
                tcp::connect(uri.host_str().unwrap(), host_port, &cfg).await?
            },
            s => return Err(From::from(format!("Unknown uri scheme : {}", s))),
        };

        let serializer = match serializer_type {
            SerializerType::Json => Box::new(json::JsonSerializer {}),
            _ => Box::new(json::JsonSerializer {}),
        };

        Ok(
            Connection {
                sock,
                serializer,
                ctl_channel: Some(ctl_channel),
                pending_requests: HashSet::new(),
                pending_transactions: HashMap::new(),

                pending_sub: HashMap::new(),
                subscriptions: HashMap::new(),
            }
        )
    }

    /// Event loop that handles outbound/inboud events
    pub async fn event_loop(mut self) -> Result<(), WampError> {
        let mut ctl_channel = self.ctl_channel.take().unwrap();

        loop {
            match select! {
                // Peer sent us a message
                msg = self.recv() => {
                    self.handle_peer_msg(msg?).await
                },
                // Peer wants to send a message
                req = ctl_channel.recv() => {
                    let req = req.ok_or::<WampError>(WampError::RealmClientDied)?;
                    self.handle_peer_request(req).await
                }
            } {
                Status::Shutdown => break,
                Status::Ok => {},
            }
        }

        self.shutdown().await;
        Ok(())
    }

    /// Handles unsolicited messages from the peer (events, rpc calls, etc...)
    async fn handle_peer_msg(&mut self, msg: Msg) -> Status {

        // Make sure we were expecting this message if it has a request ID
        if let Some(ref request) = msg.request_id() {
            if self.pending_requests.remove(request) == false {
                warn!("Peer sent a response to an unknown request : {:X}", request);
                return Status::Ok;
            }
        }
        match msg {
            Msg::Subscribed{request, subscription} => recv::subscribed(self, request, subscription).await,
            Msg::Unsubscribed{request} => recv::unsubscribed(self, request).await,
            Msg::Published{request, publication} => recv::published(self, request, publication).await,
            Msg::Event{subscription, publication, details, arguments, arguments_kw} => recv::event(self, subscription, publication, details, arguments, arguments_kw).await,
            Msg::Error{typ, request, details, error, arguments, arguments_kw} => recv::error(self, typ, request, details, error, arguments, arguments_kw).await,
            _ => {
                warn!("Recevied unhandled message {:?}", msg);
                Status::Ok
            }
        }
    }

    /// Handles the basic ways one can interact with the peer
    async fn handle_peer_request(&mut self, req: Request) -> Status {
        // Forward the request the the implementor
        match req {
            Request::Shutdown => Status::Shutdown,
            Request::Join{uri, roles, agent_str, res} => send::join_realm(self, uri, roles, agent_str, res).await,
            Request::Leave{res} => send::leave_realm(self, res).await,
            Request::Subscribe {uri, res} => send::subscribe(self, uri, res).await,
            Request::Unsubscribe {sub_id, res} => send::unsubscribe(self, sub_id, res).await,
            Request::Publish{uri, options, arguments, arguments_kw, res} => send::publish(self, uri, options, arguments, arguments_kw, res).await,            
        }
    }

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

    pub async fn recv(&mut self) -> Result<Msg, WampError> {

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

    pub async fn shutdown(mut self) {
        // Close the transport
        self.sock.close().await;
    }

    /// Generates a new request_id and inserts it into the pending_requests
    fn create_request(&mut self) -> WampId {
        let mut request: WampId = rand::random();
        // Pick a unique request_id
        while self.pending_requests.insert(request) == false {
            request = rand::random();
        }
        request
    }
    
}