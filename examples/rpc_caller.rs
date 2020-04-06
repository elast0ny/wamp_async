use std::error::Error;
use wamp_async::{Arg, Client, ClientConfig, ClientRole, SerializerType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Connect to the server
    let (mut client, (evt_loop, _rpc_evt_queue)) = Client::connect(
        "wss://localhost:8080/ws",
        Some(
            ClientConfig::default()
                .set_ssl_verify(false)
                // Restrict our roles
                .set_roles(vec![ClientRole::Caller])
                // Only use Json serialization
                .set_serializers(vec![SerializerType::Json]),
        ),
    )
    .await?;
    println!("Connected !!");

    // Spawn the event loop
    tokio::spawn(evt_loop);

    println!("Joining realm");
    client.join_realm("realm1").await?;

    let mut num_calls: u64 = 0;
    loop {
        println!("Calling 'peer.echo'");

        // Call the RPC endpoint and wait for the result
        match client
            .call("peer.echo", Some(vec![Arg::Integer(12)]), None)
            .await
        {
            Ok((res_args, res_kwargs)) => println!("\tGot {:?} {:?}", res_args, res_kwargs),
            Err(e) => {
                println!("Error calling ({:?})", e);
                break;
            }
        };

        // Stop after calling 5 times
        num_calls += 1;
        if num_calls >= 5 {
            break;
        }
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
    }

    println!("Leaving realm");
    client.leave_realm().await?;

    client.disconnect().await;
    Ok(())
}
