//! What startup reads from the helm's host listing to tell whether the local supervisor connected, and if not, why it
//! never will; plus the escaping that makes host-supplied text safe to show in a native alert.
//!
//! Startup asks the embedded helm `GET /api/hosts` and looks only at the reserved local row: its connection phase
//! (connected, a protocol version mismatch, an identity that does not match the recorded one, or anything else). The
//! listing decodes into the shared [`HostView`] from `farhelm-proto`, the same type the helm serializes and the GPUI
//! app receives over the UI channel, so there is no second description of a host to drift. Strict decoding is safe
//! here because the desktop binary and the helm it embeds are always the same build.

use farhelm_proto::text::is_presentation_unsafe;
use farhelm_proto::ui_channel::HostView;
use serde::Deserialize;

/// `GET /api/hosts`'s body.
#[derive(Debug, Deserialize)]
struct HostListing {
    hosts: Vec<HostView>,
}

/// Whether `host` is the reserved row for the machine the app runs on, whose supervisor the desktop app manages.
pub(crate) fn is_this_machine(host: &HostView) -> bool {
    host.kind == "local"
}

/// Decode `GET /api/hosts`'s body.
pub(crate) async fn decode_hosts(resp: reqwest::Response) -> Result<Vec<HostView>, String> {
    resp.json::<HostListing>()
        .await
        .map(|listing| listing.hosts)
        .map_err(|e| e.to_string())
}

/// Show a host-supplied value with every presentation-unsafe character (controls, direction overrides, invisible
/// characters; the shared table in `farhelm_proto::text`) written as `<U+XXXX>`, and an empty or whitespace-only value
/// named rather than shown as nothing. Moved unchanged from the old UI's `peer.rs`.
pub(crate) fn display_peer(raw: &str) -> String {
    if raw.is_empty() {
        return "(empty)".to_string();
    }
    let mut shown = String::with_capacity(raw.len());
    // Whether anything the user can actually SEE survived. An escape counts
    // (it prints as `<U+202E>`); an ordinary space does not.
    let mut anything_visible = false;
    for ch in raw.chars() {
        if is_presentation_unsafe(ch) {
            shown.push_str(&format!("<U+{:04X}>", ch as u32));
            anything_visible = true;
        } else {
            anything_visible |= !ch.is_whitespace();
            shown.push(ch);
        }
    }
    if anything_visible {
        shown
    } else {
        format!("(whitespace only: {} characters)", raw.chars().count())
    }
}

/// Show an identity so that two different ones can never look alike: spaces and the characters that start an escape
/// or a placeholder are escaped too, on top of [`display_peer`]. Comparisons must still use the raw bytes. Moved
/// unchanged from the old UI's `peer.rs`.
pub(crate) fn display_identity(raw: &str) -> String {
    let escaped: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_graphic() && !matches!(ch, '<' | '(' | ')') {
                ch.to_string()
            } else {
                format!("<U+{:04X}>", ch as u32)
            }
        })
        .collect();
    // Keep the empty-value placeholder; all spaces have already gained a
    // visible spelling, including in an otherwise blank identity.
    display_peer(&escaped)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A host row as tests need it: only the kind and connection phase matter to startup, so every other field takes a
    /// fixed placeholder.
    pub(crate) fn host(kind: &str, state: farhelm_proto::ui_channel::HostStateView) -> HostView {
        HostView {
            id: 1,
            entry_uuid: String::new(),
            kind: kind.to_string(),
            is_session_owned: false,
            tensorlake: None,
            environment: None,
            destination: None,
            alias: None,
            name: "this machine".to_string(),
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            yolo_without_asking: false,
            commands_without_asking: false,
            own_sessions_without_asking: false,
            links_without_asking: false,
            local_links_without_asking: false,
            icon: Default::default(),
            color: Default::default(),
            state,
            incarnation: 0,
            trash_epoch: 0,
        }
    }

    /// The helm's pinned `GET /api/hosts` body, the same file the helm's own contract test holds its serializer to.
    const HOST_LIST_JSON: &str = include_str!("../../farhelm-helm/http-contract/host-list.json");

    /// Startup must decode the listing the helm really serves, or the desktop app refuses to start against its own
    /// helm. Decoding the helm's pinned contract fixture (rather than rows this test serialized itself) means a helm-side
    /// change to the shape fails here as well as in the helm's contract test.
    #[farhelm_testtrace::test]
    fn the_helms_pinned_listing_decodes_with_its_local_row() {
        let hosts = serde_json::from_str::<HostListing>(HOST_LIST_JSON)
            .unwrap()
            .hosts;
        assert_eq!(
            hosts.len(),
            12,
            "the fixture's rows, one per connection phase"
        );
        let local: Vec<_> = hosts.iter().filter(|host| is_this_machine(host)).collect();
        assert_eq!(local.len(), 1, "exactly one reserved local row");
        assert!(
            matches!(
                local[0].state,
                farhelm_proto::ui_channel::HostStateView::Connected { .. }
            ),
            "the local row's phase decodes as connected: {:?}",
            local[0].state
        );
    }

    /// Why this matters: startup's refusal asks the user to compare a recorded identity with the one the supervisor
    /// reports. Two identities that differ only by a character that renders as nothing (a variation selector, which the
    /// shared list leaves alone for emoji's sake) would look identical. Spec: identities escape every non-ASCII
    /// character, so such a pair renders differently, while a plain UUID renders as itself; spaces are visible even
    /// beside ordinary text. Ported from the old GUI's `peer.rs`.
    #[farhelm_testtrace::test]
    fn identities_that_differ_invisibly_render_differently() {
        let recorded = "8f1c2a7e-0000-4000-8000-000000000001";
        let reported = format!("{recorded}\u{FE0F}");
        // Premise: the ordinary peer escaping leaves the selector alone.
        assert_eq!(display_peer(&reported), reported);
        assert_eq!(display_identity(recorded), recorded);
        assert_ne!(display_identity(recorded), display_identity(&reported));
        assert!(display_identity(&reported).ends_with("<U+FE0F>"));
        assert_eq!(display_identity(""), "(empty)");
        assert_eq!(display_identity("   "), "<U+0020><U+0020><U+0020>");
        assert_eq!(
            display_identity(&format!("{recorded} ")),
            format!("{recorded}<U+0020>")
        );
        assert_eq!(
            display_identity(" install  id "),
            "<U+0020>install<U+0020><U+0020>id<U+0020>"
        );
    }

    /// Escape syntax and the empty marker must not hide a changed installation. The first two pairs used to produce
    /// identical labels; the third checks that escaping placeholder punctuation cannot introduce a new collision. The
    /// refusal shows these display values, while any comparison still uses the raw bytes. Ported from the old GUI's
    /// `peer.rs`.
    #[farhelm_testtrace::test]
    fn identity_escape_notation_and_empty_marker_cannot_be_impersonated() {
        for (raw, imitation) in [
            ("left right", "left<U+0020>right"),
            ("", "(empty)"),
            ("(install)", "<U+0028>install<U+0029>"),
        ] {
            assert_ne!(
                raw, imitation,
                "the fixture must compare distinct identities"
            );
            assert_ne!(
                display_identity(raw),
                display_identity(imitation),
                "distinct identities {raw:?} and {imitation:?} must stay distinguishable"
            );
        }
        assert_eq!(display_identity("(empty)"), "<U+0028>empty<U+0029>");
        assert_eq!(display_identity("<"), "<U+003C>");
        for host in [
            "example.com",
            "127.0.0.1",
            "[::1]",
            "sub-domain.example.org:8443",
        ] {
            assert_eq!(display_identity(host), host);
        }
    }

    /// Why this matters: one listed invisible (U+3164, a Hangul filler) is not whitespace, so a value made only of it
    /// used to render as a blank. Spec: such a value renders visibly escaped, and an ordinary emoji with a variation
    /// selector still renders as itself. Ported from the old GUI's `peer.rs`.
    #[farhelm_testtrace::test]
    fn newly_listed_invisibles_are_escaped_and_emoji_are_not() {
        assert_eq!(display_peer("\u{3164}"), "<U+3164>");
        assert_eq!(display_peer("a\u{034F}b\u{E0041}"), "a<U+034F>b<U+E0041>");
        assert_eq!(
            display_peer("ship it \u{2764}\u{FE0F}"),
            "ship it \u{2764}\u{FE0F}"
        );
    }

    /// Peer-supplied text must not be able to lay out the sentence around it: an RLO inside an identity reverses
    /// everything after it, and two identities that differ only by a zero-width joiner render identically. Every
    /// presentation-unsafe character shows as an escape, ordinary text is untouched, and strong right-to-left letters
    /// (legitimate text with no control character) pass through. Ported from the old GUI's `peer.rs`.
    #[farhelm_testtrace::test]
    fn directional_and_invisible_characters_are_escaped_rather_than_rendered() {
        for hostile in [
            "\u{202E}reversed",
            "id\u{200B}with-zero-width",
            "id\u{2066}isolated",
            "id\u{FEFF}bom",
            "id\u{0007}bell",
            "id\u{2028}line-separator",
        ] {
            let shown = display_peer(hostile);
            for ch in hostile.chars().filter(|ch| is_presentation_unsafe(*ch)) {
                assert!(
                    !shown.contains(ch),
                    "{hostile:?} must not render {ch:?} literally: {shown}"
                );
                assert!(
                    shown.contains(&format!("<U+{:04X}>", ch as u32)),
                    "{hostile:?} must show {ch:?} as an escape: {shown}"
                );
            }
        }
        assert_eq!(
            display_peer("d1444cac-789f-4264-879c-38010250630a"),
            "d1444cac-789f-4264-879c-38010250630a"
        );
        assert_eq!(
            display_peer("\u{05e9}\u{05dc}\u{05d5}\u{05dd}"),
            "\u{05e9}\u{05dc}\u{05d5}\u{05dd}"
        );
    }

    /// An identity that is empty, or nothing but whitespace, must render as something a person can see and name, so
    /// the refusal never compares a gap with a gap. A value whose only content is an escape is visible because of the
    /// escape. Ported from the old GUI's `peer.rs`.
    #[farhelm_testtrace::test]
    fn empty_and_blank_identities_get_an_unambiguous_display_form() {
        assert_eq!(display_peer(""), "(empty)");
        assert_eq!(display_peer("   "), "(whitespace only: 3 characters)");
        assert_eq!(
            display_peer("\u{00A0}"),
            "(whitespace only: 1 characters)",
            "a non-breaking space is still invisible, and is still whitespace"
        );
        assert_eq!(display_peer("\u{202E}"), "<U+202E>");
    }

    /// Host text reaches a native alert, so hidden and direction-changing characters must be visible and an identity
    /// must not hide its spaces.
    #[farhelm_testtrace::test]
    fn host_text_is_shown_with_hidden_characters_escaped() {
        assert_eq!(display_peer("v1\u{202e}2"), "v1<U+202E>2");
        assert_eq!(display_peer(""), "(empty)");
        assert_eq!(display_peer("  "), "(whitespace only: 2 characters)");
        assert_eq!(display_identity("a b"), "a<U+0020>b");
    }
}
