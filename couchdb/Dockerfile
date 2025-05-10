FROM couchdb:latest

# ヘルスチェック用のツールをインストール
RUN apt-get update && apt-get install -y curl && apt-get clean

# 初期化スクリプトを追加
COPY docker-entrypoint.sh /usr/local/bin/custom-entrypoint.sh
RUN chmod +x /usr/local/bin/custom-entrypoint.sh

# エントリポイントを設定
ENTRYPOINT ["/usr/local/bin/custom-entrypoint.sh"]
CMD ["couchdb"]
