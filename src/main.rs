use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{Disks, System};
use tokio::time::interval;

// ==========================================
// 1. Enum & Struct 定義
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
// 2. Trait & System Collector 実装
// ==========================================

pub trait MetricsCollector {
    fn collect_host_info(&self) -> HostInfo;
    fn collect_metrics(&self) -> SystemMetrics;
    fn build_payload(&self) -> MonitorPayload;
}

pub struct SystemCollector {
    sys: Arc<Mutex<System>>,
}

impl SystemCollector {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys: Arc::new(Mutex::new(sys)),
        }
    }
}

impl MetricsCollector for SystemCollector {
    fn collect_host_info(&self) -> HostInfo {
        let hostname = System::host_name().unwrap_or_else(|| "unknown-host".to_string());
        let os_name = System::name().unwrap_or_default().to_lowercase();

        let os_type = if os_name.contains("darwin") || os_name.contains("mac") {
            OsType::MacOS
        } else if os_name.contains("linux") {
            OsType::Linux
        } else if os_name.contains("windows") {
            OsType::Windows
        } else {
            OsType::Unknown
        };

        HostInfo {
            hostname,
            os_type,
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    fn collect_metrics(&self) -> SystemMetrics {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_all();

        // CPU使用率 (%) - sysinfo 0.30 用の修正
        let cpu_usage_percent = sys.global_cpu_info().cpu_usage();

        // メモリ情報 (バイト単位)
        let total_mem = sys.total_memory();
        let used_mem = sys.used_memory();
        let mem_percent = if total_mem > 0 {
            (used_mem as f32 / total_mem as f32) * 100.0
        } else {
            0.0
        };

        // ディスク情報 (全マウントポイントの合算)
        let disks = Disks::new_with_refreshed_list();
        let mut total_disk: u64 = 0;
        let mut used_disk: u64 = 0;

        for disk in &disks {
            total_disk += disk.total_space();
            used_disk += disk.total_space() - disk.available_space();
        }

        let disk_percent = if total_disk > 0 {
            (used_disk as f32 / total_disk as f32) * 100.0
        } else {
            0.0
        };

        let uptime_seconds = System::uptime();

        SystemMetrics {
            cpu_usage_percent,
            memory: MemoryMetrics {
                total_bytes: total_mem,
                used_bytes: used_mem,
                usage_percent: mem_percent,
            },
            disk: DiskMetrics {
                total_bytes: total_disk,
                used_bytes: used_disk,
                usage_percent: disk_percent,
            },
            uptime_seconds,
        }
    }

    fn build_payload(&self) -> MonitorPayload {
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
// 3. メイン処理 & 送信ループ
// ==========================================

#[tokio::main]
async fn main() {
    println!("--- 軽量モニターAgent 起動 (実機メトリクス収集モード) ---");

    let collector = SystemCollector::new();
    let client = reqwest::Client::new();
    let target_url = "https://httpbin.org/post";

    let mut ticker = interval(Duration::from_secs(10));

    loop {
        ticker.tick().await;

        let payload = collector.build_payload();
        println!(
            "[Unix: {}] Host: {} ({:?}) | CPU: {:.1}% | Mem: {:.1}% | Disk: {:.1}%",
            payload.timestamp_unix,
            payload.host_info.hostname,
            payload.host_info.os_type,
            payload.metrics.cpu_usage_percent,
            payload.metrics.memory.usage_percent,
            payload.metrics.disk.usage_percent
        );

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

// ==========================================
// 4. ユニットテスト モジュール
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_type_serde() {
        let os = OsType::MacOS;
        let json = serde_json::to_string(&os).unwrap();
        assert_eq!(json, "\"macos\"");

        let deserialized: OsType = serde_json::from_str("\"linux\"").unwrap();
        assert_eq!(deserialized, OsType::Linux);
    }

    #[test]
    fn test_zero_total_memory_handling() {
        let total_mem = 0;
        let used_mem = 0;
        let mem_percent = if total_mem > 0 {
            (used_mem as f32 / total_mem as f32) * 100.0
        } else {
            0.0
        };
        assert_eq!(mem_percent, 0.0);
    }

    struct MockCollector;
    impl MetricsCollector for MockCollector {
        fn collect_host_info(&self) -> HostInfo {
            HostInfo {
                hostname: "test-host".to_string(),
                os_type: OsType::Linux,
                agent_version: "0.1.0".to_string(),
            }
        }
        fn collect_metrics(&self) -> SystemMetrics {
            SystemMetrics {
                cpu_usage_percent: 45.0,
                memory: MemoryMetrics {
                    total_bytes: 100,
                    used_bytes: 50,
                    usage_percent: 50.0,
                },
                disk: DiskMetrics {
                    total_bytes: 200,
                    used_bytes: 100,
                    usage_percent: 50.0,
                },
                uptime_seconds: 3600,
            }
        }
        fn build_payload(&self) -> MonitorPayload {
            MonitorPayload {
                host_info: self.collect_host_info(),
                metrics: self.collect_metrics(),
                timestamp_unix: 1700000000,
            }
        }
    }

    #[test]
    fn test_build_payload_with_mock() {
        let collector = MockCollector;
        let payload = collector.build_payload();

        assert_eq!(payload.host_info.hostname, "test-host");
        assert_eq!(payload.host_info.os_type, OsType::Linux);
        assert_eq!(payload.metrics.cpu_usage_percent, 45.0);
        assert_eq!(payload.timestamp_unix, 1700000000);
    }
}
