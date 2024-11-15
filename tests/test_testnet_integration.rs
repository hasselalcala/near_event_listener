use near_crypto::InMemorySigner;
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::transactions::RpcTransactionResponse;
use near_primitives::transaction::SignedTransaction;
use near_primitives::transaction::{Action, FunctionCallAction, Transaction, TransactionV0};
use near_primitives::types::BlockReference;
use near_primitives::views::{QueryRequest, TxExecutionStatus};
use serde_json::json;
use std::time::Instant;

use near_event_listener::EventLog;
use near_event_listener::NearEventListener;
pub struct TestnetContractWrapper {
    rpc_client: JsonRpcClient,
    contract_id: String,
    signer: InMemorySigner,
}

impl TestnetContractWrapper {
    fn new(signer_account: &str) -> anyhow::Result<Self> {
        let rpc_client = JsonRpcClient::connect("https://rpc.testnet.near.org");

        let home_dir = std::env::var("HOME")?;
        let credentials_path = format!(
            "{home_dir}/.near-credentials/testnet/{signer_account}.json",
            home_dir = home_dir,
            signer_account = signer_account
        );
        
        println!("Trying to load credentials from: {}", credentials_path);
        let signer = InMemorySigner::from_file(std::path::Path::new(&credentials_path))?;
        
        Ok(Self {
            rpc_client,
            contract_id: "simplecontract.testnet".to_string(),  // Hardcodeamos el contrato objetivo
            signer,
        })
    }

    pub async fn get_nonce_and_block_hash(
        &self,
    ) -> anyhow::Result<(u64, near_primitives::hash::CryptoHash)> {
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
            near_jsonrpc_primitives::types::query::QueryResponseKind::AccessKey(access_key) => {
                Ok((access_key.nonce + 1, access_key_query_response.block_hash))
            }
            _ => Err(anyhow::anyhow!("Failed to get access key")),
        }
    }

    pub async fn set_greeting(&self, greeting: String) -> anyhow::Result<RpcTransactionResponse> {
        let (nonce, block_hash) = self.get_nonce_and_block_hash().await?;

        let args = serde_json::json!({
            "greeting": greeting
        });

        let tx = Transaction::V0(TransactionV0 {
            signer_id: self.signer.account_id.clone(),
            public_key: self.signer.public_key.clone(),
            nonce,
            receiver_id: self.contract_id.parse()?,
            block_hash,
            actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
                method_name: "set_greeting".to_string(),
                args: serde_json::to_vec(&args)?,
                gas: 300_000_000_000_000,
                deposit: 0,
            }))],
        });

        let (hash, _) = tx.get_hash_and_size();

        // Firmamos el hash
        let signature = self.signer.sign(hash.as_ref());

        // Creamos la transacción firmada
        let signed_tx = SignedTransaction::new(signature, tx);

        let request = methods::send_tx::RpcSendTransactionRequest {
            signed_transaction: signed_tx,
            wait_until: TxExecutionStatus::Final,
        };

        let _sent_at = Instant::now();

        match self.rpc_client.call(request).await {
            Ok(response) => Ok(response),
            Err(err) => {
                if let Some(
                    near_jsonrpc_primitives::types::transactions::RpcTransactionError::TimeoutError,
                ) = err.handler_error()
                {
                    Err(anyhow::anyhow!("Transaction timeout"))
                } else {
                    Err(err.into())
                }
            }
        }
    }
}

#[tokio::test]
async fn test_integration_using_testnet() -> anyhow::Result<()> {
    // Inicializamos el wrapper con el contrato de testnet
    let contract_wrapper = TestnetContractWrapper::new("hasselalcalag.testnet")?;
    
    println!(
        "Setting greeting on contract: {}",
        contract_wrapper.contract_id
    );

    // Enviamos la transacción
    let tx_result = contract_wrapper
        .set_greeting("Hello from testnet!".to_string())
        .await?;

    // Obtenemos el block height de la transacción
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

    // Configuramos el canal para recibir eventos
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let tx_clone = tx.clone();

    // Inicializamos el listener
    let mut listener = NearEventListener::builder("https://rpc.testnet.near.org")
        .account_id(&contract_wrapper.contract_id)
        .method_name("set_greeting")
        .last_processed_block(block_height - 1)
        .build()?;

    // Iniciamos el listener en un task separado
    let listener_handle = tokio::spawn(async move {
        listener
            .start(move |event_log| {
                println!("Captured event: {:?}", event_log);
                let _ = tx_clone.try_send(event_log.clone());
            })
            .await
    });

    // Definimos el evento esperado
    let expected_event = EventLog {
        standard: "nep171".to_string(),
        version: "1.0.0".to_string(),
        event: "set_greeting".to_string(),
        data: json!([{
            "greeting": "Hello from testnet!"
        }]),
    };

    // Esperamos el evento con timeout
    let received_event = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
        .await
        .map_err(|_| anyhow::anyhow!("Timeout waiting for event"))?
        .ok_or_else(|| anyhow::anyhow!("Channel closed"))?;

    // Verificamos que el evento recibido coincida con el esperado
    assert_eq!(
        received_event, expected_event,
        "Received event does not match expected event"
    );

    // Limpiamos el listener
    listener_handle.abort();

    Ok(())
}
