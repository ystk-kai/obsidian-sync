# Obsidian LiveSync Proxy

LiveSync Proxy は Obsidian LiveSync プラグイン用の WebSocket プロキシで、CouchDB との通信を効率化します。この実装は Rust で開発された高速かつメモリ効率の良いサーバーです。

## 概要

Obsidian LiveSync プラグインは、ノートの同期と複数デバイス間の共有を可能にします。このプロキシは以下の機能を提供します：

- WebSocket 経由の双方向通信
- CouchDB データベースとの効率的な接続
- レプリケーションの最適化
- ドキュメント変更のリアルタイム同期

## アーキテクチャ

このプロジェクトは Domain-Driven Design (DDD) の原則に基づいて設計されています：

- **ドメイン層**: ビジネスロジックとモデルを定義
- **アプリケーション層**: ユースケースと操作フローを実装
- **インフラストラクチャ層**: 外部サービス (CouchDB) とのインタラクションを管理
- **インターフェース層**: WebSocket と HTTP エンドポイントを提供

## 機能

- WebSocket を介したリアルタイム同期
- CouchDB へのセキュアな接続
- レプリケーション処理の最適化
- 健全性チェックとメトリクス
- 軽量コンテナに最適化

## 環境変数

サーバーは以下の環境変数で設定できます：

| 変数名 | 説明 | デフォルト値 |
|--------|------|-------------|
| `SERVER_HOST` | サーバーのホスト | `0.0.0.0` |
| `SERVER_PORT` | サーバーのポート | `3000` |
| `COUCHDB_URL` | CouchDB サーバーの URL | `http://localhost:5984` |
| `COUCHDB_USERNAME` | CouchDB ユーザー名 | `admin` |
| `COUCHDB_PASSWORD` | CouchDB パスワード | `password` |
| `RUST_LOG` | ログレベル（trace, debug, info, warn, error） | `info` |

## コンテナでの実行

Docker Compose を使用して簡単に実行できます：

```bash
docker-compose up -d
```

または Docker を直接使用する場合：

```bash
docker run -p 3000:3000 \
  -e COUCHDB_URL=http://couchdb:5984 \
  -e COUCHDB_USERNAME=admin \
  -e COUCHDB_PASSWORD=password \
  livesync-proxy
```

## 開発環境のセットアップ

### 前提条件

- Rust 1.76.0 以上
- CouchDB 3.x

### ビルド

```bash
# リポジトリをクローン
git clone https://github.com/yourusername/obsidian-sync.git
cd obsidian-sync/livesync-proxy

# 依存関係をインストールしてビルド
cargo build

# テストの実行
cargo test

# 実行
cargo run
```

## API エンドポイント

### WebSocket 接続

- `ws://localhost:3000/db` - LiveSync WebSocket 接続

### HTTP エンドポイント

- `GET /` - 静的なウェルカムページ
- `GET /health` - ヘルスチェックエンドポイント
- `GET /metrics` - Prometheus 形式のメトリクス
- `GET /api/status` - サーバーステータス情報

## モニタリングとメトリクス

サーバーは `/metrics` エンドポイントで Prometheus 形式のメトリクスを提供します：

- `livesync_proxy_http_requests_total` - HTTP リクエスト数
- `livesync_proxy_http_request_duration_seconds` - リクエスト処理時間
- `livesync_proxy_websocket_connections_count` - アクティブな WebSocket 接続数
- `livesync_proxy_document_sync_total` - ドキュメント同期処理数
- `livesync_proxy_replication_total` - レプリケーション処理数

## ヘルスチェック

`/health` エンドポイントは以下の情報を提供します：

- 全体的なサーバーの状態
- 稼働時間
- CouchDB 接続状態
- バージョン情報
