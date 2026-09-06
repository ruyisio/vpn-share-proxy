use serde::{Deserialize, Serialize};

/// Running state of the VPN Gateway daemon
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunningState {
    Standby,
    WaitingVpn,
    Active,
    Error,
}

/// Overall daemon status payload returned to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub state: RunningState,
    pub tun_interface: Option<String>,
    pub table_id: Option<u32>,
    pub hotspot_interfaces: Vec<String>,
    pub ipv4_active: bool,
    pub ipv6_active: bool,
    pub client_count: usize,
    pub uptime_secs: u64,
    pub version: &'static str,
}
