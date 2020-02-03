use std::error::Error;
use wamp_async::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let mut client = Client::connect("tcp://localhost:8081", None).await?;
    println!("Connected !!");

    // Spawn the event loop
    tokio::spawn(client.event_loop()?);

    println!("Joining realm");
    client.join_realm("realm1").await?;

    let max_events = 10;
    let mut cur_event_num: usize = 0;

    // If one of the args is "pub", start as a publisher
    if let Some(_) = std::env::args().find(|a| a == "pub") {
        loop {
            match client.publish("peer.heartbeat", None, None, true).await {
                Ok(pub_id) => println!("\tSending hearbeat {:X}", pub_id),
                Err(e) => {
                    println!("publish error {}", e);
                    break;
                }
            }
            cur_event_num += 1;
            //Exit before sleeping
            if cur_event_num >= max_events {
                break;
            }
            tokio::time::delay_for(std::time::Duration::from_secs(2)).await
        }  
    // Start as a subscriber
    } else {
        println!("Subscribing to peer.heartbeat events. Start another instance with a 'pub' argument");
        let (sub_id, mut heartbeat_queue) = client.subscribe("peer.heartbeat").await?;
        println!("Waiting for {} heartbeats...", max_events);

        while cur_event_num < max_events {
            match heartbeat_queue.recv().await {
                Some((pub_id, args, kwargs)) => println!("\tGot {:X} (args: {:?}, kwargs: {:?})", pub_id, args, kwargs),
                None => println!("Subscription is done"),
            };
            cur_event_num += 1;
        }

        client.unsubscribe(sub_id).await?;
    }
    
    println!("Leaving realm");
    client.leave_realm().await?;

    client.disconnect().await;
    Ok(())
}