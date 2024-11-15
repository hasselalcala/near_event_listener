use thiserror::Error;

#[derive(Error, Debug)]
pub enum ListenerError {
    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Invalid event format: {0}")]
    InvalidEventFormat(String),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Missing field: {0}")]
    MissingField(String),
}
