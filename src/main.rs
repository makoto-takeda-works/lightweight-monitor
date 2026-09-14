use serde::{Deserialize, Serialize};

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
        MonitorPayload {
            host_info: self.collect_host_info(),
            metrics: self.collect_metrics(),
            timestamp_unix: 1789311600,
        }
    }
}

fn main() {
    println!("--- 軽量モニターAgent 初期化 ---");

    let collector = DummyCollector;
    let payload = collector.build_payload();

    match serde_json::to_string_pretty(&payload) {
        Ok(json_str) => {
            println!("生成されたJSONデータ:\n{}", json_str);
        }
        Err(e) => {
            eprintln!("JSON変換エラー: {}", e);
        }
    }
}
