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

    /// Get the base URL of the CouchDB server
    fn get_base_url(&self) -> String;

    /// Get authentication credentials if available
    fn get_auth_credentials(&self) -> Option<(String, String)>;
}
