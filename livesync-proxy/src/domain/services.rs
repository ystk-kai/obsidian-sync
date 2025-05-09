use async_trait::async_trait;
use serde_json::Value;

use crate::domain::models::{CouchDbDocument, DomainError};

/// Repository interface for CouchDB operations
#[async_trait]
pub trait CouchDbRepository {
    /// Get a document from the database
    async fn get_document(
        &self,
        db_name: &str,
        doc_id: &str,
    ) -> Result<CouchDbDocument, DomainError>;

    /// Save a document to the database
    async fn save_document(
        &self,
        db_name: &str,
        doc: CouchDbDocument,
    ) -> Result<CouchDbDocument, DomainError>;

    /// Delete a document from the database
    async fn delete_document(
        &self,
        db_name: &str,
        doc_id: &str,
        rev: &str,
    ) -> Result<(), DomainError>;

    /// Query the database with a view
    async fn query_view(
        &self,
        db_name: &str,
        design_doc: &str,
        view_name: &str,
        options: Value,
    ) -> Result<Vec<CouchDbDocument>, DomainError>;

    /// Ensure a database exists
    async fn ensure_database(&self, db_name: &str) -> Result<(), DomainError>;

    /// Replicate data between databases
    async fn replicate(
        &self,
        source: &str,
        target: &str,
        options: Value,
    ) -> Result<Value, DomainError>;
}

/// Message broker interface for WebSocket operations
#[async_trait]
pub trait MessageBroker {
    /// Send a message to a specific client
    async fn send_message(&self, client_id: &str, message: Value) -> Result<(), DomainError>;

    /// Broadcast a message to all connected clients
    async fn broadcast_message(&self, message: Value) -> Result<(), DomainError>;

    /// Register a new client connection
    async fn register_client(&self, client_id: &str) -> Result<(), DomainError>;

    /// Unregister a client connection
    async fn unregister_client(&self, client_id: &str) -> Result<(), DomainError>;
}
