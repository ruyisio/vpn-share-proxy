use crate::types::{GatewayConfig, ProxyMode};
use std::path::PathBuf;
use std::process::Command;

pub(crate) const FWMARK: &str = "0x80000001";
pub(crate) const FWMARK_MASK: &str = "0x80000001/0xffffffff";

/// IPT_SO_GET_INFO / IP6T_SO_GET_INFO — proves kernel netfilter tables without a helper binary.
const IPT_SO_GET_INFO: i32 = 64;
const IPT_TABLE_MAXNAMELEN: usize = 32;

/// Prefer module-bundled helpers, then absolute system paths — never PATH.
pub(crate) fn find_nf_helper(name: &str) -> Option<String> {
    let mut paths = Vec::new();
    for base in [
        "/data/adb/modules/vpn_share_proxy/bin",
        "/data/adb/modules/vpngw/bin",
        "/data/adb/modules/vpn_gateway/bin",
        "/system/bin",
        "/system/xbin",
        "/vendor/bin",
    ] {
        paths.push(PathBuf::from(base).join(name));
    }
    paths
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Probe legacy ip_tables via sockopt (same API iptables uses). Falls back to /proc.
pub(crate) fn probe_iptables_sockopt(v6: bool) -> bool {
    unsafe {
        let fd = if v6 {
            libc::socket(
                libc::AF_INET6,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                libc::IPPROTO_RAW,
            )
        } else {
            libc::socket(
                libc::AF_INET,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                libc::IPPROTO_RAW,
            )
        };
        if fd < 0 {
            return false;
        }
        #[repr(C)]
        struct IptGetinfo {
            name: [u8; IPT_TABLE_MAXNAMELEN],
            valid_hooks: u32,
            hook_entry: [u32; 5],
            underflow: [u32; 5],
            num_entries: u32,
            size: u32,
        }
        let mut info = std::mem::MaybeUninit::<IptGetinfo>::zeroed();
        let info_ptr = info.as_mut_ptr();
        let name = b"filter\0";
        std::ptr::copy_nonoverlapping(name.as_ptr(), (*info_ptr).name.as_mut_ptr(), name.len());
        let mut len = std::mem::size_of::<IptGetinfo>() as libc::socklen_t;
        let level = if v6 {
            libc::IPPROTO_IPV6
        } else {
            libc::IPPROTO_IP
        };
        let rc = libc::getsockopt(
            fd,
            level,
            IPT_SO_GET_INFO,
            info_ptr as *mut libc::c_void,
            &mut len,
        );
        libc::close(fd);
        rc == 0
    }
}

pub(crate) fn run(bin: &str, args: &[&str]) {
    let _ = Command::new(bin).args(args).output();
}

pub(crate) fn pin_jump(bin: &str, table_args: &[&str], root: &str, chain: &str) {
    let mut del = Vec::new();
    del.extend_from_slice(table_args);
    del.extend_from_slice(&["-w", "2", "-D", root, "-j", chain]);
    for _ in 0..5 {
        let out = Command::new(bin).args(&del).output();
        if !matches!(out, Ok(ref o) if o.status.success()) {
            break;
        }
    }
    let mut create = Vec::new();
    create.extend_from_slice(table_args);
    create.extend_from_slice(&["-w", "2", "-N", chain]);
    run(bin, &create);
    let mut insert = Vec::new();
    insert.extend_from_slice(table_args);
    insert.extend_from_slice(&["-w", "2", "-I", root, "-j", chain]);
    run(bin, &insert);
}

pub(crate) fn flush_chain(bin: &str, table_args: &[&str], chain: &str) {
    let mut args = Vec::new();
    args.extend_from_slice(table_args);
    args.extend_from_slice(&["-w", "2", "-F", chain]);
    run(bin, &args);
}

pub(crate) fn pin_family(bin: &str) {
    pin_jump(bin, &[], "FORWARD", "VSP_FWD");
    pin_jump(bin, &["-t", "mangle"], "PREROUTING", "VSP_PRE");
    pin_jump(bin, &["-t", "nat"], "PREROUTING", "VSP_NAT");
}

pub(crate) fn teardown_family(bin: &str) {
    run(bin, &["-w", "2", "-D", "FORWARD", "-j", "VSP_FWD"]);
    run(bin, &["-w", "2", "-F", "VSP_FWD"]);
    run(bin, &["-w", "2", "-X", "VSP_FWD"]);
    run(
        bin,
        &["-w", "2", "-t", "mangle", "-D", "PREROUTING", "-j", "VSP_PRE"],
    );
    run(bin, &["-w", "2", "-t", "mangle", "-F", "VSP_PRE"]);
    run(bin, &["-w", "2", "-t", "mangle", "-X", "VSP_PRE"]);
    run(
        bin,
        &["-w", "2", "-t", "nat", "-D", "PREROUTING", "-j", "VSP_NAT"],
    );
    run(bin, &["-w", "2", "-t", "nat", "-F", "VSP_NAT"]);
    run(bin, &["-w", "2", "-t", "nat", "-X", "VSP_NAT"]);
}

/// Shared mangle mark/RETURN for allowlist (identical on v4/v6 helpers).
pub(crate) fn apply_mangle_allow(bin: &str, config: &GatewayConfig, managed: &[String]) {
    for iface in managed {
        for mac in &config.mac_allow_list {
            run(
                bin,
                &[
                    "-w",
                    "2",
                    "-t",
                    "mangle",
                    "-A",
                    "VSP_PRE",
                    "-i",
                    iface,
                    "-m",
                    "mac",
                    "--mac-source",
                    mac,
                    "-j",
                    "RETURN",
                ],
            );
        }
        run(
            bin,
            &[
                "-w",
                "2",
                "-t",
                "mangle",
                "-A",
                "VSP_PRE",
                "-i",
                iface,
                "-j",
                "MARK",
                "--set-mark",
                FWMARK,
            ],
        );
    }
}

/// Shared mangle MARK for blocklist (identical on v4/v6 helpers).
pub(crate) fn apply_mangle_block(bin: &str, config: &GatewayConfig, managed: &[String]) {
    for iface in managed {
        for mac in &config.mac_block_list {
            run(
                bin,
                &[
                    "-w",
                    "2",
                    "-t",
                    "mangle",
                    "-A",
                    "VSP_PRE",
                    "-i",
                    iface,
                    "-m",
                    "mac",
                    "--mac-source",
                    mac,
                    "-j",
                    "MARK",
                    "--set-mark",
                    FWMARK,
                ],
            );
        }
    }
}

/// TCPMSS must be installed *before* any `-o tun -j ACCEPT`.
/// `ACCEPT` terminates the chain; a later TCPMSS rule never sees client→tun SYNs.
/// Call this before [`apply_fwd_accept`].
pub(crate) fn apply_tcpmss_and_reply(bin: &str, tun: &str) {
    // Non-terminating: clamp then continue to a later ACCEPT.
    run(
        bin,
        &[
            "-w",
            "2",
            "-A",
            "VSP_FWD",
            "-o",
            tun,
            "-p",
            "tcp",
            "--tcp-flags",
            "SYN,RST",
            "SYN",
            "-j",
            "TCPMSS",
            "--clamp-mss-to-pmtu",
        ],
    );
    run(bin, &["-w", "2", "-A", "VSP_FWD", "-i", tun, "-j", "ACCEPT"]);
}

pub(crate) fn apply_fwd_accept(bin: &str, config: &GatewayConfig, tun: &str, private_ranges: &[&str]) {
    let managed = config.managed_ifaces_sorted();
    match config.proxy_mode {
        ProxyMode::Global => {
            for range in private_ranges {
                run(
                    bin,
                    &["-w", "2", "-A", "VSP_FWD", "-s", range, "-o", tun, "-j", "ACCEPT"],
                );
            }
        }
        ProxyMode::MacAllowlist | ProxyMode::MacBlocklist => {
            for iface in &managed {
                run(
                    bin,
                    &["-w", "2", "-A", "VSP_FWD", "-i", iface, "-o", tun, "-j", "ACCEPT"],
                );
            }
        }
    }
}

pub(crate) fn flush_proxy_chains(bin: &str) {
    flush_chain(bin, &[], "VSP_FWD");
    flush_chain(bin, &["-t", "mangle"], "VSP_PRE");
    flush_chain(bin, &["-t", "nat"], "VSP_NAT");
}
