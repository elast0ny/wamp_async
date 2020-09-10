use std::error::Error;

use serde::{Deserialize, Serialize};

use wamp_async::{
    try_into_any_value, Client, ClientConfig, ClientRole, SerializerType, WampKwArgs,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MyStruct {
    field1: String,
}

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

    let my_struct = MyStruct {
        field1: "value1".to_string(),
    };
    let positional_args = vec![
        12i64.into(),
        13.3f64.into(),
        u32::MAX.into(),
        i32::MIN.into(),
        u64::MAX.into(),
        "str".into(),
        vec![-1].into(),
        try_into_any_value(&my_struct).unwrap(),
    ];
    let mut keyword_args = WampKwArgs::new();
    keyword_args.insert("key".to_string(), try_into_any_value(&my_struct).unwrap());

    for (send_args, send_kwargs) in vec![
        (None, None),
        (Some(positional_args.clone()), None),
        (None, Some(keyword_args.clone())),
        (Some(positional_args.clone()), Some(keyword_args.clone())),
    ] {
        let send_args_copy = send_args.clone();
        let send_kwargs_copy = send_kwargs.clone();

        // Call the RPC endpoint and wait for the result
        println!(
            "Calling 'peer.echo' with {:?} and {:?}",
            send_args, send_kwargs
        );
        match client.call("peer.echo", send_args, send_kwargs).await {
            Ok((res_args, res_kwargs)) => {
                println!("\tGot {:?} {:?}", res_args, res_kwargs);
                assert_eq!(res_args, send_args_copy);
                assert_eq!(res_kwargs, send_kwargs_copy);
            }
            Err(e) => {
                println!("Error calling ({:?})", e);
                break;
            }
        };

        println!();
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
    }

    println!("Leaving realm");
    client.leave_realm().await?;

    client.disconnect().await;
    Ok(())
}
