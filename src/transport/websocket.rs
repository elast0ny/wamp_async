use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use log::*;
use std::str::FromStr;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    client_async,
    tungstenite::{handshake::client::Request, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::client::ClientConfig;
use crate::serializer::SerializerType;
use crate::transport::{Transport, TransportError};

struct WsCtx {
    is_bin: bool,
    client: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

#[async_trait]
impl Transport for WsCtx {
    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        trace!("Send[0x{:X}] : {:?}", data.len(), data);
        let res = if self.is_bin {
            self.client.send(Message::Binary(Vec::from(data))).await
        } else {
            let str_payload = std::str::from_utf8(data).unwrap().to_owned();
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
                }
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
                }
                Message::Binary(b) => {
                    if !self.is_bin {
                        error!("Got websocket Binary message but only Text is allowed");
                        return Err(TransportError::UnexpectedResponse);
                    }
                    b
                }
                Message::Ping(d) => {
                    if let Err(e) = self.client.send(Message::Pong(d)).await {
                        error!("Failed to respond to websocket Ping : {:?}", e);
                        return Err(TransportError::UnexpectedResponse);
                    }
                    continue;
                }
                _ => {
                    error!("Unexpected websocket message type : {:?}", msg);
                    return Err(TransportError::UnexpectedResponse);
                }
            };

            break;
        }

        Ok(payload)
    }

    async fn close(&mut self) {
        match self.client.close(None) {
            _ => { /*ignore result*/ }
        };
    }
}

pub async fn connect(
    url: &url::Url,
    config: &ClientConfig,
) -> Result<(Box<dyn Transport + Send>, SerializerType), TransportError> {
    let mut request = Request::builder().uri(url.as_ref());

    if !config.get_agent().is_empty() {
        request = request.header("User-Agent", config.get_agent());
    }

    let serializer_list = config
        .get_serializers()
        .iter()
        .map(|x| x.to_str())
        .collect::<Vec<&str>>()
        .join(",");
    request = request.header("Sec-WebSocket-Protocol", serializer_list);

    for (key, value) in config.get_websocket_headers() {
        request = request.header(key, value);
    }

    let sock = match url.scheme() {
        "ws" => MaybeTlsStream::Plain(
            crate::transport::tcp::connect_raw(
                url.host_str().unwrap(),
                url.port_or_known_default().unwrap(),
            )
            .await?,
        ),
        "wss" => MaybeTlsStream::NativeTls(
            crate::transport::tcp::connect_tls(
                url.host_str().unwrap(),
                url.port_or_known_default().unwrap(),
                config,
            )
            .await?,
        ),
        _ => panic!("ws::connect called but uri doesnt have websocket scheme"),
    };

    let (client, resp) = match client_async(request.body(()).unwrap(), sock).await {
        Ok(v) => v,
        Err(e) => {
            error!("Websocket failed to connect : {:?}", e);
            return Err(TransportError::ConnectionFailed);
        }
    };

    let mut picked_serializer: Option<SerializerType> = None;
    for (key, value) in resp.headers().iter() {
        let val = match value.to_str() {
            Ok(v) => v,
            Err(_) => continue,
        };
        trace!("Header '{}' = '{}'", key.as_str(), val);
        if key.as_str().to_lowercase() == "sec-websocket-protocol" {
            let header_se = match SerializerType::from_str(val) {
                Ok(s) => s,
                Err(e) => {
                    //Hope that theres another serializer we support in the header
                    warn!("{:?}", e);
                    continue;
                }
            };
            picked_serializer = Some(header_se);
            break;
        }
    }

    let picked_serializer = match picked_serializer {
        Some(s) => s,
        None => {
            return Err(TransportError::SerializerNotSupported(
                "<unknown>".to_string(),
            ))
        }
    };

    Ok((
        Box::new(WsCtx {
            is_bin: match picked_serializer {
                SerializerType::MsgPack => true,
                SerializerType::Cbor=> true,
                _ => false,
            },
            client,
        }),
        picked_serializer,
    ))
}
