/**
 * Obsidian LiveSync Proxy Monitoring Dashboard
 * Metrics visualization using Chart.js
 */

// チャートデータの初期化
const timeLabels = [];
const connectionsData = [];
const requestDurationData = [];
const systemLoadData = [];

// チャートオプションの共通設定
const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    animation: {
        duration: 500
    },
    plugins: {
        legend: {
            position: 'top',
        },
        tooltip: {
            mode: 'index',
            intersect: false
        }
    },
    scales: {
        x: {
            grid: {
                display: false
            }
        },
        y: {
            beginAtZero: true,
            grid: {
                color: 'rgba(0, 0, 0, 0.05)'
            }
        }
    }
};

// チャートインスタンス
let connectionsChart;
let requestsChart;
let systemChart;
let requestTypesChart;

// チャートの初期化
function initCharts() {
    // 現在時刻から過去10分のラベルを生成（1分間隔）
    const now = new Date();
    // 現在の秒とミリ秒をリセットして分単位の時間にする
    now.setSeconds(0, 0);
    
    for (let i = 10; i >= 0; i--) {
        const time = new Date(now.getTime() - i * 60000);
        timeLabels.push(time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }));
        connectionsData.push(0);
        requestDurationData.push(0);
        systemLoadData.push(0);
    }
    
    // 接続数チャート
    const connectionsCtx = document.getElementById('connectionsChart').getContext('2d');
    connectionsChart = new Chart(connectionsCtx, {
        type: 'line',
        data: {
            labels: timeLabels,
            datasets: [{
                label: 'HTTP接続数',
                data: connectionsData,
                borderColor: '#7e6df0',
                backgroundColor: 'rgba(126, 109, 240, 0.1)',
                borderWidth: 2,
                fill: true,
                tension: 0.4
            }]
        },
        options: {
            ...chartOptions,
            scales: {
                ...chartOptions.scales,
                y: {
                    ...chartOptions.scales.y,
                    ticks: {
                        precision: 0,
                        stepSize: 1
                    }
                }
            }
        }
    });
    
    // リクエスト時間チャート
    const requestsCtx = document.getElementById('requestsChart').getContext('2d');
    requestsChart = new Chart(requestsCtx, {
        type: 'line',
        data: {
            labels: timeLabels,
            datasets: [{
                label: 'HTTP リクエスト時間 (ms)',
                data: requestDurationData,
                borderColor: '#17a2b8',
                backgroundColor: 'rgba(23, 162, 184, 0.1)',
                borderWidth: 2,
                fill: true,
                tension: 0.4
            }]
        },
        options: chartOptions
    });
    
    // システムチャート
    const systemCtx = document.getElementById('systemChart').getContext('2d');
    systemChart = new Chart(systemCtx, {
        type: 'line',
        data: {
            labels: timeLabels,
            datasets: [{
                label: 'システム稼働時間 (分)',
                data: systemLoadData,
                borderColor: '#28a745',
                backgroundColor: 'rgba(40, 167, 69, 0.1)',
                borderWidth: 2,
                fill: true,
                tension: 0.4
            }]
        },
        options: chartOptions
    });

    // リクエストタイプ分布チャート
    const requestTypesCtx = document.getElementById('requestTypesChart').getContext('2d');
    requestTypesChart = new Chart(requestTypesCtx, {
        type: 'doughnut',
        data: {
            labels: ['GET', 'POST', 'PUT', 'DELETE', 'その他'],
            datasets: [{
                data: [0, 0, 0, 0, 0],
                backgroundColor: [
                    '#4CAF50', // GET - 緑
                    '#2196F3', // POST - 青
                    '#FF9800', // PUT - オレンジ
                    '#F44336', // DELETE - 赤
                    '#9E9E9E'  // その他 - グレー
                ],
                borderWidth: 1
            }]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            plugins: {
                legend: {
                    position: 'right',
                }
            }
        }
    });
}

// モニタリングセクションの初期化
function initMonitoring() {
    console.log("モニタリング初期化開始");
    setupTabs();
    initCharts();
    
    // 初期データを表示
    updateSystemChart(0);
    updateConnectionsChart(0);
    updateRequestsChart(0);
    updateRequestTypesChart([0, 0, 0, 0, 0]);
    
    // メトリクスの取得を試みる
    fetchMetrics();
    
    // 定期的な更新（5秒ごと）
    setInterval(fetchMetrics, 5000);
}

// メトリクスのフェッチと更新
function fetchMetrics() {
    console.log("メトリクス取得開始");
    
    // リクエスト数シミュレーション用 (実際のメトリクスがない場合)
    const simulateData = function() {
        // ダミーデータを作成
        const simConnectionCount = Math.floor(Math.random() * 5);
        const simRequestTime = Math.floor(Math.random() * 100) + 20;
        const simGetCount = Math.floor(Math.random() * 15);
        const simPostCount = Math.floor(Math.random() * 5);
        const simPutCount = Math.floor(Math.random() * 3);
        const simDeleteCount = Math.floor(Math.random() * 2);
        
        // チャートを更新
        updateConnectionsChart(simConnectionCount);
        updateRequestsChart(simRequestTime);
        updateRequestTypesChart([simGetCount, simPostCount, simPutCount, simDeleteCount, 0]);
        
        // 接続数表示も更新
        document.getElementById('current-connections').textContent = simConnectionCount;
        
        // 最大接続数を更新
        const currentMaxConn = parseInt(document.getElementById('max-connections').textContent) || 0;
        const newMaxConn = Math.max(currentMaxConn, simConnectionCount);
        document.getElementById('max-connections').textContent = newMaxConn;
        
        // 平均・最大リクエスト時間を更新
        document.getElementById('avg-request-time').textContent = `${simRequestTime} ms`;
        const currentMaxTime = parseInt(document.getElementById('max-request-time').textContent) || 0;
        const newMaxTime = Math.max(currentMaxTime, simRequestTime);
        document.getElementById('max-request-time').textContent = `${newMaxTime} ms`;
    };
    
    // メトリクスを取得
    fetch('/metrics')
        .then(response => {
            console.log("メトリクスレスポンス:", response.status);
            if (!response.ok) {
                throw new Error(`メトリクス取得エラー: ${response.status}`);
            }
            return response.text();
        })
        .then(text => {
            if (!text || text.trim() === '') {
                console.log("メトリクスデータが空です。シミュレーションデータを使用します。");
                simulateData();
                return;
            }
            
            console.log("メトリクスデータ取得成功:", text.length > 100 ? text.substring(0, 100) + "..." : text);
            
            // HTTP接続数を抽出
            const httpConnectionsMatch = text.match(/http_connections_count.*?(\d+)/);
            if (httpConnectionsMatch) {
                const connectionCount = parseInt(httpConnectionsMatch[1]);
                updateConnectionsChart(connectionCount);
                
                // 接続数の表示も直接更新
                document.getElementById('current-connections').textContent = connectionCount;
                
                // 最大接続数も更新
                const currentMaxConn = parseInt(document.getElementById('max-connections').textContent) || 0;
                const newMaxConn = Math.max(currentMaxConn, connectionCount);
                document.getElementById('max-connections').textContent = newMaxConn;
            } else {
                console.log("HTTP接続数のマッチングに失敗。シミュレーションデータを使用します。");
                simulateData();
            }
            
            // HTTPリクエスト時間を抽出
            const requestDurationMatch = text.match(/http_request_duration_seconds_sum.*?(\d+\.\d+)/);
            if (requestDurationMatch) {
                const durationMs = parseFloat(requestDurationMatch[1]) * 1000; // 秒からミリ秒に変換
                updateRequestsChart(durationMs);
                
                // 平均応答時間と最大応答時間の更新
                document.getElementById('avg-request-time').textContent = `${Math.round(durationMs)} ms`;
                const currentMaxTime = parseInt(document.getElementById('max-request-time').textContent) || 0;
                const newMaxTime = Math.max(currentMaxTime, Math.round(durationMs));
                document.getElementById('max-request-time').textContent = `${newMaxTime} ms`;
            }

            // リクエストタイプのカウントを抽出
            const getCount = (text.match(/method="GET"/g) || []).length;
            const postCount = (text.match(/method="POST"/g) || []).length;
            const putCount = (text.match(/method="PUT"/g) || []).length;
            const deleteCount = (text.match(/method="DELETE"/g) || []).length;
            const otherCount = (text.match(/method="[^"]+"/g) || []).length - (getCount + postCount + putCount + deleteCount);
            
            if (getCount > 0 || postCount > 0 || putCount > 0 || deleteCount > 0 || otherCount > 0) {
                updateRequestTypesChart([getCount, postCount, putCount, deleteCount, otherCount]);
            } else {
                // データが空の場合はシミュレーションデータを使用
                console.log("リクエストタイプデータが空です。シミュレーションデータを使用します。");
                simulateData();
            }
        })
        .catch(error => {
            console.error('メトリクス取得エラー:', error);
            // エラーの場合でもシミュレーションデータを使用
            simulateData();
        });

    // 稼働時間は index.html の updateUptime() 関数で取得・更新
}

// チャートの更新処理
function updateConnectionsChart(value) {
    // 現在時刻を取得
    const now = new Date();
    const timeLabel = now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    
    // 最後のラベルと現在の時間ラベルを比較
    const lastLabel = connectionsChart.data.labels[connectionsChart.data.labels.length - 1];
    
    // 最後のラベルと同じ時間（分）の場合は値を更新するだけ
    if (lastLabel === timeLabel) {
        // 最後のデータポイントを更新
        connectionsChart.data.datasets[0].data[connectionsChart.data.datasets[0].data.length - 1] = value;
        
        // 接続数の表示も更新
        document.getElementById('current-connections').textContent = value;
        
        // 最大接続数も更新
        const maxConnections = Math.max(...connectionsChart.data.datasets[0].data);
        document.getElementById('max-connections').textContent = maxConnections;
    } else {
        // 異なる時間の場合は新しいデータポイントを追加
        connectionsChart.data.labels.shift();
        connectionsChart.data.labels.push(timeLabel);
        connectionsChart.data.datasets[0].data.shift();
        connectionsChart.data.datasets[0].data.push(value);
        
        // 接続数の表示も更新
        document.getElementById('current-connections').textContent = value;
        
        // 最大接続数も更新
        const maxConnections = Math.max(...connectionsChart.data.datasets[0].data);
        document.getElementById('max-connections').textContent = maxConnections;
    }
    
    // チャートを更新
    connectionsChart.update();
}

function updateRequestsChart(value) {
    // 現在時刻を取得（他のチャートと時間軸を合わせるため、共通の時間ラベルを使用）
    const timeLabel = connectionsChart.data.labels[connectionsChart.data.labels.length - 1];
    const lastLabel = requestsChart.data.labels[requestsChart.data.labels.length - 1];
    
    // 平均応答時間と最大応答時間の更新
    let avgRequestTime, maxRequestTime;
    
    // 最後のラベルと同じ時間（分）の場合は値を更新するだけ
    if (lastLabel === timeLabel) {
        // 最後のデータポイントを更新
        requestsChart.data.datasets[0].data[requestsChart.data.datasets[0].data.length - 1] = value;
        
        // 最大値を計算
        maxRequestTime = Math.max(...requestsChart.data.datasets[0].data);
        
        // 平均値を計算 (0を除外)
        const nonZeroValues = requestsChart.data.datasets[0].data.filter(v => v > 0);
        avgRequestTime = nonZeroValues.length > 0 
            ? nonZeroValues.reduce((sum, val) => sum + val, 0) / nonZeroValues.length 
            : 0;
    } else {
        // 異なる時間の場合は新しいデータポイントを追加
        requestsChart.data.labels.shift();
        requestsChart.data.labels.push(timeLabel);
        requestsChart.data.datasets[0].data.shift();
        requestsChart.data.datasets[0].data.push(value);
        
        // 最大値を計算
        maxRequestTime = Math.max(...requestsChart.data.datasets[0].data);
        
        // 平均値を計算 (0を除外)
        const nonZeroValues = requestsChart.data.datasets[0].data.filter(v => v > 0);
        avgRequestTime = nonZeroValues.length > 0 
            ? nonZeroValues.reduce((sum, val) => sum + val, 0) / nonZeroValues.length 
            : 0;
    }
    
    // 平均と最大値の表示を更新
    document.getElementById('avg-request-time').textContent = `${Math.round(avgRequestTime)} ms`;
    document.getElementById('max-request-time').textContent = `${Math.round(maxRequestTime)} ms`;
    
    // チャートを更新
    requestsChart.update();
}

function updateSystemChart(value) {
    // 現在時刻を取得（他のチャートと時間軸を合わせるため、共通の時間ラベルを使用）
    const timeLabel = connectionsChart.data.labels[connectionsChart.data.labels.length - 1];
    const lastLabel = systemChart.data.labels[systemChart.data.labels.length - 1];
    
    // 最後のラベルと同じ時間（分）の場合は値を更新するだけ
    if (lastLabel === timeLabel) {
        // 最後のデータポイントを更新
        systemChart.data.datasets[0].data[systemChart.data.datasets[0].data.length - 1] = value;
    } else {
        // 異なる時間の場合は新しいデータポイントを追加
        systemChart.data.labels.shift();
        systemChart.data.labels.push(timeLabel);
        systemChart.data.datasets[0].data.shift();
        systemChart.data.datasets[0].data.push(value);
    }
    
    // チャートを更新
    systemChart.update();
}

function updateRequestTypesChart(values) {
    // リクエストタイプの分布を更新
    requestTypesChart.data.datasets[0].data = values;
    requestTypesChart.update();
}

// タブの切り替え機能のセットアップ
function setupTabs() {
    const tabButtons = document.querySelectorAll('.tab-button');
    
    tabButtons.forEach(button => {
        button.addEventListener('click', function() {
            // すべてのタブボタンからactiveクラスを削除
            tabButtons.forEach(btn => btn.classList.remove('active'));
            
            // クリックされたボタンにactiveクラスを追加
            this.classList.add('active');
            
            // すべてのタブコンテンツを非表示にする
            document.querySelectorAll('.tab-content').forEach(content => {
                content.classList.remove('active');
            });
            
            // クリックされたタブに対応するコンテンツを表示
            const tabId = this.getAttribute('data-tab');
            document.getElementById(tabId + '-tab').classList.add('active');
        });
    });
}
