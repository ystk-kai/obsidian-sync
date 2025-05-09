use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::{RwLock, broadcast};

use crate::domain::models::DomainError;
use crate::domain::services::MessageBroker;

/// Implementation of the message broker for WebSocket connections
pub struct WebSocketBroker {
    // Map of client ID to broadcast sender
    connections: Arc<RwLock<HashMap<String, broadcast::Sender<Value>>>>,
    // Global broadcast channel for all clients
    global_tx: broadcast::Sender<(String, Value)>,
}

impl WebSocketBroker {
    pub fn new(capacity: usize) -> Self {
        let (global_tx, _) = broadcast::channel(capacity);

        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            global_tx,
        }
    }

    /// Start the broker's background task for message distribution
    pub fn start(self: Arc<Self>) {
        let this = self.clone();

        tokio::spawn(async move {
            let mut rx = this.global_tx.subscribe();

            while let Ok((client_id, message)) = rx.recv().await {
                if let Err(err) = this.send_message(&client_id, message).await {
                    tracing::error!("Failed to send message to client {}: {:?}", client_id, err);
                }
            }
        });
    }

    /// Create a new connection channel for a client
    pub fn create_connection_channel(&self, capacity: usize) -> broadcast::Sender<Value> {
        let (tx, _) = broadcast::channel(capacity);
        tx
    }
}

#[async_trait]
impl MessageBroker for WebSocketBroker {
    async fn send_message(&self, client_id: &str, message: Value) -> Result<(), DomainError> {
        let connections = self.connections.read().await;

        if let Some(tx) = connections.get(client_id) {
            tx.send(message).map_err(|e| {
                DomainError::WebSocketError(format!("Failed to send message: {}", e))
            })?;
            Ok(())
        } else {
            Err(DomainError::WebSocketError(format!(
                "Client {} not found",
                client_id
            )))
        }
    }

    async fn broadcast_message(&self, message: Value) -> Result<(), DomainError> {
        let connections = self.connections.read().await;

        let mut errors = Vec::new();

        for (client_id, tx) in connections.iter() {
            if let Err(e) = tx.send(message.clone()) {
                errors.push((client_id.clone(), e.to_string()));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(DomainError::WebSocketError(format!(
                "Failed to broadcast to some clients: {:?}",
                errors
            )))
        }
    }

    async fn register_client(&self, client_id: &str) -> Result<(), DomainError> {
        let mut connections = self.connections.write().await;

        if connections.contains_key(client_id) {
            return Err(DomainError::WebSocketError(format!(
                "Client {} already registered",
                client_id
            )));
        }

        let (tx, _) = broadcast::channel(100);
        connections.insert(client_id.to_string(), tx);

        Ok(())
    }

    async fn unregister_client(&self, client_id: &str) -> Result<(), DomainError> {
        let mut connections = self.connections.write().await;

        if connections.remove(client_id).is_none() {
            return Err(DomainError::WebSocketError(format!(
                "Client {} not found",
                client_id
            )));
        }

        Ok(())
    }
}
