//! Browser requests cross from an untrusted session host to the user's machine.
//!
//! Validate and classify here, before an approval can be offered. Classification is lexical:
//! no DNS lookup can establish what a domain will resolve to later on the user's network.
//! A native opener exists only when the desktop shell embeds the helm; server helms leave
//! opening to the user's browser click and must never open on their own server machine.

use std::net::Ipv4Addr;
use std::sync::Arc;

use farhelm_proto::approvals::ApprovalAction;
use farhelm_proto::{AgentReply, ErrorKind};

use crate::AppState;
use crate::agent_requests::AgentOrigin;
use crate::store::BrowserPermission;

/// Only the desktop shell supplies this capability. The callback returns after the operating
/// system accepted the request; it cannot promise that the destination finished loading.
pub type LinkOpener = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// Bound both the data shown on a card and the string handed to the native opener.
const MAX_URL_BYTES: usize = 4096;

/// Validate the same raw-string rule as the terminal's isPlainWebUrl, preserving the URL.
///
/// WHATWG parsing alone repairs missing separators and strips whitespace. Reject those inputs
/// before parsing, so an approval describes exactly what would open. The host category is a
/// permission decision, not evidence that a name can never resolve to a private address.
pub(crate) fn permission_for_url(raw: &str) -> Result<BrowserPermission, &'static str> {
    if raw.len() > MAX_URL_BYTES {
        return Err("the URL is too long (maximum 4096 bytes)");
    }
    if !(raw
        .get(..7)
        .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
        || raw
            .get(..8)
            .is_some_and(|s| s.eq_ignore_ascii_case("https://")))
    {
        return Err("only absolute http or https URLs with a host can be opened");
    }
    // JavaScript's \s includes BOM but not Unicode NEXT LINE. Spell out that exact
    // set rather than Rust's slightly different char::is_whitespace classification.
    if raw.chars().any(|c| {
        matches!(c,
            '\u{0}'..='\u{20}' | '\u{7f}' | '\u{a0}' | '\u{1680}' |
            '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
            '\u{205f}' | '\u{3000}' | '\u{feff}'
        )
    }) {
        return Err("URLs with raw whitespace or control characters cannot be opened");
    }
    let parsed = url::Url::parse(raw).map_err(|_| "the URL is malformed")?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none() {
        return Err("only absolute http or https URLs with a host can be opened");
    }
    let local = match parsed.host().expect("host checked above") {
        url::Host::Ipv4(ip) => local_ipv4(ip),
        url::Host::Ipv6(ip) => {
            ip.to_ipv4_mapped().is_some_and(local_ipv4)
                || ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
        }
        url::Host::Domain(name) => {
            let name = name.trim_end_matches('.');
            !name.contains('.')
                || ["localhost", "local", "internal", "lan", "home", "home.arpa"]
                    .iter()
                    .any(|suffix| name == *suffix || name.ends_with(&format!(".{suffix}")))
        }
    };
    Ok(if local {
        BrowserPermission::LocalNetwork
    } else {
        BrowserPermission::Public
    })
}

/// Non-public IPv4 literals, including short/hex forms after WHATWG normalization.
fn local_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || a == 0
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
}

/// Hold a validated link for approval, then open only through the desktop's capability.
///
/// Web Allow/Open already ran synchronously in the user's click handler. A native failure is
/// returned to the requesting CLI; no URL or opener error is written to the session's terminal
/// except the CLI's own short refusal.
pub(crate) async fn open_url(
    state: &AppState,
    origin: AgentOrigin,
    session: &str,
    url: String,
) -> anyhow::Result<AgentReply> {
    let permission = permission_for_url(&url).map_err(|message| {
        anyhow::Error::new(crate::SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::InvalidRequest,
            message: message.to_string(),
        })
    })?;
    crate::approvals::ask(
        state,
        origin,
        session,
        ApprovalAction::OpenUrl {
            url: url.clone(),
            local_network: permission == BrowserPermission::LocalNetwork,
            preapproved: false,
        },
    )
    .await?;
    if !crate::agent_requests::origin_is_live(state, origin) {
        anyhow::bail!("the requesting host's connection changed before the link could open; retry");
    }
    if let Some(opener) = state.link_opener.clone() {
        tokio::task::spawn_blocking(move || opener(&url))
            .await?
            .map_err(|_| anyhow::anyhow!("the default browser could not open the link"))?;
    }
    Ok(AgentReply::UrlOpened {})
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parsing must not repair a request into a different approval target. Match the terminal
    /// link predicate's raw-input exclusions, including BOM whitespace and case-insensitive schemes.
    #[farhelm_testtrace::test]
    fn browser_url_rule_preserves_the_terminal_link_boundary() {
        for url in [
            "https://example.test/path?q=1#part",
            "HTTP://example.test/",
            "https://example.test/%20",
            "https://example.test/\u{85}",
        ] {
            assert_eq!(
                permission_for_url(url),
                Ok(BrowserPermission::Public),
                "{url:?}"
            );
        }
        for url in [
            "//example.test/",
            "https:example.test",
            "http:/example.test",
            "file:///tmp/example",
            "javascript:alert(1)",
            "https://",
            "https://example.test:bad/",
            " https://example.test/",
            "https://example.test/\nnext",
            "https://example.test/\u{feff}",
            "https://example.test/\u{7f}",
        ] {
            assert!(permission_for_url(url).is_err(), "{url:?}");
        }
        assert!(
            permission_for_url(&format!(
                "https://example.test/{}",
                "x".repeat(MAX_URL_BYTES)
            ))
            .is_err()
        );
    }

    /// Classify the browser's parsed host, including unusual IPv4 spellings and mapped IPv6,
    /// rather than letting a public-looking raw spelling use the public grant for a local target.
    #[farhelm_testtrace::test]
    fn browser_local_network_links_never_use_the_public_grant() {
        for host in [
            "localhost",
            "nested.localhost.",
            "printer",
            "printer.local",
            "router.home.arpa",
            "box.internal",
            "box.lan",
            "127.1",
            "0x7f000001",
            "10.1.2.3",
            "172.31.2.3",
            "192.168.1.2",
            "169.254.1.1",
            "100.64.1.1",
            "0.1.2.3",
            "224.0.0.1",
            "[::1]",
            "[::]",
            "[fd00::1]",
            "[fe80::1]",
            "[::ffff:127.0.0.1]",
        ] {
            assert_eq!(
                permission_for_url(&format!("https://{host}/")),
                Ok(BrowserPermission::LocalNetwork),
                "{host}"
            );
        }
        for host in [
            "example.test",
            "localhost.example.test",
            "8.8.8.8",
            "[2001:4860:4860::8888]",
            "127.0.0.1@example.test",
        ] {
            assert_eq!(
                permission_for_url(&format!("https://{host}/")),
                Ok(BrowserPermission::Public),
                "{host}"
            );
        }
    }
}
