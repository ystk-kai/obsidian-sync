use anyhow::{anyhow, Result};
use async_trait::async_trait;
use axum::body::Body as AxumBody;
use axum::http::{HeaderMap, Response as AxumResponse};
use axum::response::Response;
use bytes::Bytes;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::{Client, Method, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use std::str::FromStr;
use tracing::{debug, error, info, warn};

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
            .timeout(std::time::Duration::from_secs(60))
            .connection_verbose(true)
            .user_agent("Obsidian-LiveSync-Proxy/1.0")
            .build()
            .expect("Failed to create HTTP client");

        // ベースURLが/で終わるように調整
        let base_url = if base_url.ends_with('/') {
            base_url.to_string()
        } else {
            format!("{}/", base_url)
        };

        debug!("Creating CouchDB client with URL: {}", base_url);

        Self {
            client,
            base_url,
            username: username.to_string(),
            password: password.to_string(),
        }
    }

    /// CouchDBサーバーにpingを送信して接続を確認
    pub async fn ping(&self) -> Result<()> {
        let url = format!("{}/", self.base_url);
        debug!("Pinging CouchDB at {}", url);

        // 認証情報をデバッグ出力
        if !self.username.is_empty() && !self.password.is_empty() {
            debug!(
                "Using credentials - Username: {}, Password length: {}",
                self.username,
                self.password.len()
            );
        } else {
            debug!(
                "No credentials provided or empty credentials, connecting without authentication"
            );
        }

        // 認証が必要かどうかを確認
        let mut req_builder = self.client.get(&url);

        if !self.username.is_empty() && !self.password.is_empty() {
            debug!("Adding basic authentication to ping request");
            req_builder = req_builder.basic_auth(&self.username, Some(&self.password));
        }

        // リクエストのヘッダーを表示
        debug!("Sending ping request to CouchDB");

        let response = req_builder.send().await?;

        // ステータスを事前に取得
        let status = response.status();
        debug!("CouchDB ping response status: {}", status);

        // レスポンスボディをログに出力（エラー情報を取得するため）
        if !status.is_success() {
            if let Ok(body_text) = response.text().await {
                debug!("CouchDB ping response body: {}", body_text);
            }
            error!("CouchDB ping failed with status: {}", status);
            return Err(anyhow!("CouchDB ping failed with status: {}", status));
        }

        debug!("CouchDB ping successful");
        Ok(())
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

    /// HTTPリクエストをCouchDBに転送する
    pub async fn http_forward_request(
        &self,
        method: &str,
        path: &str,
        query: Option<String>,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<Response<AxumBody>> {
        // URLを構築
        let mut url = format!("{}{}", self.base_url, path);
        if let Some(ref q) = query {
            url.push('?');
            url.push_str(q);
        }

        // より詳細なリクエスト情報をログに出力
        info!("Forwarding request to CouchDB: {} {}", method, url);
        debug!("Request headers: {:?}", headers);
        debug!("Request body size: {} bytes", body.len());

        // HTTPメソッドを解析
        let method = Method::from_str(method).unwrap_or(Method::GET);

        // longpollリクエストの検出 - _changesエンドポイントでfeed=longpollパラメータを含む場合
        let is_longpoll = path.contains("/_changes")
            && query.as_ref().is_some_and(|q| q.contains("feed=longpoll"));

        // 通常の_changesリクエストの検出（longpollでない場合も含む）
        let is_changes_request = path.contains("/_changes");

        // bulk_docsリクエストの検出（大きなデータ転送が予想される）
        let _is_bulk_docs = path.contains("/_bulk_docs");

        // クライアントを選択（通常用とlongpoll用で別々のタイムアウト設定）
        let client = if is_longpoll {
            // longpoll用に長いタイムアウトを持つクライアントを作成
            info!(
                "Detected longpoll request, using extended timeout: {} {}",
                method, url
            );
            Client::builder()
                .timeout(std::time::Duration::from_secs(120)) // 120秒のタイムアウト（CouchDBの設定より長く）
                .connection_verbose(true)
                .user_agent("Obsidian-LiveSync-Proxy/1.0")
                .tcp_keepalive(Some(std::time::Duration::from_secs(30))) // TCP keepaliveを有効化
                .tcp_nodelay(true) // TCPノーディレイを有効化（レイテンシ削減）
                .pool_idle_timeout(std::time::Duration::from_secs(120)) // 接続プールのアイドルタイムアウトを延長
                .pool_max_idle_per_host(10) // ホストごとの最大アイドル接続数を増加
                .build()
                .expect("Failed to create HTTP client for longpoll")
        } else if is_changes_request {
            // 通常の_changesリクエスト用のクライアント（longpollではない）
            info!("Detected regular _changes request: {} {}", method, url);
            Client::builder()
                .timeout(std::time::Duration::from_secs(90)) // 90秒のタイムアウト
                .connection_verbose(true)
                .user_agent("Obsidian-LiveSync-Proxy/1.0")
                .tcp_nodelay(true) // TCPノーディレイを有効化
                .build()
                .expect("Failed to create HTTP client for changes request")
        } else {
            // 通常のクライアントを使用
            self.client.clone()
        };

        // reqwestのリクエストビルダーを構築
        let mut req_builder = client.request(method.clone(), &url);

        // Abortエラーを防ぐために必要なヘッダーを追加（_changesリクエスト用）
        if is_changes_request {
            // Connection: keep-aliveを明示的に設定
            req_builder = req_builder.header("Connection", "keep-alive");

            // フラグメント応答を許可
            req_builder = req_builder.header("Accept", "application/json");

            // より詳細なログ出力
            info!("Added special headers for _changes request: {}", url);
            if is_longpoll {
                info!(
                    "This is a longpoll request with path: {}, query: {:?}",
                    path, query
                );
            }
        }

        // 認証情報を追加（空でない場合のみ）
        if !self.username.is_empty() && !self.password.is_empty() {
            debug!("Adding basic auth for user: {}", self.username);
            req_builder = req_builder.basic_auth(&self.username, Some(&self.password));
        } else {
            debug!(
                "No credentials provided or empty credentials, connecting without authentication"
            );
        }

        // ヘッダーを追加（Hostヘッダーは除外し、認証関連ヘッダーも上書き）
        for (key, value) in headers.iter() {
            if key.as_str().to_lowercase() != "host"
                && key.as_str().to_lowercase() != "authorization"
            {
                req_builder = req_builder.header(key.as_str(), value);
            }
        }

        // リクエストボディを追加（空でなければ）
        if !body.is_empty() {
            req_builder = req_builder.body(body);
        }

        // リクエストを送信
        let response = match req_builder.send().await {
            Ok(resp) => resp,
            Err(e) => {
                // 接続エラーの詳細をログに出力
                error!("Connection error with CouchDB: {}", e);

                // 構造化されたエラーハンドリング
                match e {
                    // longpollリクエストのAbortエラー - クライアント側で中断された場合
                    err if is_longpoll
                        && (err.to_string().contains("aborted")
                            || err.to_string().contains("canceled")) =>
                    {
                        info!(
                            "Longpoll request was aborted by client, this is often normal: {} {}",
                            method, url
                        );
                        return AxumResponse::builder()
                            .status(StatusCode::NO_CONTENT)
                            .header(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("application/json"),
                            )
                            .body(AxumBody::from(r#"{"ok":true,"reason":"request_aborted"}"#))
                            .map_err(|e| anyhow!("Failed to build abort response: {}", e));
                    }
                    // タイムアウトエラー - 特に長時間リクエストで発生
                    err if err.is_timeout() => {
                        warn!(
                            "Request timed out: {} {} after {} seconds",
                            method,
                            url,
                            if is_longpoll { 120 } else { 30 }
                        );
                        return AxumResponse::builder()
                            .status(StatusCode::GATEWAY_TIMEOUT)
                            .header(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("application/json"),
                            )
                            .body(AxumBody::from(format!(
                                r#"{{"error":"Request timed out after {} seconds","reason":"timeout"}}"#,
                                if is_longpoll { 120 } else { 30 }
                            )))
                            .map_err(|e| anyhow!("Failed to build timeout response: {}", e));
                    }
                    // 接続エラー - サーバーに到達できない
                    err if err.is_connect() => {
                        error!("Connection failed: {} {}: {}", method, url, err);
                        return AxumResponse::builder()
                            .status(StatusCode::BAD_GATEWAY)
                            .header(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("application/json"),
                            )
                            .body(AxumBody::from(
                                r#"{"error":"Failed to connect to CouchDB","reason":"connection_failed"}"#.to_string()
                            ))
                            .map_err(|e| anyhow!("Failed to build connection error response: {}", e));
                    }
                    // その他のエラー
                    _ => {
                        error!("Unexpected error: {} for {} {}", e, method, url);
                        return AxumResponse::builder()
                            .status(StatusCode::BAD_GATEWAY)
                            .header(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("application/json"),
                            )
                            .body(AxumBody::from(format!(
                                r#"{{"error":"Connection to CouchDB failed: {}","reason":"unexpected_error"}}"#,
                                e
                            )))
                            .map_err(|e| anyhow!("Failed to build error response: {}", e));
                    }
                }
            }
        };

        // レスポンスステータスとヘッダーを取得
        let status = response.status();
        let headers = response.headers().clone();
        info!("CouchDB responded with status: {}", status);
        debug!("Response headers: {:?}", headers);

        // Axumのレスポンスを構築
        let mut axum_response_builder = AxumResponse::builder().status(status);

        // ヘッダーを転送（ただし一部の特別なヘッダーは除外）
        for (key, value) in headers.iter() {
            if let Ok(name) = HeaderName::from_str(key.as_str()) {
                if let Ok(val) = HeaderValue::from_bytes(value.as_bytes()) {
                    axum_response_builder = axum_response_builder.header(name, val);
                }
            }
        }

        // ストリーミングレスポンスでなく、完全なボディを取得してからレスポンスを返す
        // 特にchunkedエンコーディングの場合に問題が発生することがあるため
        let body_bytes = match response.bytes().await {
            Ok(bytes) => {
                debug!("Successfully read response body: {} bytes", bytes.len());
                bytes
            }
            Err(e) => {
                error!("Failed to read response body: {}", e);
                // エラーの場合はエラーメッセージを含むレスポンスを返す
                return AxumResponse::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .header(
                        HeaderName::from_static("content-type"),
                        HeaderValue::from_static("application/json"),
                    )
                    .body(AxumBody::from(format!(
                        r#"{{"error":"Failed to read response body: {}"}}"#,
                        e
                    )))
                    .map_err(|e| anyhow!("Failed to build error response: {}", e));
            }
        };

        // レスポンスを構築して返す
        debug!("Building final response with {} bytes", body_bytes.len());
        let axum_response = axum_response_builder
            .body(AxumBody::from(body_bytes))
            .map_err(|e| anyhow!("Failed to build response: {}", e))?;

        Ok(axum_response)
    }

    /// デフォルトデータベース名を取得
    pub fn get_dbname(&self) -> String {
        // 必要に応じてフィールド追加後、ここで返す
        // 仮実装: "obsidian" を返す
        "obsidian".to_string()
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

    /// HTTPリクエストをCouchDBに転送する
    async fn forward_request(
        &self,
        method: &str,
        path: &str,
        query: Option<String>,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<Response<AxumBody>, DomainError> {
        match self
            .http_forward_request(method, path, query, headers, body)
            .await
        {
            Ok(response) => Ok(response),
            Err(e) => Err(DomainError::CouchDbError(format!(
                "Failed to forward request: {}",
                e
            ))),
        }
    }
}
