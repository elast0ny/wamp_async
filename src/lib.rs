use std::error::Error;

use log::*;
use url::*;
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

mod transport;
mod common;
mod message;
pub use common::*;
pub use transport::*;
pub use message::*;

enum WampTransport {
    Websocket,
    Tcp,
}

pub struct WampClient {
    conn: Option<TcpStream>,
    serial_impl: WampSerializer,
    max_msg_size: u32,
    transport: WampTransport,
}

impl WampClient {
    pub fn new() -> Self {
        WampClient {
            conn: None,
            serial_impl: WampSerializer::MsgPack,
            max_msg_size: 512,
            transport: WampTransport::Tcp,
        }
    }
    pub async fn connect<T: AsRef<str>>(&mut self, uri: T, realm: T) -> Result<(), Box<dyn Error>> {
        
        if self.conn.is_some() {
            warn!("Already connected !");
            return Ok(());
        }
        
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

        self.conn = match uri.scheme() {
            "ws" | "wss" => {
                Some(ws::connect(&uri).await?)
            },
            "tcp" => {
                let host_port = match uri.port() {
                    Some(p) => p,
                    None => {
                        return Err(From::from(format!("No port specified for tcp host")));
                    },
                };

                // Perform the TCP connection
                let stream = tcp::connect(host_str, host_port, None, Some(self.max_msg_size)).await?;
                self.transport = WampTransport::Tcp;
                Some(stream)
            },
            s => return Err(From::from(format!("Unknown uri scheme : {}", s))),
        };
        
        Ok(())        
    }

    pub async fn send<B: AsRef<[u8]>>(&mut self, msg_type: WampMsg, payload: &B) -> Result<(), Box<dyn Error>> {
        match &mut self.conn {
            Some(ref mut sock) => {
                match self.transport {
                    WampTransport::Websocket => {
                        Err(From::from(format!("Not implemented !")))
                    },
                    // Send on TCP transport
                    WampTransport::Tcp => {
                        Ok(tcp::send(sock, &tcp::TcpMsg::Regular, payload).await?)
                    }
                }
            }
            None => return Err(From::from("Tried to send() while not connected...")),
        }   
    }

    pub async fn recv(&mut self) -> Result<WampMsg, Box<dyn Error>> {

        if self.conn.is_none() {
            return Err(From::from("Tried to recv() while not connected..."));
        }

        let sock: &mut TcpStream = self.conn.as_mut().unwrap();

        // Get the bytes from the transport
        let data: Vec<u8> = match self.transport {
            WampTransport::Websocket => {
                return Err(From::from(format!("Not implemented !")));
            },
            // Send on TCP transport
            WampTransport::Tcp => {
                let mut data: Vec<u8>;
                let mut transport_type: tcp::TcpMsg;
                loop {
                    let tmp = tcp::recv(sock).await?;
                    transport_type = tmp.0;
                    data = tmp.1;
                    match transport_type {
                        tcp::TcpMsg::Regular => break,
                        tcp::TcpMsg::Ping => {/*TODO : Handle ping*/},
                        tcp::TcpMsg::Pong => {/*TODO : Handle pong*/},
                    };
                }
                data
            }
        };

        return Err(From::from(format!("Not implemented !")));
    }
}