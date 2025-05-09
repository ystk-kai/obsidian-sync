use axum::{Router, extract::State, routing::get};
use metrics::{counter, gauge, histogram};
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
        let labels = [
            ("path", path.to_string()),
            ("method", method.to_string()),
            ("status", status.to_string()),
        ];
        counter!("http_requests_total", &labels).increment(1);
    }

    // レスポンス時間を記録
    pub fn record_request_duration(&self, path: &str, method: &str, start: Instant) {
        let duration = start.elapsed().as_secs_f64();
        let labels = [("path", path.to_string()), ("method", method.to_string())];
        histogram!("http_request_duration_seconds", &labels).record(duration);
    }

    // WebSocket接続数を更新
    pub fn update_websocket_connections(&self, count: usize) {
        gauge!("websocket_connections_count").set(count as f64);
    }

    // ドキュメント同期をカウント
    pub fn record_document_sync(&self, db_name: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        let labels = [
            ("database", db_name.to_string()),
            ("result", result.to_string()),
        ];
        counter!("document_sync_total", &labels).increment(1);
    }

    // レプリケーションをカウント
    pub fn record_replication(&self, source: &str, target: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        let labels = [
            ("source", source.to_string()),
            ("target", target.to_string()),
            ("result", result.to_string()),
        ];
        counter!("replication_total", &labels).increment(1);
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
