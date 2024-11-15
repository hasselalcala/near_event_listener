use near_event_listener::NearEventListener;
use near_workspaces::sandbox;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let worker = sandbox().await?;
    let rpc_address = worker.rpc_addr();

    let mut listener = NearEventListener::builder(&rpc_address)
        .account_id("contract-account.near")
        .method_name("set_greeting")
        .last_processed_block(0)
        .build()?;

    listener.start(|event_log| {
        // User can process the data as they prefer
        println!("Received event:");
        println!("Standard: {}", event_log.standard);
        println!("Version: {}", event_log.version);
        println!("Event: {}", event_log.event);
        
        // Examples of how the user can process data
        match event_log.data {
            Value::Array(arr) => {
                for item in arr {
                    if let Some(greeting) = item.get("greeting") {
                        println!("Greeting: {}", greeting);
                    }
                }
            },
            Value::Object(obj) => {
                if let Some(greeting) = obj.get("greeting") {
                    println!("Greeting: {}", greeting);
                }
            },
            _ => println!("Data en otro formato: {:?}", event_log.data),
        }
    }).await?;

    Ok(())
}