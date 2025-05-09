use axum::{extract::State, routing::get, Router};
use metrics::{counter, histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::sync::Arc;
use std::time::Instant;

// メトリクスの状態
pub struct MetricsState {
    recorder_handle: PrometheusHandle,
}

impl MetricsState {
    pub fn new() -> Self {
        // Prometheusレコーダーを作成
        let recorder_handle = PrometheusBuilder::new()
            .add_global_label("service", "livesync_proxy")
            // Define histogram buckets
            .set_buckets_for_metric(
                Matcher::Full("http_request_duration_seconds".to_string()),
                &[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0],
            )
            .unwrap()
            .install_recorder()
            .unwrap();

        Self { recorder_handle }
    }

    // HTTPリクエストをカウント
    pub fn record_request(&self, path: &str, method: &str, status: u16) {
        // metrics 0.24.2では、ラベル付きメトリクスのサポート方法が変更されています
        let metric_name = format!(
            "http_requests_total_path_{}_method_{}_status_{}",
            path, method, status
        );
        counter!(metric_name).increment(1);
    }

    // レスポンス時間を記録
    pub fn record_request_duration(&self, path: &str, method: &str, start: Instant) {
        let duration = start.elapsed().as_secs_f64();
        let metric_name = format!(
            "http_request_duration_seconds_path_{}_method_{}",
            path, method
        );
        histogram!(metric_name).record(duration);
    }

    // ドキュメント同期をカウント
    pub fn record_document_sync(&self, db_name: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        let metric_name = format!("document_sync_total_database_{}_result_{}", db_name, result);
        counter!(metric_name).increment(1);
    }

    // レプリケーションをカウント
    pub fn record_replication(&self, source: &str, target: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        let metric_name = format!(
            "replication_total_source_{}_target_{}_result_{}",
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

// メトリクスのハンドラー
async fn metrics_handler(State(state): State<Arc<MetricsState>>) -> String {
    state.recorder_handle.render()
}

// メトリクスのルーターを作成
pub fn create_metrics_router<S>(state: Arc<MetricsState>) -> Router<S> {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}
