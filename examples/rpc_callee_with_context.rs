use std::error::Error;
use std::sync::{Arc, RwLock};

use wamp_async::{Client, ClientConfig, SerializerType, WampArgs, WampKwArgs};

#[derive(Debug)]
struct MyState {
    calls_count: usize,
}

// This is the wrapping boilerplate to pass `my_state` into the closure.
// `wamp_async` registeres the handlers for 'static lifetime, so we have to
// wrap the context (`my_state`) with a reference counter (`Arc`) and mutex (`RwLock`),
// and then we have to *move* the context into the closure, and to bump the
// reference counter (`Arc::clone`) on every call to the handler, and *move*
// the cloned value into the async block which also needs to be pinned.
fn echo_with_context(
    wamp_client: Arc<Client>,
    my_state: Arc<RwLock<MyState>>,
) -> wamp_async::RpcFunc {
    Box::new(
        move |args: Option<WampArgs>, kwargs: Option<WampKwArgs>| -> wamp_async::RpcFuture {
            let wamp_client = Arc::clone(&wamp_client);
            let my_state = Arc::clone(&my_state);
            Box::pin(async move {
                // This is the original implementation
                println!("peer.echo {:?} {:?}", args, kwargs);
                // We can mutate the state behind RwLock
                {
                    let mut my_state = my_state.write().unwrap();
                    my_state.calls_count += 1;
                    println!("{:?}", *my_state);
                }
                // We can asynchronously perform any other WAMP action,
                // e.g. recursively call ourselves.
                wamp_client.call("peer.echo", None, None).await.unwrap();

                Ok((args, kwargs))
            })
        },
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Connect to the server
    let (mut client, (evt_loop, rpc_evt_queue)) = Client::connect(
        "wss://localhost:8080/ws",
        Some(
            ClientConfig::default()
                // Allow invalid/self signed certs
                .set_ssl_verify(false)
                // Use MsgPack first or fallback to Json
                .set_serializers(vec![SerializerType::MsgPack, SerializerType::Json]),
        ),
    )
    .await?;
    println!("Connected !!");

    // Spawn the event loop
    tokio::spawn(evt_loop);

    // Handle RPC events in separate tasks
    tokio::spawn(async move {
        let mut rpc_event_queue = rpc_evt_queue.unwrap();
        loop {
            // Wait for an RPC call
            let rpc_event = match rpc_event_queue.recv().await {
                Some(e) => e,
                None => break,
            };

            // Execute the function call
            tokio::spawn(rpc_event);
        }
    });

    println!("Joining realm");
    client.join_realm("realm1").await?;
    let client = Arc::new(client);

    let my_state = MyState { calls_count: 0 };
    // Register our function to a uri
    client
        .register(
            "peer.echo",
            echo_with_context(Arc::clone(&client), Arc::new(RwLock::new(my_state))),
        )
        .await?;

    tokio::time::delay_for(std::time::Duration::from_secs(10 * 60)).await;

    Ok(())
}
