# wamp_async

[![crates.io](https://img.shields.io/crates/v/wamp_async.svg)](https://crates.io/crates/wamp_async)
[![mio](https://docs.rs/wamp_async/badge.svg)](https://docs.rs/wamp_async/)
![Lines of Code](https://tokei.rs/b1/github/elast0ny/wamp_async)

An asynchronous [WAMP](https://wamp-proto.org/) client implementation written in rust.

## Usage

For usage examples, see :

- [Publisher/Subscriber](https://github.com/elast0ny/wamp_async/blob/master/examples/pubsub.rs)

  ```rust
  // Publish event with no arguments and with acknowledgment
  let ack_id = client.publish("peer.heartbeat", None, None, true).await?;
  println!("Ack id {}", ack_id.unwrap());
  ```

  ```rust
  // Register for events
  let (_sub_id, mut event_queue) = client.subscribe("peer.heartbeat", SubscribeOptions::empty()).await?;
  // Wait for the next event
  match event_queue.recv().await {
      Some((_pub_id, _details, args, kwargs)) => println!("Event(args: {:?}, kwargs: {:?})", args, kwargs),
      None => println!("Event queue closed"),
  };
  ```

- RPC [caller](https://github.com/elast0ny/wamp_async/blob/master/examples/rpc_caller.rs) & [callee](https://github.com/elast0ny/wamp_async/blob/master/examples/rpc_callee.rs)

  ```rust
  // Call endpoint with one argument
  let (args, kwargs) = client.call("peer.echo", Some(vec![12.into()]), None).await?;
  println!("RPC returned {:?} {:?}", args, kwargs);
  ```

  ```rust
  // Declare your RPC function
  async fn rpc_echo(args: Option<WampArgs>, kwargs: Option<WampKwArgs>) -> Result<(Option<WampArgs>, Option<WampKwArgs>), WampError> {
      println!("peer.echo {:?} {:?}", args, kwargs);
      Ok((args, kwargs))
  }

  // Register the function
  let rpc_id = client.register("peer.echo", rpc_echo).await?;
  ```

## Structs Serialization and Deserialization

```rust
// Call endpoint with one argument
let (args, kwargs) = client.call("peer.echo", Some(vec![12.into()]), None).await?;
// or
let (args, kwargs) = client.call("peer.echo", Some(wamp_async::try_into_args((12,))), None).await?;
println!("RPC returned {:?} {:?}", args, kwargs);
```

```rust
#[derive(serde::Deserialize, serde::Serialize)]
struct MyKwArgs {
    name: String,
}

// Declare your RPC function
async fn rpc_echo(args: Option<WampArgs>, kwargs: Option<WampKwArgs>) -> Result<(Option<WampArgs>, Option<WampKwArgs>), WampError> {
    // You only need serde-deserializable structure (e.g. a tuple of two integers)
    let valid_args: (i32, i32) = if let Some(args) = args {
        wamp_async::try_from_args(args)?
    } else {
        return Err(wamp_async::WampError::UnknownError("positional args are required".to_string()));
    };
    println!("Two integers are: {}, {}", valid_args.0, valid_args.1);

    // You can also use a custom struct and use a little bit of Rust helpers
    let valid_kwargs: Option<MyKwArgs> = kwargs.map(wamp_async::try_from_kwargs).transpose()?;

    if let Some(MyKwArgs { name }) = valid_kwargs {
        println!("Name is {}", name);
    } else {
        println!("There were no kwargs specified");
    }

    Ok((
        Some(wamp_async::try_into_args(valid_args)?),
        valid_kwargs.map(wamp_async::try_into_kwargs).transpose()?,
    ))
}

// Register the function
let rpc_id = client.register("peer.echo", rpc_echo).await?;
```

## Features

| Feature          | Desciption                                                                             | Status |
| ---------------- | -------------------------------------------------------------------------------------- | ------ |
| Websocket        | Use [websocket](https://en.wikipedia.org/wiki/WebSocket) as the transport              | ✔      |
| Secure Websocket | Websocket over HTTPS                                                                   | ✔      |
| RawSocket        | Use lightweight TCP as the transport                                                   | ✔      |
| Secure RawSocket | RawSocket with TLS                                                                     | ✔      |
| MsgPack          | Use [MessagePack](https://en.wikipedia.org/wiki/MessagePack) for message serialization | ✔      |
| JSON             | Uses [JSON](https://en.wikipedia.org/wiki/JSON#Example) for message serialization      | ✔      |

### Client

#### Basic profile :

| Feature    | Desciption                                   | Status |
| ---------- | -------------------------------------------- | ------ |
| Publisher  | Ability to publish messages on topics        | ✔      |
| Subscriber | Can subscribe and receive events for a topic | ✔      |
| Caller     | Ability to call RPC endpoints                | ✔      |
| Callee     | Ability to register RPC endpoints            | ✔      |

#### Advanced profile:

| Feature                     | Desciption                                                      | Status      |
| --------------------------- | --------------------------------------------------------------- | ----------- |
| Client Authentication       | Low-level support for Client Authentication (Ticket-based, CRA) | ✔           |
| Pattern Based Subscriptions | Wildcard, and Prefix matches for channel topics                 | ✔           |
| Progressive Calls           | Partial results reported from Callee to Caller                  | help wanted |

## License

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](http://opensource.org/licenses/MIT)

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
