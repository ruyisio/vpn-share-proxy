use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// One NIC for App interface picker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetIfaceInfo {
    pub name: String,
    pub operstate: String,
    /// Heuristic hint only — may be wrong across ROMs.
    pub recommended: bool,
    pub hint: String,
    pub managed: bool,
}

/// Hotspot connected client information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedDevice {
    pub ip: IpAddr,
    pub mac: String,
    pub iface: String,
    pub hostname: Option<String>,
    pub policy: String,
    pub first_seen: u64,
    pub last_seen: u64,
}
