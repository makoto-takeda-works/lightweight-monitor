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

        // CPU使用率は測定間隔が必要なため、一度リフレッシュしてわずかに待機
        sys.refresh_cpu_usage();
        thread::sleep(Duration::from_millis(200));
        sys.refresh_cpu_usage();

        // CPU計測
        let cpu_usage_percent = sys.global_cpu_info().cpu_usage();

        // メモリ計測
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let memory_percent = if total_memory > 0 {
            (used_memory as f32 / total_memory as f32) * 100.0
        } else {
            0.0
        };

        // ディスク計測（全マウントポイントの合計を計算）
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

fn main() {
    println!("--- 軽量モニターAgent (sysinfo実機データ収集) ---");

    let collector = SysinfoCollector;
    let payload = collector.build_payload();

    match serde_json::to_string_pretty(&payload) {
        Ok(json_str) => {
            println!("取得した実機メトリクスJSON:\n{}", json_str);
        }
        Err(e) => {
            eprintln!("JSON変換エラー: {}", e);
        }
    }
}
