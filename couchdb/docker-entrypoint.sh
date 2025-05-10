#!/bin/bash
set -e

# システムデータベース初期化スクリプト
initialize_system_databases() {
    echo "システムデータベースの初期化処理を開始します..."
    
    # _users データベースの作成
    echo "システムデータベースの作成: _users"
    curl -s -X PUT http://${COUCHDB_USER}:${COUCHDB_PASSWORD}@127.0.0.1:5984/_users > /dev/null
    
    # _replicator データベースの作成
    echo "システムデータベースの作成: _replicator"
    curl -s -X PUT http://${COUCHDB_USER}:${COUCHDB_PASSWORD}@127.0.0.1:5984/_replicator > /dev/null
    
    # _global_changes データベースの作成
    echo "システムデータベースの作成: _global_changes"
    curl -s -X PUT http://${COUCHDB_USER}:${COUCHDB_PASSWORD}@127.0.0.1:5984/_global_changes > /dev/null
    
    # COUCHDB_DBNAME 環境変数で指定されたデータベースを作成
    # 環境変数が設定されていない場合はデフォルト値 "obsidian" を使用
    DB_NAME=${COUCHDB_DBNAME:-obsidian}
    echo "アプリケーションデータベースの作成: ${DB_NAME}"
    curl -s -X PUT http://${COUCHDB_USER}:${COUCHDB_PASSWORD}@127.0.0.1:5984/${DB_NAME} > /dev/null
    
    echo "システムデータベースの初期化が完了しました"
}

# 初期化フラグファイルのパス
INIT_FLAG_FILE="/opt/couchdb/data/.system_dbs_initialized"

# 初期化済みかどうかチェック
if [ -f "$INIT_FLAG_FILE" ]; then
    echo "システムデータベースは既に初期化済みです"
else
    # 起動完了後に初期化を実行するための関数をバックグラウンドで実行
    (
        # CouchDBの起動を待機
        echo "CouchDBの起動を待機しています..."
        until curl -s http://127.0.0.1:5984/ > /dev/null; do
            sleep 1
        done
        
        # 少し待機してからシステムデータベースを初期化（より確実に起動完了を待つため）
        sleep 5
        
        # システムデータベースを初期化
        initialize_system_databases
        
        # 初期化完了フラグを作成
        touch "$INIT_FLAG_FILE"
    ) &
fi

# 元のCouchDBエントリポイントを実行（実際のCouchDBプロセスを起動）
echo "CouchDBを起動しています..."
exec /docker-entrypoint.sh "$@"

# 管理者ユーザーを使用してシステムDBを作成
# _users データベースの作成
echo "システムデータベースの作成: _users"
curl -s -X PUT http://${COUCHDB_USER}:${COUCHDB_PASSWORD}@127.0.0.1:5984/_users > /dev/null

# _replicator データベースの作成
echo "システムデータベースの作成: _replicator"
curl -s -X PUT http://${COUCHDB_USER}:${COUCHDB_PASSWORD}@127.0.0.1:5984/_replicator > /dev/null

# _global_changes データベースの作成
echo "システムデータベースの作成: _global_changes"
curl -s -X PUT http://${COUCHDB_USER}:${COUCHDB_PASSWORD}@127.0.0.1:5984/_global_changes > /dev/null

echo "システムデータベースの初期化が完了しました"
