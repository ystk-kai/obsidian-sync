use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderMap, Response};
use bytes::Bytes;
use serde_json::Value;

use crate::domain::{
    models::{CouchDbDocument, DomainError},
    services::CouchDbRepository,
};

/// Service for handling LiveSync operations
pub struct LiveSyncService {
    couchdb_repo: Arc<dyn CouchDbRepository + Send + Sync>,
}

impl LiveSyncService {
    pub fn new(couchdb_repo: Arc<dyn CouchDbRepository + Send + Sync>) -> Self {
        Self { couchdb_repo }
    }

    /// Handle a document sync operation
    pub async fn handle_document_sync(
        &self,
        db_name: &str,
        document: CouchDbDocument,
    ) -> Result<CouchDbDocument, DomainError> {
        // Ensure the database exists
        self.couchdb_repo.ensure_database(db_name).await?;

        // Save the document
        let saved_doc = self.couchdb_repo.save_document(db_name, document).await?;

        Ok(saved_doc)
    }

    /// Handle a replication operation
    pub async fn handle_replication(
        &self,
        source: &str,
        target: &str,
        options: Value,
    ) -> Result<Value, DomainError> {
        // Perform the replication
        self.couchdb_repo.replicate(source, target, options).await
    }

    /// Get the CouchDB URL for proxying requests
    pub fn get_couchdb_url(&self) -> String {
        self.couchdb_repo.get_base_url()
    }

    /// Get CouchDB authentication credentials if available
    pub fn get_couchdb_auth(&self) -> Option<(String, String)> {
        self.couchdb_repo.get_auth_credentials()
    }

    /// Get the CouchDB repository reference
    pub fn get_couchdb_repository(&self) -> &Arc<dyn CouchDbRepository + Send + Sync> {
        &self.couchdb_repo
    }

    /// HTTP リクエストをCouchDBに転送する
    pub async fn forward_request(
        &self,
        method: &str,
        path: &str,
        query: Option<&str>,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<Response<Body>, DomainError> {
        // CouchDBリポジトリの参照を取得
        let repo = self.couchdb_repo.as_ref();

        // リクエストを転送
        repo.forward_request(method, path, query, headers, body)
            .await
    }
}
