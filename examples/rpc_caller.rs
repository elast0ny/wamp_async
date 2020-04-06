use std::error::Error;
use wamp_async::{Arg, Client, ClientConfig, ClientRole, SerializerType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let mut client = Client::connect(
        "wss://localhost:8080",
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

    let (evt_loop, _) = client.event_loop()?;

    let (wait_event_loop_tx, wait_event_tool_rx) = tokio::sync::oneshot::channel();

    // Spawn the event loop
    tokio::spawn(async move {
        wait_event_loop_tx.send(()).unwrap();
        evt_loop.await
    });

    wait_event_tool_rx.await?;

    println!("Joining realm");
    client.join_realm("realm1").await?;

    // Call an RPC endpoint 5 times
    let mut num_calls: u64 = 0;
    loop {
        println!("Calling 'peer.echo'");
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
