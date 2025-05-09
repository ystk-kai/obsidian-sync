# Obsidian LiveSync Proxy 開発者ガイド

このドキュメントは Obsidian LiveSync Proxy の開発者向けガイドです。プロジェクトのアーキテクチャ、コード構成、および拡張方法について説明します。

## 環境セットアップ

### 必要条件

- Rust 1.86.0 以降
- Cargo (Rustのパッケージマネージャ)
- Docker (オプション、統合テスト用)

### セットアップ手順

1. リポジトリをクローン:
```bash
git clone <リポジトリURL>
cd obsidian-sync
```

2. 依存関係をビルド:
```bash
cd livesync-proxy
cargo build
```

3. 開発サーバーを実行:
```bash
cargo run
```

4. テストを実行:
```bash
cargo test
```

## アーキテクチャ概要

LiveSync Proxy は Domain-Driven Design (DDD) の原則に従って構築されています。コードは以下の4つの主要レイヤーに分かれています：

1. **ドメイン層** (`src/domain/`) - ビジネスロジックとモデルを定義します
2. **アプリケーション層** (`src/application/`) - ユースケースとサービスを実装します
3. **インフラストラクチャ層** (`src/infrastructure/`) - 外部システムとの連携を担当します
4. **インターフェース層** (`src/interfaces/`) - ユーザーインターフェースとAPIを提供します

これらのレイヤーは依存関係の方向が内側に向かうように設計されています：

```
インターフェース層 → アプリケーション層 → ドメイン層 ← インフラストラクチャ層
```

## ディレクトリ構造

```
livesync-proxy/
├── Cargo.toml                  # プロジェクト設定とパッケージ依存関係
├── Dockerfile                  # コンテナビルド設定
├── src/
│   ├── main.rs                 # アプリケーションのエントリーポイント
│   ├── lib.rs                  # ライブラリのエントリーポイント
│   ├── domain.rs               # ドメインモジュールのエクスポート
│   ├── application.rs          # アプリケーションモジュールのエクスポート
│   ├── infrastructure.rs       # インフラストラクチャモジュールのエクスポート
│   ├── interfaces.rs           # インターフェースモジュールのエクスポート
│   ├── utils.rs                # ユーティリティ関数
│   ├── domain/                 # ドメイン層
│   │   ├── models.rs           # ドメインモデルの定義
│   │   └── services.rs         # ドメインサービスのインターフェース
│   ├── application/            # アプリケーション層
│   │   └── services.rs         # アプリケーションサービスの実装
│   ├── infrastructure/         # インフラストラクチャ層
│   │   ├── config.rs           # 設定管理
│   │   └── couchdb.rs          # CouchDB クライアント実装
│   └── interfaces/             # インターフェース層
│       ├── web.rs              # Webモジュールのエクスポート
│       └── web/                # Web関連のコンポーネント
│           ├── handlers.rs     # リクエストハンドラー
│           ├── health.rs       # ヘルスチェック機能
│           ├── metrics.rs      # メトリクス機能
│           └── server.rs       # HTTPサーバー設定
├── static/                     # 静的ファイル
│   └── index.html              # ウェルカムページ
└── tests/                      # 統合テスト
    ├── couchdb_repository_test.rs    # CouchDBリポジトリのテスト
    └── livesync_service_test.rs      # LiveSyncサービスのテスト
```

## 主要コンポーネント

### ドメイン層

ドメイン層には、アプリケーションのコアとなるビジネスロジックとモデルが含まれています。

#### モデル (`models.rs`)

主要なドメインモデルには以下が含まれます：

- `CouchDbDocument` - CouchDBドキュメントを表します
- `DomainError` - ドメイン固有のエラーを表します

#### サービスインターフェース (`services.rs`)

ドメイン層は、インフラストラクチャ層によって実装される必要のあるインターフェースを定義します：

- `CouchDbRepository` - CouchDBとの対話に必要なメソッドを定義します

### アプリケーション層

アプリケーション層は、ユースケース（アプリケーションの機能）を実装し、ドメイン層のオブジェクトを操作します。

#### サービス (`services.rs`)

- `LiveSyncService` - メインのアプリケーションサービスで、ドキュメント同期やレプリケーションの処理を担当します

### インフラストラクチャ層

インフラストラクチャ層は、外部サービスとの連携を担当します。

#### 設定 (`config.rs`)

- `AppConfig` - 環境変数からアプリケーション設定を読み込みます

#### CouchDB クライアント (`couchdb.rs`)

- `CouchDbClient` - CouchDBリポジトリインターフェースを実装し、RESTful APIを介してCouchDBと通信します

### インターフェース層

インターフェース層は、ユーザーおよび外部システムとの通信を担当します。

#### HTTP プロキシハンドラー (`web/handlers.rs`)

- `http_proxy_handler` - HTTP接続のプロキシ処理を担当します
- 注意: ObsidianクライアントはHTTP/HTTPS接続でCouchDBと通信します

#### ヘルスチェック (`web/health.rs`)

- `HealthState` - アプリケーションの健全性状態を管理します
- `health_handler` - ヘルスチェックエンドポイントを提供します

#### メトリクス (`web/metrics.rs`)

- `MetricsState` - アプリケーションメトリクスを管理します
- `metrics_handler` - Prometheusフォーマットのメトリクスを提供します

#### サーバー (`web/server.rs`)

- `AppState` - アプリケーション状態を管理します
- `start_web_server` - HTTPサーバーを起動します

## 拡張ガイド

### 新しいエンドポイントの追加

新しいHTTPエンドポイントを追加するには：

1. `src/interfaces/web/handlers.rs`に新しいハンドラー関数を作成します
2. `src/interfaces/web/server.rs`の`start_web_server`関数内でルーターに新しいエンドポイントを追加します

例：
```rust
// ハンドラーの追加
async fn new_endpoint_handler() -> impl IntoResponse {
    "New endpoint response".to_string()
}

// ルーターに追加
let app = Router::new()
    .route("/new-endpoint", get(new_endpoint_handler))
    // ...他のルート
```

### CouchDB機能の拡張

CouchDBとの連携を拡張するには：

1. `src/domain/services.rs`の`CouchDbRepository`トレイトに新しいメソッドを追加します
2. `src/infrastructure/couchdb.rs`の`CouchDbClient`にそのメソッドの実装を追加します

例：
```rust
// CouchDbRepository トレイトに追加
#[async_trait]
pub trait CouchDbRepository: Send + Sync {
    // ...既存のメソッド
    
    // 新しいメソッド
    async fn new_couchdb_operation(&self, param: &str) -> Result<Value, DomainError>;
}

// CouchDbClient に実装を追加
#[async_trait]
impl CouchDbRepository for CouchDbClient {
    // ...既存の実装
    
    // 新しいメソッドの実装
    async fn new_couchdb_operation(&self, param: &str) -> Result<Value, DomainError> {
        // 実装内容
    }
}
```

## テスト戦略

プロジェクトには以下のテストタイプが含まれています：

1. **単体テスト** - 個々のコンポーネントをテストします（各モジュール内の`#[cfg(test)]`ブロック）
2. **統合テスト** - コンポーネント間の統合をテストします（`tests/`ディレクトリ）

テストの実行方法：

```bash
# すべてのテストを実行
cargo test

# 特定のテストを実行
cargo test couchdb_repository

# テスト実行時にログを表示
RUST_LOG=debug cargo test -- --nocapture
```

## パフォーマンス最適化

アプリケーションのパフォーマンスを最適化するためのヒント：

1. **HTTP プロキシ効率化** - 効率的なリクエスト転送とレスポンス処理を確保します
2. **CouchDBバッチ処理** - 複数のオペレーションを単一リクエストにバッチ処理します
3. **接続プーリング** - HTTP接続の再利用を確保します
4. **キャッシング** - 頻繁にアクセスされるデータをキャッシュします

## コード規約

このプロジェクトでは以下のコード規約を採用しています：

1. **モジュール構成** - Rust 2024エディションの推奨に従い、`mod.rs`ファイルを避けます
2. **エラー処理** - `thiserror`と`anyhow`を使用して型安全なエラー処理を行います
3. **非同期処理** - `async/await`パターンを使用します
4. **コメント** - パブリックAPIには常にドキュメントコメント（`///`）を付けます
5. **命名規則** - Rustの標準的な命名規則に従います（`snake_case`メソッド、`CamelCase`型など）

## デプロイメント

アプリケーションは Docker を使用してデプロイされ、GitHub Actions を通じて継続的デリバリーが構成されています。

`.github/workflows/ci.yml` の CI/CD パイプラインでは以下のステップが実行されます：

1. コードのチェックアウト
2. 依存関係のキャッシュ
3. Rustツールチェーンのインストール
4. コード形式のチェック
5. Clippy による静的解析
6. テストの実行
7. リリースビルド
8. Dockerイメージのビルドとレジストリへのプッシュ

## トラブルシューティング

開発中によくある問題とその解決方法：

1. **CouchDB接続エラー** - CouchDBのURLとクレデンシャルが正しいか確認します
2. **HTTPプロキシエラー** - ルーティングとネットワーク設定を確認します
3. **メトリクスやヘルスチェックの問題** - 依存関係が適切にインストールされているか確認します

## 主要依存関係

プロジェクトは以下の主要なRustクレートに依存しています：

| 依存関係 | バージョン | 説明 |
|----------|-----------|------|
| axum | 0.8.4 | 高性能なWebフレームワーク |
| tokio | 1.45.0 | 非同期ランタイム |
| reqwest | 0.12.15 | HTTPクライアント |
| serde | 1.0.219 | シリアライズ/デシリアライズフレームワーク |
| uuid | 1.7.0 | UUID生成 |
| chrono | 0.4.35 | 日時処理 |
| tracing | 0.1.41 | ロギングとトレーシング |
| metrics-exporter-prometheus | 0.17.0 | メトリクス収集 |
| base64 | 0.22.1 | Base64エンコード/デコード |

## 2025年5月の更新内容

2025年5月に以下の更新が行われました：

1. すべての依存関係を最新バージョンに更新
2. 以下のようなAPI変更に対応
   - `base64` クレートの新しいエンジンAPIに対応
   - `metrics-exporter-prometheus` の新しいバケット設定方法の採用
   - `url` パーサーの最新APIに対応
3. ヘルスチェックを追加して可用性モニタリングの改善
4. プロジェクト構造を整理（GitHub Workflowsとドキュメントの再編成）

## 参考リソース

- [Axum ドキュメント](https://docs.rs/axum/latest/axum/)
- [Tower HTTP ドキュメント](https://docs.rs/tower-http/latest/tower_http/)
- [CouchDB API リファレンス](https://docs.couchdb.org/en/stable/api/index.html)
- [Prometheus Metrics](https://prometheus.io/docs/concepts/metric_types/)

## ライセンス

MIT
