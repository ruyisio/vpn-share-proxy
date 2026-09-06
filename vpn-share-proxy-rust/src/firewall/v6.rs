use super::common::{
    apply_fwd_accept, apply_mangle_allow, apply_mangle_block, apply_tcpmss_and_reply,
    flush_proxy_chains, run,
};
use super::FirewallManager;
use crate::types::{GatewayConfig, ProxyMode};

const PRIVATE_RANGES_V6: &[&str] = &["fe80::/10", "fd00::/8"];

pub(crate) fn apply(mgr: &FirewallManager, config: &GatewayConfig, tun: &str, enabled: bool) {
    let Some(ref ip6t) = mgr.ip6t_bin else {
        return;
    };
    if !mgr.kernel_ip6tables {
        return;
    }

    mgr.ensure_hooks_pinned();
    flush_proxy_chains(ip6t);

    let managed = config.managed_ifaces_sorted();

    if !enabled {
        match config.proxy_mode {
            ProxyMode::Global => {
                for range in PRIVATE_RANGES_V6 {
                    run(
                        ip6t,
                        &[
                            "-w",
                            "2",
                            "-A",
                            "VSP_FWD",
                            "-s",
                            range,
                            "-j",
                            "REJECT",
                            "--reject-with",
                            "icmp6-addr-unreachable",
                        ],
                    );
                }
            }
            ProxyMode::MacAllowlist | ProxyMode::MacBlocklist => {
                for iface in &managed {
                    run(
                        ip6t,
                        &[
                            "-w",
                            "2",
                            "-A",
                            "VSP_FWD",
                            "-i",
                            iface,
                            "-j",
                            "REJECT",
                            "--reject-with",
                            "icmp6-addr-unreachable",
                        ],
                    );
                }
            }
        }
        return;
    }

    // TCPMSS before client→tun ACCEPT (shared helper documents why).
    apply_tcpmss_and_reply(ip6t, tun);
    apply_fwd_accept(ip6t, config, tun, PRIVATE_RANGES_V6);

    match config.proxy_mode {
        ProxyMode::MacAllowlist => apply_mangle_allow(ip6t, config, &managed),
        ProxyMode::MacBlocklist => apply_mangle_block(ip6t, config, &managed),
        ProxyMode::Global => {}
    }
}
