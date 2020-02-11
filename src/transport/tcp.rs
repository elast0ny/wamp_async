use log::*;

use async_trait::async_trait;
use tokio::net::TcpStream;
use native_tls::TlsConnector;
use tokio_tls;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

use crate::ClientConfig;
use crate::transport::{Transport, TransportError};
use crate::serializer::SerializerType;

pub const MAX_MSG_SZ: u32 = 1 << 24;
pub const MIN_MSG_SZ: u32 = 1 << 9;

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
    msg_size: u32,
    serializer: SerializerType,
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
                x if x == SerializerType::Json as u8 => SerializerType::Json,
                x if x == SerializerType::MsgPack as u8 => SerializerType::MsgPack,
                _ => SerializerType::Invalid,
            })
    }
}
impl HandshakeCtx {
    pub fn new() -> Self {
        let client: [u8; 4] = [
            0x7F, // Magic value
            0xF0 & // Max msg length
            ((SerializerType::MsgPack as u8) & 0x0F), // Serialized
            0, 0 // Reserved
        ];
        HandshakeCtx {
            msg_size: 0,
            serializer: SerializerType::Json,
            client,
            server: [0,0,0,0],
        }
    }

    /// Sets the maximum message size to the next or equal power of two of msg_size
    pub fn set_msg_size(&mut self, msg_size: u32) {
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
        
        self.msg_size = req_size;
        self.client[1] = (self.client[1] & 0x0F) | (0xF0);
    }

    pub fn set_serializer(&mut self, serializer: SerializerType) {
        self.serializer = serializer;
        self.client[1] = (self.client[1] & 0xF0) | ((serializer as u8) & 0x0F);
    }

    pub fn srv_resp_bytes(&mut self) -> &mut [u8; 4] {
        &mut self.server
    }

    pub fn validate(&self) -> Result<(), TransportError> {
        if self.server[0] != 0x7f || self.server[2] != 0 || self.server[3] != 0{
            return Err(TransportError::UnexpectedResponse);
        }

        if self.server[1] != self.client[1] {
            // lower 4 bits should be 0 on error
            if self.server[1] & 0x0F != 0 {
                return Err(TransportError::UnexpectedResponse);
            }

            let server_error: u8 = (self.server[1] & 0xF0) >> 4 as u8;
            return Err(
                match server_error {
                    1 => TransportError::SerializerNotSupported(self.serializer),
                    2 => TransportError::InvalidMaximumMsgSize(self.msg_size),
                    4 => TransportError::MaximumServerConn,
                    _ => TransportError::UnexpectedResponse,
                }
            );
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

enum SockWrapper {
    Plain(TcpStream),
    Tls(tokio_tls::TlsStream<TcpStream>),
}
impl SockWrapper {
    pub fn close(&mut self) {
        let sock = match self {
            SockWrapper::Plain(ref mut s) => s,
            SockWrapper::Tls(s) => s.get_mut(),
        };

        match sock.shutdown() {_=>{},};
    }
}

impl SockWrapper {
    pub async fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let res = match self {
            SockWrapper::Plain(ref mut s) => s.write_all(bytes).await,
            SockWrapper::Tls(s) => s.write_all(bytes).await,
        };

        if let Err(e) = res {
            debug!("Failed to send on RawSocket : {:?}", e);
            return Err(TransportError::SendFailed);
        }
        
        Ok(())
    }

    pub async fn read_exact(&mut self, out_bytes: &mut[u8]) -> Result<(), TransportError> {
        let res = match self {
            SockWrapper::Plain(ref mut s) => s.read_exact(out_bytes).await,
            SockWrapper::Tls(s) => s.read_exact(out_bytes).await,
        };

        if let Err(e) = res {
            debug!("Failed to recv on RawSocket : {:?}", e);
            return Err(TransportError::ReceiveFailed);
        }

        Ok(())
    }
}
struct TcpTransport {
    sock: SockWrapper,
}
impl Drop for TcpTransport {
    fn drop(&mut self) {
        self.sock.close();
    }
}

#[async_trait]
impl Transport for TcpTransport {
    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let payload: &[u8] = data.as_ref();
        let header: MsgPrefix = MsgPrefix::new_from(&TcpMsg::Regular, Some(payload.len() as u32));
        
        trace!("Send[0x{:X}] : {:?} ({:?})", std::mem::size_of_val(&header), header.bytes, header);
        self.sock.write_all(&header.bytes).await?;
    
        trace!("Send[0x{:X}] : {:?}", payload.len(), payload);
        self.sock.write_all(payload).await?;

        Ok(())
    }
    
    async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
        let mut payload: Vec<u8>;
        let mut header: MsgPrefix = MsgPrefix::new();
    
        loop {
            self.sock.read_exact(&mut header.bytes).await?;
            trace!("Recv[0x{:X}] : {:?} - ({:?})", std::mem::size_of_val(&header), header, header);
        
            // Validate the 4 byte header
            let msg_type = match header.msg_type() {
                Some(m) => m,
                None => {
                    error!("RawSocket message had an invalid header");
                    return Err(TransportError::ReceiveFailed);
                },
            };
            
            payload = Vec::with_capacity(header.payload_len() as usize);
            unsafe {payload.set_len(header.payload_len() as usize)};
            self.sock.read_exact(&mut payload).await?;
            trace!("Recv[0x{:X}] : {:?}", payload.len(), payload);

            match msg_type {
                TcpMsg::Regular => break,
                _ => continue, //TODO : Handle ping/pong
            }
        }
    
        Ok(payload)
    }

    async fn close(&mut self) {
        self.sock.close();
    }
}

pub async fn connect(host_ip: &str, host_port: u16, is_tls: bool, config: &ClientConfig) -> Result<(Box<dyn Transport + Send>, SerializerType), TransportError> {
    
    let host_addr = format!("{}:{}", host_ip, host_port);
    let mut handshake = HandshakeCtx::new();
    let mut msg_size: u32 = MAX_MSG_SZ;
    if let Some(m) = config.get_max_msg_size() {
        msg_size = m;
    }
    handshake.set_msg_size(msg_size);

    for serializer in config.get_serializers() {
        trace!("Connecting to host : {}", host_addr);
        let mut stream = if is_tls {
            SockWrapper::Tls(connect_tls(host_ip, host_port, config).await?)
        } else {
            SockWrapper::Plain(connect_raw(host_ip, host_port).await?)
        };
        handshake.set_serializer(*serializer);
        trace!("\tSending handshake : {:?}", handshake);
        
        // Preform the WAMP handshake
        if let Err(e) = stream.write_all(handshake.as_ref()).await {
            error!("Failed to send on RawSocket handshake : {:?}", e);
            return Err(TransportError::ConnectionFailed);
        }
        if let Err(e) = stream.read_exact(handshake.srv_resp_bytes()).await {
            error!("RawSocket fail to receive handshake reply : {}", e);
            return Err(TransportError::ConnectionFailed);
        }

        if let Err(e) = handshake.validate() {
            match e {
                TransportError::SerializerNotSupported(_) => {
                    warn!("{:?}", e);
                    stream.close();
                    continue;
                },
                TransportError::InvalidMaximumMsgSize(_) => {
                    error!("{:?}", e);
                    break;
                },
                _ => break,
            };
        }

        return Ok((Box::new(
            TcpTransport {
                sock: stream,
            }
        ), *serializer));
    }

    return Err(TransportError::ConnectionFailed);
}

pub async fn connect_raw(host_ip: &str, host_port: u16) -> Result<TcpStream, TransportError> {
    let host_addr = format!("{}:{}", host_ip, host_port);

    match TcpStream::connect(&host_addr).await {
        Ok(s) => Ok(s),
        Err(e) => {
            error!("Failed to connect to server using raw tcp: {:?}", e);
            return Err(TransportError::ConnectionFailed);
        },
    }
}


pub async fn connect_tls(host_url: &str, host_port: u16, cfg: &ClientConfig) -> Result<tokio_tls::TlsStream<TcpStream>, TransportError> {
    let stream = connect_raw(host_url, host_port).await?;
    let mut tls_cfg = TlsConnector::builder();
    
    if !cfg.get_ssl_verify() {
        tls_cfg.danger_accept_invalid_certs(true);
    }

    let cx = match tls_cfg.build() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create TLS context : {:?}", e);
            return Err(TransportError::ConnectionFailed);
        },
    };
    let cx = tokio_tls::TlsConnector::from(cx);
    match cx.connect(host_url, stream).await {
        Ok(s) => Ok(s),
        Err(e) => {
            error!("Failed to establish TLS handshake : {:?}", e);
            return Err(TransportError::ConnectionFailed);
        }
    }
}