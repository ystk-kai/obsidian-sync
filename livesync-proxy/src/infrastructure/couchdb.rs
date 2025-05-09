use async_trait::async_trait;
use base64::Engine;
use reqwest::{Client, StatusCode};
use serde_json::Value;

use crate::domain::models::{CouchDbDocument, DomainError};
use crate::domain::services::CouchDbRepository;

/// Implementation of the CouchDB repository
pub struct CouchDbClient {
    client: Client,
    base_url: String,
    auth_header: String,
}

impl CouchDbClient {
    pub fn new(base_url: &str, username: &str, password: &str) -> Self {
        let auth_string = format!("{}:{}", username, password);
        let auth_header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(auth_string)
        );

        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            auth_header,
        }
    }

    /// Get the full URL for a database operation
    fn db_url(&self, db_name: &str, path: Option<&str>) -> String {
        match path {
            Some(p) => format!("{}/{}/{}", self.base_url, db_name, p),
            None => format!("{}/{}", self.base_url, db_name),
        }
    }

    /// CouchDBサーバーにpingを送信し、接続性をチェックする
    pub async fn ping(&self) -> Result<(), DomainError> {
        let url = format!("{}/_up", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", &self.auth_header)
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Ping request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(DomainError::CouchDbError(format!(
                "CouchDB ping failed with status: {}",
                status
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl CouchDbRepository for CouchDbClient {
    async fn get_document(
        &self,
        db_name: &str,
        doc_id: &str,
    ) -> Result<CouchDbDocument, DomainError> {
        let url = self.db_url(db_name, Some(doc_id));

        let response = self
            .client
            .get(&url)
            .header("Authorization", &self.auth_header)
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Request failed: {}", e)))?;

        if response.status() == StatusCode::NOT_FOUND {
            return Err(DomainError::CouchDbError(format!(
                "Document {} not found",
                doc_id
            )));
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(DomainError::CouchDbError(format!(
                "CouchDB error: {} - {}",
                status, error_text
            )));
        }

        let document = response
            .json::<CouchDbDocument>()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Failed to parse document: {}", e)))?;

        Ok(document)
    }

    async fn save_document(
        &self,
        db_name: &str,
        doc: CouchDbDocument,
    ) -> Result<CouchDbDocument, DomainError> {
        let url = if doc.id.is_empty() {
            // If ID is empty, let CouchDB generate one
            self.db_url(db_name, None)
        } else {
            // Use the provided ID
            self.db_url(db_name, Some(&doc.id))
        };

        let method = if doc.rev.is_some() {
            self.client.put(url)
        } else {
            self.client.post(url)
        };

        let response = method
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(&doc)
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(DomainError::CouchDbError(format!(
                "CouchDB error: {} - {}",
                status, error_text
            )));
        }

        #[derive(serde::Deserialize)]
        struct SaveResponse {
            id: String,
            #[serde(rename = "rev")]
            _rev: String, // アンダースコアを追加して未使用フィールドであることを示す
            ok: bool,
        }

        let save_response = response
            .json::<SaveResponse>()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Failed to parse response: {}", e)))?;

        if !save_response.ok {
            return Err(DomainError::CouchDbError(
                "Save operation not successful".to_string(),
            ));
        }

        // Return the updated document
        self.get_document(db_name, &save_response.id).await
    }

    async fn delete_document(
        &self,
        db_name: &str,
        doc_id: &str,
        rev: &str,
    ) -> Result<(), DomainError> {
        let url = self.db_url(db_name, Some(doc_id));

        let response = self
            .client
            .delete(&url)
            .header("Authorization", &self.auth_header)
            .query(&[("rev", rev)])
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(DomainError::CouchDbError(format!(
                "CouchDB error: {} - {}",
                status, error_text
            )));
        }

        Ok(())
    }

    async fn query_view(
        &self,
        db_name: &str,
        design_doc: &str,
        view_name: &str,
        options: Value,
    ) -> Result<Vec<CouchDbDocument>, DomainError> {
        let path = format!("_design/{}/view/{}", design_doc, view_name);
        let url = self.db_url(db_name, Some(&path));

        let mut request = self
            .client
            .get(&url)
            .header("Authorization", &self.auth_header);

        // Add query parameters from options
        if let Some(obj) = options.as_object() {
            for (key, value) in obj {
                if let Some(val_str) = value.as_str() {
                    request = request.query(&[(key, val_str)]);
                }
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(DomainError::CouchDbError(format!(
                "CouchDB error: {} - {}",
                status, error_text
            )));
        }

        #[derive(serde::Deserialize)]
        struct ViewResponse {
            rows: Vec<ViewRow>,
        }

        #[derive(serde::Deserialize)]
        struct ViewRow {
            doc: Option<CouchDbDocument>,
        }

        let view_response = response.json::<ViewResponse>().await.map_err(|e| {
            DomainError::CouchDbError(format!("Failed to parse view response: {}", e))
        })?;

        let documents = view_response
            .rows
            .into_iter()
            .filter_map(|row| row.doc)
            .collect();

        Ok(documents)
    }

    async fn ensure_database(&self, db_name: &str) -> Result<(), DomainError> {
        let url = self.db_url(db_name, None);

        // First check if the database exists
        let response = self
            .client
            .head(&url)
            .header("Authorization", &self.auth_header)
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Request failed: {}", e)))?;

        if response.status().is_success() {
            // Database exists
            return Ok(());
        }

        if response.status() != StatusCode::NOT_FOUND {
            let status = response.status();
            return Err(DomainError::CouchDbError(format!(
                "CouchDB error: {}",
                status
            )));
        }

        // Create the database
        let response = self
            .client
            .put(&url)
            .header("Authorization", &self.auth_header)
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(DomainError::CouchDbError(format!(
                "Failed to create database: {} - {}",
                status, error_text
            )));
        }

        Ok(())
    }

    async fn replicate(
        &self,
        source: &str,
        target: &str,
        options: Value,
    ) -> Result<Value, DomainError> {
        let url = format!("{}/_replicate", self.base_url);

        let mut payload = serde_json::json!({
            "source": source,
            "target": target
        });

        // Merge options if provided
        if let Some(obj) = options.as_object() {
            if let Some(payload_obj) = payload.as_object_mut() {
                for (key, value) in obj {
                    payload_obj.insert(key.clone(), value.clone());
                }
            }
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(DomainError::CouchDbError(format!(
                "Replication failed: {} - {}",
                status, error_text
            )));
        }

        let result = response.json::<Value>().await.map_err(|e| {
            DomainError::CouchDbError(format!("Failed to parse replication response: {}", e))
        })?;

        Ok(result)
    }

    fn get_base_url(&self) -> String {
        self.base_url.clone()
    }

    fn get_auth_credentials(&self) -> Option<(String, String)> {
        // Extract username and password from auth header if it's a Basic auth header
        if self.auth_header.starts_with("Basic ") {
            let encoded = self.auth_header.trim_start_matches("Basic ").trim();
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) {
                if let Ok(auth_str) = String::from_utf8(decoded) {
                    if let Some((username, password)) = auth_str.split_once(':') {
                        return Some((username.to_string(), password.to_string()));
                    }
                }
            }
        }
        None
    }
}
