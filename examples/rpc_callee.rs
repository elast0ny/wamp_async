use std::error::Error;
use std::sync::atomic::{AtomicU8, Ordering};

use lazy_static::*;

use wamp_async::{Client, WampArgs, WampKwArgs, WampError};

lazy_static! {
    static ref RPC_CALL_COUNT: AtomicU8 = {AtomicU8::new(0)};
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
    let mut client = Client::connect("tcp://localhost:8081", None).await?;
    println!("Connected !!");

    let (evt_loop, rpc_event_queue) = client.event_loop()?;

    // Spawn the event loop
    tokio::spawn(evt_loop);
    // Handle RPC events in seperate tasks
    tokio::spawn(async move {
        let mut rpc_event_queue = rpc_event_queue.unwrap();
        loop {
            // Wait for an RPC call
            let rpc_event = match rpc_event_queue.recv().await {
                Some(e) => e,
                None => break,
            };

            //Run it in its own task
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
        if call_num >= 5 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // unregister the rpc endpoint
    client.unregister(rpc_id).await?;

    println!("Leaving realm");
    client.leave_realm().await?;

    client.disconnect().await;
    Ok(())
}