use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;

// ==========================================
// 1. Enum (列挙型) & Struct (構造体) の定義
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OsType {
    MacOS,
    Linux,
    Windows,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub hostname: String,
    pub os_type: OsType,
    pub agent_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f32,
    pub memory: MemoryMetrics,
    pub disk: DiskMetrics,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorPayload {
    pub host_info: HostInfo,
    pub metrics: SystemMetrics,
    pub timestamp_unix: u64,
}

// ==========================================
// 2. Trait & Collector 実装
// ==========================================

pub trait MetricsCollector {
    fn collect_host_info(&self) -> HostInfo;
    fn collect_metrics(&self) -> SystemMetrics;
    fn build_payload(&self) -> MonitorPayload;
}

pub struct DummyCollector;

impl MetricsCollector for DummyCollector {
    fn collect_host_info(&self) -> HostInfo {
        HostInfo {
            hostname: "demo-host".to_string(),
            os_type: OsType::MacOS,
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    fn collect_metrics(&self) -> SystemMetrics {
        SystemMetrics {
            cpu_usage_percent: 15.5,
            memory: MemoryMetrics {
                total_bytes: 16 * 1024 * 1024 * 1024,
                used_bytes: 4 * 1024 * 1024 * 1024,
                usage_percent: 25.0,
            },
            disk: DiskMetrics {
                total_bytes: 500 * 1024 * 1024 * 1024,
                used_bytes: 125 * 1024 * 1024 * 1024,
                usage_percent: 25.0,
            },
            uptime_seconds: 3600,
        }
    }

    fn build_payload(&self) -> MonitorPayload {
        // 現在の UNIX タイムスタンプを動的に取得
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        MonitorPayload {
            host_info: self.collect_host_info(),
            metrics: self.collect_metrics(),
            timestamp_unix: now_unix,
        }
    }
}

// ==========================================
// 3. 非同期メイン関数 & 10秒周期送信ループ
// ==========================================

#[tokio::main]
async fn main() {
    println!("--- 軽量モニターAgent 起動 (10秒周期送信モード) ---");

    let collector = DummyCollector;
    // リクエスト間で接続を再利用するため Client を1つ生成
    let client = reqwest::Client::new();

    // 動作確認用テストエンドポイント (送信されたJSONをそのままレスポンスとして返すテスト用サーバー)
    let target_url = "https://httpbin.org/post";

    // 10秒間隔のタイマーを作成
    let mut ticker = interval(Duration::from_secs(10));

    loop {
        // 10秒の経過を待機
        ticker.tick().await;

        let payload = collector.build_payload();
        println!("[Unix: {}] メトリクスを送信中...", payload.timestamp_unix);

        // HTTP POST リクエスト送信 (JSON自動シリアライズ)
        match client.post(target_url).json(&payload).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    println!(" -> 送信成功: ステータスコード {}", response.status());
                } else {
                    eprintln!(" -> 送信エラー: ステータスコード {}", response.status());
                }
            }
            Err(err) => {
                eprintln!(" -> 通信エラーが発生しました: {}", err);
            }
        }
    }
}
