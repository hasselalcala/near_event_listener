# NEAR Event Listener

A robust event listening solution for the NEAR Protocol that enables real-time monitoring of smart contract events with customizable callbacks.

## Overview

NEAR Event Listener is a Rust library that facilitates the monitoring of smart contract events on the NEAR blockchain. It provides a simple yet powerful interface to track specific method calls and their associated events, with support for both TestNet and Sandbox environments.

### Key Features

* **Flexible Event Monitoring**: Listen to specific methods on any NEAR smart contract
* **Real-time Updates**: Continuous polling of new blocks for event detection
* **Customizable Callbacks**: Process events with user-defined callback functions
* **Error Handling**: Robust error management with custom error types
* **Multiple Environments**: Support for TestNet and Sandbox testing
* **Builder Pattern**: Easy-to-use builder pattern for listener configuration

## How it Works

### Event Listening Flow

1. **Initialization**:
   * Create listener instance with:
     * RPC endpoint URL
     * Target account ID
     * Method name to monitor
     * Starting block height

2. **Monitoring Phase**:
   * Continuous block polling
   * Transaction filtering
   * Event extraction
   * Callback execution

3. **Event Processing**:
   * Parse EVENT_JSON format
   * Extract event data
   * Execute user-defined callbacks
   * Handle errors gracefully

## Technical Implementation

### Core Components
 
```rust
pub struct NearEventListener {
    client: JsonRpcClient,
    account_id: String,
    method_name: String,
    last_processed_block: u64,
}
```

```rust
pub struct EventLog {
pub standard: String,
pub version: String,
pub event: String,
pub data: Value,
}
```

### Key Methods

* `builder`: Create new listener instance
* `start`: Begin event monitoring
* `process_log`: Parse and validate event logs
* `find_transaction_in_block`: Locate relevant transactions
* `get_logs`: Extract event logs from transactions

## Usage

### TestNet Example

```rust
use near_event_listener::NearEventListener;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listener = NearEventListener::builder("https://rpc.testnet.near.org")
    .account_id("account.testnet")
    .method_name("method_to_listen")
    .last_processed_block(0)
    .build()?;
    listener.start(|event_log| {
    println!("Event received: {:?}", event_log);
    }).await?;
    Ok(())
}
```

### Sandbox Testing

```rust
use near_event_listener::NearEventListener;
use near_workspaces::sandbox;
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
    println!("Event received: {:?}", event_log);
    }).await?;
    Ok(())
}
```

## Development

### Prerequisites

* Rust
* NEAR CLI
* Tokio runtime
* near-sdk-rs

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

## Security Considerations

* Implements proper error handling
* Validates event formats
* Handles RPC connection issues
* Manages block processing failures
* Ensures proper data parsing

## Error Handling

The library includes a comprehensive error handling system:

```rust
pub enum ListenerError {
    RpcError(String),
    InvalidEventFormat(String),
    JsonError(serde_json::Error),
    MissingField(String),
}
```

## Near Event Listener Client

[Near Event Listener Client](https://github.com/hasselalcala/near_event_listener_client)

## Acknowledgements

This implementation was built for the NEAR Protocol ecosystem to facilitate event monitoring and real-time updates for decentralized applications.

