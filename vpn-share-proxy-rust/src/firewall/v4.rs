use super::common::{
    apply_fwd_accept, apply_mangle_allow, apply_mangle_block, apply_tcpmss_and_reply,
    flush_proxy_chains, run, FWMARK_MASK,
};
use super::FirewallManager;
use crate::android::AndroidEnv;
use crate::types::{DnsRedirectMode, GatewayConfig, ProxyMode};
use tracing::{debug, warn};

const PRIVATE_RANGES_V4: &[&str] = &["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"];

fn resolve_dns_v4(config: &GatewayConfig) -> Option<String> {
    match config.dns_redirect {
        DnsRedirectMode::Off => None,
        DnsRedirectMode::Custom => config
            .custom_dns_v4
            .map(|ip| ip.to_string())
            .or_else(|| Some("8.8.8.8".to_string())),
        DnsRedirectMode::Auto => AndroidEnv::get_vpn_dns(false),
    }
}

pub(crate) fn apply(mgr: &FirewallManager, config: &GatewayConfig, tun: &str) {
    let Some(ref ipt) = mgr.ipt_bin else {
        warn!(
            "skip IPv4 firewall: no iptables binary (kernel_iptables={})",
            mgr.kernel_iptables
        );
        return;
    };
    if !mgr.kernel_iptables {
        warn!("skip IPv4 firewall: kernel has no ip_tables");
        return;
    }

    mgr.ensure_hooks_pinned();
    flush_proxy_chains(ipt);

    let managed = config.managed_ifaces_sorted();
    // TCPMSS before client→tun ACCEPT (shared helper documents why).
    apply_tcpmss_and_reply(ipt, tun);
    apply_fwd_accept(ipt, config, tun, PRIVATE_RANGES_V4);

    match config.proxy_mode {
        ProxyMode::MacAllowlist => apply_mangle_allow(ipt, config, &managed),
        ProxyMode::MacBlocklist => apply_mangle_block(ipt, config, &managed),
        ProxyMode::Global => debug!("Global mode: no mangle bypass marks"),
    }

    if let Some(dns) = resolve_dns_v4(config) {
        let dns_target = format!("{}:53", dns);
        match config.proxy_mode {
            ProxyMode::Global => {
                for range in PRIVATE_RANGES_V4 {
                    run(
                        ipt,
                        &[
                            "-w",
                            "2",
                            "-t",
                            "nat",
                            "-A",
                            "VSP_NAT",
                            "!",
                            "-i",
                            tun,
                            "-m",
                            "mark",
                            "!",
                            "--mark",
                            FWMARK_MASK,
                            "-s",
                            range,
                            "-p",
                            "udp",
                            "--dport",
                            "53",
                            "-j",
                            "DNAT",
                            "--to",
                            &dns_target,
                        ],
                    );
                }
            }
            ProxyMode::MacAllowlist | ProxyMode::MacBlocklist => {
                for iface in &managed {
                    run(
                        ipt,
                        &[
                            "-w",
                            "2",
                            "-t",
                            "nat",
                            "-A",
                            "VSP_NAT",
                            "-i",
                            iface,
                            "-m",
                            "mark",
                            "!",
                            "--mark",
                            FWMARK_MASK,
                            "-p",
                            "udp",
                            "--dport",
                            "53",
                            "-j",
                            "DNAT",
                            "--to",
                            &dns_target,
                        ],
                    );
                }
            }
        }
    }
}
