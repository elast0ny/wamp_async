use std::error::Error;
use std::collections::HashSet;

use log::*;
use url::*;

mod common;
mod transport;
mod serializer;
mod message;

pub use common::*;
pub use transport::*;
pub use serializer::*;
pub use message::*;

pub struct WampConfig {
    agent: String,
    roles: Vec<ClientRole>,
    serializers: Vec<SerializerType>,
    max_msg_size: u32,
}

impl WampConfig {
    pub fn new() -> Self {
        // Config with default values
        WampConfig {
            agent: String::from(DEFAULT_AGENT_STR),
            roles: vec![
                ClientRole::Caller,
                ClientRole::Callee,
                ClientRole::Publisher,
                ClientRole::Subscriber
                ],
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

    /// Sets the serializers that will be used in order of preference (serializers[0] will be attempted first)
    pub fn set_serializer_list(mut self, serializers: Vec<SerializerType>) -> Self {
        self.serializers = serializers;
        self
    }

    /// Sets the roles that are intended to be used by the client
    pub fn set_roles(mut self, roles: Vec<ClientRole>) -> Self {
        self.roles = roles;
        self
    }
}

pub struct WampClient {
    /// Configuration struct used to customize the client
    config: WampConfig,
    /// Generic transport
    conn: Box<dyn Transport>,
    /// Generic serializer
    serializer: Box<dyn SerializerImpl>,
    /// Roles supported by the server
    server_roles: HashSet<String>,
    /// Current Session ID
    session_id: Option<WampId>,
}

impl WampClient {

    pub async fn connect<T: AsRef<str>>(uri: T, realm: Option<T>, cfg: Option<WampConfig>) -> Result<Self, Box<dyn Error>> {
        
        let uri = match Url::parse(uri.as_ref()) {
            Ok(u) => u,
            Err(e) => return Err(From::from(format!("Failed parsing uri : {}", e))),
        };
        
        let host_str = match uri.host_str() {
            Some(s) => s,
            None => {
                return Err(From::from(format!("No host address in the connection uri.")));
            },
        };

        let config = match cfg {
            Some(c) => c,
            // Set defaults
            None => WampConfig::new(),
        };

        // Connect to the router using the requested transport
        let (conn, serializer_type) = match uri.scheme() {
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
                tcp::connect(host_str, host_port, &config).await?
            },
            s => return Err(From::from(format!("Unknown uri scheme : {}", s))),
        };

        let mut client = WampClient {
            config,
            conn,
            serializer: match serializer_type {
                SerializerType::Json => Box::new(serializer::json::JsonSerializer {}),
                _ => Box::new(serializer::json::JsonSerializer {}),
            },
            server_roles: HashSet::new(),
            session_id: None,
        };

        // Join the realm
        if let Some(r) = realm {
            client.join_realm(r).await?;
        }

        Ok(client)
    }

    pub async fn join_realm<T: AsRef<str>>(&mut self, realm: T) -> Result<(), Box<dyn Error>> {
        let mut details: WampDict = WampDict::new();
        let mut roles: WampDict = WampDict::new();

        // Add all of our roles
        for role in &self.config.roles {
            roles.insert(role.to_string(), MsgVal::Dict(WampDict::new()));    
        }
        details.insert("roles".to_owned(), MsgVal::Dict(roles));

        if self.config.agent.len() != 0 {
            details.insert("agent".to_owned(), MsgVal::String(self.config.agent.clone()));
        }

        // Send hello with our info
        self.send(&Msg::Hello {
            realm: String::from(realm.as_ref()),
            details: details,
        }).await?;

        // Receive the WELCOME message
        let resp = self.recv().await?;
        let (session_id, mut server_roles) = match resp {
            Msg::Welcome{session, details} => (session, details),
            m => return Err(From::from(format!("Server did not respond with WELCOME : {:?}", m))),
        };

        // Add the server roles
        for (role,_) in server_roles.drain().take(1) {
            self.server_roles.insert(role);
        }

        // Set the current session
        self.session_id = Some(session_id);

        debug!("Connected with session_id {:?} !", session_id);

        Ok(())
    }

    pub async fn send(&mut self, msg: &Msg) -> Result<(), Box<dyn Error>> {

        // Serialize the data
        let payload = self.serializer.pack(msg)?;

        match std::str::from_utf8(&payload) {
            Ok(v) => debug!("Send : {}", v),
            Err(_) => debug!("Send : {:?}", msg),
        };

        // Send to host
        self.conn.send(&payload).await
    }

    pub async fn recv(&mut self) -> Result<Msg, Box<dyn Error>> {

        // Receive a full message from the host
        let payload = self.conn.recv().await?;

        // Deserialize into a Msg
        let msg = self.serializer.unpack(&payload);

        match std::str::from_utf8(&payload) {
            Ok(v) => debug!("Recv : {}", v),
            Err(_) => debug!("Recv : {:?}", msg),
        };

        msg
    }
}