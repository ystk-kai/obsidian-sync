use axum::response::IntoResponse;
use axum::{extract::State, routing::get, Router};
use metrics::{counter, histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

/// メトリクス収集状態
pub struct MetricsState {
    pub recorder_handle: PrometheusHandle,
    pub request_counts: RwLock<RequestCounts>,
}

/// リクエスト数の集計
pub struct RequestCounts {
    pub total: u64,
    pub success: u64,
    pub error: u64,
    pub longpoll_requests: u64,
    pub longpoll_errors: u64,
    pub bulk_docs_requests: u64,
    pub bulk_docs_errors: u64,
}

impl MetricsState {
    /// 新しいメトリクス状態を作成
    pub fn new() -> Self {
        let builder = PrometheusBuilder::new();
        let builder = builder
            .set_buckets_for_metric(
                Matcher::Full("http_request_duration_seconds".to_string()),
                &[
                    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
                ],
            )
            .expect("Failed to set duration buckets");

        let recorder_handle = builder
            .install_recorder()
            .expect("Failed to install recorder");

        Self {
            recorder_handle,
            request_counts: RwLock::new(RequestCounts {
                total: 0,
                success: 0,
                error: 0,
                longpoll_requests: 0,
                longpoll_errors: 0,
                bulk_docs_requests: 0,
                bulk_docs_errors: 0,
            }),
        }
    }

    /// リクエスト処理時間を記録
    pub fn record_request_duration(&self, path: &str, method: &str, start: Instant) {
        let duration = start.elapsed();
        self.record_request_duration_value(path, method, duration);
    }

    /// リクエスト処理時間を直接値で記録
    pub fn record_request_duration_value(&self, path: &str, _method: &str, duration: Duration) {
        let seconds = duration.as_secs_f64();
        let metric_name = format!("http_request_duration_seconds_{}", path.replace("/", "_"));
        histogram!(metric_name).record(seconds);
    }

    /// リクエストを記録（基本形）
    pub async fn record_request(&self, path: &str, method: &str, status_code: u16) {
        let is_success = status_code < 400;
        let is_longpoll = path.contains("/_changes") && path.contains("feed=longpoll");
        let is_bulk_docs = path.contains("/_bulk_docs");

        // 詳細なメトリクスラベルを設定
        let status_range = match status_code {
            s if s < 200 => "1xx",
            s if s < 300 => "2xx",
            s if s < 400 => "3xx",
            s if s < 500 => "4xx",
            _ => "5xx",
        };

        // ラベル付きのカスタムメトリクス名を作成してカウンター更新
        let metric_name = format!(
            "http_requests_path_{}_method_{}_status_{}",
            path.replace("/", "_"),
            method,
            status_range
        );
        counter!(metric_name).increment(1);

        // 内部カウンタを更新
        let mut counts = self.request_counts.write().await;
        counts.total += 1;

        if is_success {
            counts.success += 1;
        } else {
            counts.error += 1;
        }

        if is_longpoll {
            counts.longpoll_requests += 1;
            if !is_success {
                counts.longpoll_errors += 1;
            }
        }

        if is_bulk_docs {
            counts.bulk_docs_requests += 1;
            if !is_success {
                counts.bulk_docs_errors += 1;
            }
        }

        // カウンターの合計値を更新
        counter!("http_requests_total").increment(1);

        // リクエスト処理の詳細をログに記録
        let log_message = format!(
            "Request: {} {} -> {} (Total: {}, Success: {}, Error: {})",
            method, path, status_code, counts.total, counts.success, counts.error
        );

        if is_longpoll {
            info!(
                "{} [Longpoll: {}/{}]",
                log_message, counts.longpoll_requests, counts.longpoll_errors
            );
        } else if is_bulk_docs {
            info!(
                "{} [BulkDocs: {}/{}]",
                log_message, counts.bulk_docs_requests, counts.bulk_docs_errors
            );
        } else {
            info!("{}", log_message);
        }
    }

    // HTTPプロキシリクエストを記録（互換性のために残す）
    pub fn record_http_proxy_request(
        &self,
        method: String,
        path: String,
        _status: u16,
        elapsed: Duration,
    ) {
        let metric_name = format!(
            "http_request_duration_seconds_path_{}_method_{}",
            path.replace("/", "_"),
            method
        );
        let duration = elapsed.as_secs_f64();
        histogram!(metric_name).record(duration);
    }

    // ドキュメント同期をカウント
    pub fn record_document_sync(&self, db_name: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        let metric_name = format!("document_sync_database_{}_result_{}", db_name, result);
        counter!(metric_name).increment(1);
    }

    // レプリケーションをカウント
    pub fn record_replication(&self, source: &str, target: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        let metric_name = format!(
            "replication_source_{}_target_{}_result_{}",
            source, target, result
        );
        counter!(metric_name).increment(1);
    }
}

impl Default for MetricsState {
    fn default() -> Self {
        Self::new()
    }
}

/// メトリクスエンドポイントハンドラー
pub async fn metrics_handler(State(state): State<Arc<MetricsState>>) -> impl IntoResponse {
    state.recorder_handle.render()
}

// メトリクスのルーターを作成
pub fn create_metrics_router<S>(state: Arc<MetricsState>) -> Router<S> {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}
