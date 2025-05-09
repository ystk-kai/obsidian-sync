use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::domain::{
    models::{CouchDbDocument, DomainError, LiveSyncMessage, MessageType},
    services::{CouchDbRepository, MessageBroker},
};

/// Service for handling LiveSync operations
pub struct LiveSyncService {
    couchdb_repo: Arc<dyn CouchDbRepository + Send + Sync>,
    message_broker: Arc<dyn MessageBroker + Send + Sync>,
    client_connections: Arc<RwLock<Vec<String>>>,
}

impl LiveSyncService {
    pub fn new(
        couchdb_repo: Arc<dyn CouchDbRepository + Send + Sync>,
        message_broker: Arc<dyn MessageBroker + Send + Sync>,
    ) -> Self {
        Self {
            couchdb_repo,
            message_broker,
            client_connections: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Handle an incoming client message
    pub async fn handle_message(
        &self,
        client_id: &str,
        message: LiveSyncMessage,
    ) -> Result<(), DomainError> {
        match message.message_type {
            MessageType::Connection => self.handle_connection(client_id, message).await,
            MessageType::Sync => self.handle_sync(client_id, message).await,
            MessageType::Replicate => self.handle_replication(client_id, message).await,
            MessageType::Heartbeat => Ok(()), // Just acknowledge heartbeats
            MessageType::Error => self.handle_error(client_id, message).await,
        }
    }

    /// Handle a connection message
    async fn handle_connection(
        &self,
        client_id: &str,
        _message: LiveSyncMessage,
    ) -> Result<(), DomainError> {
        // Register the client
        self.message_broker.register_client(client_id).await?;

        // Add to our internal tracking
        {
            let mut connections = self.client_connections.write().await;
            if !connections.contains(&client_id.to_string()) {
                connections.push(client_id.to_string());
            }
        }

        // Send a connection acknowledgment
        let response = LiveSyncMessage {
            id: Uuid::new_v4(),
            message_type: MessageType::Connection,
            payload: serde_json::json!({
                "status": "connected",
                "client_id": client_id,
                "server_time": chrono::Utc::now().to_rfc3339(),
            }),
        };

        self.message_broker
            .send_message(client_id, serde_json::to_value(response).unwrap())
            .await
    }

    /// Handle a sync message
    async fn handle_sync(
        &self,
        client_id: &str,
        message: LiveSyncMessage,
    ) -> Result<(), DomainError> {
        // Extract sync data from the message
        let db_name = message.payload["database"]
            .as_str()
            .ok_or_else(|| DomainError::InvalidMessage("Missing database name".to_string()))?;

        // Ensure the database exists
        self.couchdb_repo.ensure_database(db_name).await?;

        // Process the document if provided
        if let Some(_doc) = message.payload["document"].as_object() {
            let document: CouchDbDocument =
                serde_json::from_value(message.payload["document"].clone()).map_err(|e| {
                    DomainError::InvalidMessage(format!("Invalid document format: {}", e))
                })?;

            // Save the document
            let saved_doc = self.couchdb_repo.save_document(db_name, document).await?;

            // Send back confirmation
            let response = LiveSyncMessage {
                id: Uuid::new_v4(),
                message_type: MessageType::Sync,
                payload: serde_json::json!({
                    "status": "success",
                    "document_id": saved_doc.id,
                    "rev": saved_doc.rev,
                }),
            };

            self.message_broker
                .send_message(client_id, serde_json::to_value(response).unwrap())
                .await?;
        }

        Ok(())
    }

    /// Handle a replication message
    async fn handle_replication(
        &self,
        client_id: &str,
        message: LiveSyncMessage,
    ) -> Result<(), DomainError> {
        // Extract replication details
        let source = message.payload["source"]
            .as_str()
            .ok_or_else(|| DomainError::InvalidMessage("Missing source database".to_string()))?;

        let target = message.payload["target"]
            .as_str()
            .ok_or_else(|| DomainError::InvalidMessage("Missing target database".to_string()))?;

        let options = message.payload["options"].clone();

        // Perform the replication
        let result = self.couchdb_repo.replicate(source, target, options).await?;

        // Send back the result
        let response = LiveSyncMessage {
            id: Uuid::new_v4(),
            message_type: MessageType::Replicate,
            payload: serde_json::json!({
                "status": "success",
                "result": result,
            }),
        };

        self.message_broker
            .send_message(client_id, serde_json::to_value(response).unwrap())
            .await
    }

    /// Handle an error message
    async fn handle_error(
        &self,
        client_id: &str,
        message: LiveSyncMessage,
    ) -> Result<(), DomainError> {
        // Log the error
        tracing::error!("Client {} reported error: {:?}", client_id, message.payload);

        // We don't need to send a response for error messages
        Ok(())
    }

    /// Handle a client disconnection
    pub async fn handle_disconnection(&self, client_id: &str) -> Result<(), DomainError> {
        self.message_broker.unregister_client(client_id).await?;

        // Remove from our internal tracking
        {
            let mut connections = self.client_connections.write().await;
            connections.retain(|id| id != client_id);
        }

        Ok(())
    }
}
