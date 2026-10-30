//! A host's short display name, derived without changing its registry identity.
//!
//! Aliases express the user's choice and stay verbatim. Unaliased SSH hosts
//! use their first hostname label, while addresses remain complete. This is
//! presentation policy shared with clients, never a host-resolution rule.
//! Inputs and output retain raw peer text; renderers still escape and isolate it.

use core::net::{IpAddr, Ipv6Addr};

/// Derive the short form from this host alone, preserving an explicit alias.
///
/// Without an alias, the local host is `Local`; an SSH destination loses its
/// scheme, last-`@` user prefix, brackets and an explicit port before choosing
/// the first hostname label. IP addresses (including scoped IPv6) stay whole.
/// A leading dot retains the remainder rather than producing an empty label.
///
/// This does not validate an SSH destination or disambiguate equal names. It
/// returns raw text, including controls in an alias, so presentation escaping
/// remains the caller's responsibility. A missing remote destination is empty.
pub fn host_short_name(local: bool, destination: Option<&str>, alias: Option<&str>) -> String {
    if let Some(alias) = alias {
        return alias.to_owned();
    }
    if local {
        return "Local".to_owned();
    }

    let destination = destination.unwrap_or_default();
    let uri = destination.starts_with("ssh://");
    let destination = destination.strip_prefix("ssh://").unwrap_or(destination);
    let host = destination
        .rsplit_once('@')
        .map_or(destination, |(_, host)| host);
    let host = destination_host(host, uri);

    // A scope is part of the address's display spelling. Parsing the address
    // separately also preserves scopes containing dots instead of treating one
    // as the boundary of a hostname label.
    let address = host.parse::<IpAddr>().is_ok()
        || host
            .split_once('%')
            .is_some_and(|(address, _)| address.parse::<Ipv6Addr>().is_ok());
    if address {
        return host.to_owned();
    }
    match host.split_once('.') {
        Some((first, _)) if !first.is_empty() => first.to_owned(),
        _ => host.to_owned(),
    }
}

/// Remove only port syntax that cannot be confused with an unbracketed IPv6 address.
///
/// A bare SSH name has no port suffix; only URI names and bracketed addresses
/// admit one. Keeping that distinction avoids turning the last IPv6 segment
/// into a supposed port or interpreting a dot-less SSH configuration alias.
fn destination_host(host: &str, uri: bool) -> &str {
    if let Some(bracketed) = host.strip_prefix('[')
        && let Some((address, suffix)) = bracketed.split_once(']')
        && (suffix.is_empty() || suffix.strip_prefix(':').is_some_and(is_port))
    {
        return address;
    }
    if uri
        && let Some((name, port)) = host.rsplit_once(':')
        && !name.contains(':')
        && is_port(port)
    {
        return name;
    }
    host
}

/// Recognize the explicit numeric suffix without validating its network range.
fn is_port(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::host_short_name;

    /// A stored alias wins even for local hosts and remains raw presentation input.
    ///
    /// The short form must never overwrite user intent, normalize an alias or
    /// escape it before the renderer can enforce its ordinary text boundary.
    #[test]
    fn explicit_alias_is_preserved() {
        for local in [false, true] {
            for alias in ["Build Box", "literal.example", "بناء\u{202e}<x>", ""] {
                assert_eq!(
                    host_short_name(local, Some("user@other.example"), Some(alias)),
                    alias
                );
            }
        }
        assert_eq!(host_short_name(true, None, None), "Local");
    }

    /// Destination decoration is removed before selecting a nonempty hostname label.
    ///
    /// Last-`@` splitting preserves OpenSSH user spellings; URI-only port
    /// handling leaves ordinary dot-less SSH configuration aliases unchanged.
    #[test]
    fn hostnames_use_the_first_label_after_destination_syntax() {
        for (destination, expected) in [
            ("build", "build"),
            ("build.example.com", "build"),
            ("user@build.example.com", "build"),
            ("first@last@build.example.com", "build"),
            ("ssh://user@build.example.com:2222", "build"),
            ("ssh://build:2222", "build"),
            ("gpubox", "gpubox"),
            ("gpubox:2222", "gpubox:2222"),
            (".example.com", ".example.com"),
            ("user@.example.com", ".example.com"),
        ] {
            assert_eq!(host_short_name(false, Some(destination), None), expected);
        }
        assert_eq!(host_short_name(false, None, None), "");
    }

    /// Addresses retain every segment, including URI ports and IPv6 zone spellings.
    ///
    /// Cutting an IPv4 address at its first dot or treating an IPv6 segment as
    /// a port would turn the caption into a different and misleading identity.
    #[test]
    fn addresses_remain_whole() {
        for (destination, expected) in [
            ("192.0.2.7", "192.0.2.7"),
            ("ssh://user@192.0.2.7:2222", "192.0.2.7"),
            ("fe80::1", "fe80::1"),
            ("user@::1", "::1"),
            ("ssh://[::1]:22", "::1"),
            ("[2001:db8::7]:2222", "2001:db8::7"),
            ("fe80::1%en0", "fe80::1%en0"),
            ("fe80::1%interface.100", "fe80::1%interface.100"),
            ("ssh://user@[fe80::1%en0]:22", "fe80::1%en0"),
        ] {
            assert_eq!(host_short_name(false, Some(destination), None), expected);
        }
    }
}
