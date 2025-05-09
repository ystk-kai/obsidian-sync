#!/bin/bash
set -e

# 環境変数の設定
: ${BACKUP_GIT_REPO:?"BACKUP_GIT_REPO must be set"}
: ${BACKUP_GIT_TOKEN:?"BACKUP_GIT_TOKEN must be set"}
: ${BACKUP_GIT_BRANCH:=main}
: ${BACKUP_COMMIT_MSG_PREFIX:="[Backup]"}

# タイムゾーン設定の確認
echo "Current timezone: $(cat /etc/timezone)"
export TZ=Asia/Tokyo
echo "Using timezone: $TZ"

# タイムスタンプの生成（JST）
TIMESTAMP=$(TZ=Asia/Tokyo date +"%Y%m%d-%H%M%S")
echo "Generated timestamp: ${TIMESTAMP}"
BACKUP_DIR="/backup"
TEMP_DIR="/tmp/backup-${TIMESTAMP}"
GIT_REPO_DIR="/tmp/git-repo"

echo "=== Starting backup process at $(date) ==="
echo "Environment variables:"
echo "BACKUP_DIR=${BACKUP_DIR}"
echo "TEMP_DIR=${TEMP_DIR}"
echo "GIT_REPO_DIR=${GIT_REPO_DIR}"

# 一時ディレクトリの作成
mkdir -p ${TEMP_DIR}
mkdir -p ${GIT_REPO_DIR}
mkdir -p ${BACKUP_DIR}

# バックアップディレクトリの権限確認
echo "Checking backup directory permissions:"
ls -ld ${BACKUP_DIR}

# CouchDBデータのバックアップ
echo "Backing up CouchDB data..."
BACKUP_FILENAME="couchdb_data_${TIMESTAMP}.tar.gz"
echo "Backup filename: ${BACKUP_FILENAME}"
tar -czf ${TEMP_DIR}/${BACKUP_FILENAME} -C /data .
echo "CouchDB data backup completed. Size: $(du -h ${TEMP_DIR}/${BACKUP_FILENAME} | cut -f1)"

# Gitリポジトリのクローン
echo "Cloning Git repository..."
REPO_URL=$(echo ${BACKUP_GIT_REPO} | sed "s|https://|https://${BACKUP_GIT_TOKEN}@|")

# Gitリポジトリのクローン（新規作成または既存リポジトリの更新）
if git clone --depth 1 --branch ${BACKUP_GIT_BRANCH} --single-branch ${REPO_URL} ${GIT_REPO_DIR} 2>/dev/null; then
    echo "Repository cloned successfully."
else
    echo "Repository doesn't exist or branch not found. Creating new repository..."
    mkdir -p ${GIT_REPO_DIR}
    cd ${GIT_REPO_DIR}
    git init
    git checkout -b ${BACKUP_GIT_BRANCH}
    git config --local user.email "backup@obsidian-sync.local"
    git config --local user.name "Obsidian Sync Backup"
    echo "# Obsidian Sync Backups" > README.md
    git add README.md
    git commit -m "Initial commit"
    git remote add origin ${REPO_URL}
fi

# バックアップファイルの移動
echo "Moving backup files to the repository..."
cd ${GIT_REPO_DIR}
git config --local user.email "backup@obsidian-sync.local"
git config --local user.name "Obsidian Sync Backup"

# バックアップディレクトリを作成
mkdir -p backups
echo "Copying ${TEMP_DIR}/${BACKUP_FILENAME} to Git repository..."
cp -v ${TEMP_DIR}/${BACKUP_FILENAME} backups/

# All backups are preserved in Git history, no need to manage backup rotation

# 追加したファイルをコミット
echo "Committing changes..."
git add backups/
git status
if git commit -m "${BACKUP_COMMIT_MSG_PREFIX} Backup ${TIMESTAMP}"; then
    echo "Changes committed successfully."
else
    echo "No changes to commit or commit failed."
fi

# リモートリポジトリにプッシュ
echo "Pushing to remote repository..."
if git push --set-upstream origin ${BACKUP_GIT_BRANCH}; then
    echo "Backup pushed to remote repository successfully."
else
    echo "Failed to push to remote repository. Will try again next time."
fi

# バックアップデータを永続的なボリュームにコピー
echo "Copying backup to persistent volume..."
echo "Source: ${TEMP_DIR}/${BACKUP_FILENAME}"
echo "Destination: ${BACKUP_DIR}/${BACKUP_FILENAME}"
cp -v ${TEMP_DIR}/${BACKUP_FILENAME} ${BACKUP_DIR}/

# ローカルバックアップの検証
echo "Verifying local backups..."
ls -la ${BACKUP_DIR}/
echo "Total backup files: $(ls -1 ${BACKUP_DIR}/*.tar.gz 2>/dev/null | wc -l || echo "0")"

# 一時ディレクトリの削除
echo "Cleaning up temporary directories..."
rm -rf ${TEMP_DIR}
rm -rf ${GIT_REPO_DIR}

echo "=== Backup process completed at $(date) ==="
