//! Showing text this UI did not write: escaping, and per-value direction
//! isolation.
//!
//! The boundary this module draws is AUTHORSHIP, not distance: on one side
//! the sentences this UI writes as literals, on the other every value it
//! merely relays. Identities, build strings, transport errors, remediations
//! and ssh destinations are all the second kind. Most of them do come from
//! another machine — under `--ssh`, a genuinely different one — but some do
//! not: the helm's own build header, a reqwest error this process generated
//! itself, and this client's compiled-in build stamp all travel as relayed
//! values too. They are treated identically on purpose. The renderer cannot
//! verify provenance, so a rule that relaxed for "our own" values would be a
//! rule that relaxes whenever someone mislabels one.
//!
//! Dioxus interpolation already makes all of it inert as MARKUP (values
//! become text nodes, never parsed HTML), so the risk left is visual: a bidi
//! override inside an identity can reorder the sentence around it and make an
//! adopt button approve one install while appearing to name another.
//!
//! Two layers answer that, and neither is sufficient alone:
//!
//! - [`display_peer`] escapes every directional and invisible control into
//!   a visible `<U+XXXX>` form. That is a KNOWN-BAD list (see
//!   [`must_escape`]), so it cannot be the whole defence.
//! - [`DetailPart`] splits a sentence into this UI's own runs and the
//!   relayed ones, and [`PeerLine`] renders each relayed run into its own
//!   direction-isolated element. That is category-free: it bounds strong-RTL
//!   letters, which carry no control character for an escape rule to catch.
//!
//! Both layers are applied at RENDER time, which is what [`DetailPart::Peer`]
//! carrying its value raw is for — the escaping cannot be baked in earlier
//! without losing the ability to isolate each value as its own element. What
//! holds instead is narrower and is the invariant that matters: the adopt
//! request body is the only place a relayed value is sent BACK raw, because
//! it is a comparison the helm performs rather than anything a person reads.
//! Everything a user sees has been through [`display_peer`].
//!
//! ## Why this is its own module
//!
//! Nothing here is host-specific. The hosts panel is the heaviest user, but
//! the session list, the session view's stale-host notice and the build-skew
//! prompt all build sentences out of the same runs — any surface that mixes
//! our words with someone else's belongs here. Keeping the primitives beside
//! the hosts panel made that shared status look incidental, and made the
//! panel the place you had to read to understand a rule that governs the
//! whole UI.

use dioxus::prelude::*;

/// Whether this character must be shown as an escape rather than rendered.
///
/// Three groups, all invisible or layout-affecting and none of them
/// legitimate inside an identity, a build string, or an ssh destination:
///
/// - The C0/C1 control ranges and DEL — including the line and paragraph
///   separators, which would break a value across lines mid-sentence.
/// - The bidirectional formatting controls. These are the spoofing vector:
///   an RLO inside an identity reverses the text that FOLLOWS it, so
///   "recorded X, reported Y" can be made to read with the two swapped
///   while the underlying values are unchanged.
/// - The zero-width and joining characters, which let two different
///   identities render identically.
///
/// Enumerated rather than derived from Unicode's general categories, because
/// this crate carries no Unicode tables and pulling one in for a display
/// rule would be a large dependency for a small list. That makes the set a
/// KNOWN-BAD list rather than a proof: a future format character outside it
/// would render as itself. The direction-isolated element each value sits in
/// (see [`DetailPart`]) is the second, category-free layer that bounds the
/// damage either way.
fn must_escape(ch: char) -> bool {
    ch.is_control()
        || matches!(ch,
            '\u{061C}'                    // Arabic letter mark
            | '\u{200E}' | '\u{200F}'     // LTR/RTL marks
            | '\u{202A}'..='\u{202E}'     // embeddings and overrides
            | '\u{2066}'..='\u{2069}'     // isolates
            | '\u{00AD}'                  // soft hyphen
            | '\u{200B}'..='\u{200D}'     // zero-width space/non-joiner/joiner
            | '\u{2060}'                  // word joiner
            | '\u{180E}'                  // Mongolian vowel separator
            | '\u{FEFF}'                  // zero-width no-break space / BOM
            | '\u{2028}' | '\u{2029}'     // line and paragraph separators
        )
}

/// One peer-supplied value as it should be SHOWN.
///
/// Escapes everything [`must_escape`] rejects, and gives the two degenerate
/// values an unambiguous form of their own. That last part is not cosmetic:
/// an identity that renders as nothing at all appears in the adopt button as
/// `adopt ` and in the mismatch evidence as a gap, so a user is asked to
/// approve something they cannot see. Saying `(empty)` is the difference
/// between an odd-looking prompt and an invisible one.
///
/// The result is for DISPLAY only. Everything that has to match on the far
/// end — the adopt request's `reported`, above all — uses the raw value.
pub(crate) fn display_peer(raw: &str) -> String {
    if raw.is_empty() {
        return "(empty)".to_string();
    }
    let mut shown = String::with_capacity(raw.len());
    // Whether anything the user can actually SEE survived. An escape counts
    // (it prints as `<U+202E>`); an ordinary space does not.
    let mut anything_visible = false;
    for ch in raw.chars() {
        if must_escape(ch) {
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

/// One run of a rendered detail: either this UI's own words, or a value that
/// came from somewhere else.
///
/// The split exists so that a peer value can never lay out the sentence
/// AROUND it. Each `Peer` run is rendered into its own element carrying
/// `dir="ltr"` and CSS `unicode-bidi: isolate`, which bounds directional
/// resolution to that element — so "recorded as install X; now reports Y"
/// keeps X and Y on the sides the labels put them on, whatever they contain.
/// Concatenating everything into one string and interpolating it, which is
/// what this replaced, gives that guarantee up: the escaping in
/// [`display_peer`] would still hold, but only for the control characters it
/// knows about, and strong-RTL letters need no control character at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DetailPart {
    /// This UI's own wording — trusted, and never escaped.
    Text(String),
    /// A value from the helm, a supervisor, or the registry. Escaped and
    /// isolated when rendered; carried raw here.
    Peer(String),
}

impl DetailPart {
    /// Our own words.
    ///
    /// `pub(crate)` alongside its sibling because the build-skew notice
    /// (`skew`) is a second surface built out of these runs: it mixes this
    /// UI's sentence with a version string the helm supplied, which is
    /// exactly the shape this type exists for.
    pub(crate) fn text(words: impl Into<String>) -> DetailPart {
        DetailPart::Text(words.into())
    }

    /// Someone else's.
    pub(crate) fn peer(value: impl Into<String>) -> DetailPart {
        DetailPart::Peer(value.into())
    }
}

/// A detail as one flat string, for tests.
///
/// Deliberately not used for rendering: flattening throws away exactly the
/// per-value isolation [`DetailPart`] exists to provide, so a renderer that
/// reached for this would be undoing the guarantee while looking tidier.
///
/// `pub(crate)` rather than private because the surfaces that BUILD these
/// runs live in other modules — `hosts` above all — and their tests assert
/// on the sentence a phase produces, not on the runs it happens to be cut
/// into.
#[cfg(test)]
pub(crate) fn detail_text(parts: &[DetailPart]) -> String {
    parts
        .iter()
        .map(|part| match part {
            DetailPart::Text(text) => text.clone(),
            DetailPart::Peer(value) => display_peer(value),
        })
        .collect()
}

/// Render a detail: our words as plain runs, every peer value isolated in
/// its own fixed-direction element.
///
/// `dir="ltr"` AND the stylesheet's `unicode-bidi: isolate` together are
/// what make the isolation real — the attribute sets the base direction, the
/// property stops the run participating in the surrounding paragraph's
/// bidirectional resolution at all.
#[component]
pub(crate) fn PeerLine(class: String, parts: Vec<DetailPart>) -> Element {
    rsx! {
        div { class: "{class}",
            for (index , part) in parts.iter().enumerate() {
                match part {
                    DetailPart::Text(text) => rsx! {
                        span { key: "{index}", "{text}" }
                    },
                    DetailPart::Peer(value) => rsx! {
                        span {
                            key: "{index}",
                            class: "peer-value",
                            dir: "ltr",
                            "{display_peer(value)}"
                        }
                    },
                }
            }
        }
    }
}

/// Render peer-authored multiline text without surrendering its line shape.
///
/// The newlines are structural and trusted: the server uses them to separate
/// the action a user is about to authorize. Each line's contents are still a
/// peer value, escaped and direction-isolated independently so one hostile
/// path cannot reorder another line or the confirmation controls around it.
#[component]
pub(crate) fn PeerBlock(class: String, text: String) -> Element {
    let lines = text.lines().collect::<Vec<_>>();
    let last = lines.len().saturating_sub(1);
    rsx! {
        pre { class: "{class}",
            for (index, line) in lines.iter().enumerate() {
                span {
                    key: "{index}",
                    class: "peer-value peer-line",
                    dir: "ltr",
                    "{display_peer(line)}"
                }
                if index != last {
                    br {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Peer-supplied text must not be able to lay out the sentence around
    /// it. The escaping half is asserted here; the isolation half is
    /// structural (each value is its own `Peer` run — asserted at a surface
    /// that BUILDS the runs, `hosts`'s
    /// `the_mismatch_evidence_keeps_each_identity_in_its_own_run`).
    ///
    /// Directional overrides are the concrete attack: an RLO inside an
    /// identity reverses everything after it, so "recorded A, reported B"
    /// can be made to read as its own opposite while the values are
    /// untouched. Zero-width characters are the quieter one: two identities
    /// that differ only by a zero-width joiner render identically.
    #[test]
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
            for ch in hostile.chars().filter(|ch| must_escape(*ch)) {
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
        // Ordinary text is untouched: escaping that mangled real identities
        // would make the panel unreadable for the 99% case.
        assert_eq!(
            display_peer("d1444cac-789f-4264-879c-38010250630a"),
            "d1444cac-789f-4264-879c-38010250630a"
        );
        // Strong-RTL letters carry no control character and are legitimate
        // text, so they pass through — the per-value isolation is what
        // bounds THEIR reordering, not the escaping.
        assert_eq!(display_peer("שלום"), "שלום");
    }

    /// An identity that is empty, or nothing but whitespace, must render as
    /// something a person can see and name.
    ///
    /// The failure this prevents is an adopt button reading `adopt ` and a
    /// mismatch whose two sides are a gap and a gap: the user is asked to
    /// approve something invisible, which is worse than an ugly label.
    #[test]
    fn empty_and_blank_identities_get_an_unambiguous_display_form() {
        assert_eq!(display_peer(""), "(empty)");
        assert_eq!(display_peer("   "), "(whitespace only: 3 characters)");
        assert_eq!(
            display_peer("\u{00A0}"),
            "(whitespace only: 1 characters)",
            "a non-breaking space is still invisible, and is still whitespace"
        );
        // A value whose only content is an escape is visible BECAUSE of the
        // escape, so it renders as itself rather than as the blank form.
        assert_eq!(display_peer("\u{202E}"), "<U+202E>");
    }

    /// A confirmation keeps trusted newlines while every hostile character
    /// inside those lines becomes visible rather than controlling layout.
    #[test]
    fn multiline_peer_text_preserves_lines_and_escapes_each_value() {
        let raw = "write /home/alice\u{202E}/unit\nrestart service\u{200B}";
        let shown = raw.lines().map(display_peer).collect::<Vec<_>>();

        assert_eq!(shown.len(), 2);
        assert_eq!(shown[0], "write /home/alice<U+202E>/unit");
        assert_eq!(shown[1], "restart service<U+200B>");
    }
}
