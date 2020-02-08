# wamp_async

An asynchronous [WAMP](https://wamp-proto.org/) implementation written in rust.

## Features
`wamp_async` is primarily written for those who want to write WAMP __client__ applications using async Rust. In fact, there is currently no plan to implement a `wamp_sync` based server.

| Feature | Desciption | Status |
|---------|------------|--------|
|Websocket| Use [websocket](https://en.wikipedia.org/wiki/WebSocket) as the transport | TODO |
|Secure Websocket| Websocket over HTTPS | TODO |
| RawSocket | Use lightweight TCP as the transport | ✔ |
| Secure RawSocket | RawSocket with TLS | TODO |
|MsgPack| Use [MessagePack](https://en.wikipedia.org/wiki/MessagePack) for message serialization | TODO |
|JSON | Uses [JSON](https://en.wikipedia.org/wiki/JSON#Example) for message serialization | ✔ |
### Client
Basic profile :
| Feature | Desciption | Status |
|---------|------------|--------|
| Publisher | Ability to publish messages on topics | ✔ |
| Subscriber | Can subscribe and receive events for a topic | ✔ |
| Caller | Ability to call RPC endpoints | TODO |
| Callee | Ability to register RPC endpoints | TODO |

Advanced profile:
| Feature | Desciption | Status |
|---------|------------|--------|
| Publisher | Ability to publish messages on topics | ✔ |
| Subscriber | Can subscribe and receive events for a topic | ✔ |
| Caller | Ability to call RPC endpoints | ✔ |
| Callee | Ability to register RPC endpoints | ✔ |

### Server
Not implemented yet.