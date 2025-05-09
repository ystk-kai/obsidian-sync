const express = require('express');
const http = require('http');
const { createProxyMiddleware } = require('http-proxy-middleware');
const path = require('path');
const fetch = require('node-fetch');

// 簡易メトリクス収集用
const metrics = {
  httpConnections: 0,
  requestDurations: {}, // パスごとのリクエスト時間を記録
  startTime: Date.now(),
  activeRequests: 0
};

// メトリクスミドルウェア
const metricsMiddleware = (req, res, next) => {
  const start = Date.now();
  metrics.activeRequests++; // アクティブリクエスト数をインクリメント
  
  // レスポンス完了時に処理時間を計測
  res.on('finish', () => {
    metrics.activeRequests--; // アクティブリクエスト数をデクリメント
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

// CouchDBの接続状態を管理するオブジェクト
const couchdbStatus = {
  available: false,
  lastChecked: Date.now(),
  errorMessage: 'Not checked yet'
};

// CouchDBの接続状態を確認する関数
const checkCouchDBConnection = async () => {
  try {
    const authHeader = 'Basic ' + Buffer.from(`${couchdbUrlObj.username}:${couchdbUrlObj.password}`).toString('base64');
    const response = await fetch(`${couchdbBaseUrl}/`, {
      headers: {
        'Authorization': authHeader
      }
    });
    
    if (response.ok) {
      couchdbStatus.available = true;
      couchdbStatus.errorMessage = null;
    } else {
      couchdbStatus.available = false;
      couchdbStatus.errorMessage = `HTTP Error: ${response.status} ${response.statusText}`;
    }
  } catch (error) {
    couchdbStatus.available = false;
    couchdbStatus.errorMessage = `接続エラー: ${error.message}`;
    console.error('CouchDB接続確認エラー:', error);
  }
  
  couchdbStatus.lastChecked = Date.now();
  return couchdbStatus.available;
};

// 初期接続確認
checkCouchDBConnection().then(isConnected => {
  console.log(`CouchDB接続状態: ${isConnected ? '接続成功' : '接続失敗'}`);
  if (!isConnected) {
    console.log(`エラー: ${couchdbStatus.errorMessage}`);
  }
});

// 定期的に接続を確認（60秒ごと）
setInterval(checkCouchDBConnection, 60000);

// Expressアプリケーションの作成
const app = express();

// メトリクスミドルウェアを適用
app.use(metricsMiddleware);

// メトリクスエンドポイント
app.get('/metrics', (req, res) => {
  let metricsOutput = '';
  
  // HTTP接続数
  metricsOutput += '# TYPE http_connections_count gauge\n';
  metricsOutput += `http_connections_count{service="livesync_proxy"} ${metrics.activeRequests}\n\n`;
  
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
    status: couchdbStatus.available ? 'ok' : 'degraded',
    uptime_seconds,
    version: '0.1.0',
    services: {
      couchdb: {
        available: couchdbStatus.available,
        last_checked: {
          secs_since_epoch: Math.floor(couchdbStatus.lastChecked / 1000),
          nanos_since_epoch: (couchdbStatus.lastChecked % 1000) * 1000000
        },
        error_message: couchdbStatus.errorMessage
      }
    }
  });
});

// APIステータスエンドポイント
app.get('/api/status', (req, res) => {
  res.json({
    services: {
      couchdb: {
        available: couchdbStatus.available,
        error: couchdbStatus.errorMessage,
        last_checked: Math.floor(couchdbStatus.lastChecked / 1000)
      }
    },
    status: couchdbStatus.available ? 'ok' : 'degraded',
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

// サーバー起動
app.listen(PORT, () => {
  console.log(`LiveSync HTTPプロキシサーバーが起動しました: http://localhost:${PORT}`);
  console.log(`CouchDBへの接続先: ${couchdbBaseUrl}`);
});
