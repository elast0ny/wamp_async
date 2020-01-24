use std::error::Error;

use wamp_async::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let client = WampClient::connect("tcp://localhost:8081", Some("realm1"), None).await?;

    println!("Connected !!");

    /*  Subscribe to a uri. 
        Returns an mpsc where the pub events will forwarded to.
    */
    // let events_queue = client.subscribe(uri).await?;
    // events_queue.get()
    
    /*  Subscribe to a uri. 
        Returns a closure that implements calling the callback on pub events.
        The caller is responsible to either block on the closure or run it in a task.
    */
    // let sub_callback = client.subscribe_callback(uri, callback);
    // tokio::spawn(sub_callback);


    client.shutdown().await;

    Ok(())
}