use log::*;
use std::error::Error;

use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

use crate::common::*;
use crate::message::*;

pub fn error_to_str(err_num: u8) -> &'static str {
    match err_num {
        0 => "illegal (must not be used)",
        1 => "serializer unsupported",
        2 => "maximum message length unacceptable",
        3 => "use of reserved bits (unsupported feature)",
        4 => "maximum connection count reached",
        _ => "Unknown error",
    }
}

pub const MAX_MSG_SZ: u32 = 1 << 24;
pub const MIN_MSG_SZ: u32 = 1 << 9;

#[repr(u8)]
#[derive(Debug)]
pub enum WampSerializer {
    Invalid = 0,
    Json = 1,
    MsgPack = 2,
    // 3 - 15 reserved
}

#[repr(u8)]
#[derive(Debug)]
pub enum TcpMsg {
    Regular = 0,
    Ping = 1,
    Pong = 2,
    // 3 - 7 reserved
}

impl TcpMsg {
    pub fn from_id(in_id: u8) -> Option<Self> {
        match in_id {
            x if x == TcpMsg::Regular as u8 => Some(TcpMsg::Regular),
            x if x == TcpMsg::Ping as u8 => Some(TcpMsg::Ping),
            x if x == TcpMsg::Pong as u8 => Some(TcpMsg::Pong),
            _ => None,
        }
    }
    pub fn to_id(&self) -> u8 {
        match self {
            &TcpMsg::Regular => TcpMsg::Regular as u8,
            &TcpMsg::Ping => TcpMsg::Ping as u8,
            &TcpMsg::Pong => TcpMsg::Pong as u8,
        }
    }
}

struct HandshakeCtx {
    client: [u8; 4],
    server: [u8; 4],
}
impl AsRef<[u8]> for HandshakeCtx {
    fn as_ref(&self) -> &[u8] {
        &self.client
    }
}
impl std::fmt::Debug for HandshakeCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:02X}{:02X}{:02X}{:02X} (MsgSize : 0x{:X}, Serializer : {:?})",
            self.client[0], self.client[1], self.client[2], self.client[3],
            1 << ((self.client[1] >> 4) + 9),
            match self.client[1] & 0x0F {
                x if x == WampSerializer::Json as u8 => WampSerializer::Json,
                x if x == WampSerializer::MsgPack as u8 => WampSerializer::MsgPack,
                _ => WampSerializer::Invalid,
            })
    }
}
impl HandshakeCtx {
    pub fn new() -> Self {
        let client: [u8; 4] = [
            0x7F, // Magic value
            0xF0 & // Max msg length
            ((WampSerializer::MsgPack as u8) & 0x0F), // Serialized
            0, 0 // Reserved
        ];
        HandshakeCtx {
            client,
            server: [0,0,0,0],
        }
    }

    /// Sets the maximum message size to the next or equal power of two of msg_size
    pub fn set_msg_size(mut self, msg_size: u32) -> Self {
        let req_size: u32 = match msg_size.checked_next_power_of_two() {
            Some(p) => {
                if p < MIN_MSG_SZ {
                    MIN_MSG_SZ
                } else if p > MAX_MSG_SZ {
                    MAX_MSG_SZ
                } else {
                    p
                }
            },
            None => {
                MAX_MSG_SZ
            },
        };

        if msg_size != req_size {
            warn!("Adjusted max TCP message size from {} to {}", msg_size, req_size);
        }
 
        self.client[1] = (self.client[1] & 0x0F) | (0xF0);

        self
    }

    pub fn set_serializer(mut self, serializer: WampSerializer) -> Self {
        self.client[1] = (self.client[1] & 0xF0) | ((serializer as u8) & 0x0F);
        self
    }

    pub fn srv_resp_bytes(&mut self) -> &mut [u8; 4] {
        &mut self.server
    }

    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.server[0] != 0x7f || self.server[2] != 0 || self.server[3] != 0{
            return Err(From::from(format!("Server's response was not a WAMP-TCP reply")));
        }

        if self.server[1] != self.client[1] {
            let server_error: u8 = (self.server[1] & 0xF0) >> 4 as u8;
            return Err(From::from(format!("Server did not accept our handshake : {}", error_to_str(server_error))));
        }

        Ok(())
    }
}

struct MsgPrefix {
    pub bytes: [u8; 4],
}
impl std::fmt::Debug for MsgPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Type : {}, PayloadLen : {}",
            self.payload_type(),
            self.payload_len())
    }
}
impl MsgPrefix {
    pub fn new_from(msg_type: &TcpMsg, msg_len: Option<u32>) -> Self {
        let bytes: [u8; 4] = [
            0x7 & msg_type.to_id(),
            0,
            0,
            0,
        ];

        let mut h = MsgPrefix {
            bytes
        };

        if let Some(len) = msg_len {
            h.set_msg_len(len);   
        }
        h
    }

    pub fn new() -> Self {
        let bytes: [u8; 4] = [
            0,
            0,
            0,
            0,
        ];

        MsgPrefix { bytes }
    }

    pub fn set_msg_len(&mut self, len :u32) {
        let len_bytes = len.to_be_bytes();
        self.bytes[1] = len_bytes[1];
        self.bytes[2] = len_bytes[2];
        self.bytes[3] = len_bytes[3];
    }

    pub fn payload_type(&self) -> u8 {
        self.bytes[0] & 0x7
    }

    pub fn msg_type(&self) -> Option<TcpMsg> {

        // First 5 bits must be 0
        if self.bytes[0] & 0xF8 != 0 {
            return None;
        }

        TcpMsg::from_id(self.payload_type())
    }

    pub fn payload_len(&self) -> u32 {
        (self.bytes[3] as u32 + 
        ((self.bytes[2] as u32) << 8) + 
        ((self.bytes[1] as u32) << 16))
    }
}

pub async fn connect(host_ip: &str, host_port: u16, serializer: Option<WampSerializer>, msg_size: Option<u32>) -> Result<TcpStream, Box<dyn Error>> {
    
    let host_addr = format!("{}:{}", host_ip, host_port);

    let serializer = match serializer {
        Some(s) => s,
        None => WampSerializer::MsgPack,
    };

    let msg_size = match msg_size {
        Some(s) => s,
        None => MAX_MSG_SZ,
    };

    debug!("Connecting to host : {}", host_addr);
    let mut stream = TcpStream::connect(&host_addr).await?;
    let mut handshake = HandshakeCtx::new().set_msg_size(msg_size).set_serializer(serializer);
    debug!("\tSending handshake : {:?}", handshake);
    
    // Preform the WAMP handshake
    stream.write_all(handshake.as_ref()).await?;
    stream.read_exact(handshake.srv_resp_bytes()).await?;

    if let Err(e) = handshake.validate() {
        return Err(From::from(format!("TCP handshake failed : {}", e)));
    }
    Ok(stream)
}

pub async fn send<T: AsyncWriteExt + std::marker::Unpin, B: AsRef<[u8]>>(sock: &mut T, msg_type: &TcpMsg, data: &B) -> Result<(), Box<dyn Error>> {
    let payload: &[u8] = data.as_ref();
    let header: MsgPrefix = MsgPrefix::new_from(&msg_type, Some(payload.len() as u32));
    
    debug!("Send[0x{:X}] : {:?} ({:?})", std::mem::size_of_val(&header), header.bytes, header);
    sock.write_all(&header.bytes).await?;

    debug!("Send[0x{:X}] : {:?}", payload.len(), payload);
    sock.write_all(payload).await?;

    Ok(())
}

pub async fn recv<T: AsyncReadExt + std::marker::Unpin>(sock: &mut T) -> Result<(TcpMsg, Vec<u8>), Box<dyn Error>> {
    let mut payload: Vec<u8>;
    let mut header: MsgPrefix = MsgPrefix::new();

    sock.read_exact(&mut header.bytes).await?;
    debug!("Recv[0x{:X}] : {:?} - ({:?})", std::mem::size_of_val(&header), header, header);

    // Validate the 4 byte header
    let msg_type = match header.msg_type() {
        Some(m) => m,
        None => return Err(From::from("Received invalid WAMP header")),
    };
    
    payload = Vec::with_capacity(header.payload_len() as usize);
    unsafe {payload.set_len(header.payload_len() as usize)};
    sock.read_exact(&mut payload).await?;
    debug!("Recv[0x{:X}] : {:?}", payload.len(), payload);

    Ok((msg_type, payload))
}