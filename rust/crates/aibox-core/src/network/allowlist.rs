//! Allowlist parsing, hostname resolution, and private-range DNS rebinding protection.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::sync::OnceLock;

use regex::Regex;

/// `(network_u32, mask_u32, label)` for every IPv4 range blocked from DNS resolution.
pub const PRIVATE_OR_RESERVED_IPV4_RANGES: &[(u32, u32, &str)] = &[
    (0x0000_0000, 0xFF00_0000, "0.0.0.0/8"),
    (0x0A00_0000, 0xFF00_0000, "10.0.0.0/8"),
    (0x6440_0000, 0xFFC0_0000, "100.64.0.0/10"),
    (0x7F00_0000, 0xFF00_0000, "127.0.0.0/8"),
    (0xA9FE_0000, 0xFFFF_0000, "169.254.0.0/16"),
    (0xAC10_0000, 0xFFF0_0000, "172.16.0.0/12"),
    (0xC0A8_0000, 0xFFFF_0000, "192.168.0.0/16"),
    (0xE000_0000, 0xE000_0000, "224.0.0.0/3"),
];

/// Resolves a hostname to a list of IPv4 or IPv4+IPv6 address strings.
pub trait HostResolver {
    fn resolve_ipv4(&self, host: &str) -> Vec<String>;

    /// Resolves a hostname to IPv4 and IPv6 address strings.
    /// Defaults to `resolve_ipv4` for IPv4-only resolvers.
    fn resolve_ips(&self, host: &str) -> Vec<String> {
        self.resolve_ipv4(host)
    }
}

impl<F> HostResolver for F
where
    F: Fn(&str) -> Vec<String>,
{
    fn resolve_ipv4(&self, host: &str) -> Vec<String> {
        self(host)
    }
}

/// System DNS resolver using `std::net::ToSocketAddrs`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemResolver;

impl HostResolver for SystemResolver {
    fn resolve_ipv4(&self, host: &str) -> Vec<String> {
        let Ok(addrs) = (host, 0u16).to_socket_addrs() else {
            return Vec::new();
        };
        addrs
            .filter_map(|addr| match addr {
                SocketAddr::V4(v4) => Some(v4.ip().to_string()),
                SocketAddr::V6(_) => None,
            })
            .collect()
    }

    fn resolve_ips(&self, host: &str) -> Vec<String> {
        let Ok(addrs) = (host, 0u16).to_socket_addrs() else {
            return Vec::new();
        };
        addrs.map(|addr| addr.ip().to_string()).collect()
    }
}

/// Deterministic in-memory resolver for unit tests.
#[derive(Debug, Default, Clone)]
pub struct StaticResolver {
    records: BTreeMap<String, Vec<String>>,
}

impl StaticResolver {
    pub fn new(records: BTreeMap<String, Vec<String>>) -> Self {
        Self { records }
    }
}

impl HostResolver for StaticResolver {
    fn resolve_ipv4(&self, host: &str) -> Vec<String> {
        self.records.get(host).cloned().unwrap_or_default()
    }

    fn resolve_ips(&self, host: &str) -> Vec<String> {
        self.records.get(host).cloned().unwrap_or_default()
    }
}

/// Returns `true` if `addr` falls within any private, loopback, link-local, CGNAT,
/// multicast, or reserved IPv4 range.
pub fn is_private_or_reserved_ipv4(addr: Ipv4Addr) -> bool {
    let ip = u32::from(addr);
    PRIVATE_OR_RESERVED_IPV4_RANGES
        .iter()
        .any(|&(net, mask, _)| (ip & mask) == net)
}

/// Returns `true` if `addr` is unspecified, loopback, multicast, unique-local (`fc00::/7`),
/// link-local (`fe80::/10`), or an IPv4-mapped private/reserved address.
pub fn is_private_or_reserved_ipv6(addr: Ipv6Addr) -> bool {
    if addr.is_unspecified() || addr.is_loopback() || addr.is_multicast() {
        return true;
    }
    let seg0 = addr.segments()[0];
    // fc00::/7 (Unique Local) and fe80::/10 (Link-Local)
    if (seg0 & 0xfe00) == 0xfc00 || (seg0 & 0xffc0) == 0xfe80 {
        return true;
    }
    if let Some(v4) = addr.to_ipv4_mapped() {
        return is_private_or_reserved_ipv4(v4);
    }
    false
}

/// Returns `true` if `addr` (IPv4 or IPv6) falls within a private or reserved range.
pub fn is_private_or_reserved_ip(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => is_private_or_reserved_ipv4(v4),
        IpAddr::V6(v6) => is_private_or_reserved_ipv6(v6),
    }
}

fn separator_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[,;\s]+").expect("valid separator regex"))
}

fn safe_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z0-9.:/-]+$").expect("valid safe token regex"))
}

fn host_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*$")
            .expect("valid hostname regex")
    })
}

/// Splits raw allowlist input on commas, semicolons, and whitespace, keeping only
/// lowercase safe tokens (`[a-z0-9.:/-]+`) in first-seen order.
pub fn parse_allowlist_entries(raw_text: &str) -> Vec<String> {
    let sep = separator_re();
    let safe = safe_token_re();
    let mut result = Vec::new();
    for part in sep.split(raw_text) {
        if part.is_empty() {
            continue;
        }
        let item = part.trim().to_ascii_lowercase();
        if item.is_empty() || !safe.is_match(&item) {
            continue;
        }
        if !result.contains(&item) {
            result.push(item);
        }
    }
    result
}

/// Returns `true` if `entry` is a valid lowercase DNS hostname.
pub fn is_valid_hostname(entry: &str) -> bool {
    host_re().is_match(entry)
}

/// Extracts unique valid hostnames from parsed allowlist entries, skipping CIDRs and IPs.
pub fn extract_allowlist_hostnames(entries: &[String]) -> Vec<String> {
    let mut hostnames = Vec::new();
    for entry in entries {
        if entry.contains('/') || entry.parse::<IpAddr>().is_ok() {
            continue;
        }
        if is_valid_hostname(entry) && !hostnames.contains(entry) {
            hostnames.push(entry.clone());
        }
    }
    hostnames
}

fn parse_ipv4_subnet(entry: &str) -> Option<(Ipv4Addr, u8)> {
    let (ip_str, prefix_str) = entry.split_once('/')?;
    let ip: Ipv4Addr = ip_str.parse().ok()?;
    let prefix: u8 = prefix_str.trim().parse().ok()?;
    if prefix > 32 {
        return None;
    }
    let mask = if prefix == 0 {
        0u32
    } else {
        !0u32 << (32 - u32::from(prefix))
    };
    Some((Ipv4Addr::from(u32::from(ip) & mask), prefix))
}

fn parse_ipv6_subnet(entry: &str) -> Option<(Ipv6Addr, u8)> {
    let (ip_str, prefix_str) = entry.split_once('/')?;
    let ip: Ipv6Addr = ip_str.parse().ok()?;
    let prefix: u8 = prefix_str.trim().parse().ok()?;
    if prefix > 128 {
        return None;
    }
    let mask = if prefix == 0 {
        0u128
    } else {
        !0u128 << (128 - u32::from(prefix))
    };
    Some((Ipv6Addr::from(u128::from(ip) & mask), prefix))
}

fn resolve_allowlist_cidrs_inner(
    entries: &[String],
    resolver: &dyn HostResolver,
    allow_ipv6: bool,
) -> (Vec<String>, Vec<String>) {
    let mut cidrs = Vec::new();
    let mut warnings = Vec::new();

    let mut append_unique = |cidr: String| {
        if !cidrs.contains(&cidr) {
            cidrs.push(cidr);
        }
    };

    for entry in entries {
        if entry.contains('/') {
            if let Some((net, prefix)) = parse_ipv4_subnet(entry) {
                append_unique(format!("{net}/{prefix}"));
            } else if allow_ipv6 {
                if let Some((net, prefix)) = parse_ipv6_subnet(entry) {
                    append_unique(format!("{net}/{prefix}"));
                } else {
                    warnings.push(format!("Invalid CIDR: {entry}"));
                }
            } else {
                warnings.push(format!("Invalid CIDR: {entry}"));
            }
            continue;
        }

        if let Ok(addr) = entry.parse::<IpAddr>() {
            match addr {
                IpAddr::V4(v4) => append_unique(format!("{v4}/32")),
                IpAddr::V6(v6) => {
                    if allow_ipv6 {
                        append_unique(format!("{v6}/128"));
                    } else {
                        warnings.push(format!("Only IPv4 addresses are supported: {entry}"));
                    }
                }
            }
            continue;
        }

        if !is_valid_hostname(entry) {
            warnings.push(format!("Invalid host entry: {entry}"));
            continue;
        }

        let resolved = if allow_ipv6 {
            resolver.resolve_ips(entry)
        } else {
            resolver.resolve_ipv4(entry)
        };
        let mut added_for_host = 0usize;
        for ip in resolved
            .iter()
            .filter_map(|ip_str| ip_str.parse::<IpAddr>().ok())
        {
            // Block DNS rebinding to loopback, link-local, host, or LAN unless entered explicitly as IP/CIDR.
            match ip {
                IpAddr::V4(v4) if !is_private_or_reserved_ipv4(v4) => {
                    append_unique(format!("{v4}/32"));
                    added_for_host += 1;
                }
                IpAddr::V6(v6) if allow_ipv6 && !is_private_or_reserved_ipv6(v6) => {
                    append_unique(format!("{v6}/128"));
                    added_for_host += 1;
                }
                _ => {}
            }
        }
        if added_for_host == 0 {
            if allow_ipv6 {
                warnings.push(format!("No public IP addresses resolved for '{entry}'."));
            } else {
                warnings.push(format!("No public IPv4 addresses resolved for '{entry}'."));
            }
        }
    }

    (cidrs, warnings)
}

/// Resolves allowlist entries (CIDRs, IPv4 addresses, or hostnames) into deduplicated
/// IPv4 CIDRs and human-readable warnings.
pub fn resolve_allowlist_cidrs(
    entries: &[String],
    resolver: &dyn HostResolver,
) -> (Vec<String>, Vec<String>) {
    resolve_allowlist_cidrs_inner(entries, resolver, false)
}

/// Resolves allowlist entries (IPv4/IPv6 CIDRs, IP addresses, or hostnames) into
/// deduplicated CIDRs and human-readable warnings, blocking private/reserved DNS rebinding.
pub fn resolve_allowlist_cidrs_with_ipv6(
    entries: &[String],
    resolver: &dyn HostResolver,
) -> (Vec<String>, Vec<String>) {
    resolve_allowlist_cidrs_inner(entries, resolver, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_and_reserved_table_covers_every_range() {
        assert_eq!(PRIVATE_OR_RESERVED_IPV4_RANGES.len(), 8);

        let blocked_samples = [
            ("0.0.0.0", "0.0.0.0/8"),
            ("0.255.255.255", "0.0.0.0/8"),
            ("10.0.0.1", "10.0.0.0/8"),
            ("10.255.255.254", "10.0.0.0/8"),
            ("100.64.0.1", "100.64.0.0/10"),
            ("100.127.255.254", "100.64.0.0/10"),
            ("127.0.0.1", "127.0.0.0/8"),
            ("127.255.255.254", "127.0.0.0/8"),
            ("169.254.169.254", "169.254.0.0/16"),
            ("172.16.0.1", "172.16.0.0/12"),
            ("172.31.255.254", "172.16.0.0/12"),
            ("192.168.0.1", "192.168.0.0/16"),
            ("192.168.255.254", "192.168.0.0/16"),
            ("224.0.0.1", "224.0.0.0/3"),
            ("240.0.0.1", "224.0.0.0/3"),
            ("255.255.255.255", "224.0.0.0/3"),
        ];
        for (ip_str, _range) in blocked_samples {
            assert!(
                ip_str
                    .parse::<Ipv4Addr>()
                    .is_ok_and(is_private_or_reserved_ipv4),
                "{ip_str} should be blocked"
            );
        }

        let public_samples = [
            "1.1.1.1",
            "8.8.8.8",
            "9.255.255.255",
            "11.0.0.1",
            "100.63.255.254",
            "100.128.0.1",
            "126.255.255.254",
            "128.0.0.1",
            "169.253.255.254",
            "169.255.0.1",
            "172.15.255.254",
            "172.32.0.1",
            "192.167.255.254",
            "192.169.0.1",
            "223.255.255.254",
        ];
        for ip_str in public_samples {
            assert!(
                ip_str
                    .parse::<Ipv4Addr>()
                    .is_ok_and(|ip| !is_private_or_reserved_ipv4(ip)),
                "{ip_str} should be public"
            );
        }
    }

    #[test]
    fn ipv6_allowlist_resolution_and_private_rebinding_protection() {
        let entries = parse_allowlist_entries(
            "2606:4700:4700::1111, 2001:db8:abcd:1::/64, dual.example, private6.example",
        );
        let resolver = StaticResolver::new(BTreeMap::from([
            (
                "dual.example".to_string(),
                vec![
                    "1.1.1.1".to_string(),
                    "2606:4700:4700::1001".to_string(),
                    "fe80::1".to_string(),
                    "fc00::1".to_string(),
                    "::1".to_string(),
                    "::ffff:127.0.0.1".to_string(),
                ],
            ),
            (
                "private6.example".to_string(),
                vec!["fd12:3456:789a::1".to_string(), "ff02::1".to_string()],
            ),
        ]));
        let (cidrs, warnings) = resolve_allowlist_cidrs_with_ipv6(&entries, &resolver);
        assert_eq!(
            cidrs,
            vec![
                "2606:4700:4700::1111/128",
                "2001:db8:abcd:1::/64",
                "1.1.1.1/32",
                "2606:4700:4700::1001/128",
            ]
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("private6.example"));
    }

    #[test]
    fn invalid_allowlist_entries_never_panic_and_emit_warnings() {
        let entries = vec![
            "999.999.999.999".to_string(),
            "2001:db8::1/129".to_string(),
            "1.2.3.4/33".to_string(),
            "not/a/cidr".to_string(),
            "bad$host!".to_string(),
            "-leading-dash.test".to_string(),
            "garbage-dns.test".to_string(),
        ];
        let resolver = StaticResolver::new(BTreeMap::from([(
            "garbage-dns.test".to_string(),
            vec![
                "not-an-ip".to_string(),
                "999.0.0.1".to_string(),
                "".to_string(),
                "2001:db8::99999".to_string(),
            ],
        )]));
        let (cidrs_v4, warnings_v4) = resolve_allowlist_cidrs(&entries, &resolver);
        assert!(cidrs_v4.is_empty());
        assert_eq!(warnings_v4.len(), 7);

        let (cidrs_v6, warnings_v6) = resolve_allowlist_cidrs_with_ipv6(&entries, &resolver);
        assert!(cidrs_v6.is_empty());
        assert_eq!(warnings_v6.len(), 7);
    }

    #[test]
    fn allowlist_resolves_and_blocks_private_dns_rebinding_matches_qt_test() {
        let entries =
            parse_allowlist_entries("GitHub.com, 1.1.1.1; 10.0.0.0/8\nlocal-rebind.test bad$host");
        assert_eq!(
            entries,
            vec!["github.com", "1.1.1.1", "10.0.0.0/8", "local-rebind.test"]
        );

        let resolver = StaticResolver::new(BTreeMap::from([
            (
                "github.com".to_string(),
                vec!["140.82.121.4".to_string(), "192.168.1.50".to_string()],
            ),
            (
                "local-rebind.test".to_string(),
                vec!["127.0.0.1".to_string(), "10.1.2.3".to_string()],
            ),
        ]));
        let (cidrs, warnings) = resolve_allowlist_cidrs(&entries, &resolver);
        assert_eq!(cidrs, vec!["140.82.121.4/32", "1.1.1.1/32", "10.0.0.0/8"]);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn extract_hostnames_skips_cidrs_ips_and_duplicates() {
        let entries = vec![
            "github.com".to_string(),
            "1.1.1.1".to_string(),
            "10.0.0.0/8".to_string(),
            "2001:db8::1".to_string(),
            "bad_host!".to_string(),
            "github.com".to_string(),
            "api.example.com".to_string(),
        ];
        assert_eq!(
            extract_allowlist_hostnames(&entries),
            vec!["github.com", "api.example.com"]
        );
    }
}
