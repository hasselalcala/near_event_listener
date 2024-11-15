use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventLog {
    pub standard: String,
    pub version: String,
    pub event: String,
    pub data: Value, // Ahora data es un Value genérico que puede contener cualquier JSON
}
