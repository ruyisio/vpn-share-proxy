mod common;
mod v4;
mod v6;

use anyhow::Result;
use std::fs;
use std::path::Path;
use tracing::{info, warn};

use common::{find_nf_helper, pin_family, probe_iptables_sockopt, teardown_family};

pub struct FirewallManager {
    pub(crate) kernel_iptables: bool,
    pub(crate) kernel_ip6tables: bool,
    pub(crate) ipt_bin: Option<String>,
    pub(crate) ip6t_bin: Option<String>,
}

impl FirewallManager {
    pub fn new() -> Result<Self> {
        let proc_v4 = Path::new("/proc/net/ip_tables_names").exists();
        let proc_v6 = Path::new("/proc/net/ip6_tables_names").exists();
        let sock_v4 = probe_iptables_sockopt(false);
        let sock_v6 = probe_iptables_sockopt(true);
        let kernel_iptables = sock_v4 || proc_v4;
        let kernel_ip6tables = sock_v6 || proc_v6;

        let ipt_bin = find_nf_helper("iptables");
        let ip6t_bin = find_nf_helper("ip6tables");

        if kernel_iptables {
            info!(
                "Firewall: kernel ip_tables ok (sockopt={}, proc={})",
                sock_v4, proc_v4
            );
        } else {
            warn!("Firewall: kernel has no legacy ip_tables");
        }
        match &ipt_bin {
            Some(p) => info!("Firewall: using helper {}", p),
            None if kernel_iptables => warn!(
                "Firewall: kernel supports ip_tables but no iptables helper found; \
                 filter/mangle/nat rules cannot be programmed (need helper or sockopt backend)"
            ),
            None => {}
        }

        Ok(Self {
            kernel_iptables,
            kernel_ip6tables,
            ipt_bin,
            ip6t_bin,
        })
    }

    pub fn setup_kernel_forwarding(&self, v4: bool, v6: bool) {
        if v4 {
            let _ = fs::write("/proc/sys/net/ipv4/ip_forward", "1");
            if let Ok(entries) = fs::read_dir("/proc/sys/net/ipv4/conf") {
                for entry in entries.flatten() {
                    let rp = entry.path().join("rp_filter");
                    if rp.exists() {
                        let _ = fs::write(rp, "0");
                    }
                }
            }
        }
        if v6 {
            let _ = fs::write("/proc/sys/net/ipv6/conf/all/forwarding", "1");
        }
    }

    pub fn ensure_hooks_pinned(&self) {
        if let Some(ref ipt) = self.ipt_bin {
            if self.kernel_iptables {
                pin_family(ipt);
            }
        }
        if let Some(ref ip6t) = self.ip6t_bin {
            if self.kernel_ip6tables {
                pin_family(ip6t);
            }
        }
    }

    pub fn apply_proxy_rules(&self, config: &crate::types::GatewayConfig, tun: &str) {
        v4::apply(self, config, tun);
    }

    pub fn apply_ipv6_proxy_rules(
        &self,
        config: &crate::types::GatewayConfig,
        tun: &str,
        enabled: bool,
    ) {
        v6::apply(self, config, tun, enabled);
    }

    pub fn cleanup(&self) {
        if let Some(ref ipt) = self.ipt_bin {
            teardown_family(ipt);
        }
        if let Some(ref ip6t) = self.ip6t_bin {
            teardown_family(ip6t);
        }
    }
}
