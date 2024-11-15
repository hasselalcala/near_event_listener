use near_crypto::InMemorySigner;
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_jsonrpc_primitives::types::transactions::{RpcTransactionError, TransactionInfo};
use near_primitives::hash::CryptoHash;
use near_primitives::transaction::{Action, FunctionCallAction, Transaction, TransactionV0};
use near_primitives::types::BlockReference;
use near_primitives::views::{QueryRequest, TxExecutionStatus};
use near_sdk::NearToken;
use near_workspaces::{network::Sandbox, AccountId, Worker};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use near_event_listener::EventLog;
use near_event_listener::NearEventListener;

const CONTRACT_FILEPATH: &str = "tests/simple_contract.wasm";
const HUNDRED_NEAR: NearToken = NearToken::from_near(100);
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct TxBuilder {
    signer: InMemorySigner,
    receiver_id: AccountId,
}

impl TxBuilder {
    pub fn new(signer: InMemorySigner, receiver_id: AccountId) -> Self {
        Self {
            signer,
            receiver_id,
        }
    }

    pub fn build(
        &self,
        nonce: u64,
        block_hash: CryptoHash,
        method_name: String,
        args: serde_json::Value,
    ) -> (TransactionV0, CryptoHash) {
        let args = serde_json::to_vec(&args).unwrap();

        let tx = TransactionV0 {
            signer_id: self.signer.account_id.clone(),
            public_key: self.signer.public_key.clone(),
            nonce,
            receiver_id: self.receiver_id.clone(),
            block_hash,
            actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
                method_name,
                args,
                gas: 300_000_000_000_000,
                deposit: 0,
            }))],
        };

        let tx_hash = Transaction::V0(tx.clone()).get_hash_and_size().0;
        (tx, tx_hash)
    }
}

#[derive(Debug)]
struct ContractWrapper {
    worker: Worker<Sandbox>,
    contract: near_workspaces::Contract,
    rpc_client: Arc<JsonRpcClient>,
    signer: Arc<InMemorySigner>,
    current_nonce: Mutex<u64>,
    tx_builder: Mutex<TxBuilder>,
}

impl ContractWrapper {
    async fn new(contract_path: &str) -> anyhow::Result<Self> {
        let worker = near_workspaces::sandbox().await?;
        let wasm = std::fs::read(contract_path)?;
        let contract = worker.dev_deploy(&wasm).await?;
        let rpc_client = Arc::new(JsonRpcClient::connect(worker.rpc_addr()));

        let account = worker
            .root_account()?
            .create_subaccount("listenerclient")
            .initial_balance(HUNDRED_NEAR)
            .transact()
            .await?
            .into_result()?;

        let signer = InMemorySigner {
            account_id: account.id().clone(),
            public_key: account.secret_key().public_key().to_string().parse()?,
            secret_key: account.secret_key().to_string().parse()?,
        };

        let tx_builder = TxBuilder::new(signer.clone(), contract.id().clone());

        Ok(Self {
            worker,
            contract,
            rpc_client,
            signer: Arc::new(signer.clone()),
            current_nonce: Mutex::new(0),
            tx_builder: Mutex::new(tx_builder),
        })
    }

    pub async fn get_nonce_and_tx_hash(&self) -> Result<(u64, CryptoHash), anyhow::Error> {
        let access_key_query_response = self
            .rpc_client
            .call(methods::query::RpcQueryRequest {
                block_reference: BlockReference::latest(),
                request: QueryRequest::ViewAccessKey {
                    account_id: self.signer.account_id.clone(),
                    public_key: self.signer.public_key.clone(),
                },
            })
            .await?;

        match access_key_query_response.kind {
            QueryResponseKind::AccessKey(access_key) => {
                let mut current_nonce = self.current_nonce.lock().await;
                let new_nonce = std::cmp::max(access_key.nonce, *current_nonce) + 1;
                *current_nonce = new_nonce;
                println!("Using nonce: {}", new_nonce);
                Ok((new_nonce, access_key_query_response.block_hash))
            }
            _ => Err(anyhow::anyhow!("Failed to extract current nonce").into()),
        }
    }

    async fn set_greeting(
        &self,
        greeting: String,
        method_name: String,
    ) -> anyhow::Result<near_jsonrpc_primitives::types::transactions::RpcTransactionResponse> {
        println!("Getting nonce and block hash");
        let tx_builder = self.tx_builder.lock().await;

        let (nonce, block_hash) = self.get_nonce_and_tx_hash().await?;

        let args = serde_json::json!({
            "greeting": greeting.clone()
        });

        let (tx, _tx_hash) = tx_builder.build(nonce, block_hash, method_name.clone(), args);

        let signer = near_crypto::Signer::from((*self.signer).clone());
        let signed_tx = Transaction::V0(tx).sign(&signer);

        let request = methods::send_tx::RpcSendTransactionRequest {
            signed_transaction: signed_tx,
            wait_until: TxExecutionStatus::Final,
        };

        let sent_at = Instant::now();
        println!("Sending transaction");
        match self.rpc_client.call(request.clone()).await {
            Ok(response) => {
                self.log_response_time(sent_at);
                Ok(response)
            }
            Err(err) => {
                if let Some(RpcTransactionError::TimeoutError) = err.handler_error() {
                    let tx_hash = request.signed_transaction.get_hash();
                    let sender_account_id =
                        request.signed_transaction.transaction.signer_id().clone();
                    self.wait_for_transaction(tx_hash, sender_account_id, sent_at)
                        .await
                } else {
                    Err(err.into())
                }
            }
        }
    }

    fn log_response_time(&self, sent_at: Instant) {
        let delta = sent_at.elapsed().as_secs();
        println!("Response received after: {}s", delta);
    }

    async fn wait_for_transaction(
        &self,
        tx_hash: CryptoHash,
        sender_account_id: near_primitives::types::AccountId,
        sent_at: Instant,
    ) -> Result<near_jsonrpc_primitives::types::transactions::RpcTransactionResponse, anyhow::Error>
    {
        loop {
            let response = self
                .rpc_client
                .call(methods::tx::RpcTransactionStatusRequest {
                    transaction_info: TransactionInfo::TransactionId {
                        tx_hash,
                        sender_account_id: sender_account_id.clone(),
                    },
                    wait_until: TxExecutionStatus::Final,
                })
                .await;

            if sent_at.elapsed() > DEFAULT_TIMEOUT {
                return Err(anyhow::anyhow!(
                    "Time limit exceeded for the transaction to be recognized"
                ));
            }

            match response {
                Ok(response) => {
                    self.log_response_time(sent_at);
                    return Ok(response);
                }
                Err(err) => {
                    if let Some(RpcTransactionError::TimeoutError) = err.handler_error() {
                        continue;
                    }
                    return Err(err.into());
                }
            }
        }
    }
}

#[tokio::test]
async fn test_integration_using_sandbox() -> anyhow::Result<()> {
    let contract_wrapper = ContractWrapper::new(CONTRACT_FILEPATH).await?;
    let account_id = contract_wrapper.contract.id().clone();

    println!("account_id: {}", account_id);

    let tx_result = contract_wrapper
        .set_greeting("Hello, World!".to_string(), "set_greeting".to_string())
        .await?;

    // Obtain block height from the transaction
    let block_height = if let Some(final_execution) = &tx_result.final_execution_outcome {
        match final_execution {
            near_primitives::views::FinalExecutionOutcomeViewEnum::FinalExecutionOutcome(
                outcome,
            ) => {
                let block = contract_wrapper
                    .rpc_client
                    .call(methods::block::RpcBlockRequest {
                        block_reference: BlockReference::BlockId(
                            near_primitives::types::BlockId::Hash(
                                outcome.transaction_outcome.block_hash,
                            ),
                        ),
                    })
                    .await?;
                block.header.height
            }
            _ => 0,
        }
    } else {
        0
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let tx_clone = tx.clone();

    let mut listener = NearEventListener::builder(&contract_wrapper.worker.rpc_addr())
        .account_id(&account_id.as_str())
        .method_name("set_greeting")
        .last_processed_block(block_height - 1)
        .build()?;

    let listener_handle = tokio::spawn(async move {
        listener
            .start(move |event_log| {
                println!("Captured event: {:?}", event_log);
                let _ = tx_clone.try_send(event_log.clone());
            })
            .await
    });

    let expected_event = EventLog {
        standard: "nep171".to_string(),
        version: "1.0.0".to_string(),
        event: "set_greeting".to_string(),
        data: json!([{
            "greeting": "Hello, World!"
        }]),
    };

    // Esperar a recibir el evento (con timeout)
    let received_event = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
        .await
        .map_err(|_| anyhow::anyhow!("Timeout waiting for event"))?
        .ok_or_else(|| anyhow::anyhow!("Channel closed"))?;

    assert_eq!(
        received_event, expected_event,
        "El evento recibido no coincide con el esperado"
    );

    listener_handle.abort();

    Ok(())
}
