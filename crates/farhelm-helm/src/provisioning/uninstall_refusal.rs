//! Uninstall remedies describe the host the user sees, without exposing state-machine labels.
//!
//! These functions choose words only. The service retains every admission check and
//! decides which refusals carry the typed permission to offer Remove instead.

use crate::manager::{HostState, peer_text};
use crate::store::HostKind;

const REMOVE_INSTEAD: &str =
    "To stop using the host without uninstalling, remove it from the list instead";

/// Explain why an answering host cannot be checked before uninstalling it.
///
/// Only these states offer Remove: no usable connection exists through this
/// entry, and removing the entry leaves the host untouched. A twin's name is a
/// fresh display-name lookup, never an authority to uninstall through that entry.
/// Its kind selects advice the other entry can actually carry out: the local
/// host has no uninstall action in the panel.
pub(super) fn unusable_host(state: &HostState, twin: Option<(&str, HostKind)>) -> Option<String> {
    let reason = match state {
        HostState::VersionSkew {
            peer_protocol,
            peer_build,
            our_protocol,
            our_build,
            ..
        } => {
            let host = peer_text(peer_build);
            let helm = peer_text(our_build);
            if peer_protocol < our_protocol {
                format!(
                    "Farhelm can't uninstall from this host yet: it runs farhelm {host}, which is \
                     too old for this helm ({helm}) to talk to, so Farhelm can't check whether any \
                     of its sessions are still running. Update the host first (its update button \
                     in the host list), then choose uninstall again"
                )
            } else {
                // A development or prerelease label is not a stable upgrade target.
                // Parse the original build, before presentation escaping changes it.
                let target = semver::Version::parse(peer_build)
                    .ok()
                    .filter(|version| version.pre.is_empty() && version.build.is_empty());
                let update = match target {
                    Some(_) => format!("Update this helm to farhelm {host} or later"),
                    None => "Update this helm".to_string(),
                };
                format!(
                    "Farhelm can't uninstall from this host: it runs farhelm {host}, which is \
                     newer than this helm ({helm}), and this helm can't talk to it, so it can't \
                     check whether any of its sessions are still running. {update}, or uninstall \
                     from a helm that already runs that version"
                )
            }
        }
        HostState::IdentityMismatch { recorded, reported } => format!(
            "Farhelm can't uninstall from this host: the Farhelm install at its ssh destination \
             is not the one this entry was set up with (identity {} on record, {} now), for \
             example because the host was reinstalled or the destination now reaches another \
             machine. If it is the machine you mean, choose adopt in the host's menu, then \
             uninstall again; otherwise correct the destination in the host's settings",
            peer_text(recorded),
            peer_text(reported),
        ),
        HostState::IdentityUnverified { .. } => {
            "Farhelm can't uninstall from this host: something answers at its ssh destination \
             but doesn't say which Farhelm install it is, so Farhelm can't confirm it is the \
             host you set up or check its sessions. Farhelm keeps checking, and this clears \
             once the host identifies itself; if the destination is wrong, correct it in the \
             host's settings"
                .to_string()
        }
        HostState::Duplicate { twin: owner, .. } => {
            let other = twin
                .map(|(name, kind)| {
                    // Match the GUI's label for the reserved default name;
                    // other names keep the user's alias.
                    let name = match (kind, name) {
                        (HostKind::Local, "this machine") => "local (this machine)",
                        _ => name,
                    };
                    peer_text(name)
                })
                .unwrap_or_else(|| format!("another entry (host {owner})"));
            let uninstall = match twin {
                Some((_, HostKind::Local)) => "To remove Farhelm from the helm's own machine, \
                    run farhelm uninstall there"
                    .to_string(),
                Some((_, HostKind::Ssh)) => format!(
                    "To remove Farhelm from that machine, choose uninstall on {other}, \
                     then remove this entry"
                ),
                // No twin means the registry no longer holds the entry that owned
                // the machine: it was removed, and this entry stays a duplicate
                // until Retry lets it connect (SPEC.md's duplicate remedy). Pointing
                // the user at the removed entry would be advice they cannot follow.
                // Uninstall's ordinary checks still apply after the retry, so this
                // promises only that uninstall can be chosen again, not that it
                // will succeed.
                None => "That entry has since been removed: press retry so this entry \
                    can connect to the machine, then choose uninstall on this entry"
                    .to_string(),
            };
            format!(
                "Farhelm can't uninstall through this entry: it reaches the same machine as \
                 {other}, so nothing is connected through this one. {uninstall}. If this \
                 entry should point somewhere else, change its ssh destination in its settings \
                 and press retry"
            )
        }
        _ => return None,
    };
    Some(format!("{reason}. {REMOVE_INSTEAD}"))
}

/// Keep connection advice independent of internal retry phases and their spelling.
pub(super) fn not_connected() -> String {
    "Farhelm can't uninstall from this host while it isn't connected, because it has to check \
     that none of its sessions are running. Once the host is connected, choose uninstall again"
        .to_string()
}

/// A fresh probe must identify the machine whose sessions were checked earlier.
/// Missing identities remain explicit; this refusal offers Retry rather than
/// implying that the install can be adopted from inside the uninstall dialog.
pub(super) fn changed_identity(recorded: Option<&str>, reported: Option<&str>) -> String {
    let reported = reported
        .map(peer_text)
        .unwrap_or_else(|| "no identity".into());
    let recorded = recorded
        .map(peer_text)
        .unwrap_or_else(|| "no identity".into());
    format!(
        "Farhelm can't uninstall from this host: the Farhelm answering at its ssh destination \
         reports {reported}, but this entry has {recorded} on record, so Farhelm can't tell it \
         is the same machine whose sessions it just checked. Choose retry in the host's menu \
         and follow what the host list then says, then choose uninstall again"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both protocol directions explain which side to update; development
    /// builds must never become a fictitious stable upgrade target.
    #[farhelm_testtrace::test]
    fn version_refusals_name_both_builds_and_the_correct_update() {
        for (peer, ours, build, remedy) in [
            (1, 2, "1.2.3", "Update the host first"),
            (
                3,
                2,
                "1.2.3",
                "Update this helm to farhelm \"1.2.3\" or later",
            ),
            (3, 2, "0.0.0-unreleased", "Update this helm, or uninstall"),
            (3, 2, "1.2.3-rc.1", "Update this helm, or uninstall"),
            (3, 2, "1.2.3+dev", "Update this helm, or uninstall"),
            (3, 2, "unknown\nversion", "Update this helm, or uninstall"),
        ] {
            let state = HostState::VersionSkew {
                peer_protocol: peer,
                peer_build: build.into(),
                our_protocol: ours,
                our_build: "0.9.0".into(),
                remediation: String::new(),
            };
            let text = unusable_host(&state, None).unwrap();
            assert!(text.contains(&peer_text(build)), "{text}");
            assert!(text.contains("0.9.0") && text.contains(remedy), "{text}");
            assert_plain_remove_refusal(&text);
            assert!(!text.contains('\n'));
        }
    }

    /// An identity mismatch permits an explicit adoption; an unidentified
    /// install does not. Peer bytes must not disguise the identities quoted.
    #[farhelm_testtrace::test]
    fn identity_refusals_keep_their_distinct_remedies_and_escape_peer_text() {
        let mismatch = unusable_host(
            &HostState::IdentityMismatch {
                recorded: "old\nidentity".into(),
                reported: "new\u{202e}".into(),
            },
            None,
        )
        .unwrap();
        assert!(
            mismatch.contains(r#""old\nidentity""#) && mismatch.contains(r#""new\u{202e}""#),
            "{mismatch}"
        );
        assert!(!mismatch.contains('\n') && !mismatch.contains('\u{202e}'));
        assert!(mismatch.contains("choose adopt") && mismatch.contains("settings"));
        assert_plain_remove_refusal(&mismatch);
        let unverified = unusable_host(
            &HostState::IdentityUnverified {
                recorded: "old".into(),
            },
            None,
        )
        .unwrap();
        assert!(unverified.contains("keeps checking") && unverified.contains("settings"));
        assert!(!unverified.contains("adopt"));
        assert_plain_remove_refusal(&unverified);
    }

    /// The entry to uninstall through is named as in the host list. Once that
    /// entry is gone, the refusal still names it by a fallback, but must not
    /// send the user to it: the remedy that works is Retry on this entry,
    /// then uninstall here.
    #[farhelm_testtrace::test]
    fn twin_refusals_name_the_other_entry_or_its_missing_entry_fallback() {
        let state = HostState::Duplicate {
            twin: 42,
            identity: "shared".into(),
        };
        let text = unusable_host(&state, Some(("build\nbox", HostKind::Ssh))).unwrap();
        assert!(
            text.contains(r#"choose uninstall on "build\nbox""#),
            "{text}"
        );
        assert!(text.contains("press retry"));
        // The shared closing sentence also says "press retry", so pin the
        // removed-entry advice's absence directly.
        assert!(!text.contains("has since been removed"), "{text}");
        assert_plain_remove_refusal(&text);

        let removed = unusable_host(&state, None).unwrap();
        assert!(
            removed.contains("same machine as another entry (host 42)"),
            "{removed}"
        );
        assert!(
            removed.contains("press retry so this entry can connect to the machine, then choose uninstall on this entry"),
            "{removed}"
        );
        assert!(
            !removed.contains("choose uninstall on another"),
            "{removed}"
        );
        assert_plain_remove_refusal(&removed);
        for (name, shown) in [("desk", "desk"), ("this machine", "local (this machine)")] {
            let local = unusable_host(&state, Some((name, HostKind::Local))).unwrap();
            assert!(
                local.contains(&format!("same machine as \"{shown}\"")),
                "{local}"
            );
            assert!(local.contains("run farhelm uninstall there"), "{local}");
            assert!(!local.contains("choose uninstall on"), "{local}");
            assert_plain_remove_refusal(&local);
        }
    }

    /// Plain refusals explain reconnection without granting the Remove
    /// remedy reserved for the five answering-but-unusable situations.
    #[farhelm_testtrace::test]
    fn neighbouring_refusals_explain_connection_and_identity_recovery() {
        assert!(not_connected().contains("Once the host is connected"));
        for (recorded, reported) in [(Some("old"), None), (None, Some("new\nidentity"))] {
            let text = changed_identity(recorded, reported);
            assert!(text.contains("no identity") && text.contains("Choose retry"));
            assert!(!text.contains('\n'));
            assert!(!text.contains("remove it"));
        }
    }

    /// No state-machine token should leak back into the explanatory dialog.
    fn assert_plain_remove_refusal(text: &str) {
        for token in [
            "version-skew",
            "identity-mismatch",
            "identity-unverified",
            "duplicate",
            "phase",
            "supervisor answers there",
        ] {
            assert!(!text.contains(token), "{text}");
        }
        assert!(text.starts_with("Farhelm") && text.ends_with(REMOVE_INSTEAD));
    }
}
