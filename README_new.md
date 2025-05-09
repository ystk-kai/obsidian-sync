# obsidian-sync

このプロジェクトはObsidianのLiveSyncプラグインとCouchDBを使用した同期環境をDockerで構築するためのものです。

<p align="center">
  <img src="docs/images/logo.png" alt="Obsidian Sync Logo" width="200"/>
</p>

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Docker](https://img.shields.io/badge/Docker-Ready-blue)](https://www.docker.com/)
[![Obsidian](https://img.shields.io/badge/Obsidian-LiveSync-purple)](https://obsidian.md/)

## Overview

ObsidianのLiveSyncプラグインを使用して、複数のデバイス間でスムーズにノートを同期するためのDockerベースの環境です。CouchDBとWebSocketプロキシを使用して、セキュアで高速な同期を実現します。

```mermaid
flowchart TD
    A[Obsidian クライアント] --WebSocket--> B[LiveSync Proxy]
    C[Obsidian モバイル] --WebSocket--> B
    B --HTTP--> D[CouchDB]
    E[バックアップスケジューラー] --定期実行--> F[バックアップサービス]
    F --データ抽出--> D
    F --Git Push--> G[Gitリポジトリ]
    
    style A fill:#9999ff,stroke:#333,stroke-width:2px
    style C fill:#9999ff,stroke:#333,stroke-width:2px
    style B fill:#88dd88,stroke:#333,stroke-width:2px
    style D fill:#ffcc66,stroke:#333,stroke-width:2px
    style E fill:#ff9966,stroke:#333,stroke-width:2px
    style F fill:#ff9966,stroke:#333,stroke-width:2px
    style G fill:#dddddd,stroke:#333,stroke-width:2px
```

## Requirements

- Docker Engine 20.x以降またはDocker Desktop最新版
- Docker Compose v2（`docker compose` コマンド）
- 開発時：Rust 1.86.0以降

## Quick Start

1. リポジトリをクローン：
   ```bash
   git clone <リポジトリURL>
   cd obsidian-sync
   ```

2. 環境変数ファイルを作成：
   ```bash
   cp .env.example .env
   # エディタで.envを開き、パスワードなどを変更してください
   ```

3. Docker Composeでサービスを起動：
   ```bash
   # 開発モード
   docker compose up --build
   
   # 本番モード（バックグラウンド起動）
   docker compose up -d
   ```

4. ブラウザで以下のURLにアクセス：
   - LiveSync Proxy: http://localhost:3000
   - CouchDB管理画面: http://localhost:5984/_utils/

## Configuration

`.env`ファイルで以下の環境変数を設定できます：

| 環境変数 | 説明 | デフォルト値 | 必須 |
|----------|------|-------------|------|
| `COMPOSE_PROJECT_NAME` | Dockerコンポーズプロジェクト名 | obsidian-sync | いいえ |
| `HOST_COUCHDB_PORT` | CouchDBのホスト側ポート | 5984 | いいえ |
| `HOST_PROXY_PORT` | Proxyのホスト側ポート | 3000 | いいえ |
| `COUCHDB_USER` | CouchDB管理者ユーザー名 | admin | はい |
| `COUCHDB_PASSWORD` | CouchDB管理者パスワード | change_this_password | はい |
| `BACKUP_SCHEDULE` | バックアップスケジュール（Cron形式） | 0 2 * * * | いいえ |
| `BACKUP_RUN_ON_STARTUP` | 起動時にバックアップを実行するか | false | いいえ |
| `BACKUP_GIT_REPO` | バックアップ先Gitリポジトリ | なし | はい |
| `BACKUP_GIT_BRANCH` | Gitリポジトリのブランチ | main | いいえ |
| `BACKUP_GIT_TOKEN` | GitリポジトリアクセスToken | なし | はい |

## Obsidian LiveSync Setup

Obsidianアプリ内でLiveSyncプラグインをインストールし、以下のように設定してください：

1. リモートデータベースタイプ: WebSocketサーバー
2. WebSocket URL: `ws://[サーバーのIPまたはホスト名]:3000/db`

## Backup

本システムには自動バックアップ機能が組み込まれています。デフォルトでは毎日日本時間の午前2時にバックアップが実行され、Gitリポジトリに保存されます。

### 自動バックアップの設定

`.env`ファイルで以下の設定が可能です：

```
# バックアップスケジュール（CRON形式）- 日本時間で実行されます
BACKUP_SCHEDULE=0 2 * * *  # 毎日午前2時に実行

# Gitリポジトリ設定
BACKUP_GIT_REPO=https://github.com/username/obsidian-backup.git
BACKUP_GIT_BRANCH=main
BACKUP_GIT_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxx
```

### 手動バックアップの実行

バックアップを手動で実行するには：

```bash
# Gitリポジトリへのバックアップを実行
docker compose run backup
```

### 従来の方法（ローカルバックアップのみ）

ローカルディスクのみにバックアップする場合：

```bash
# CouchDBのデータボリュームをエクスポート
docker run --rm -v obsidian-sync_couchdb_data:/data -v $(pwd)/backup:/backup \
  alpine tar -czf /backup/couchdb_data_$(date +%Y%m%d).tar.gz -C /data .
```

## Project Structure

```
obsidian-sync/
├── backup/            # バックアップ関連ファイル
│   ├── backup.sh      # バックアップスクリプト
│   ├── Dockerfile     # バックアップサービス用Docker設定
│   └── Dockerfile.scheduler # スケジューラ用Docker設定
├── couchdb/           # CouchDB関連ファイル
├── docs/              # ドキュメント
├── livesync-proxy/    # プロキシサーバー（Rust）
└── compose.yaml       # Dockerコンポーズ定義
```

## Development

開発環境のセットアップと貢献方法については、[開発者ガイド](docs/developer-guide.md)を参照してください。

## License

MIT License - 詳細は [LICENSE](LICENSE) ファイルを参照してください。

## More Information

詳細な仕様や設定方法については、[仕様書](docs/specification.md)を参照してください。
