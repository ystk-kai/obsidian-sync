use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, error, info};

use crate::domain::models::{CouchDbDocument, DomainError};
use crate::domain::services::CouchDbRepository;

/// CouchDB クライアント
pub struct CouchDbClient {
    client: Client,
    base_url: String,
    username: String,
    password: String,
}

impl CouchDbClient {
    /// 新しいCouchDBクライアントを作成
    pub fn new(base_url: &str, username: &str, password: &str) -> Self {
        let client = Client::builder()
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.to_string(),
            username: username.to_string(),
            password: password.to_string(),
        }
    }

    /// CouchDBサーバーにpingを送信して接続を確認
    pub async fn ping(&self) -> Result<()> {
        let url = format!("{}/", self.base_url);
        debug!("Pinging CouchDB at {}", url);

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => {
                debug!("CouchDB ping successful");
                Ok(())
            }
            status => {
                error!("CouchDB ping failed with status: {}", status);
                Err(anyhow!("CouchDB ping failed with status: {}", status))
            }
        }
    }

    /// データベースが存在するか確認
    pub async fn database_exists(&self, db_name: &str) -> Result<bool> {
        let url = format!("{}/{}", self.base_url, db_name);
        debug!("Checking if database exists: {}", db_name);

        let response = self
            .client
            .head(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        Ok(response.status() == StatusCode::OK)
    }

    /// データベースを作成
    pub async fn create_database(&self, db_name: &str) -> Result<()> {
        let url = format!("{}/{}", self.base_url, db_name);
        debug!("Creating database: {}", db_name);

        let response = self
            .client
            .put(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        match response.status() {
            StatusCode::CREATED => {
                info!("Database created: {}", db_name);
                Ok(())
            }
            StatusCode::PRECONDITION_FAILED => {
                debug!("Database already exists: {}", db_name);
                Ok(())
            }
            status => {
                error!(
                    "Failed to create database {} with status: {}",
                    db_name, status
                );
                Err(anyhow!("Failed to create database with status: {}", status))
            }
        }
    }
}

// CouchDbRepositoryトレイトの実装
#[async_trait]
impl CouchDbRepository for CouchDbClient {
    /// ドキュメントを取得
    async fn get_document(
        &self,
        db_name: &str,
        doc_id: &str,
    ) -> Result<CouchDbDocument, DomainError> {
        let url = format!("{}/{}/{}", self.base_url, db_name, doc_id);
        debug!("Getting document: {}/{}", db_name, doc_id);

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Failed to get document: {}", e)))?;

        if response.status() == StatusCode::NOT_FOUND {
            return Err(DomainError::CouchDbError(format!(
                "Document {} not found",
                doc_id
            )));
        }

        if !response.status().is_success() {
            return Err(DomainError::CouchDbError(format!(
                "Failed to get document with status: {}",
                response.status()
            )));
        }

        let doc = response
            .json::<CouchDbDocument>()
            .await
            .map_err(|e| DomainError::InvalidMessage(format!("Failed to parse document: {}", e)))?;

        Ok(doc)
    }

    /// ドキュメントを保存
    async fn save_document(
        &self,
        db_name: &str,
        doc: CouchDbDocument,
    ) -> Result<CouchDbDocument, DomainError> {
        let doc_id = doc.id.clone();
        let url = format!("{}/{}/{}", self.base_url, db_name, doc_id);
        debug!("Saving document: {}/{}", db_name, doc_id);

        let response = self
            .client
            .put(&url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&doc)
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Failed to save document: {}", e)))?;

        if !response.status().is_success() {
            return Err(DomainError::CouchDbError(format!(
                "Failed to save document with status: {}",
                response.status()
            )));
        }

        // 必要なフィールドだけを含む構造体を定義
        #[derive(Deserialize)]
        struct RevOnly {
            rev: String,
        }

        let save_response = response.json::<RevOnly>().await.map_err(|e| {
            DomainError::InvalidMessage(format!("Failed to parse save response: {}", e))
        })?;

        // 更新された_revを持つドキュメントを返す
        let mut updated_doc = doc;
        updated_doc.rev = Some(save_response.rev);

        Ok(updated_doc)
    }

    /// ドキュメントを削除
    async fn delete_document(
        &self,
        db_name: &str,
        doc_id: &str,
        rev: &str,
    ) -> Result<(), DomainError> {
        let url = format!("{}/{}/{}?rev={}", self.base_url, db_name, doc_id, rev);
        debug!("Deleting document: {}/{} (rev: {})", db_name, doc_id, rev);

        let response = self
            .client
            .delete(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Failed to delete document: {}", e)))?;

        if !response.status().is_success() {
            return Err(DomainError::CouchDbError(format!(
                "Failed to delete document with status: {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// ビューに対してクエリを実行
    async fn query_view(
        &self,
        db_name: &str,
        design_doc: &str,
        view_name: &str,
        options: Value,
    ) -> Result<Vec<CouchDbDocument>, DomainError> {
        let url = format!(
            "{}/{}/_design/{}/_view/{}",
            self.base_url, db_name, design_doc, view_name
        );
        debug!(
            "Querying view: {}/{}/_design/{}/_view/{}",
            self.base_url, db_name, design_doc, view_name
        );

        let mut request = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password));

        // オプションがオブジェクトの場合、クエリパラメータとして追加
        if let Some(obj) = options.as_object() {
            for (key, value) in obj {
                if let Some(value_str) = value.as_str() {
                    request = request.query(&[(key, value_str)]);
                }
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| DomainError::CouchDbError(format!("Failed to query view: {}", e)))?;

        if !response.status().is_success() {
            return Err(DomainError::CouchDbError(format!(
                "Failed to query view with status: {}",
                response.status()
            )));
        }

        #[derive(Deserialize)]
        struct ViewResponse {
            rows: Vec<ViewRow>,
        }

        #[derive(Deserialize)]
        struct ViewRow {
            doc: Option<CouchDbDocument>,
        }

        let view_response = response.json::<ViewResponse>().await.map_err(|e| {
            DomainError::InvalidMessage(format!("Failed to parse view response: {}", e))
        })?;

        let docs = view_response
            .rows
            .into_iter()
            .filter_map(|row| row.doc)
            .collect();

        Ok(docs)
    }

    /// データベースの存在を確認し、必要に応じて作成
    async fn ensure_database(&self, db_name: &str) -> Result<(), DomainError> {
        match self.database_exists(db_name).await {
            Ok(true) => {
                debug!("Database exists: {}", db_name);
                Ok(())
            }
            Ok(false) => {
                info!("Database does not exist, creating: {}", db_name);
                self.create_database(db_name).await.map_err(|e| {
                    DomainError::CouchDbError(format!("Failed to create database: {}", e))
                })
            }
            Err(e) => Err(DomainError::CouchDbError(format!(
                "Failed to check database existence: {}",
                e
            ))),
        }
    }

    /// データベース間のレプリケーションを実行
    async fn replicate(
        &self,
        source: &str,
        target: &str,
        options: Value,
    ) -> Result<Value, DomainError> {
        let url = format!("{}/_replicate", self.base_url);
        debug!("Replicating from {} to {}", source, target);

        let mut replication_body = serde_json::json!({
            "source": source,
            "target": target,
        });

        // オプションがオブジェクトの場合、レプリケーション設定に統合
        if let Some(obj) = options.as_object() {
            if let Some(repl_obj) = replication_body.as_object_mut() {
                for (key, value) in obj {
                    repl_obj.insert(key.clone(), value.clone());
                }
            }
        }

        let response = self
            .client
            .post(&url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&replication_body)
            .send()
            .await
            .map_err(|e| {
                DomainError::CouchDbError(format!("Failed to start replication: {}", e))
            })?;

        if !response.status().is_success() {
            return Err(DomainError::CouchDbError(format!(
                "Failed to replicate with status: {}",
                response.status()
            )));
        }

        let result = response.json::<Value>().await.map_err(|e| {
            DomainError::InvalidMessage(format!("Failed to parse replication response: {}", e))
        })?;

        Ok(result)
    }

    /// CouchDBサーバーのベースURLを取得
    fn get_base_url(&self) -> String {
        self.base_url.clone()
    }

    /// 認証情報を取得
    fn get_auth_credentials(&self) -> Option<(String, String)> {
        Some((self.username.clone(), self.password.clone()))
    }
}
