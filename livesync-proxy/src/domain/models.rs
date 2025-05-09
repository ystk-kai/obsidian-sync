use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a WebSocket message in the Obsidian LiveSync protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSyncMessage {
    pub id: Uuid,
    pub message_type: MessageType,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Connection,
    Sync,
    Replicate,
    Error,
    Heartbeat,
}

/// Represents a CouchDB document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouchDbDocument {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("CouchDB error: {0}")]
    CouchDbError(String),

    #[error("WebSocket error: {0}")]
    WebSocketError(String),
}
