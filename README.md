# wamp_async
[![crates.io](https://img.shields.io/crates/v/wamp_async.svg)](https://crates.io/crates/wamp_async)
[![mio](https://docs.rs/wamp_async/badge.svg)](https://docs.rs/wamp_async/)
![Lines of Code](https://tokei.rs/b1/gitlab/elast0ny/wamp_async-rs)

An asynchronous [WAMP](https://wamp-proto.org/) client implementation written in rust.

## Usage

For usage examples, see :
- [Publisher/Subscriber](https://gitlab.com/elast0ny/wamp_async-rs/-/blob/master/examples/pubsub.rs)
```rust
match client.publish("peer.heartbeat", None, None, true).await {
    Ok(pub_id) => println!("\tPublish id {:X}", pub_id),
    Err(e) => println!("publish error {}", e),
};
//<...>
let (_sub_id, mut event_queue) = client.subscribe("peer.heartbeat").await?;

match event_queue.recv().await {
    Some((_pub_id, args, kwargs)) => println!("\tEvent(args: {:?}, kwargs: {:?})", args, kwargs),
    None => println!("event queue is closed"),
};
```
- [RPC caller](https://gitlab.com/elast0ny/wamp_async-rs/-/blob/master/examples/rpc_caller.rs)
```rust
match client.call("peer.echo", Some(vec![Arg::Integer(12)]), None).await {
    Ok((res_args, res_kwargs)) => println!("\tGot {:?} {:?}", res_args, res_kwargs),
    Err(e) => println!("Error calling ({:?})", e),
};
```
- [RPC callee](https://gitlab.com/elast0ny/wamp_async-rs/-/blob/master/examples/rpc_callee.rs)
```rust
async fn rpc_echo(args: WampArgs, kwargs: WampKwArgs) -> Result<(WampArgs, WampKwArgs), WampError> {
    println!("peer.echo {:?} {:?}", args, kwargs);
    Ok((args, kwargs))
}
//<...>
let rpc_id = client.register("peer.echo", rpc_echo).await?;
```

## Features
| Feature | Desciption | Status |
|---------|------------|--------|
|Websocket| Use [websocket](https://en.wikipedia.org/wiki/WebSocket) as the transport | ✔ |
|Secure Websocket| Websocket over HTTPS | ✔ |
| RawSocket | Use lightweight TCP as the transport | ✔ |
| Secure RawSocket | RawSocket with TLS | ✔ |
|MsgPack| Use [MessagePack](https://en.wikipedia.org/wiki/MessagePack) for message serialization | ✔ |
|JSON | Uses [JSON](https://en.wikipedia.org/wiki/JSON#Example) for message serialization | ✔ |
### Client
#### Basic profile :

| Feature | Desciption | Status |
|---------|------------|--------|
| Publisher | Ability to publish messages on topics | ✔ |
| Subscriber | Can subscribe and receive events for a topic | ✔ |
| Caller | Ability to call RPC endpoints | ✔ |
| Callee | Ability to register RPC endpoints | ✔ |

#### Advanced profile:

TODO

## License

 * [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
 * [MIT license](http://opensource.org/licenses/MIT)

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
