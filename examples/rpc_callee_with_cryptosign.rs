use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};

use lazy_static::*;

use wamp_async::{
    Client, ClientConfig, ClientState, SerializerType, WampArgs, WampError, WampKwArgs,
};

lazy_static! {
    static ref RPC_CALL_COUNT: AtomicU64 = AtomicU64::new(0);
}

// Simply return the rpc arguments
async fn echo(
    args: Option<WampArgs>,
    kwargs: Option<WampKwArgs>,
) -> Result<(Option<WampArgs>, Option<WampKwArgs>), WampError> {
    println!("peer.echo {:?} {:?}", args, kwargs);
    let _ = RPC_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
    Ok((args, kwargs))
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct MyArgs(i64, i64);

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct MyKwArgs {
    name: String,
    age: u8,
}

// Validate structure and return the rpc arguments
async fn strict_echo(
    args: Option<WampArgs>,
    kwargs: Option<WampKwArgs>,
) -> Result<(Option<WampArgs>, Option<WampKwArgs>), WampError> {
    println!("peer.strict_echo raw input {:?} {:?}", args, kwargs);

    let valid_args: Option<MyArgs> = args.map(wamp_async::try_from_args).transpose()?;
    let valid_kwargs: Option<MyKwArgs> = kwargs.map(wamp_async::try_from_kwargs).transpose()?;
    println!(
        "peer.strict_echo deserialized input {:?} {:?}",
        valid_args, valid_kwargs
    );

    let _ = RPC_CALL_COUNT.fetch_add(1, Ordering::Relaxed);

    Ok((
        valid_args.map(wamp_async::try_into_args).transpose()?,
        valid_kwargs.map(wamp_async::try_into_kwargs).transpose()?,
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Connect to the server
    let (mut client, (evt_loop, rpc_evt_queue)) = Client::connect(
        "ws://0.0.0.0:8080/ws",
        Some(
            ClientConfig::default()
                // Allow invalid/self signed certs
                .set_ssl_verify(false)
                // Use MsgPack first or fallback to Json
                .set_serializers(vec![SerializerType::Json]),
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

    println!("Joining with cryptosign");
    client
        .join_realm_with_cryptosign(
            "realm",
            "id",
            String::from("public_key"),
            String::from("private_key"),
        ).await?;

    // Register our functions to a uri
    let echo_rpc_id = client.register("peer.echo", echo).await?;
    let strict_echo_rpc_id = client.register("peer.strict_echo", strict_echo).await?;

    println!("Waiting for 'peer.echo' to be called at least 4 times");
    loop {
        let call_num = RPC_CALL_COUNT.load(Ordering::Relaxed);
        if call_num >= 4 || !client.is_connected() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    // Client should not have disconnected
    if let ClientState::Disconnected(Err(e)) = client.get_cur_status() {
        println!("Client disconnected because of : {:?}", e);
        return Err(From::from("Unexpected disconnect".to_string()));
    }

    client.unregister(echo_rpc_id).await?;
    client.unregister(strict_echo_rpc_id).await?;

    println!("Leaving realm");
    client.leave_realm().await?;

    client.disconnect().await;

    Ok(())
}
