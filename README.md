# wamp_async

An asynchronous [WAMP](https://wamp-proto.org/) implementation written in rust.

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
`wamp_async` is primarily written for those who want to write WAMP __client__ applications using async Rust. There is currently no plan to implement a `wamp_async` based server.

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

Not implemented yet.