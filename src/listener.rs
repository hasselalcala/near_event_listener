use crate::{EventLog, ListenerError};
use near_jsonrpc_client::errors::{JsonRpcError, JsonRpcServerError};
use near_jsonrpc_client::methods::{block::RpcBlockError, chunk::ChunkReference};
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::transactions::RpcTransactionResponse;
use near_primitives::hash::CryptoHash;
use near_primitives::types::{BlockId, BlockReference, Finality};
use near_primitives::views::{ActionView, BlockView, ChunkView, FinalExecutionOutcomeViewEnum};
use near_sdk::AccountId;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug)]
pub struct NearEventListener {
    pub client: JsonRpcClient,
    pub account_id: String,
    pub method_name: String,
    pub last_processed_block: u64,
}

pub struct NearEventListenerBuilder {
    rpc_url: String,
    account_id: String,
    method_name: String,
    last_processed_block: u64,
}

impl NearEventListenerBuilder {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            account_id: String::new(),
            method_name: String::new(),
            last_processed_block: 0,
        }
    }

    pub fn account_id(mut self, account_id: &str) -> Self {
        self.account_id = account_id.to_string();
        self
    }

    pub fn method_name(mut self, method_name: &str) -> Self {
        self.method_name = method_name.to_string();
        self
    }

    pub fn last_processed_block(mut self, block: u64) -> Self {
        self.last_processed_block = block;
        self
    }

    pub fn build(self) -> Result<NearEventListener, ListenerError> {
        if self.account_id.is_empty() {
            return Err(ListenerError::MissingField("account_id".to_string()));
        }
        if self.method_name.is_empty() {
            return Err(ListenerError::MissingField("method_name".to_string()));
        }

        let client = JsonRpcClient::connect(&self.rpc_url);

        Ok(NearEventListener {
            client,
            account_id: self.account_id,
            method_name: self.method_name,
            last_processed_block: self.last_processed_block,
        })
    }
}

impl NearEventListener {
    pub fn builder(rpc_url: &str) -> NearEventListenerBuilder {
        NearEventListenerBuilder::new(rpc_url)
    }

    pub async fn start<F>(&mut self, callback: F) -> Result<(), ListenerError>
    where
        F: FnMut(EventLog) + Send + 'static,
    {
        println!(
            "Starting event listener for account: {}, method: {}",
            self.account_id, self.method_name
        );

        self.start_polling(callback).await
    }

    async fn start_polling<F>(&mut self, mut callback: F) -> Result<(), ListenerError>
    where
        F: FnMut(EventLog) + Send + 'static,
    {
        println!("Starting polling...");

        loop {
            println!("Last processed block: {}", self.last_processed_block);
            let block_reference = self.specify_block_reference();

            match self.fetch_block(block_reference).await {
                Ok(block) => {
                    println!("Processing block: {:#?}", block.header.height);

                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                    if let Some((tx_hash, sender_account_id)) =
                        self.find_transaction_in_block(&block).await?
                    {
                        let logs = self.get_logs(&tx_hash, &sender_account_id).await?;

                        // if let Some(log) = logs.first() {
                        //     if let Ok(event_log) = Self::process_log(log) {
                        //         println!("\nEmitted event: {:?}\n", event_log);
                        //         callback(event_log);
                        //     }
                        // }
                        println!("Logs: {:?}", logs);
                        println!("Logs length: {}", logs.len());
                        //for log in logs {
                            if let Ok(event_log) = Self::process_log(&log) {
                                println!("\nEmitted event: {:?}\n", event_log);
                                callback(event_log);
                            }
                        //}
                    }

                    self.last_processed_block = block.header.height;
                    println!("Saved new block height: {}", self.last_processed_block);
                }
                Err(err) => self.handle_block_error(err).await?,
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    fn specify_block_reference(&self) -> BlockReference {
        if self.last_processed_block == 0 {
            BlockReference::Finality(Finality::Final)
        } else {
            BlockReference::BlockId(BlockId::Height(self.last_processed_block + 1))
        }
    }

    async fn fetch_block(
        &self,
        block_reference: BlockReference,
    ) -> Result<BlockView, JsonRpcError<RpcBlockError>> {
        let block_request = methods::block::RpcBlockRequest { block_reference };
        self.client.call(block_request).await
    }

    async fn fetch_chunk(&self, chunk_hash: CryptoHash) -> Result<ChunkView, ListenerError> {
        let chunk_reference = ChunkReference::ChunkHash {
            chunk_id: chunk_hash,
        };

        let chunk_request = methods::chunk::RpcChunkRequest { chunk_reference };

        match self.client.call(chunk_request).await {
            Ok(chunk) => Ok(chunk),
            Err(e) => {
                println!("Error fetching chunk: {:?}", e);
                Err(ListenerError::RpcError(e.to_string()))
            }
        }
    }

    pub async fn find_transaction_in_block(
        &self,
        block: &BlockView,
    ) -> Result<Option<(String, AccountId)>, ListenerError> {
        for chunk_header in &block.chunks {
            let chunk_hash = chunk_header.chunk_hash;
            let chunk = self.fetch_chunk(chunk_hash).await?;
            for transaction in &chunk.transactions {
                if transaction.receiver_id == self.account_id {
                    for action in &transaction.actions {
                        if let ActionView::FunctionCall {
                            method_name: action_method_name,
                            ..
                        } = action
                        {
                            if *action_method_name == self.method_name {
                                return Ok(Some((
                                    transaction.hash.to_string(),
                                    transaction.signer_id.clone(),
                                )));
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    async fn get_logs(
        &self,
        tx_hash: &str,
        sender_account_id: &AccountId,
    ) -> Result<Vec<String>, ListenerError> {
        let tx_hash = CryptoHash::from_str(tx_hash)
            .map_err(|e| ListenerError::InvalidEventFormat(e.to_string()))?;

        let transaction_status_request = methods::tx::RpcTransactionStatusRequest {
            transaction_info: methods::tx::TransactionInfo::TransactionId {
                tx_hash,
                sender_account_id: sender_account_id.clone(),
            },
            wait_until: near_primitives::views::TxExecutionStatus::None,
        };

        let transaction_status_response = self
            .client
            .call(transaction_status_request)
            .await
            .map_err(|e| ListenerError::RpcError(e.to_string()))?;

        let logs = self.extract_logs(&transaction_status_response);
        Ok(logs)
    }

    pub fn extract_logs(&self, response: &RpcTransactionResponse) -> Vec<String> {
        let mut logs = Vec::new();

        if let Some(final_outcome_enum) = &response.final_execution_outcome {
            match final_outcome_enum {
                FinalExecutionOutcomeViewEnum::FinalExecutionOutcome(final_outcome) => {
                    logs.extend(final_outcome.transaction_outcome.outcome.logs.clone());

                    for receipt_outcome in &final_outcome.receipts_outcome {
                        logs.extend(receipt_outcome.outcome.logs.clone());
                    }
                }
                FinalExecutionOutcomeViewEnum::FinalExecutionOutcomeWithReceipt(
                    final_outcome_with_receipt,
                ) => {
                    println!("Something is missing: {:?}", final_outcome_with_receipt);
                }
            }
        }

        logs
    }

    pub fn process_log(log: &str) -> Result<EventLog, ListenerError> {
        if !log.starts_with("EVENT_JSON:") {
            return Err(ListenerError::InvalidEventFormat(
                "Log does not start with EVENT_JSON:".to_string(),
            ));
        }

        let json_str = &log["EVENT_JSON:".len()..];

        let event_log: EventLog = serde_json::from_str(json_str).map_err(|e| {
            println!("Error deserializing JSON: {}", e);
            ListenerError::JsonError(e)
        })?;

        Ok(event_log)
    }

    async fn handle_block_error(
        &mut self,
        err: JsonRpcError<RpcBlockError>,
    ) -> Result<(), ListenerError> {
        match err.handler_error() {
            Some(methods::block::RpcBlockError::UnknownBlock { .. }) => {
                println!("(i) Unknown block!");
                self.last_processed_block += 1;
                println!("Saved new block height: {}", self.last_processed_block);
                Ok(())
            }
            Some(err) => Err(ListenerError::RpcError(format!("Block error: {:?}", err))),
            _ => match err {
                JsonRpcError::ServerError(JsonRpcServerError::ResponseStatusError(status)) => {
                    println!("(i) Server error occurred: status code {}", status);
                    tokio::time::sleep(Duration::from_secs(5)).await;

                    Ok(())
                }
                _ => Err(ListenerError::RpcError(format!(
                    "Non-handler error: {:?}",
                    err
                ))),
            },
        }
    }
}
