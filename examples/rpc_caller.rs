use std::error::Error;
use wamp_async::{Client, WampArgs, WampKwArgs, WampError, Arg};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let mut client = Client::connect("tcp://localhost:8081", None).await?;
    println!("Connected !!");

    let (evt_loop, _) = client.event_loop()?;

    // Spawn the event loop
    tokio::spawn(evt_loop);
    
    println!("Joining realm");
    client.join_realm("realm1").await?;

    // Call an RPC endpoint 5 times
    let mut num_calls: u8 = 0;
    loop {
        println!("Calling 'peer.echo'");
        match client.call("peer.echo", Some(vec![Arg::Integer(12)]), None).await {
            Ok(v) => println!("\tGot {:?} {:?}", v.0, v.1),
            Err(e) => {
                println!("Error calling ({})", e);
                break;
            }
        };
        num_calls += 1;
        if num_calls >= 5 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    println!("Leaving realm");
    client.leave_realm().await?;

    client.disconnect().await;
    Ok(())
}