#!/bin/bash
set -e

# タイムゾーン情報を表示
echo "Current timezone setting: $(cat /etc/timezone)"
date
echo "TZ environment variable: $TZ"

# .envファイルを読み込む（存在する場合）
if [ -f "/.env" ]; then
    echo "Loading environment variables from /.env"
    # コメントを除外し、行末のコメントもフィルタリングして環境変数のみを読み込む
    while IFS= read -r line; do
        # 行頭がコメントでなく、かつ変数の割り当てを含む行のみ処理
        if [[ ! $line =~ ^#.*$ ]] && [[ $line == *=* ]]; then
            # 行から変数名と値を抽出（#以降のコメントを除去）
            var_name=$(echo "$line" | cut -d= -f1)
            # 変数名がアルファベットまたはアンダースコアで始まり、アルファベット、数字、アンダースコアのみを含む場合のみエクスポート
            if [[ $var_name =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]]; then
                var_value=$(echo "$line" | cut -d= -f2- | sed 's/\s*#.*$//')
                # 変数をエクスポート
                export "$var_name=$var_value"
                echo "Exported: $var_name"
            fi
        fi
    done < /.env
fi

# 環境変数の確認と表示
echo "Starting backup scheduler with the following configuration:"
echo "BACKUP_SCHEDULE: ${BACKUP_SCHEDULE}"
echo "RUN_ON_STARTUP: ${RUN_ON_STARTUP:-false}"
echo "COMPOSE_PROJECT_NAME: ${COMPOSE_PROJECT_NAME:-obsidian-sync}"

# プロジェクト名の設定（環境変数から取得、デフォルトはobsidian-sync）
COMPOSE_PROJECT_NAME=${COMPOSE_PROJECT_NAME:-obsidian-sync}

# Docker関連ファイルの存在確認
echo "Checking for Docker and Docker Compose configuration files:"
ls -la / | grep -E "compose|.env" || echo "No compose files found in root"
ls -la /app | grep -E "compose|.env" || echo "No compose files found in /app"

# Docker Compose 実行コマンドの準備
if command -v docker-compose &> /dev/null; then
    COMPOSE_CMD="docker-compose"
else
    COMPOSE_CMD="docker compose"
fi
echo "Using Docker Compose command: $COMPOSE_CMD"

# compose.yamlファイルの検索
COMPOSE_FILE=""
for path in "/compose.yaml" "/app/compose.yaml" "$(pwd)/compose.yaml" "/usr/share/compose.yaml"; do
    if [ -f "$path" ]; then
        COMPOSE_FILE="$path"
        echo "Found compose file at $COMPOSE_FILE"
        break
    fi
done

if [ -z "$COMPOSE_FILE" ]; then
    echo "Error: No compose.yaml file found. Using default location."
    COMPOSE_FILE="/compose.yaml"
fi

# バックアップコマンドの構築
DOCKER_COMPOSE_CMD="$COMPOSE_CMD -p ${COMPOSE_PROJECT_NAME} -f ${COMPOSE_FILE} run --rm backup"
echo "Configured Docker Compose command: $DOCKER_COMPOSE_CMD"

# テストコマンドの実行（Docker Sock の接続確認）
echo "Testing Docker connection..."
if docker info &> /dev/null; then
    echo "Docker connection successful"
else
    echo "Warning: Cannot connect to Docker. Make sure the socket is properly mounted."
fi

# cronタブに追加
CRON_ENV="COMPOSE_PROJECT_NAME=${COMPOSE_PROJECT_NAME} TZ=Asia/Tokyo"
CRON_CMD="cd / && ${CRON_ENV} ${DOCKER_COMPOSE_CMD} >> /var/log/cron.log 2>&1"
echo "${BACKUP_SCHEDULE} ${CRON_CMD}" > /etc/crontabs/root
echo "Backup scheduled with cron: ${BACKUP_SCHEDULE} (JST timezone)"
echo "Backup command: ${CRON_CMD}"

# ログディレクトリの設定
mkdir -p /var/log
touch /var/log/cron.log
echo "$(date): Starting backup scheduler" >> /var/log/cron.log

# 初期実行（オプション）
if [ "${RUN_ON_STARTUP}" = "true" ]; then
    echo "Running initial backup..."
    echo "$(date): Running initial backup" >> /var/log/cron.log
    eval "${DOCKER_COMPOSE_CMD}" >> /var/log/cron.log 2>&1
    echo "Initial backup completed with status: $?"
fi

# ログ出力を確認できるようにする
echo "Starting cron daemon and tailing logs..."
mkdir -p /var/log
touch /var/log/cron.log
tail -f /var/log/cron.log &
TAIL_PID=$!

# シグナルハンドラの設定
trap "echo 'Shutting down...'; kill $TAIL_PID; exit 0" SIGINT SIGTERM

# cronデーモンの起動（TZ環境変数を設定）
TZ=Asia/Tokyo crond -f -d 8 &
CRON_PID=$!

# 両方のプロセスが終了するまで待機
wait $CRON_PID
