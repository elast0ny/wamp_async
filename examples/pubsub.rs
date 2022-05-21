use std::error::Error;
use wamp_async::{Client, ClientConfig, OptionBuilder, SubscribeOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Connect to the server
    let (mut client, (evt_loop, _rpc_evt_queue)) = Client::connect(
        "wss://localhost:8080/ws",
        Some(ClientConfig::default().set_ssl_verify(false)),
    )
    .await?;
    println!("Connected !!");

    // Spawn the event loop
    tokio::spawn(evt_loop);

    println!("Joining realm");
    client.join_realm("realm1").await?;

    let max_events = 10;
    let mut cur_event_num: usize = 0;

    // If one of the args is "pub", start as a publisher
    if let Some(_) = std::env::args().find(|a| a == "pub") {
        loop {
            match client.publish("peer.heartbeat", None, None, true).await {
                Ok(pub_id) => println!("\tSent event id {}", pub_id.unwrap()),
                Err(e) => {
                    println!("publish error {}", e);
                    break;
                }
            };
            cur_event_num += 1;
            // Exit before sleeping
            if cur_event_num >= max_events {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await
        }
    // Start as a subscriber
    } else {
        println!(
            "Subscribing to peer.heartbeat events. Start another instance with a 'pub' argument"
        );
        let (sub_id, mut heartbeat_queue) = client.subscribe("peer.heartbeat", SubscribeOptions::new()).await?;
        println!("Waiting for {} heartbeats...", max_events);

        while cur_event_num < max_events {
            match heartbeat_queue.recv().await {
                Some((pub_id, details, args, kwargs)) => {
                    println!("\tGot {} (details: {:?}, args: {:?}, kwargs: {:?})", pub_id, details args, kwargs)
                }
                None => println!("Subscription is done"),
            };
            cur_event_num += 1;
        }

        client.unsubscribe(sub_id).await?;
    }

    println!("Leaving realm");
    client.leave_realm().await?;

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    client.disconnect().await;
    Ok(())
}
