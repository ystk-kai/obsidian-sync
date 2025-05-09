use serde::{Deserialize, Serialize};

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

    #[error("HTTP proxy error: {0}")]
    HttpProxyError(String),
}
