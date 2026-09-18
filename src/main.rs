use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

pub trait MetricsCollector {
    fn collect_host_info(&self) -> HostInfo;
    fn collect_metrics(&self) -> SystemMetrics;
    fn build_payload(&self) -> MonitorPayload;
}

pub struct SysinfoCollector;

impl MetricsCollector for SysinfoCollector {
    fn collect_host_info(&self) -> HostInfo {
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
            hostname: System::host_name().unwrap_or_else(|| "unknown-host".to_string()),
            os_type,
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    fn collect_metrics(&self) -> SystemMetrics {
        let mut sys = System::new_all();

        sys.refresh_cpu_usage();
        thread::sleep(Duration::from_millis(200));
        sys.refresh_cpu_usage();

        let cpu_usage_percent = sys.global_cpu_info().cpu_usage();

        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let memory_percent = if total_memory > 0 {
            (used_memory as f32 / total_memory as f32) * 100.0
        } else {
            0.0
        };

        let disks = Disks::new_with_refreshed_list();
        let mut total_disk: u64 = 0;
        let mut used_disk: u64 = 0;
        for disk in &disks {
            let total = disk.total_space();
            let available = disk.available_space();
            total_disk += total;
            used_disk += total.saturating_sub(available);
        }
        let disk_percent = if total_disk > 0 {
            (used_disk as f32 / total_disk as f32) * 100.0
        } else {
            0.0
        };

        SystemMetrics {
            cpu_usage_percent,
            memory: MemoryMetrics {
                total_bytes: total_memory,
                used_bytes: used_memory,
                usage_percent: memory_percent,
            },
            disk: DiskMetrics {
                total_bytes: total_disk,
                used_bytes: used_disk,
                usage_percent: disk_percent,
            },
            uptime_seconds: System::uptime(),
        }
    }

    fn build_payload(&self) -> MonitorPayload {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        MonitorPayload {
            host_info: self.collect_host_info(),
            metrics: self.collect_metrics(),
            timestamp_unix: now,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- 軽量モニターAgent (HTTP POST送信検証) ---");

    let collector = SysinfoCollector;
    let payload = collector.build_payload();

    // 動作確認用の公開テスティングAPI
    let target_url = "https://httpbin.org/post";

    println!("送信先URL: {}", target_url);
    println!("メトリクス送信中...");

    let client = reqwest::Client::new();
    let response = client
        .post(target_url)
        .json(&payload)
        .send()
        .await?;

    println!("レスポンスステータス: {}", response.status());

    let response_text = response.text().await?;
    println!("サーバー返却データ:\n{}", response_text);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1. テスト専用のモックコレクター（実機の環境情報に依存しない）
    pub struct DummyCollector;

    impl MetricsCollector for DummyCollector {
        fn collect_host_info(&self) -> HostInfo {
            HostInfo {
                hostname: "test-host".to_string(),
                os_type: OsType::MacOS,
                agent_version: "0.1.0".to_string(),
            }
        }

        fn collect_metrics(&self) -> SystemMetrics {
            SystemMetrics {
                cpu_usage_percent: 12.5,
                memory: MemoryMetrics {
                    total_bytes: 1000,
                    used_bytes: 500,
                    usage_percent: 50.0,
                },
                disk: DiskMetrics {
                    total_bytes: 2000,
                    used_bytes: 1000,
                    usage_percent: 50.0,
                },
                uptime_seconds: 1234,
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

    // 2. OsType の serde シリアライズ/デシリアライズ検証（lowercase化の動作確認）
    #[test]
    fn test_os_type_serde() {
        let os = OsType::MacOS;
        let json_str = serde_json::to_string(&os).expect("Failed to serialize OsType");
        assert_eq!(json_str, "\"macos\"");

        let deserialized: OsType = serde_json::from_str("\"macos\"").expect("Failed to deserialize OsType");
        assert_eq!(deserialized, OsType::MacOS);
    }

    // 3. DummyCollector を用いた MetricsCollector トレイティの実装検証
    #[test]
    fn test_dummy_collector_payload() {
        let collector = DummyCollector;
        let payload = collector.build_payload();

        assert_eq!(payload.host_info.hostname, "test-host");
        assert_eq!(payload.host_info.os_type, OsType::MacOS);
        assert_eq!(payload.metrics.cpu_usage_percent, 12.5);
        assert_eq!(payload.metrics.memory.used_bytes, 500);
        assert_eq!(payload.timestamp_unix, 1700000000);
    }

    // 4. MonitorPayload 全体が正しい JSON キー構造へ変換されるか検証
    #[test]
    fn test_payload_json_structure() {
        let collector = DummyCollector;
        let payload = collector.build_payload();

        let json_value = serde_json::to_value(&payload).expect("Failed to convert payload to serde_json::Value");

        assert_eq!(json_value["host_info"]["hostname"], "test-host");
        assert_eq!(json_value["host_info"]["os_type"], "macos");
        assert_eq!(json_value["metrics"]["cpu_usage_percent"], 12.5);
        assert_eq!(json_value["metrics"]["memory"]["usage_percent"], 50.0);
    }
}