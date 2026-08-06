//! What the session list SHOWS, derived from what the helm sent: the
//! optimistic-rename overlay, and the count banner above the rows.
//!
//! Everything here is a pure function of a listing reply plus `ListView`'s
//! own pending corrections — no signals, no rendering, no I/O — which is why
//! it can be unit-tested at all. `list` keeps the component, the state, and
//! the event handlers; this module keeps the decisions those handlers and
//! that render are made OF.
//!
//! ## The optimistic-rename bargain
//!
//! A title refreshed only by the 3-second poll would take up to a full
//! interval to show a user the result of their own rename, so a landed
//! rename is painted over the server's rows immediately (PLAN_M5.md item 6)
//! — the same bargain `tabs::visible_tabs` makes for tabs. What makes it
//! safe rather than a second source of truth is the pairing:
//! [`apply_optimistic_renames`] paints at RENDER time and never mutates the
//! stored listing, and [`prune_optimistic_renames`] retires each correction
//! the moment a reply that could have seen it arrives. Neither is meaningful
//! without the other, which is why they live together.
//!
//! ## The banner is not decoration
//!
//! [`count_banner`] carries PLAN_M2.md acceptance 5: a list the client could
//! not walk to the end must SAY it did not, in the same place a complete one
//! says it did. The wording is pinned by the browser suite, so treat the
//! strings as a contract rather than as copy.

use std::collections::HashMap;

use crate::Session;
use crate::api::SessionListing;

/// The listing as it should be RENDERED: the server's rows with this
/// view's own just-landed renames painted over them (PLAN_M5.md item 6).
///
/// The same optimistic-rendering bargain `tabs::visible_tabs` makes, for
/// the same reason — a title refreshed only by a 3-second poll would take
/// up to a full interval to show the user the result of their own rename —
/// and applied at RENDER time rather than by mutating the stored listing,
/// so the correction cannot outlive `prune_optimistic_renames`' judgement
/// about it. A rename for an id the listing does not carry is simply not
/// applied: there is no row to paint, and inventing one would claim a
/// session the server did not list.
///
/// Only the title is overridden. Everything else in the row — status,
/// annotation, tabs — is whatever the listing says, because a rename
/// changes nothing else about a session and a stale copy of those fields
/// is exactly what the poll exists to replace.
pub(crate) fn apply_optimistic_renames(
    sessions: &[Session],
    renamed: &HashMap<String, (String, u64)>,
) -> Vec<Session> {
    sessions
        .iter()
        .map(|session| match renamed.get(&session.id) {
            Some((title, _)) => Session {
                title: title.clone(),
                ..session.clone()
            },
            None => session.clone(),
        })
        .collect()
}

/// Retire the optimistic renames this listing reply settles, leaving the
/// ones it says nothing about.
///
/// `index` is the reply's own poll sequence number and is the whole point
/// of the exercise: a reply that STARTED before the rename's own response
/// completed is not evidence about it either way, so its old title cannot
/// be read as the server disagreeing. Without that distinction "the server
/// disagrees" and "the server has not told this client yet" look
/// identical, and the row would flip back to the old title until the next
/// poll — the wobble this scheme exists to prevent
/// (`session_view::SessionView`'s `opened_tabs` carries the same argument
/// for tabs).
///
/// The comparison is a CONSERVATIVE bound, not a claim about when the
/// server changed: the durable write lands before the rename's reply is
/// read, so a poll launched earlier may perfectly well observe the new
/// title. That only ever makes this hold a correction slightly longer than
/// strictly necessary, which is the harmless direction.
///
/// Three outcomes, in the order they are decided:
///
/// - The server now reports the same title: the rename graduated, and the
///   correction has nothing left to correct.
/// - This reply is one that is GUARANTEED to postdate the rename and it
///   reports something else — a different title, or no such session at
///   all: the server is authoritative and wins, whether that is another
///   client's later rename or this view being wrong about what landed.
/// - This reply may predate the rename: keep the correction untouched.
pub(crate) fn prune_optimistic_renames(
    renamed: &mut HashMap<String, (String, u64)>,
    server: &[Session],
    index: u64,
) {
    renamed.retain(|id, (title, observed_from)| {
        match server.iter().find(|session| &session.id == id) {
            Some(session) if &session.title == title => false,
            _ => index < *observed_from,
        }
    });
}

/// The count line above the rows, decided but not rendered.
///
/// The wording is split across two FIELDS rather than concatenated into one
/// string, because the renderer emits them as two separate text runs inside
/// the single banner element. Collapsing them into one `text` would fuse
/// those runs and change the DOM the browser suite reads. `class` carries
/// which of the two banners this is; the caller interpolates all three
/// verbatim.
pub(crate) struct CountBanner {
    /// The banner element's full class attribute — the variant is part of
    /// it (`truncation-banner` vs. `session-count`), and the stylesheet and
    /// the browser suite both select on that.
    pub(crate) class: &'static str,
    /// The count sentence itself.
    pub(crate) text: String,
    /// The incoherence suffix, when there is one to add. `None` is the
    /// common case and renders no node at all rather than an empty one.
    pub(crate) incoherence: Option<&'static str>,
}

/// What the count line says about this listing.
///
/// The count ALWAYS renders. PLAN_M2.md acceptance 5 asks for the shortfall
/// to be shown rather than plumbed, and a line that appears only when
/// something is wrong makes its own absence carry meaning nobody can read: a
/// user seeing no banner cannot tell "this is all of them" from "this UI
/// forgot to say".
///
/// The WORDING is what varies, and "showing N of M" is reserved for a walk
/// that did not finish — the client walks the cursor to exhaustion
/// (`api::fetch_sessions`), so an incomplete list means a ceiling was hit,
/// the helm reported more behind its last page, or the counts came back
/// incoherent. Both count conditions are checked because they can disagree:
/// totals can differ under concurrent creation without `truncated` being
/// set, and a ceiling sets `truncated` without the totals having to differ.
pub(crate) fn count_banner(listing: &SessionListing) -> CountBanner {
    if listing.truncated || (listing.sessions.len() as u64) < listing.total {
        CountBanner {
            class: "banner truncation-banner",
            text: format!(
                "showing {} of {} sessions",
                listing.sessions.len(),
                listing.total
            ),
            // Named separately from a plain shortfall: the rows and the
            // count disagree in a direction that means the list CHANGED
            // under the walk, not that there is more of it.
            incoherence: listing
                .incoherent
                .then_some(" — the list changed while it was being read; refreshing"),
        }
    } else {
        CountBanner {
            class: "banner session-count",
            text: format!("{} sessions", listing.total),
            incoherence: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionStatus;

    /// A session with the given id and title; every other field is
    /// whatever is cheapest, since only those two matter to the rename
    /// helpers below.
    fn session(id: &str, title: &str) -> Session {
        Session {
            id: id.into(),
            title: title.into(),
            cwd: "/tmp".into(),
            invocation: "agent".into(),
            status: SessionStatus::Unknown,
            annotation: None,
            restart_offer: crate::RestartOffer::FreshOnly,
            tabs: Vec::new(),
            host: None,
            host_name: None,
            stale: false,
        }
    }

    /// The rename's user-visible promise is that the new title shows up at
    /// once, everywhere the row shows a title — so the overlay has to
    /// reach the rendered `Session` itself rather than only the one span
    /// the row happens to print, and it must leave every other row and
    /// every other field alone (a rename changes nothing but the title,
    /// and the poll's status is fresher than anything this view holds).
    #[test]
    fn optimistic_renames_replace_only_the_renamed_row_s_title() {
        let listing = vec![session("a", "old-a"), session("b", "b")];
        let renamed: HashMap<String, (String, u64)> = [("a".to_string(), ("new-a".to_string(), 7))]
            .into_iter()
            .collect();

        let rendered = apply_optimistic_renames(&listing, &renamed);
        assert_eq!(rendered[0].title, "new-a");
        assert_eq!(rendered[0].cwd, listing[0].cwd, "only the title is ours");
        assert_eq!(rendered[1], listing[1], "an untouched row stays identical");

        let unknown: HashMap<String, (String, u64)> =
            [("gone".to_string(), ("ghost".to_string(), 0))]
                .into_iter()
                .collect();
        assert_eq!(
            apply_optimistic_renames(&listing, &unknown),
            listing,
            "a correction for a session the listing does not carry invents no row"
        );
    }

    /// The sequence check is the load-bearing half: a listing reply that
    /// was already in flight when the rename landed reports the OLD title
    /// truthfully and must not be read as the server disagreeing, or the
    /// row visibly flips back for a whole poll interval. A reply that
    /// postdates the rename is authoritative in both directions —
    /// agreement retires the correction, and disagreement (another
    /// client's later rename, or a session that has left the listing)
    /// retires it too, because the server wins.
    #[test]
    fn optimistic_renames_retire_only_on_a_reply_that_could_have_seen_them() {
        let mut renamed: HashMap<String, (String, u64)> =
            [("a".to_string(), ("new-a".to_string(), 5))]
                .into_iter()
                .collect();

        prune_optimistic_renames(&mut renamed, &[session("a", "old-a")], 4);
        assert!(
            renamed.contains_key("a"),
            "a poll that started before the rename says nothing about it"
        );

        let mut graduated = renamed.clone();
        prune_optimistic_renames(&mut graduated, &[session("a", "new-a")], 4);
        assert!(
            graduated.is_empty(),
            "the server now reports our title, even on an early poll: nothing left to correct"
        );

        let mut contradicted = renamed.clone();
        prune_optimistic_renames(&mut contradicted, &[session("a", "someone-else")], 6);
        assert!(
            contradicted.is_empty(),
            "a reply that could have seen the rename and reports another title wins"
        );

        let mut vanished = renamed.clone();
        prune_optimistic_renames(&mut vanished, &[], 6);
        assert!(
            vanished.is_empty(),
            "a session the listing no longer carries has no row to correct"
        );
    }

    /// A listing carrying `rows` sessions, with the three count fields set
    /// explicitly — the banner reads nothing else, so the sessions
    /// themselves are placeholders.
    fn listing(rows: usize, total: u64, truncated: bool, incoherent: bool) -> SessionListing {
        SessionListing {
            sessions: (0..rows)
                .map(|n| session(&format!("s{n}"), &format!("title-{n}")))
                .collect(),
            total,
            truncated,
            incoherent,
        }
    }

    /// Pins every branch of the banner against the EXACT strings and classes
    /// the browser suite selects on (`.session-count` /
    /// `.truncation-banner` in `e2e/tests/terminal.spec.ts`).
    ///
    /// This wording is a contract, not copy: PLAN_M2.md acceptance 5 is the
    /// requirement that a list the client could not walk to the end says so,
    /// and it is stated in the same place a complete list states its count —
    /// so a change to either string breaks a promise about honesty, not just
    /// a selector. Table-driven because the interesting part is the BOUNDARY
    /// between the two wordings, and the two conditions that can trigger the
    /// truncated form are independent (see `count_banner`'s own docs).
    #[test]
    fn the_count_banner_pins_each_branchs_class_and_wording() {
        // (listing, expected class, expected text)
        let cases = [
            // A complete walk: the total, and no claim to be a subset.
            (
                listing(4, 4, false, false),
                "banner session-count",
                "4 sessions",
            ),
            // A ceiling was hit: `truncated` set even though the counts
            // happen to agree — the case a shortfall check alone misses.
            (
                listing(4, 4, true, false),
                "banner truncation-banner",
                "showing 4 of 4 sessions",
            ),
            // A plain shortfall with `truncated` UNSET: the helm reported
            // more than the walk collected, which totals-disagree-without-
            // the-flag covers (concurrent creation).
            (
                listing(2, 700, false, false),
                "banner truncation-banner",
                "showing 2 of 700 sessions",
            ),
            // Both conditions at once — the shape the browser suite drives.
            (
                listing(2, 700, true, false),
                "banner truncation-banner",
                "showing 2 of 700 sessions",
            ),
            // Zero rows against a non-zero total is still a shortfall, not
            // the empty-list case (which `ListView` handles before ever
            // reaching the banner).
            (
                listing(0, 3, false, false),
                "banner truncation-banner",
                "showing 0 of 3 sessions",
            ),
        ];
        for (listing, class, text) in cases {
            let banner = count_banner(&listing);
            assert_eq!(banner.class, class, "class for {text:?}");
            assert_eq!(banner.text, text);
            assert_eq!(
                banner.incoherence, None,
                "no incoherence was reported, so no suffix belongs on {text:?}"
            );
        }
    }

    /// The incoherence note is its OWN field, appended to the truncated
    /// wording rather than replacing it.
    ///
    /// Kept separate from `text` because the renderer emits the two as
    /// separate text runs inside one banner element; fusing them would
    /// change the DOM without changing any string, which is exactly the kind
    /// of regression a text-only assertion sails past. The claim it makes is
    /// also distinct from a plain shortfall — the rows and the count
    /// disagree in a direction that means the list CHANGED under the walk —
    /// so it must not appear merely because a walk stopped short.
    #[test]
    fn the_incoherence_note_is_a_separate_run_on_the_truncated_banner() {
        let banner = count_banner(&listing(5, 3, true, true));
        assert_eq!(banner.class, "banner truncation-banner");
        assert_eq!(
            banner.text, "showing 5 of 3 sessions",
            "the counts are reported as they came back, contradiction and all"
        );
        assert_eq!(
            banner.incoherence,
            Some(" — the list changed while it was being read; refreshing")
        );

        // Truncated but coherent: the suffix stays off.
        assert_eq!(
            count_banner(&listing(2, 700, true, false)).incoherence,
            None
        );

        // `incoherent` cannot reach the complete-walk branch on a real
        // listing: `api::fetch_sessions` sets `truncated: truncated ||
        // incoherent`, so an incoherent walk always takes the truncated
        // form. Asserted anyway, because that pairing is enforced at the
        // CONSTRUCTOR and nothing stops this function being handed a listing
        // built some other way — a future change on either side must not
        // quietly strand the note on a banner whose wording does not admit
        // anything is wrong.
        let complete = count_banner(&listing(4, 4, false, false));
        assert_eq!(complete.class, "banner session-count");
        assert_eq!(complete.incoherence, None);
    }
}
