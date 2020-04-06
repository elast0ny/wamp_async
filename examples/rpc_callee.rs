use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};

use lazy_static::*;

use wamp_async::{
    Client, ClientConfig, ClientState, SerializerType, WampArgs, WampError, WampKwArgs,
};

lazy_static! {
    static ref RPC_CALL_COUNT: AtomicU64 = { AtomicU64::new(0) };
}

// Simply return the rpc arguments
async fn echo(args: WampArgs, kwargs: WampKwArgs) -> Result<(WampArgs, WampKwArgs), WampError> {
    println!("peer.echo {:?} {:?}", args, kwargs);
    let _ = RPC_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
    Ok((args, kwargs))
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

    // Register our function to a uri
    let rpc_id = client.register("peer.echo", echo).await?;

    println!("Waiting for 'peer.echo' to be called at least 5 times");
    loop {
        let call_num = RPC_CALL_COUNT.load(Ordering::Relaxed);
        if call_num >= 5 || !client.is_connected() {
            break;
        }
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
    }

    // Client should not have disconnected
    if let ClientState::Disconnected(Err(e)) = client.get_cur_status() {
        println!("Client disconnected because of : {:?}", e);
        return Err(From::from("Unexpected disconnect".to_string()));
    }

    client.unregister(rpc_id).await?;

    println!("Leaving realm");
    client.leave_realm().await?;

    client.disconnect().await;

    Ok(())
}
