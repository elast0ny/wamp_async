use log::*;
use tokio_tungstenite::{
    tungstenite::{Message, handshake::client::Request},
    client_async, WebSocketStream,
    stream::Stream,
};
use async_trait::async_trait;
use tokio::net::TcpStream;
use futures::{SinkExt, StreamExt};

use crate::client::ClientConfig;
use crate::transport::{Transport, TransportError};
use crate::serializer::SerializerType;

struct WsCtx {
    is_bin: bool,
    client: WebSocketStream<Stream<TcpStream, tokio_tls::TlsStream<TcpStream>>>,
}

#[async_trait]
impl Transport for WsCtx {
    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let payload: &[u8] = data.as_ref();
        
        trace!("Send[0x{:X}] : {:?}", payload.len(), payload);
        let res = if self.is_bin {
            self.client.send(Message::Binary(Vec::from(payload))).await
        } else {
            let str_payload = std::str::from_utf8(payload).unwrap().to_owned();
            trace!("Text('{}')", str_payload);
            self.client.send(Message::Text(str_payload)).await
        };

        if let Err(e) = res {
            error!("Failed to send on websocket : {:?}", e);
            return Err(TransportError::SendFailed);
        }

        Ok(())
    }
    
    async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
        let payload;
        // Receive a message
        loop {
            let msg: Message = match self.client.next().await {
                Some(Ok(m)) => m,
                Some(Err(e)) => {
                    error!("Failed to recv from websocket : {:?}", e);
                    return Err(TransportError::ReceiveFailed);
                },
                None => return Err(TransportError::ReceiveFailed),
            };

            trace!("Recv[] : {:?}", msg);

            payload = match msg {
                Message::Text(s) => {
                    if self.is_bin {
                        error!("Got websocket Text message but only Binary is allowed");
                        return Err(TransportError::UnexpectedResponse);
                    }
                    Vec::from(s.as_bytes())
                },
                Message::Binary(b) => {
                    if !self.is_bin {
                        error!("Got websocket Binary message but only Text is allowed");
                        return Err(TransportError::UnexpectedResponse);
                    }
                    b
                },
                Message::Ping(d) => {
                    if let Err(e) = self.client.send(Message::Pong(d)).await {
                        error!("Failed to respond to websocket Ping : {:?}", e);
                        return Err(TransportError::UnexpectedResponse);
                    }
                    continue;
                },
                _ => {
                    error!("Unexpected websocket message type : {:?}", msg);
                    return Err(TransportError::UnexpectedResponse)
                }
            };

            break;
        }
    
        Ok(payload)
    }

    async fn close(&mut self) {
        match self.client.close(None) {_=>{/*ignore result*/},};
    }
}

pub async fn connect(url: &url::Url, config: &ClientConfig) -> Result<(Box<dyn Transport + Send>, SerializerType), TransportError> {
    let mut request = Request::builder().uri(url.as_ref());

    if config.get_agent().len() > 0 {
        request = request.header("User-Agent", config.get_agent());
    }

    let serializer_list = config.get_serializers().iter().map(|x| x.to_str()).collect::<Vec<&str>>().join(",");
    request = request.header("Sec-WebSocket-Protocol", serializer_list);

    let sock = match url.scheme() {
        "ws" => {
            Stream::Plain(crate::transport::tcp::connect_raw(url.host_str().unwrap(), url.port_or_known_default().unwrap()).await?)
        },
        "wss" => {
            Stream::Tls(crate::transport::tcp::connect_tls(url.host_str().unwrap(), url.port_or_known_default().unwrap(), config).await?)
        },
        _ => panic!("ws::connect called but uri doesnt have websocket scheme"),
    };
    
    let (client,resp) = match client_async(request.body(()).unwrap(), sock).await {
        Ok(v) => v,
        Err(e) => {
            error!("Websocket failed to connect : {:?}", e);
            return Err(TransportError::ConnectionFailed)
        },
    };

    let mut picked_serializer: SerializerType = SerializerType::Invalid;
    for (key, value) in resp.headers().iter() {
        let val = match value.to_str() {
            Ok(v) => v,
            Err(_) => continue,
        };
        trace!("Header '{}' = '{}'", key.as_str(), val);
        if key.as_str().to_lowercase() == "sec-websocket-protocol" {
            picked_serializer = SerializerType::from_str(val);
            break;
        }
    }

    match picked_serializer {
        SerializerType::Invalid => {
            return Err(TransportError::SerializerNotSupported(picked_serializer))
        },
        _ => {},
    };

    Ok((
        Box::new(WsCtx {
        is_bin: match picked_serializer {
            SerializerType::MsgPack => true,
            _ => false,
        },
        client,
    }), picked_serializer))
}