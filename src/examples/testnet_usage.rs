use near_event_listener::NearEventListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listener = NearEventListener::builder("https://rpc.testnet.near.org")
        .account_id("account.testnet")
        .method_name("method_to_listen")
        .last_processed_block(0)
        .build()?;

    listener.start(|event_log| {
        println!("Standard: {}", event_log.standard);
        println!("Version: {}", event_log.version);
        println!("Event: {}", event_log.event);
        println!("Data: {}", data);
     }
    ).await;
    
    Ok(())
}