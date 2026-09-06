//! Pure NETLINK_ROUTE FIB rule management — no `/system/bin/ip`.
//! Probes the kernel via AF_NETLINK; works whenever CONFIG_IP_MULTIPLE_TABLES is on.

use anyhow::{bail, Context, Result};
use std::net::{Ipv4Addr, Ipv6Addr};
use tracing::{debug, info, warn};

const RTM_NEWRULE: u16 = 32;
const RTM_DELRULE: u16 = 33;

const NLM_F_REQUEST: u16 = 1;
const NLM_F_ACK: u16 = 4;
const NLM_F_CREATE: u16 = 0x400;
const NLM_F_EXCL: u16 = 0x200;

const NLMSG_ERROR: u16 = 2;

const FR_ACT_TO_TBL: u8 = 1;
const FR_ACT_GOTO: u8 = 2;
const FR_ACT_NOP: u8 = 3;
const FR_ACT_UNSPEC: u8 = 0;

const FRA_SRC: u16 = 2;
const FRA_IIFNAME: u16 = 3;
const FRA_GOTO: u16 = 4;
const FRA_PRIORITY: u16 = 6;
const FRA_FWMARK: u16 = 10;
const FRA_SUPPRESS_PREFIXLEN: u16 = 14;
const FRA_TABLE: u16 = 15;
const FRA_FWMASK: u16 = 16;

const RT_TABLE_MAIN: u32 = 254;
const RT_TABLE_UNSPEC: u8 = 0;

const PREF_LO_BYPASS: u32 = 5000;
const PREF_VPN_RETURN: u32 = 5010;
const PREF_VPN_BYPASS: u32 = 5020;
const PREF_MAC_BYPASS: u32 = 5028;
const PREF_HOTSPOT_BASE: u32 = 5030;
const PREF_HOTSPOT_STEP: u32 = 10;
const PREF_HOTSPOT_SLOTS: u32 = 32;
const PREF_ANCHOR_NOP: u32 = 6000;

const MANAGED_PREFS: &[u32] = &[5000, 5010, 5020, 5028, 6000];

fn hotspot_prefs() -> impl Iterator<Item = u32> {
    (0..PREF_HOTSPOT_SLOTS).map(|i| PREF_HOTSPOT_BASE + i * PREF_HOTSPOT_STEP)
}

const PRIVATE_RANGES_V4: &[(Ipv4Addr, u8)] = &[
    (Ipv4Addr::new(10, 0, 0, 0), 8),
    (Ipv4Addr::new(172, 16, 0, 0), 12),
    (Ipv4Addr::new(192, 168, 0, 0), 16),
];

const PRIVATE_RANGES_V6: &[(Ipv6Addr, u8)] = &[
    (Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0), 10),
    (Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 0), 8),
];

const FWMARK_BYPASS: u32 = 0x8000_0001;
const FWMARK_MASK: u32 = 0xffff_ffff;

pub struct NetlinkRuleManager;

impl NetlinkRuleManager {
    pub fn new() -> Result<Self> {
        // Probe: can we open NETLINK_ROUTE?
        let fd = Self::open_socket()?;
        unsafe { libc::close(fd) };
        Ok(Self)
    }

    /// True when the kernel accepts FIB rule netlink (multi-table routing).
    pub fn kernel_supports_rules() -> bool {
        Self::open_socket().map(|fd| {
            unsafe { libc::close(fd) };
            true
        }).unwrap_or(false)
    }

    fn open_socket() -> Result<i32> {
        let fd = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                libc::NETLINK_ROUTE,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error())
                .context("AF_NETLINK/NETLINK_ROUTE unavailable (kernel lacks netlink route?)");
        }
        // Bound local addr so we receive ACKs
        let mut local: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        local.nl_family = libc::AF_NETLINK as libc::sa_family_t;
        local.nl_pid = 0; // kernel assigns
        let rc = unsafe {
            libc::bind(
                fd,
                &local as *const _ as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            )
        };
        if rc < 0 {
            let err = std::io::Error::last_os_error();
            unsafe { libc::close(fd) };
            return Err(err).context("bind(AF_NETLINK) failed");
        }
        Ok(fd)
    }

    pub fn flush_managed_rules(&self, v4: bool, v6: bool) {
        let prefs: Vec<u32> = MANAGED_PREFS
            .iter()
            .copied()
            .chain(hotspot_prefs())
            .collect();
        for pref in prefs {
            if v4 {
                for _ in 0..16 {
                    if Self::del_by_pref(libc::AF_INET as u8, pref).is_err() {
                        break;
                    }
                }
            }
            if v6 {
                for _ in 0..16 {
                    if Self::del_by_pref(libc::AF_INET6 as u8, pref).is_err() {
                        break;
                    }
                }
            }
        }
    }

    fn del_by_pref(family: u8, pref: u32) -> Result<()> {
        let attrs = vec![(FRA_PRIORITY, pref.to_ne_bytes().to_vec())];
        Self::send(
            family,
            RTM_DELRULE,
            NLM_F_REQUEST | NLM_F_ACK,
            FR_ACT_UNSPEC,
            0,
            0,
            RT_TABLE_UNSPEC,
            &attrs,
        )
    }

    pub fn apply_ipv4_rules(
        &self,
        tun: &str,
        table: u32,
        mode: &crate::types::ProxyMode,
        managed_ifaces: &[String],
    ) -> Result<()> {
        debug!(
            "Netlink IPv4 rules: tun={} table={} mode={:?} managed={:?}",
            tun, table, mode, managed_ifaces
        );
        if !Self::kernel_supports_rules() {
            bail!("kernel does not support NETLINK_ROUTE FIB rules");
        }
        self.flush_managed_rules(true, false);
        let fam = libc::AF_INET as u8;

        self.add_goto_iif(fam, "lo", PREF_ANCHOR_NOP, PREF_LO_BYPASS)?;
        self.add_lookup_suppress(fam, tun, RT_TABLE_MAIN, 0, PREF_VPN_RETURN)?;
        self.add_goto_iif(fam, tun, PREF_ANCHOR_NOP, PREF_VPN_BYPASS)?;
        self.add_fwmark_goto(fam, FWMARK_BYPASS, FWMARK_MASK, PREF_ANCHOR_NOP, PREF_MAC_BYPASS)?;

        let mut pref = PREF_HOTSPOT_BASE;
        match mode {
            crate::types::ProxyMode::Global => {
                for &(ip, plen) in PRIVATE_RANGES_V4 {
                    self.add_from_lookup_v4(ip, plen, table, pref)?;
                    pref += PREF_HOTSPOT_STEP;
                }
            }
            crate::types::ProxyMode::MacAllowlist | crate::types::ProxyMode::MacBlocklist => {
                for iface in managed_ifaces {
                    if iface == tun || iface == "lo" {
                        continue;
                    }
                    self.add_iif_lookup(fam, iface, table, pref)?;
                    pref += PREF_HOTSPOT_STEP;
                }
            }
        }
        self.add_nop(fam, PREF_ANCHOR_NOP)?;

        info!("IPv4 FIB rules installed via Netlink (table {})", table);
        Ok(())
    }

    pub fn apply_ipv6_rules(
        &self,
        tun: &str,
        table: u32,
        mode: &crate::types::ProxyMode,
        managed_ifaces: &[String],
    ) -> Result<()> {
        debug!(
            "Netlink IPv6 rules: tun={} table={} mode={:?} managed={:?}",
            tun, table, mode, managed_ifaces
        );
        self.flush_managed_rules(false, true);
        let fam = libc::AF_INET6 as u8;

        self.add_goto_iif(fam, "lo", PREF_ANCHOR_NOP, PREF_LO_BYPASS)?;
        self.add_lookup_suppress(fam, tun, RT_TABLE_MAIN, 0, PREF_VPN_RETURN)?;
        self.add_goto_iif(fam, tun, PREF_ANCHOR_NOP, PREF_VPN_BYPASS)?;
        self.add_fwmark_goto(fam, FWMARK_BYPASS, FWMARK_MASK, PREF_ANCHOR_NOP, PREF_MAC_BYPASS)?;

        let mut pref = PREF_HOTSPOT_BASE;
        match mode {
            crate::types::ProxyMode::Global => {
                for &(ip, plen) in PRIVATE_RANGES_V6 {
                    self.add_from_lookup_v6(ip, plen, table, pref)?;
                    pref += PREF_HOTSPOT_STEP;
                }
            }
            crate::types::ProxyMode::MacAllowlist | crate::types::ProxyMode::MacBlocklist => {
                for iface in managed_ifaces {
                    if iface == tun || iface == "lo" {
                        continue;
                    }
                    self.add_iif_lookup(fam, iface, table, pref)?;
                    pref += PREF_HOTSPOT_STEP;
                }
            }
        }
        self.add_nop(fam, PREF_ANCHOR_NOP)?;

        info!("IPv6 FIB rules installed via Netlink (table {})", table);
        Ok(())
    }

    fn add_iif_lookup(&self, family: u8, iif: &str, table: u32, priority: u32) -> Result<()> {
        let mut attrs = Vec::new();
        attrs.push((FRA_PRIORITY, priority.to_ne_bytes().to_vec()));
        attrs.push((FRA_TABLE, table.to_ne_bytes().to_vec()));
        attrs.push((FRA_IIFNAME, cstr(iif)));
        let hdr_table = if table < 256 {
            table as u8
        } else {
            RT_TABLE_UNSPEC
        };
        Self::send(
            family,
            RTM_NEWRULE,
            NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
            FR_ACT_TO_TBL,
            0,
            0,
            hdr_table,
            &attrs,
        )
    }

    fn add_goto_iif(&self, family: u8, iif: &str, goto_pref: u32, priority: u32) -> Result<()> {
        let mut attrs = Vec::new();
        attrs.push((FRA_PRIORITY, priority.to_ne_bytes().to_vec()));
        attrs.push((FRA_GOTO, goto_pref.to_ne_bytes().to_vec()));
        attrs.push((FRA_IIFNAME, cstr(iif)));
        Self::send(
            family,
            RTM_NEWRULE,
            NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
            FR_ACT_GOTO,
            0,
            0,
            RT_TABLE_UNSPEC,
            &attrs,
        )
    }

    fn add_lookup_suppress(
        &self,
        family: u8,
        iif: &str,
        table: u32,
        suppress_prefixlen: u32,
        priority: u32,
    ) -> Result<()> {
        let mut attrs = Vec::new();
        attrs.push((FRA_PRIORITY, priority.to_ne_bytes().to_vec()));
        attrs.push((FRA_TABLE, table.to_ne_bytes().to_vec()));
        attrs.push((
            FRA_SUPPRESS_PREFIXLEN,
            suppress_prefixlen.to_ne_bytes().to_vec(),
        ));
        attrs.push((FRA_IIFNAME, cstr(iif)));
        let hdr_table = if table < 256 { table as u8 } else { RT_TABLE_UNSPEC };
        Self::send(
            family,
            RTM_NEWRULE,
            NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
            FR_ACT_TO_TBL,
            0,
            0,
            hdr_table,
            &attrs,
        )
    }

    fn add_fwmark_goto(
        &self,
        family: u8,
        mark: u32,
        mask: u32,
        goto_pref: u32,
        priority: u32,
    ) -> Result<()> {
        let mut attrs = Vec::new();
        attrs.push((FRA_PRIORITY, priority.to_ne_bytes().to_vec()));
        attrs.push((FRA_FWMARK, mark.to_ne_bytes().to_vec()));
        attrs.push((FRA_FWMASK, mask.to_ne_bytes().to_vec()));
        attrs.push((FRA_GOTO, goto_pref.to_ne_bytes().to_vec()));
        Self::send(
            family,
            RTM_NEWRULE,
            NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
            FR_ACT_GOTO,
            0,
            0,
            RT_TABLE_UNSPEC,
            &attrs,
        )
    }

    fn add_from_lookup_v4(&self, ip: Ipv4Addr, prefix_len: u8, table: u32, priority: u32) -> Result<()> {
        let mut attrs = Vec::new();
        attrs.push((FRA_PRIORITY, priority.to_ne_bytes().to_vec()));
        attrs.push((FRA_TABLE, table.to_ne_bytes().to_vec()));
        attrs.push((FRA_SRC, ip.octets().to_vec()));
        let hdr_table = if table < 256 { table as u8 } else { RT_TABLE_UNSPEC };
        Self::send(
            libc::AF_INET as u8,
            RTM_NEWRULE,
            NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
            FR_ACT_TO_TBL,
            prefix_len,
            0,
            hdr_table,
            &attrs,
        )
    }

    fn add_from_lookup_v6(&self, ip: Ipv6Addr, prefix_len: u8, table: u32, priority: u32) -> Result<()> {
        let mut attrs = Vec::new();
        attrs.push((FRA_PRIORITY, priority.to_ne_bytes().to_vec()));
        attrs.push((FRA_TABLE, table.to_ne_bytes().to_vec()));
        attrs.push((FRA_SRC, ip.octets().to_vec()));
        let hdr_table = if table < 256 { table as u8 } else { RT_TABLE_UNSPEC };
        Self::send(
            libc::AF_INET6 as u8,
            RTM_NEWRULE,
            NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
            FR_ACT_TO_TBL,
            prefix_len,
            0,
            hdr_table,
            &attrs,
        )
    }

    fn add_nop(&self, family: u8, priority: u32) -> Result<()> {
        let attrs = vec![(FRA_PRIORITY, priority.to_ne_bytes().to_vec())];
        Self::send(
            family,
            RTM_NEWRULE,
            NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
            FR_ACT_NOP,
            0,
            0,
            RT_TABLE_UNSPEC,
            &attrs,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn send(
        family: u8,
        msg_type: u16,
        flags: u16,
        action: u8,
        src_len: u8,
        dst_len: u8,
        hdr_table: u8,
        attrs: &[(u16, Vec<u8>)],
    ) -> Result<()> {
        let fd = Self::open_socket()?;

        let mut buf = Vec::with_capacity(256);
        buf.extend_from_slice(&[0u8; 16]); // nlmsghdr placeholder

        // fib_rule_hdr (12 bytes)
        buf.push(family);
        buf.push(dst_len);
        buf.push(src_len);
        buf.push(0); // tos
        buf.push(hdr_table);
        buf.push(0); // res1
        buf.push(0); // res2
        buf.push(action);
        buf.extend_from_slice(&0u32.to_ne_bytes()); // flags

        for (atype, data) in attrs {
            push_rta(&mut buf, *atype, data);
        }

        let total_len = buf.len() as u32;
        buf[0..4].copy_from_slice(&total_len.to_ne_bytes());
        buf[4..6].copy_from_slice(&msg_type.to_ne_bytes());
        buf[6..8].copy_from_slice(&flags.to_ne_bytes());
        buf[8..12].copy_from_slice(&1u32.to_ne_bytes()); // seq
        buf[12..16].copy_from_slice(&0u32.to_ne_bytes()); // pid

        let mut sa: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        sa.nl_family = libc::AF_NETLINK as libc::sa_family_t;

        let ret = unsafe {
            libc::sendto(
                fd,
                buf.as_ptr() as *const libc::c_void,
                buf.len(),
                0,
                &sa as *const _ as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            )
        };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            unsafe { libc::close(fd) };
            return Err(err).context("netlink sendto failed");
        }

        let err = read_ack(fd);
        unsafe { libc::close(fd) };

        match err {
            Ok(()) => Ok(()),
            // Duplicate create is fine
            Err(e) if e == libc::EEXIST as i32 && msg_type == RTM_NEWRULE => Ok(()),
            // Missing rule on delete → caller (flush loop) stops
            Err(e) if e == libc::ENOENT as i32 && msg_type == RTM_DELRULE => {
                Err(std::io::Error::from_raw_os_error(e).into())
            }
            Err(e) => {
                warn!("netlink rule op type={} errno={}", msg_type, e);
                Err(std::io::Error::from_raw_os_error(e))
                    .context(format!("kernel rejected FIB rule (errno {e})"))
            }
        }
    }
}

fn cstr(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

fn push_rta(buf: &mut Vec<u8>, atype: u16, data: &[u8]) {
    let rta_len = (4 + data.len()) as u16;
    buf.extend_from_slice(&rta_len.to_ne_bytes());
    buf.extend_from_slice(&atype.to_ne_bytes());
    buf.extend_from_slice(data);
    let pad = (4 - (buf.len() % 4)) % 4;
    buf.extend(std::iter::repeat(0u8).take(pad));
}

/// Read NLMSG_ERROR ack. Returns Ok(()) on success, Err(errno) on kernel error code.
fn read_ack(fd: i32) -> std::result::Result<(), i32> {
    let mut reply = [0u8; 2048];
    let n = unsafe {
        libc::recv(
            fd,
            reply.as_mut_ptr() as *mut libc::c_void,
            reply.len(),
            0,
        )
    };
    if n < 16 {
        return Err(libc::EIO);
    }
    let nlmsg_type = u16::from_ne_bytes([reply[4], reply[5]]);
    if nlmsg_type != NLMSG_ERROR {
        return Ok(());
    }
    if n < 20 {
        return Err(libc::EIO);
    }
    // nlmsgerr.error is immediately after nlmsghdr (offset 16)
    let err = i32::from_ne_bytes([reply[16], reply[17], reply[18], reply[19]]);
    if err == 0 {
        Ok(())
    } else {
        // kernel returns negative errno
        Err(-err)
    }
}
