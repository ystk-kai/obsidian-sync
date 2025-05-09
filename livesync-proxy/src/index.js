const express = require('express');
const http = require('http');
const WebSocket = require('ws');
const { createProxyMiddleware } = require('http-proxy-middleware');
const path = require('path');

// 簡易メトリクス収集用
const metrics = {
  wsConnections: 0,
  requestDurations: {}, // パスごとのリクエスト時間を記録
  startTime: Date.now()
};

// メトリクスミドルウェア
const metricsMiddleware = (req, res, next) => {
  const start = Date.now();
  
  // レスポンス完了時に処理時間を計測
  res.on('finish', () => {
    const duration = Date.now() - start;
    const path = req.path || '/';
    const method = req.method || 'GET';
    const key = `${method}:${path}`;
    
    if (!metrics.requestDurations[key]) {
      metrics.requestDurations[key] = [];
    }
    
    // 最新の10件のみ保持
    metrics.requestDurations[key].push(duration);
    if (metrics.requestDurations[key].length > 10) {
      metrics.requestDurations[key].shift();
    }
  });
  
  next();
};

// 環境変数からCouchDBのURLを取得
const COUCHDB_URL = process.env.COUCHDB_URL || 'http://admin:secret@couchdb:5984';
const PORT = process.env.PORT || 3000;

// CouchDBの基本URLを抽出（認証情報なし）
const couchdbUrlObj = new URL(COUCHDB_URL);
const couchdbBaseUrl = `${couchdbUrlObj.protocol}//${couchdbUrlObj.hostname}:${couchdbUrlObj.port}`;

// Expressアプリケーションの作成
const app = express();

// メトリクスミドルウェアを適用
app.use(metricsMiddleware);

// メトリクスエンドポイント
app.get('/metrics', (req, res) => {
  let metricsOutput = '';
  
  // WebSocket接続数
  metricsOutput += '# TYPE websocket_connections_count gauge\n';
  metricsOutput += `websocket_connections_count{service="livesync_proxy"} ${metrics.wsConnections}\n\n`;
  
  // HTTPリクエスト時間
  metricsOutput += '# TYPE http_request_duration_seconds histogram\n';
  Object.entries(metrics.requestDurations).forEach(([key, durations]) => {
    const [method, path] = key.split(':');
    if (durations.length > 0) {
      // 平均値を計算
      const sum = durations.reduce((acc, val) => acc + val, 0);
      const avg = sum / durations.length / 1000; // ミリ秒から秒に変換
      
      // ヒストグラムのバケットを模擬
      const buckets = [0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1, 5, 10];
      buckets.forEach(le => {
        const count = durations.filter(d => d / 1000 <= le).length;
        metricsOutput += `http_request_duration_seconds_bucket{service="livesync_proxy",path="${path}",method="${method}",le="${le}"} ${count}\n`;
      });
      metricsOutput += `http_request_duration_seconds_bucket{service="livesync_proxy",path="${path}",method="${method}",le="+Inf"} ${durations.length}\n`;
      metricsOutput += `http_request_duration_seconds_sum{service="livesync_proxy",path="${path}",method="${method}"} ${avg * durations.length}\n`;
      metricsOutput += `http_request_duration_seconds_count{service="livesync_proxy",path="${path}",method="${method}"} ${durations.length}\n`;
    }
  });
  
  res.setHeader('Content-Type', 'text/plain');
  res.send(metricsOutput);
});

// ヘルスチェックエンドポイント
app.get('/health', (req, res) => {
  const uptime_seconds = Math.floor((Date.now() - metrics.startTime) / 1000);
  res.json({
    status: 'ok',
    uptime_seconds,
    version: '0.1.0',
    services: {
      couchdb: {
        available: true, // 実際の状態確認ロジックを実装
        last_checked: {
          secs_since_epoch: Math.floor(Date.now() / 1000),
          nanos_since_epoch: (Date.now() % 1000) * 1000000
        },
        error_message: null
      }
    }
  });
});

// APIステータスエンドポイント
app.get('/api/status', (req, res) => {
  res.json({
    services: {
      couchdb: {
        available: true, // 実際の状態確認ロジックを実装
        error: null,
        last_checked: Math.floor(Date.now() / 1000)
      }
    },
    status: 'ok',
    version: '0.1.0'
  });
});

// CouchDBへのHTTPプロキシミドルウェアを設定
app.use('/db', createProxyMiddleware({
  target: couchdbBaseUrl,
  changeOrigin: true,
  pathRewrite: {
    '^/db': ''
  },
  onProxyReq: (proxyReq, req, res) => {
    // CouchDBへの認証情報を追加
    proxyReq.setHeader('Authorization', 'Basic ' + 
      Buffer.from(`${couchdbUrlObj.username}:${couchdbUrlObj.password}`).toString('base64'));
  }
}));

// 静的ファイルのホスティング
app.use(express.static('static'));

// HTTPサーバーの作成
const server = http.createServer(app);

// WebSocketサーバーの作成
const wss = new WebSocket.Server({ server });

// WebSocket接続のハンドリング
wss.on('connection', (ws) => {
  console.log('クライアント接続を確立');
  metrics.wsConnections++; // 接続数をインクリメント

  // メッセージの受信ハンドラ
  ws.on('message', (message) => {
    console.log('受信メッセージ: %s', message);

    // 必要に応じてCouchDBにリクエストを転送
    // ここにLiveSyncの処理ロジックを実装
  });

  // 接続終了ハンドラ
  ws.on('close', () => {
    console.log('クライアント接続が閉じられました');
    metrics.wsConnections--; // 接続数をデクリメント
  });

  // 初期接続確認メッセージ送信
  ws.send(JSON.stringify({
    type: 'connection_established',
    message: 'LiveSync Proxyに接続されました'
  }));
});

// サーバー起動
server.listen(PORT, () => {
  console.log(`LiveSync Proxyサーバーが起動しました: http://localhost:${PORT}`);
  console.log(`CouchDBへの接続先: ${couchdbBaseUrl}`);
});
