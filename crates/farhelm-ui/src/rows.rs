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
//! A title refreshed only by the server's next word on the subject would
//! take a round trip to show a user the result of their own rename, so a
//! landed rename is painted over the server's rows immediately
//! (PLAN_M5.md item 6)
//! — the same bargain `tabs::visible_tabs` makes for tabs. What makes it
//! safe rather than a second source of truth is the pairing:
//! [`apply_optimistic_renames`] paints at RENDER time and never mutates the
//! stored listing, and the two pruning halves retire each correction the
//! moment a reply that could have seen it arrives. Neither is meaningful
//! without the other, which is why they live together.
//!
//! The pruning is two functions rather than one because the two rest on
//! different evidence, and only one of them is available to every read.
//! [`settle_optimistic_renames`] compares TITLES on the rows a reply
//! returned, which any successful read can do; [`retire_vanished_renames`]
//! reads a session's ABSENCE as departure, which is only true of a read that
//! speaks for the whole fleet. Fusing them (as this module once did) makes
//! the weaker requirement govern both, and a filtered read then cannot even
//! graduate a rename the server is already reporting back.
//!
//! ## The banner is not decoration
//!
//! [`count_banner`] carries PLAN_M2.md acceptance 5: a list the helm could
//! not serve whole must SAY so, in the same place a complete one says its
//! count. The wording is pinned by the browser suite, so treat the
//! strings as a contract rather than as copy.

use std::collections::HashMap;

use crate::Session;
use crate::api::SessionListing;

/// The listing as it should be RENDERED: the server's rows with this
/// view's own just-landed renames painted over them (PLAN_M5.md item 6).
///
/// The same optimistic-rendering bargain `tabs::visible_tabs` makes, for
/// the same reason — a title refreshed only by the next listing read would
/// take a round trip to show the user the result of their own rename —
/// and applied at RENDER time rather than by mutating the stored listing,
/// so the correction cannot outlive the pruning halves' judgement about it
/// ([`settle_optimistic_renames`], [`retire_vanished_renames`]).
/// A rename for an id the listing does not carry is simply not
/// applied: there is no row to paint, and inventing one would claim a
/// session the server did not list.
///
/// Only the title is overridden. Everything else in the row — status,
/// annotation, tabs — is whatever the listing says, because a rename
/// changes nothing else about a session and a stale copy of those fields
/// is exactly what the next listing read exists to replace.
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

/// Retire the optimistic renames this reply's ROWS settle, leaving every
/// other correction alone.
///
/// Runs after every successful read, filtered or not, because nothing it
/// decides needs the reply to speak for the whole fleet: it only ever looks
/// at rows that came back, and a row that came back is the server's word on
/// that session whatever query produced it. The companion
/// [`retire_vanished_renames`] is the half that reads ABSENCE, which only a
/// read with authority may do.
///
/// `index` is the reply's own read sequence number and is the whole point of
/// the exercise: a reply that STARTED before the rename's own response
/// completed is not evidence about it either way, so its old title cannot be
/// read as the server disagreeing. Without that distinction "the server
/// disagrees" and "the server has not told this client yet" look identical,
/// and the row would flip back to the old title until the next read — the
/// wobble this scheme exists to prevent (`session_view::SessionView`'s
/// `opened_tabs` carries the same argument for tabs).
///
/// The comparison is a CONSERVATIVE bound, not a claim about when the
/// server changed: the durable write lands before the rename's reply is
/// read, so a read launched earlier may perfectly well observe the new
/// title. That only ever makes this hold a correction slightly longer than
/// strictly necessary, which is the harmless direction.
///
/// Three outcomes, in the order they are decided:
///
/// - The server now reports the same title: the rename graduated, and the
///   correction has nothing left to correct. Accepted from ANY read, even
///   one that predates the rename — a server already reporting this title
///   cannot be disagreeing with it.
/// - This reply is GUARANTEED to postdate the rename and reports a
///   DIFFERENT title: the server is authoritative and wins, whether that is
///   another client's later rename or this view being wrong about what
///   landed.
/// - The reply does not carry this session at all, or may predate the
///   rename: the correction stands.
pub(crate) fn settle_optimistic_renames(
    renamed: &mut HashMap<String, (String, u64)>,
    server: &[Session],
    index: u64,
) {
    renamed.retain(|id, (title, observed_from)| {
        match server.iter().find(|session| &session.id == id) {
            Some(session) if &session.title == title => false,
            Some(_) => index < *observed_from,
            // Absence proves nothing here — see `retire_vanished_renames`.
            None => true,
        }
    });
}

/// Retire the corrections for sessions this reply says are GONE.
///
/// Split from [`settle_optimistic_renames`] because the two rest on
/// different evidence, and only one of them is available to every read. This
/// one treats a session's absence from the reply as departure, which is only
/// true of a read that speaks for the whole fleet: a FILTERED listing omits
/// every session that did not match, and "did not match" is not "went away".
/// Calling this on a filtered reply would retire a correction on the
/// authority of a query that never asked about that row — and, since the
/// rename overlay is what keeps the user's own rename on screen, would drop
/// it back to the old title for as long as any filter is applied.
///
/// The `index` bound is the same conservative one, and matters for the same
/// reason: a read that started before the rename can be missing the session
/// for reasons that have nothing to do with it.
pub(crate) fn retire_vanished_renames(
    renamed: &mut HashMap<String, (String, u64)>,
    server: &[Session],
    index: u64,
) {
    renamed.retain(|id, (_, observed_from)| {
        server.iter().any(|session| &session.id == id) || index < *observed_from
    });
}

/// Whether a session's open actions-menu panel should close because its
/// ROW moved between two listings — an insert or removal ABOVE it in the
/// helm's own order — as opposed to merely having its own fields updated
/// in place.
///
/// `list::SessionRow`'s actions panel — and `hosts::HostRow`'s, measured
/// the identical way — is `position: fixed`, positioned from a ONE-TIME
/// measurement of its toggle's screen coordinates taken the instant it
/// opened (`menu_panel::menu_panel_style`); nothing re-measures it while it
/// stays open. A row that STAYS in the listing but changes
/// INDEX still invalidates that measurement — the toggle is no longer
/// where it was measured — even though the row's own continued presence
/// (which a separate, simpler "is this id still listed" check already
/// covers in `list::ListView`'s `commit_listing`) never changed. A row
/// whose own fields merely updated (a status tick, say) keeps its index
/// and must NOT close the menu — the user may still be reading it.
///
/// `previous` is `None` when there is no comparable baseline to diff
/// against — a first load, or recovery from a failed read — which
/// deliberately never counts as a reorder here: that recovery path is
/// `SessionRow`'s own `onmounted` heal (a remount re-measures itself from
/// scratch), not this reconciliation's job, and treating "nothing to
/// compare against" as "moved" would just race that heal for no benefit.
pub(crate) fn menu_row_reordered(
    previous: Option<&[Session]>,
    current: &[Session],
    open_id: &str,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let previous_index = previous.iter().position(|session| session.id == open_id);
    let current_index = current.iter().position(|session| session.id == open_id);
    previous_index != current_index
}

/// Whether the list should say "no sessions" and nothing else.
///
/// The plain empty-fleet line replaces the banner, the rows, and every
/// explanation around them, which is right for a fleet that genuinely has
/// nothing in it and wrong for everything else. In particular it must not
/// swallow a FILTERED reply over an empty fleet, which satisfies both counts
/// just as well: a user who has just searched would get a bare "no sessions"
/// where the two facts they need — that a filter is applied, and that it
/// matched nothing — are exactly what disappeared. "The fleet is empty" and
/// "your search found nothing" call for opposite reactions, and the filtered
/// case keeps its banner and its no-match line to tell them apart.
///
/// Both counts are checked rather than only the rows, because no rows
/// against a non-zero total is not an empty fleet — it is either a filtered
/// view whose matches were zero or a reply the cap cut, and both belong to
/// the banner and the no-match line, not to the calm fleet-empty sentence.
///
/// The reply must also COVER the fleet
/// (`SessionListing::omits_fleet_members`), which is the stricter of the two
/// filter flags and deliberately so: the ordinary view's own total is now a
/// count of the non-archived list, so `total == 0` there means "nothing
/// active", not "nothing at all". Keyed on `filtered` instead, a fleet of
/// nothing but archived sessions would announce itself as empty. The right
/// pane says "no active sessions" for that case, which is the claim the
/// reply actually supports.
pub(crate) fn is_empty_fleet(listing: &SessionListing) -> bool {
    listing.sessions.is_empty()
        && listing.total == 0
        && !listing.omits_fleet_members
        // A COMPLETE reply, or the shortcut is a claim the reply does not
        // support. A truncated reply with no rows says "I could not read
        // this list", not "there is nothing in it" — and a `matching` the
        // zero total contradicts is precisely what `count_banner` exists
        // to report. Skipping the banner here would swallow either and
        // print the calmest possible sentence over a reply that is telling
        // the client something is wrong.
        && !listing.truncated
        && matches!(listing.matching, None | Some(0))
}

/// Whether this reply holds the whole view it describes: the exact
/// negation of the helm's `truncated` flag, and nothing else — the reply
/// is one snapshot, so there is no second signal for this to weigh.
///
/// A named function rather than `!listing.truncated` at each caller
/// because three of them share the answer and must never drift: the banner
/// (which says so to the user), [`absence_is_evidence`] (which refuses to
/// read a missing row as a departure), and `list::ListView`'s auto-select
/// effect (which resolves a remembered selection directly rather than
/// treating its absence from a cut list as gone).
pub(crate) fn listing_is_complete(listing: &SessionListing) -> bool {
    !listing.truncated
}

/// Whether a reply's ABSENCES may be read as departures.
///
/// The question `list::ListView` asks before retiring an optimistic rename,
/// closing a rename editor, or dropping a delete confirmation for a session
/// that is not in this reply. Two facts have to hold, and only one of them
/// was being checked:
///
/// - **The reply speaks for the whole fleet.** A narrowed listing omits
///   every session it left out, and "left out" is not "went away". Read from
///   `SessionListing::omits_fleet_members` rather than from `filtered`, and
///   the difference is the DEFAULT view: it reads as unfiltered to a user
///   (that is the banner's question) while still hiding every archived
///   session, so a session archived from another client would otherwise
///   vanish from this reply and be mistaken for one that left.
/// - **The reply holds the whole view** ([`listing_is_complete`], i.e. the
///   helm's `truncated` flag is off). A reply the helm's cap cut is missing
///   every session past the cutoff for a reason that has nothing to do with
///   existing.
///
/// Reading either kind of absence as a departure silently discards the
/// user's own work on a large fleet: a rename they just made retires, an
/// open editor closes under them, a confirmation they are mid-decision on
/// disappears — and, worst of the set, the SELECTED session is treated as
/// deleted, tearing down the pane they are working in and auto-opening some
/// other session in its place.
///
/// The caller ANDs this with its own standing, because one more kind of read
/// has none: a mutation's immediate refetch exists to show ONE session and
/// is not a statement about the rest.
pub(crate) fn absence_is_evidence(listing: &SessionListing) -> bool {
    !listing.omits_fleet_members && listing_is_complete(listing)
}

/// The no-match line for a filtered reply with nothing to show — or `None`
/// when no such line may be printed.
///
/// The categorical claim ("no sessions match this filter") is about the
/// FLEET, and it is only true when the helm counted the matches over the
/// whole fleet and counted zero — which requires a COMPLETE listing. A
/// truncated one counted only the rows the caps retained, and a match may
/// sit past some host's cut, so its wording claims exactly what the reply
/// supports: nothing matched among the sessions that could be read. An
/// empty row vector alone claims neither; two cases would misinform:
///
/// - An IGNORED filter (`matching` absent — see `api::matching_count`)
///   would produce the line on the strength of a count nobody computed, on
///   a helm that never filtered at all.
/// - `matching == 0` over a non-empty row vector is a contradiction the
///   banner is already reporting (see [`count_banner`]) — printing
///   "nothing matched" above visible rows would be the UI arguing with
///   itself.
pub(crate) fn no_match_line(listing: &SessionListing) -> Option<&'static str> {
    if !(listing.filtered && listing.matching == Some(0) && listing.sessions.is_empty()) {
        return None;
    }
    Some(if listing_is_complete(listing) {
        "no sessions match this filter"
    } else {
        // SPEC.md's "could not read to the end", scoped to what this line
        // is allowed to claim under a cut list.
        "no matches among the sessions that could be read — the list could not be read to the end"
    })
}

/// The count line above the rows, decided but not rendered.
///
/// `class` carries which of the two banners this is and `text` the
/// sentence; the caller interpolates both verbatim. (A third field once
/// carried the paged design's "the list changed while it was being read"
/// note as its own text run; a whole-list reply cannot contradict itself,
/// so there is nothing left for it to say.)
pub(crate) struct CountBanner {
    /// The banner element's full class attribute — the variant is part of
    /// it (`truncation-banner` vs. `session-count`), and the stylesheet and
    /// the browser suite both select on that.
    pub(crate) class: &'static str,
    /// The count sentence itself.
    pub(crate) text: String,
}

/// The clause a filtered banner carries when the helm never answered the
/// filter (`api::matching_count` returning `None`).
///
/// Said plainly rather than dressed as a count, because the alternative is
/// the UI vouching for a filter that did not run: the rows on screen are the
/// whole fleet, and any "N matching" over them is a number nobody computed.
const FILTER_UNSUPPORTED_NOTE: &str =
    " — this helm does not support filtering, so the filter was ignored";

/// The clause every truncated banner carries, independent of its numbers.
///
/// SPEC.md's words on purpose. The numbers alone cannot carry the fact: a
/// supervisor that cut its own list reports a `total` counted over what it
/// retained, so a capped host reads "showing 500 of 500 sessions" — the
/// numbers agree while rows are missing, and only this clause says so. One
/// constant so the wording cannot drift between the banner's arms.
const TRUNCATION_NOTE: &str = " — could not read the list to the end";

/// What the count line says about this listing.
///
/// The count ALWAYS renders. PLAN_M2.md acceptance 5 asks for the shortfall
/// to be shown rather than plumbed, and a line that appears only when
/// something is wrong makes its own absence carry meaning nobody can read: a
/// user seeing no banner cannot tell "this is all of them" from "this UI
/// forgot to say".
///
/// The WORDING varies along independent axes, which is why this is a table
/// rather than an if/else:
///
/// - **Truncation.** "showing N of …" is reserved for a reply the helm's
///   cap cut (`truncated`, via [`listing_is_complete`]) — nothing else
///   makes a list short. The denominator printed beside N is `matching`
///   for a filtered banner and the view's own `total` for an unfiltered
///   one; the default view is why those two are not the same thing — the
///   helm answers it with a real `matching`, because its archive exclusion
///   is a server-side predicate, while the sentence a reader sees counts
///   the view.
/// - **Filtering.** A filtered list says "N matching of M sessions"
///   (PLAN_M6_75.md item 7), which is the distinction the second count
///   exists to make: without it, a filter that hid 690 of 700 rows and a
///   cap that could only serve 10 of them look identical on screen, and only
///   one of those means "there is more to see". The filtered wording is
///   chosen from the REQUEST rather than by comparing the two counts, so a
///   filter that happens to match everything still says so — a banner that
///   silently reverted to the unfiltered sentence would leave a user unsure
///   whether their filter took at all.
///
///   The archive switch is NOT one of those filters, in either position.
///   `M` is the size of the view the rows came from — the non-archived list
///   by default, the whole fleet with the switch on (`api::SessionListing::
///   total`, and `aggregate::SessionListBody::total` on the helm side) — so the ordinary
///   list reads "12 sessions" rather than "12 matching of 12 sessions". The
///   shipped alternative, an M that counted archived rows the list did not
///   show, made the two numbers disagree with nothing typed into any filter
///   and no wording able to explain the gap. The accepted consequence is
///   that "filtered" now means a filter a PERSON applied (maintainer's
///   verdict, 2026-08-22).
/// - **A filter the helm did not answer.** `matching` is absent only where
///   substituting a number would be a fabrication (`api::matching_count`),
///   and that case gets the unfiltered sentence plus a clause saying why —
///   the honest description of a screen full of unfiltered rows.
///
/// A filtered banner whose helm gave NO matching count prints the view's
/// total as its denominator, which is the ignored-filter case: those rows
/// are the whole view, so a reply the cap cut has to say "showing N of M"
/// like any other, rather than presenting an ignored filter's partial list
/// as if it were everything.
pub(crate) fn count_banner(listing: &SessionListing) -> CountBanner {
    let shown = listing.sessions.len() as u64;
    // The completeness test itself lives in `listing_is_complete`, and sharing
    // it is not tidiness: the same question decides whether an absent session
    // may be read as departed and whether the auto-select fallback may trust
    // the rows it holds. A banner saying "showing 3 of 5" while the code
    // beside it reasoned as though the list were whole would be the UI
    // disagreeing with itself about the one fact it just printed.
    let short = !listing_is_complete(listing);
    let (class, text) = match (listing.filtered, listing.matching, short) {
        (false, _, false) => (
            "banner session-count",
            format!("{} sessions", listing.total),
        ),
        (false, _, true) => (
            "banner truncation-banner",
            format!(
                "showing {shown} of {} sessions{TRUNCATION_NOTE}",
                listing.total
            ),
        ),
        (true, Some(matching), false) => (
            "banner session-count filtered",
            format!("{matching} matching of {} sessions", listing.total),
        ),
        // Three numbers, because all three are different questions: how many
        // are on screen, how many match, and how big the view is. Dropping
        // the last would leave a user unable to tell a narrow filter from a
        // small fleet.
        (true, Some(matching), true) => (
            "banner truncation-banner filtered",
            format!(
                "showing {shown} of {matching} matching sessions ({} in all){TRUNCATION_NOTE}",
                listing.total
            ),
        ),
        // No matching count to report: the sentence reverts to the
        // unfiltered one, which is what the rows actually are, and the
        // clause explains why the filter changed nothing. The `filtered`
        // modifier stays on the class — the REQUEST carried a filter, and
        // that is what the modifier has always meant.
        (true, None, false) => (
            "banner session-count filtered",
            format!("{} sessions{FILTER_UNSUPPORTED_NOTE}", listing.total),
        ),
        (true, None, true) => (
            "banner truncation-banner filtered",
            format!(
                "showing {shown} of {} sessions{TRUNCATION_NOTE}{FILTER_UNSUPPORTED_NOTE}",
                listing.total
            ),
        ),
    };
    CountBanner { class, text }
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
            created_at: 0,
            last_activity_at: 0,
            archived: false,
            tabs: Vec::new(),
            host: None,
            host_identity: None,
            host_name: None,
            stale: false,
            // Raw-created, like every session these helpers reason about: the
            // rename overlay is about titles, and a source profile is neither
            // read nor rewritten by any of it.
            source_profile: None,
            seen_activity_at: None,
        }
    }

    /// The rename's user-visible promise is that the new title shows up at
    /// once, everywhere the row shows a title — so the overlay has to
    /// reach the rendered `Session` itself rather than only the one span
    /// the row happens to print, and it must leave every other row and
    /// every other field alone (a rename changes nothing but the title,
    /// and the poll's status is fresher than anything this view holds).
    #[farhelm_testtrace::test]
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
    /// row visibly flips back until another read lands. A reply that
    /// postdates the rename is authoritative in both directions —
    /// agreement retires the correction, and disagreement (another client's
    /// later rename) retires it too, because the server wins.
    ///
    /// What this deliberately does NOT test is absence, which is
    /// `retire_vanished_renames`' evidence and not every read's to use.
    #[farhelm_testtrace::test]
    fn optimistic_renames_retire_only_on_a_reply_that_could_have_seen_them() {
        let mut renamed: HashMap<String, (String, u64)> =
            [("a".to_string(), ("new-a".to_string(), 5))]
                .into_iter()
                .collect();

        settle_optimistic_renames(&mut renamed, &[session("a", "old-a")], 4);
        assert!(
            renamed.contains_key("a"),
            "a read that started before the rename says nothing about it"
        );

        let mut graduated = renamed.clone();
        settle_optimistic_renames(&mut graduated, &[session("a", "new-a")], 4);
        assert!(
            graduated.is_empty(),
            "the server now reports our title, even on an early read: nothing left to correct"
        );

        let mut contradicted = renamed.clone();
        settle_optimistic_renames(&mut contradicted, &[session("a", "someone-else")], 6);
        assert!(
            contradicted.is_empty(),
            "a reply that could have seen the rename and reports another title wins"
        );
    }

    /// A read that carried a FILTER still graduates the renames it can see,
    /// and still never reads absence as departure.
    ///
    /// This is the bug the split exists for. With all the pruning behind a
    /// whole-fleet authority check, a server row AGREEING with the
    /// optimistic title could not retire the correction while any filter was
    /// applied — so the overlay stayed on top of the server's own rows
    /// indefinitely, and a LATER rename from another client was masked by a
    /// correction that had nothing left to correct. Agreement needs no
    /// authority: a row that came back is the server's word about that
    /// session whatever query produced it.
    #[farhelm_testtrace::test]
    fn a_filtered_reply_still_settles_the_rows_it_did_return() {
        let mut renamed: HashMap<String, (String, u64)> = [
            ("a".to_string(), ("new-a".to_string(), 5)),
            ("b".to_string(), ("new-b".to_string(), 5)),
        ]
        .into_iter()
        .collect();

        // A filtered read that matched only `a`, reporting the title this
        // view painted optimistically.
        settle_optimistic_renames(&mut renamed, &[session("a", "new-a")], 6);
        assert!(!renamed.contains_key("a"), "the rename graduated");
        assert!(
            renamed.contains_key("b"),
            "a session the filter excluded is not a session that went away"
        );
    }

    /// Absence retires a correction only for a read that speaks for the
    /// whole fleet, and only once it is late enough to be evidence.
    ///
    /// Both halves matter. Without the authority check a filtered read would
    /// retire corrections for every row it excluded — dropping the user's
    /// own rename back to the old title for as long as a filter is applied.
    /// Without the index check a read already in flight when the rename
    /// landed would do the same on a session it simply had not reached yet.
    #[farhelm_testtrace::test]
    fn only_a_read_with_authority_reads_absence_as_departure() {
        let renamed: HashMap<String, (String, u64)> = [("a".to_string(), ("new-a".to_string(), 5))]
            .into_iter()
            .collect();

        let mut early = renamed.clone();
        retire_vanished_renames(&mut early, &[], 4);
        assert!(
            early.contains_key("a"),
            "a read that started before the rename may simply not have seen it yet"
        );

        let mut vanished = renamed.clone();
        retire_vanished_renames(&mut vanished, &[], 6);
        assert!(
            vanished.is_empty(),
            "a session a later whole-fleet read does not carry has no row to correct"
        );

        // A whole-fleet read that DOES list the session leaves the
        // correction to `settle_optimistic_renames`, which is the only one
        // of the two that compares titles.
        let mut listed = renamed.clone();
        retire_vanished_renames(&mut listed, &[session("a", "old-a")], 6);
        assert!(listed.contains_key("a"));
    }

    /// A row inserted or removed ABOVE the open one shifts its index even
    /// though the row itself never left the listing — the exact case
    /// `menu_row_reordered` exists to catch, since `commit_listing`'s
    /// separate "is this id still listed" check cannot see it (the row IS
    /// still listed, just not where the panel was measured against).
    #[farhelm_testtrace::test]
    fn a_row_inserted_or_removed_above_the_open_one_is_a_reorder() {
        let before = vec![session("a", "a"), session("b", "b")];

        let inserted_above = vec![session("new", "new"), session("a", "a"), session("b", "b")];
        assert!(
            menu_row_reordered(Some(&before), &inserted_above, "b"),
            "a row inserted above the open one must count as a reorder"
        );

        let removed_above = vec![session("b", "b")];
        assert!(
            menu_row_reordered(Some(&before), &removed_above, "b"),
            "a row removed from above the open one must count as a reorder"
        );
    }

    /// The complement: a row whose own fields changed in place (a status
    /// tick, say — modeled here as a title change, since only identity and
    /// order matter to this function) keeps its INDEX, and must not read
    /// as a reorder — the menu is still anchored exactly where it was
    /// measured, and closing it out from under a user still reading it
    /// would be a regression of its own.
    #[farhelm_testtrace::test]
    fn a_same_order_content_update_is_not_a_reorder() {
        let before = vec![session("a", "a"), session("b", "b")];
        let updated_in_place = vec![session("a", "a"), session("b", "b-updated")];
        assert!(!menu_row_reordered(Some(&before), &updated_in_place, "b"));
    }

    /// Rows changing BELOW the open one leave its index — and therefore
    /// its measured toggle position — exactly where it was, so they must
    /// not read as a reorder: closing the menu there would interrupt the
    /// user for zero positioning benefit. This is the guard against an
    /// overbroad implementation that treats any listing-length change as
    /// a reorder, which the above-row tests alone cannot catch.
    #[farhelm_testtrace::test]
    fn a_row_inserted_or_removed_below_the_open_one_is_not_a_reorder() {
        let before = vec![session("a", "a"), session("b", "b")];

        let inserted_below = vec![session("a", "a"), session("b", "b"), session("new", "new")];
        assert!(
            !menu_row_reordered(Some(&before), &inserted_below, "a"),
            "a row inserted below the open one leaves its position untouched"
        );

        let removed_below = vec![session("a", "a")];
        assert!(
            !menu_row_reordered(Some(&before), &removed_below, "a"),
            "a row removed from below the open one leaves its position untouched"
        );
    }

    /// No baseline listing to diff against — a first load, or recovery
    /// from a failed read — must never read as a reorder: that recovery
    /// path is `SessionRow`'s own remount heal (see `menu_row_reordered`'s
    /// own doc), and this function racing it to the same conclusion by a
    /// different, coincidental route would buy nothing.
    #[farhelm_testtrace::test]
    fn no_baseline_listing_is_never_a_reorder() {
        let current = vec![session("a", "a")];
        assert!(!menu_row_reordered(None, &current, "a"));
    }

    /// A FLEET-WIDE listing carrying `rows` sessions, with the count fields
    /// set explicitly — the banner reads nothing else, so the sessions
    /// themselves are placeholders.
    ///
    /// This is the archive-switch-on reply: nothing is filtered and nothing
    /// is withheld, so both request flags are false. `matching` mirrors
    /// `total`, which is what `api::matching_count` substitutes when the helm
    /// makes no matching claim — the unfiltered cases must be built the way
    /// `api::fetch_sessions` actually builds them, or they would pin wording
    /// against a reply shape that cannot occur.
    fn listing(rows: usize, total: u64, truncated: bool) -> SessionListing {
        SessionListing {
            sessions: (0..rows)
                .map(|n| session(&format!("s{n}"), &format!("title-{n}")))
                .collect(),
            total,
            matching: Some(total),
            filtered: false,
            omits_fleet_members: false,
            truncated,
        }
    }

    /// The DEFAULT view's reply: no filter a person applied — so the banner
    /// treats it as unfiltered — over a request that still permits the helm
    /// to withhold every archived session.
    ///
    /// The one listing where the two request flags disagree, and therefore
    /// the fixture every test of that divergence is built from. Both flags
    /// describe what was ASKED, so `omits_fleet_members` is true here
    /// whether or not this particular fleet has an archived session in it.
    fn default_view_listing(rows: usize, total: u64) -> SessionListing {
        SessionListing {
            omits_fleet_members: true,
            ..listing(rows, total, false)
        }
    }

    /// The same, for a request that carried a filter a PERSON applied:
    /// `matching` is now its own number, and both request flags are set.
    ///
    /// Neither flag is derived from the rows below it — a request carrying
    /// a filter may match everything and still answer yes to both, which is
    /// exactly the case the banner has to keep saying "N matching of M" for.
    fn filtered_listing(rows: usize, matching: u64, total: u64, truncated: bool) -> SessionListing {
        SessionListing {
            matching: Some(matching),
            filtered: true,
            omits_fleet_members: true,
            ..listing(rows, total, truncated)
        }
    }

    /// A filtered request answered by a helm that predates server-side
    /// filtering: `matching` is absent because no honest number exists
    /// (`api::matching_count`), and the rows are the whole fleet because
    /// that helm ignored the query.
    ///
    /// Both request flags are still true, and that is the compatibility
    /// point worth pinning: they are read off the REQUEST, so an old peer
    /// serving more than it was asked for cannot flip them. Deriving them
    /// from the reply instead would let this listing claim the unfiltered
    /// wording — vouching for a filter that never ran — and grant absence
    /// the standing to retire work, on the say-so of a helm that could not
    /// have narrowed anything.
    fn old_peer_filtered_listing(rows: usize, total: u64, truncated: bool) -> SessionListing {
        SessionListing {
            matching: None,
            // The `0` below is overwritten by the line above and never
            // reaches a banner; this builds on `filtered_listing` for the
            // request flags, which are the part that has to stay in step.
            ..filtered_listing(rows, 0, total, truncated)
        }
    }

    /// Pins every branch of the banner against the EXACT strings and classes
    /// the browser suite selects on (`.session-count` / `.truncation-banner`
    /// across `e2e/tests/terminal-multihost.spec.ts` and
    /// `e2e/tests/terminal.spec.ts`).
    ///
    /// This wording is a contract, not copy: PLAN_M2.md acceptance 5 is the
    /// requirement that a list the helm could not serve whole says so,
    /// and it is stated in the same place a complete list states its count —
    /// so a change to either string breaks a promise about honesty, not just
    /// a selector. Table-driven because the interesting part is the BOUNDARY
    /// between the two wordings, and the two conditions that can trigger the
    /// truncated form are independent (see `count_banner`'s own docs).
    #[farhelm_testtrace::test]
    fn the_count_banner_pins_each_branchs_class_and_wording() {
        // (listing, expected class, expected text)
        let cases = [
            // A complete reply: the total, and no claim to be a subset.
            (listing(4, 4, false), "banner session-count", "4 sessions"),
            // The helm's cap cut the view: `truncated` set even though the
            // counts happen to agree — the flag is the whole test, not the
            // arithmetic.
            // The clause is what carries the fact when the numbers agree:
            // a supervisor that cut its own list counts `total` over what
            // it retained, so "4 of 4" alone would read as complete.
            (
                listing(4, 4, true),
                "banner truncation-banner",
                "showing 4 of 4 sessions — could not read the list to the end",
            ),
            // The shape the browser suite drives.
            (
                listing(2, 700, true),
                "banner truncation-banner",
                "showing 2 of 700 sessions — could not read the list to the end",
            ),
            // Zero rows left under the cap is still a cut list, not the
            // empty-list case (which `ListView` handles before ever
            // reaching the banner).
            (
                listing(0, 3, true),
                "banner truncation-banner",
                "showing 0 of 3 sessions — could not read the list to the end",
            ),
            // The DEFAULT view takes the unfiltered wording. The total it
            // prints is the size of the list it is showing, because archived
            // sessions are now outside both — the shape that used to say "4
            // matching of N sessions" here, with N counting rows the list
            // withheld and nothing typed into any filter.
            (
                default_view_listing(4, 4),
                "banner session-count",
                "4 sessions",
            ),
        ];
        for (listing, class, text) in cases {
            let banner = count_banner(&listing);
            assert_eq!(banner.class, class, "class for {text:?}");
            assert_eq!(banner.text, text);
        }
    }

    /// A filtered list says "N matching of M sessions" (PLAN_M6_75.md item
    /// 7) — the distinction the helm's second count exists to make.
    ///
    /// Without it, a filter that hid 690 of 700 sessions and a cap that
    /// could only serve 10 of them produce the same sentence, and only one of
    /// those means "there is more to see". The class carries a `filtered`
    /// modifier ON TOP of the existing one rather than replacing it, so
    /// every stylesheet rule and browser assertion written against
    /// `.session-count` / `.truncation-banner` keeps matching — the wording
    /// change is not an excuse to move the selectors the suite already pins.
    #[farhelm_testtrace::test]
    fn a_filtered_list_reports_both_counts() {
        let banner = count_banner(&filtered_listing(12, 12, 700, false));
        assert_eq!(banner.class, "banner session-count filtered");
        assert_eq!(banner.text, "12 matching of 700 sessions");

        // A filter matching NOTHING is still a filter, and the fleet total
        // is what keeps "0 matching" from reading as an empty fleet.
        assert_eq!(
            count_banner(&filtered_listing(0, 0, 700, false)).text,
            "0 matching of 700 sessions"
        );

        // A filter matching EVERYTHING still says so. Derived from the
        // request rather than from `matching == total`, because a banner
        // that silently reverted to the unfiltered wording here would leave
        // a user unable to tell whether their filter took.
        let matched_all = count_banner(&filtered_listing(700, 700, 700, false));
        assert_eq!(matched_all.class, "banner session-count filtered");
        assert_eq!(matched_all.text, "700 matching of 700 sessions");
    }

    /// A filtered reply the cap cut reports all THREE numbers, and a
    /// filtered reply the cap did not cut is complete however far below
    /// the fleet total its matches fall.
    #[farhelm_testtrace::test]
    fn a_truncated_filtered_reply_separates_shown_from_matching_from_the_fleet() {
        let complete_filter = count_banner(&filtered_listing(40, 40, 700, false));
        assert_eq!(
            complete_filter.class, "banner session-count filtered",
            "40 rows against a 700-session fleet is a filter working, not a cut list"
        );

        let banner = count_banner(&filtered_listing(20, 40, 700, true));
        assert_eq!(banner.class, "banner truncation-banner filtered");
        assert_eq!(
            banner.text,
            "showing 20 of 40 matching sessions (700 in all) — could not read the list to the end"
        );
    }

    /// Absence is evidence only from a reply that could have carried the
    /// missing rows.
    ///
    /// The truncation half is the one that bites on a real fleet, and it
    /// destroys the user's own work rather than merely misreporting: a reply
    /// the cap cut omits every session past the cutoff, and reading
    /// those omissions as departures retires the rename someone just made,
    /// closes the editor they have open, and drops the delete confirmation
    /// they are mid-decision on. All three are the client's own state, so
    /// nothing on the next read brings them back.
    #[farhelm_testtrace::test]
    fn absence_speaks_only_for_a_complete_unfiltered_listing() {
        assert!(
            absence_is_evidence(&listing(3, 3, false)),
            "a complete unfiltered listing carries every session there is"
        );
        assert!(
            !absence_is_evidence(&listing(500, 20_000, true)),
            "a reply the cap cut says nothing about what lay past it"
        );
        assert!(
            !absence_is_evidence(&filtered_listing(3, 3, 700, false)),
            "and a filter omits what did not match, which is not what left"
        );
        assert!(
            !absence_is_evidence(&default_view_listing(3, 3)),
            "the default view reads as unfiltered and still hides archived rows, so a session \
             archived from another client must not be mistaken for one that left"
        );
    }

    /// The helm's flag is the ONLY thing that makes a listing incomplete;
    /// the rows and the counts are never compared against each other.
    ///
    /// Pinned in the negative because the comparison used to exist (the
    /// paged design's "underfilled listing" detector) and would be easy to
    /// reintroduce as a harmless-looking `shown < total`. It is not
    /// harmless: a reply is one snapshot, so any such disagreement is a
    /// helm bug rather than a partial read, and reacting to it by refusing
    /// absence-as-departure fleet-wide would silently freeze every
    /// optimistic rename, open editor and delete confirmation for as long
    /// as the disagreement lasted.
    #[farhelm_testtrace::test]
    fn only_the_helms_flag_makes_a_listing_incomplete() {
        assert!(listing_is_complete(&listing(3, 3, false)));
        assert!(
            listing_is_complete(&listing(2, 3, false)),
            "rows against count is not this side's question"
        );
        assert!(!listing_is_complete(&listing(3, 3, true)));
        assert!(listing_is_complete(&filtered_listing(2, 3, 700, false)));
        assert!(!listing_is_complete(&filtered_listing(3, 3, 700, true)));
        assert!(listing_is_complete(&old_peer_filtered_listing(2, 3, false)));
    }

    /// The bare "no sessions" line is for an empty FLEET, never for a filter
    /// that matched nothing.
    ///
    /// The two are indistinguishable in the counts — an empty fleet under a
    /// filter reports zero rows and a zero total, exactly like an unfiltered
    /// one — which is why the request has to be consulted. Taking the plain
    /// branch for a filtered reply suppresses both the banner and the
    /// no-match line, so a user who just searched is shown a fleet that
    /// appears to have vanished. That is the opposite of what happened, and
    /// the two situations call for opposite reactions.
    #[farhelm_testtrace::test]
    fn only_an_unfiltered_empty_fleet_gets_the_bare_no_sessions_line() {
        assert!(is_empty_fleet(&listing(0, 0, false)));
        assert!(
            !is_empty_fleet(&filtered_listing(0, 0, 0, false)),
            "a filter over an empty fleet is a search that found nothing, and must say so"
        );
        assert!(
            !is_empty_fleet(&default_view_listing(0, 0)),
            "the default view's zero total means nothing ACTIVE; a fleet of archived sessions \
             must not announce itself as empty"
        );
        assert!(
            !is_empty_fleet(&listing(0, 3, false)),
            "no rows against a non-zero total is not an empty fleet, whatever else it is"
        );

        // The shortcut skips `count_banner` entirely, so it may only be taken
        // by a reply with nothing left to report. Each of these says
        // something the calmest sentence on the page would swallow.
        assert!(
            !is_empty_fleet(&listing(0, 0, true)),
            "a truncated reply with no rows could not read the list, which is not the same as \
             there being nothing in it"
        );
        assert!(
            !is_empty_fleet(&SessionListing {
                matching: Some(4),
                ..listing(0, 0, false)
            }),
            "four matched out of a zero-session fleet is a contradiction, not an empty fleet"
        );
    }

    /// A helm that never answered the filter is described as exactly that,
    /// with no count invented for it.
    ///
    /// The rows on screen are the WHOLE fleet — a helm that predates the
    /// matching count predates server-side filtering too, so it ignored the
    /// query (`api::matching_count`). Substituting the fleet total would
    /// print "700 matching of 700 sessions" over those rows and vouch for a
    /// filter that never ran, which is the one thing a count line must never
    /// do. The unfiltered sentence plus a clause is the honest reading of
    /// what is actually being shown.
    #[farhelm_testtrace::test]
    fn a_filter_the_helm_ignored_is_said_out_loud_rather_than_counted() {
        let unanswered = old_peer_filtered_listing(700, 700, false);
        let banner = count_banner(&unanswered);
        assert_eq!(banner.class, "banner session-count filtered");
        assert_eq!(
            banner.text,
            "700 sessions — this helm does not support filtering, so the filter was ignored"
        );

        // A reply the cap cut keeps both facts: the cut in the numbers,
        // the ignored filter in the clause.
        let short = old_peer_filtered_listing(20, 700, true);
        let banner = count_banner(&short);
        assert_eq!(banner.class, "banner truncation-banner filtered");
        assert_eq!(
            banner.text,
            "showing 20 of 700 sessions — could not read the list to the end — this helm does \
             not support filtering, so the filter was ignored"
        );
    }

    /// The no-match line follows the helm's COUNT, never the emptiness of
    /// this page's rows — and under a cut list it claims only what the
    /// reply supports.
    ///
    /// The categorical sentence over a truncated reply would be a lie the
    /// user acts on: the caps cut per host BEFORE the fleet filter ran, so
    /// a match may sit past some host's cut while the retained prefix
    /// counted zero. Saying the line in the wrong cases is worse than
    /// saying nothing: it contradicts the banner beside it (which is
    /// reporting how many matched, or that the filter never ran) and sends
    /// the user off to change a query that was working.
    #[farhelm_testtrace::test]
    fn the_no_match_line_follows_the_matching_count_rather_than_the_rows() {
        assert_eq!(
            no_match_line(&filtered_listing(0, 0, 700, false)),
            Some("no sessions match this filter"),
            "the helm counted the matches over a complete listing and counted none"
        );
        assert_eq!(
            no_match_line(&filtered_listing(0, 0, 700, true)),
            Some(
                "no matches among the sessions that could be read — the list could not be read \
                 to the end"
            ),
            "a cut listing's zero is a count over a prefix, and the line must say so"
        );
        assert_eq!(
            no_match_line(&filtered_listing(0, 12, 700, true)),
            None,
            "twelve matched and this reply carried none of them: a truncation, not an empty search"
        );
        assert_eq!(
            no_match_line(&old_peer_filtered_listing(0, 0, false)),
            None,
            "a helm that ignored the filter counted nothing, so it cannot be quoted as counting zero"
        );
        assert_eq!(
            no_match_line(&listing(0, 0, false)),
            None,
            "and an unfiltered listing has no filter to report on"
        );
        assert_eq!(
            no_match_line(&default_view_listing(0, 0)),
            None,
            "an empty default view is a list with nothing in it, not a search that found \
             nothing — nobody typed a query to be told about"
        );
        assert_eq!(
            no_match_line(&filtered_listing(2, 0, 700, false)),
            None,
            "zero matches over visible rows is a contradiction the banner reports; the line would \
             argue with the rows"
        );
    }
}
