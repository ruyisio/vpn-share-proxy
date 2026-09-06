use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::{Ipv4Addr, Ipv6Addr};

/// Proxy filtering mode
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    #[default]
    Global,
    MacAllowlist,
    MacBlocklist,
}

/// DNS redirect strategy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DnsRedirectMode {
    #[default]
    Off,
    Auto,
    Custom,
}

/// Daemon Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default = "default_true")]
    pub enable_ipv4: bool,
    #[serde(default = "default_false")]
    pub enable_ipv6: bool,
    #[serde(default)]
    pub dns_redirect: DnsRedirectMode,
    #[serde(default)]
    pub custom_dns_v4: Option<Ipv4Addr>,
    #[serde(default)]
    pub custom_dns_v6: Option<Ipv6Addr>,
    #[serde(default)]
    pub proxy_mode: ProxyMode,
    #[serde(default)]
    pub mac_allow_list: HashSet<String>,
    #[serde(default)]
    pub mac_block_list: HashSet<String>,
    /// Downstream NICs under management (user-selected). Used only in allow/block modes;
    /// empty means no client iif/MAC rules. Global mode ignores this set.
    #[serde(default)]
    pub managed_ifaces: HashSet<String>,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            enable_ipv4: true,
            enable_ipv6: false,
            dns_redirect: DnsRedirectMode::Off,
            custom_dns_v4: None,
            custom_dns_v6: None,
            proxy_mode: ProxyMode::Global,
            mac_allow_list: HashSet::new(),
            mac_block_list: HashSet::new(),
            managed_ifaces: HashSet::new(),
        }
    }
}

impl GatewayConfig {
    pub fn managed_ifaces_sorted(&self) -> Vec<String> {
        let mut v: Vec<_> = self.managed_ifaces.iter().cloned().collect();
        v.sort();
        v
    }

    /// Compact change-detect fingerprint (sorted inputs → stable u64).
    /// Avoids storing/comparing multi-KB MAC list strings when device lists grow.
    pub fn signature(&self, tun: Option<&str>, table: Option<u32>) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut allow: Vec<_> = self.mac_allow_list.iter().cloned().collect();
        allow.sort();
        let mut block: Vec<_> = self.mac_block_list.iter().cloned().collect();
        block.sort();
        let managed = self.managed_ifaces_sorted();

        let mut h = DefaultHasher::new();
        tun.hash(&mut h);
        table.hash(&mut h);
        self.enable_ipv4.hash(&mut h);
        self.enable_ipv6.hash(&mut h);
        std::mem::discriminant(&self.dns_redirect).hash(&mut h);
        self.custom_dns_v4.hash(&mut h);
        self.custom_dns_v6.hash(&mut h);
        std::mem::discriminant(&self.proxy_mode).hash(&mut h);
        allow.hash(&mut h);
        block.hash(&mut h);
        managed.hash(&mut h);
        h.finish()
    }
}
