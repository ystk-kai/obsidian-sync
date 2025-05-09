FROM alpine:3.18

# 必要なパッケージのインストール
RUN apk add --no-cache \
    bash \
    git \
    curl \
    tar \
    gzip \
    ca-certificates \
    tzdata \
    jq

# タイムゾーンを JST に設定
ENV TZ=Asia/Tokyo
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

# 作業ディレクトリの設定
WORKDIR /app

# バックアップスクリプトのコピー
COPY backup.sh /app/backup.sh
RUN chmod +x /app/backup.sh

# ボリュームの設定
VOLUME /backup
VOLUME /data

# エントリーポイントの設定
ENTRYPOINT ["/app/backup.sh"]
