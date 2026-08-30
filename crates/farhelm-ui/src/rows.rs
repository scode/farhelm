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
//! reads a session's ABSENCE as departure, which is only true of a walk that
//! speaks for the whole fleet. Fusing them (as this module once did) makes
//! the weaker requirement govern both, and a filtered read then cannot even
//! graduate a rename the server is already reporting back.
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
/// walk with authority may do.
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
/// true of a walk that speaks for the whole fleet: a FILTERED listing omits
/// every session that did not match, and "did not match" is not "went away".
/// Calling this on a filtered reply would retire a correction on the
/// authority of a query that never asked about that row — and, since the
/// rename overlay is what keeps the user's own rename on screen, would drop
/// it back to the old title for as long as any filter is applied.
///
/// The `index` bound is the same conservative one, and matters for the same
/// reason: a walk that started before the rename can be missing the session
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
/// Both counts are checked rather than only the rows, because a walk that
/// returned no rows against a non-zero total is a truncation, not an empty
/// fleet — that one belongs in the banner's "showing 0 of N" wording.
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
        // A COMPLETE, COHERENT reply, or the shortcut is a claim the reply
        // does not support. A truncated walk with no rows says "I could not
        // read this list", not "there is nothing in it" — and an incoherent
        // one, or a `matching` the fleet total contradicts, is precisely
        // what `count_banner` exists to report. Skipping the banner here
        // would swallow every one of those and print the calmest possible
        // sentence over a reply that is telling the client something is
        // wrong.
        && !listing.truncated
        && !listing.incoherent
        && matches!(listing.matching, None | Some(0))
}

/// Whether this reply holds every row it claims exist — the exact negation
/// of what [`count_banner`] prints "showing N of M" for.
///
/// ONE definition of "this list is short", read by three callers: the banner
/// (which says so to the user), [`absence_is_evidence`] (which refuses to
/// read a missing row as a departure), and `list::ListView`'s auto-select
/// fallback (which cannot pick "the newest-created session" out of a prefix
/// that may not contain it). Two of those are silent, which is precisely why
/// they must not carry their own approximation of the third's rule: a UI that
/// tells the user its list is incomplete and then reasons as if it were
/// complete is worse than one that does neither.
///
/// The conditions are [`count_banner`]'s, and its docs carry the full
/// argument for each — the ceilings behind `truncated`, the two count
/// contradictions the helm's own flags cannot express, and why the
/// denominator is `matching` for a filtered reply and the view's `total`
/// otherwise.
///
/// What is worth stating HERE is the plain shortfall, because it is the
/// condition that changed meaning. Fewer rows than the denominator, with no
/// flag set at all, became reachable when the list gained an order the user
/// picks: sort keys are MUTABLE — a session's activity stamp moves when it
/// prints, its title moves when someone renames it — so a live row can cross
/// the cursor's position between two pages of one walk and be served by
/// neither, while the walk ends normally with `next_cursor` absent. Nothing
/// in such a reply says a row went missing. Only the counts do.
pub(crate) fn listing_is_complete(listing: &SessionListing) -> bool {
    let shown = listing.sessions.len() as u64;
    // What the rows on screen are a subset OF — always the number the
    // banner's chosen arm prints as its denominator, which is what keeps
    // "showing N of M" measurable against the M beside it.
    //
    // For an unfiltered banner that is `total`, even where the helm reported
    // a matching count: it does so for the default view (the archive
    // exclusion is a predicate on its side), and there the two differ only by
    // rows nothing can show. Measuring against that count would make the same
    // fleet read as a complete walk in the default view and a truncated one
    // with the archive switch on, which reports no matching count at all.
    let claimed = if listing.filtered {
        listing.matching.unwrap_or(listing.total)
    } else {
        listing.total
    };
    !(listing.truncated || counts_are_incoherent(listing) || shown < claimed)
}

/// Whether this reply's counts and rows disagree in a direction that means
/// the list CHANGED under the walk, rather than that there is more of it.
///
/// The condition behind [`count_banner`]'s incoherence note, and half of
/// [`listing_is_complete`] — split out because the banner needs the two
/// answers separately: a walk can be short without anything having moved (a
/// ceiling), and the note must appear only where something actually did.
///
/// Three sources, and the last two are contradictions the helm's own flags
/// cannot express (`api::fetch_sessions` measures incoherence against the
/// VIEW's total, which is the helm's reading and stays that way):
///
/// - `incoherent` from the walk itself.
/// - **More rows than matched.** A walk can hold more rows than the final
///   page's refreshed `matching` while still sitting under the view's total,
///   so the flags alone would print "3 matching of 700" above five visible
///   rows.
/// - **More matched than exist.** `matching > total` cannot be true of any
///   view; it is what two counts read at different moments look like.
///
/// Both contradictions are checked wherever the helm made a claim to
/// contradict — `matching.is_some()`, not `filtered`. The default view is
/// why that distinction matters: it reads as unfiltered while the helm still
/// answers it with a real matching count, so keying these on `filtered`
/// would leave "5 sessions" printed above five rows the helm said only three
/// of matched.
fn counts_are_incoherent(listing: &SessionListing) -> bool {
    let shown = listing.sessions.len() as u64;
    listing.incoherent
        || listing.matching.is_some_and(|matching| shown > matching)
        || listing
            .matching
            .is_some_and(|matching| matching > listing.total)
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
/// - **The walk actually finished**, which is [`listing_is_complete`] and is
///   more than the `truncated` flag alone. A truncated walk stops at a
///   ceiling — pages, rows, bytes, time — and every session past that cutoff
///   is missing for a reason that has nothing to do with existing. So is
///   every session an UNDERFILLED walk failed to serve: with a mutable sort
///   key a live row can cross the cursor mid-walk and be skipped by a walk
///   that ends looking finished, and that row's absence is exactly as
///   meaningless as a truncated walk's.
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

/// Whether the list should say, in words, that this filter matched nothing.
///
/// The claim is about the FLEET, not about the page: "no sessions match this
/// filter" is only true when the helm counted the matches and counted zero.
/// An empty row vector is not the same statement, and reading it as one gets
/// two cases wrong in the direction that misinforms:
///
/// - A walk that matched something and collected none of it (a ceiling hit
///   on its first page, a time-bounded walk) would tell the user their
///   search found nothing while the banner beside it reports how many it
///   found.
/// - An IGNORED filter (`matching` absent — see `api::matching_count`)
///   would produce the line on the strength of a count nobody computed, on
///   a helm that never filtered at all.
///
/// Paired with the rows being empty as well, because `matching == 0` over a
/// non-empty row vector is a contradiction the banner is already reporting
/// (see [`count_banner`]) — printing "nothing matched" above visible rows
/// would be the UI arguing with itself.
pub(crate) fn no_matches(listing: &SessionListing) -> bool {
    listing.filtered && listing.matching == Some(0) && listing.sessions.is_empty()
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

/// The suffix a banner carries when the rows and the counts disagree in a
/// direction that means the list CHANGED under the walk, rather than that
/// there is more of it.
///
/// A constant because two conditions raise it — the helm's own fleet-level
/// incoherence and this module's filter-level one — and a second copy of the
/// sentence would let the two drift into meaning slightly different things.
const INCOHERENCE_NOTE: &str = " — the list changed while it was being read; refreshing";

/// The clause a filtered banner carries when the helm never answered the
/// filter (`api::matching_count` returning `None`).
///
/// Said plainly rather than dressed as a count, because the alternative is
/// the UI vouching for a filter that did not run: the rows on screen are the
/// whole fleet, and any "N matching" over them is a number nobody computed.
const FILTER_UNSUPPORTED_NOTE: &str =
    " — this helm does not support filtering, so the filter was ignored";

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
/// - **Truncation.** "showing N of …" is reserved for a walk that did not
///   finish — the client walks the cursor to exhaustion
///   (`api::fetch_sessions`), so an incomplete list means a ceiling was hit,
///   the helm reported more behind its last page, or the counts came back
///   incoherent. Every condition is checked because they can disagree:
///   totals can differ under concurrent creation without `truncated` being
///   set, and a ceiling sets `truncated` without the totals having to
///   differ. The shortfall is measured against WHATEVER THIS BANNER IS
///   ABOUT TO PRINT as its denominator, which is what keeps "showing N of
///   M" checkable against the M beside it: `matching` for a filtered banner
///   (the walk is over what matched, and measuring a working filter against
///   the view's size would report every one of them as truncated), the
///   view's own `total` for an unfiltered one. The default view is why
///   those two are not the same thing — the helm answers it with a real
///   `matching`, because its archive exclusion is a server-side predicate,
///   while the sentence a reader sees counts the view.
/// - **Filtering.** A filtered list says "N matching of M sessions"
///   (PLAN_M6_75.md item 7), which is the distinction the second count
///   exists to make: without it, a filter that hid 690 of 700 rows and a
///   walk that could only read 10 of them look identical on screen, and only
///   one of those means "there is more to see". The filtered wording is
///   chosen from the REQUEST rather than by comparing the two counts, so a
///   filter that happens to match everything still says so — a banner that
///   silently reverted to the unfiltered sentence would leave a user unsure
///   whether their filter took at all.
///
///   The archive switch is NOT one of those filters, in either position.
///   `M` is the size of the view the rows came from — the non-archived list
///   by default, the whole fleet with the switch on (`api::SessionListing::
///   total`, and `store::count_rows` on the helm side) — so the ordinary
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
/// ## Contradictions the helm's own flags cannot express
///
/// `api::fetch_sessions` measures incoherence against the VIEW's total,
/// which is the helm's own reading of the two numbers and stays that way.
/// Two contradictions live below that check and have to be caught here,
/// because both produce a confident banner that the rows underneath it
/// disprove:
///
/// - **More rows than matched.** A walk can hold more rows than the final
///   page's refreshed `matching` while still sitting under the view's total
///   — neither short nor view-incoherent — so the flags alone would print
///   "3 matching of 700" above five visible rows. Checked on any listing
///   carrying a `matching`, filtered or not: the DEFAULT view carries one
///   too, and there the unfiltered sentence would be the confident half of
///   the same contradiction.
/// - **More matched than exist.** `matching > total` cannot be true of any
///   view; it is what a list changing under a multi-page walk looks like
///   when the two counts come from different moments.
///
/// Both mean the same thing as the view-scoped version (the list moved
/// under the walk), so both raise the same note and take the same truncated
/// wording, which already exist for it.
///
/// A filtered banner whose helm gave NO matching count falls back to the
/// view's total for the same shortfall test, which is the ignored-filter
/// case: those rows are the whole view, so a walk that stopped short of it
/// has to say "showing N of M" like any other, rather than presenting an
/// ignored filter's partial list as if it were everything.
pub(crate) fn count_banner(listing: &SessionListing) -> CountBanner {
    let shown = listing.sessions.len() as u64;
    // The shortfall test itself lives in `listing_is_complete`, and sharing
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
            format!("showing {shown} of {} sessions", listing.total),
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
                "showing {shown} of {matching} matching sessions ({} in all)",
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
                "showing {shown} of {} sessions{FILTER_UNSUPPORTED_NOTE}",
                listing.total
            ),
        ),
    };
    CountBanner {
        class,
        text,
        // Only ever on a banner whose wording already admits something is
        // off: a complete-walk sentence with a note saying the list changed
        // underneath would contradict itself in one line. Both local
        // contradictions force `short`, so the two conditions never
        // disagree.
        incoherence: (short && counts_are_incoherent(listing)).then_some(INCOHERENCE_NOTE),
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
    /// row visibly flips back until another read lands. A reply that
    /// postdates the rename is authoritative in both directions —
    /// agreement retires the correction, and disagreement (another client's
    /// later rename) retires it too, because the server wins.
    ///
    /// What this deliberately does NOT test is absence, which is
    /// `retire_vanished_renames`' evidence and not every read's to use.
    #[test]
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
    #[test]
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

    /// Absence retires a correction only for a walk that speaks for the
    /// whole fleet, and only once it is late enough to be evidence.
    ///
    /// Both halves matter. Without the authority check a filtered read would
    /// retire corrections for every row it excluded — dropping the user's
    /// own rename back to the old title for as long as a filter is applied.
    /// Without the index check a walk already in flight when the rename
    /// landed would do the same on a session it simply had not reached yet.
    #[test]
    fn only_a_walk_with_authority_reads_absence_as_departure() {
        let renamed: HashMap<String, (String, u64)> = [("a".to_string(), ("new-a".to_string(), 5))]
            .into_iter()
            .collect();

        let mut early = renamed.clone();
        retire_vanished_renames(&mut early, &[], 4);
        assert!(
            early.contains_key("a"),
            "a walk that started before the rename may simply not have seen it yet"
        );

        let mut vanished = renamed.clone();
        retire_vanished_renames(&mut vanished, &[], 6);
        assert!(
            vanished.is_empty(),
            "a session a later whole-fleet walk does not carry has no row to correct"
        );

        // A whole-fleet walk that DOES list the session leaves the
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
    #[test]
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
    #[test]
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
    #[test]
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
    #[test]
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
    /// the walk actually builds them, or they would pin wording against a
    /// reply shape that cannot occur.
    fn listing(rows: usize, total: u64, truncated: bool, incoherent: bool) -> SessionListing {
        SessionListing {
            sessions: (0..rows)
                .map(|n| session(&format!("s{n}"), &format!("title-{n}")))
                .collect(),
            total,
            matching: Some(total),
            filtered: false,
            omits_fleet_members: false,
            truncated,
            incoherent,
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
            ..listing(rows, total, false, false)
        }
    }

    /// The same, for a walk that carried a filter a PERSON applied:
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
            ..listing(rows, total, truncated, false)
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
            assert_eq!(
                banner.incoherence, None,
                "no incoherence was reported, so no suffix belongs on {text:?}"
            );
        }
    }

    /// An unfiltered banner measures its shortfall against the number it
    /// PRINTS, even when the helm also sent a matching count.
    ///
    /// The default view is the case: the helm treats its archive exclusion as
    /// a predicate and answers with a `matching` that omits rows no page can
    /// show, while the banner's denominator is `total`. Measured against
    /// `matching` instead, a fleet holding one corrupt row would read as a
    /// finished walk here and as "showing N of M" the moment the archive
    /// switch went on — same fleet, same rows, two different verdicts, with
    /// the more confident one wrong.
    #[test]
    fn an_unfiltered_shortfall_is_measured_against_the_printed_total() {
        let with_an_unshowable_row = SessionListing {
            matching: Some(3),
            ..default_view_listing(3, 4)
        };
        let banner = count_banner(&with_an_unshowable_row);
        assert_eq!(banner.class, "banner truncation-banner");
        assert_eq!(
            banner.text, "showing 3 of 4 sessions",
            "three rows under a view of four is a subset, and the banner says which numbers"
        );
        assert_eq!(banner.incoherence, None);

        // And the ordinary default view, where the two counts agree, keeps
        // the confident sentence.
        assert_eq!(
            count_banner(&default_view_listing(4, 4)).class,
            "banner session-count"
        );
    }

    /// The default view holding MORE rows than the helm said matched is a
    /// contradiction, and it is reported even though the sentence reads as
    /// unfiltered.
    ///
    /// The shape that makes this reachable is new: the helm applies the
    /// archive exclusion itself, so the default view comes back with a real
    /// `matching`, and a multi-page walk can end up holding rows the final
    /// page's refreshed count no longer covers. `shown <= total` throughout,
    /// so neither the helm's own row-against-view check nor the
    /// `matching > total` one fires — the confident "5 sessions" would be
    /// printed above five rows the same reply said three of matched.
    #[test]
    fn a_default_view_holding_more_rows_than_matched_is_reported() {
        let moved_under_the_walk = SessionListing {
            matching: Some(3),
            ..default_view_listing(5, 5)
        };
        let banner = count_banner(&moved_under_the_walk);
        assert_eq!(banner.class, "banner truncation-banner");
        assert_eq!(
            banner.text, "showing 5 of 5 sessions",
            "the wording still measures against the printed denominator; the note carries the \
             contradiction"
        );
        assert_eq!(banner.incoherence, Some(INCOHERENCE_NOTE));
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

    /// A filtered list says "N matching of M sessions" (PLAN_M6_75.md item
    /// 7) — the distinction the helm's second count exists to make.
    ///
    /// Without it, a filter that hid 690 of 700 sessions and a walk that
    /// could only read 10 of them produce the same sentence, and only one of
    /// those means "there is more to see". The class carries a `filtered`
    /// modifier ON TOP of the existing one rather than replacing it, so
    /// every stylesheet rule and browser assertion written against
    /// `.session-count` / `.truncation-banner` keeps matching — the wording
    /// change is not an excuse to move the selectors the suite already pins.
    #[test]
    fn a_filtered_list_reports_both_counts() {
        let banner = count_banner(&filtered_listing(12, 12, 700, false));
        assert_eq!(banner.class, "banner session-count filtered");
        assert_eq!(banner.text, "12 matching of 700 sessions");
        assert_eq!(banner.incoherence, None);

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

    /// A filtered walk that stopped short reports all THREE numbers.
    ///
    /// The shortfall is measured against MATCHING, not against the fleet
    /// total, and that is the bug this pins: the page walk is over what
    /// matched, so comparing rows against the fleet would report every
    /// working filter as a truncated list — a permanent "there is more to
    /// see" over a list that is already complete.
    #[test]
    fn a_truncated_filtered_walk_separates_shown_from_matching_from_the_fleet() {
        let complete_filter = count_banner(&filtered_listing(40, 40, 700, false));
        assert_eq!(
            complete_filter.class, "banner session-count filtered",
            "40 rows against a 700-session fleet is a filter working, not a walk stopping"
        );

        let banner = count_banner(&filtered_listing(20, 40, 700, true));
        assert_eq!(banner.class, "banner truncation-banner filtered");
        assert_eq!(
            banner.text,
            "showing 20 of 40 matching sessions (700 in all)"
        );

        // And the shortfall alone is enough, with `truncated` unset — the
        // same independence the unfiltered branches have.
        assert_eq!(
            count_banner(&filtered_listing(20, 40, 700, false)).class,
            "banner truncation-banner filtered"
        );
    }

    /// Incoherence is reported on a filtered banner too, and stays a
    /// separate run.
    ///
    /// The check behind it is deliberately against the FLEET total rather
    /// than against `matching` (`api::fetch_sessions`), which is the helm's
    /// own reading of the two numbers: holding fewer rows than the fleet is
    /// a filter, while holding MORE rows than the fleet is a list that
    /// changed under the walk.
    #[test]
    fn a_filtered_banner_still_carries_the_incoherence_note() {
        let banner = count_banner(&SessionListing {
            incoherent: true,
            ..filtered_listing(5, 3, 3, true)
        });
        assert_eq!(banner.class, "banner truncation-banner filtered");
        assert_eq!(banner.text, "showing 5 of 3 matching sessions (3 in all)");
        assert_eq!(banner.incoherence, Some(INCOHERENCE_NOTE));
    }

    /// More rows than the filter matched is incoherence the helm's own flags
    /// cannot express, and the banner has to catch it.
    ///
    /// The shape is ordinary: a walk collects rows, the list changes, and the
    /// LAST page's refreshed `matching` comes back below the number of rows
    /// already taken — all while staying under the fleet total, so
    /// `api::fetch_sessions`' fleet-scoped check sees nothing and neither
    /// `truncated` nor `incoherent` is set. Left alone, the banner would say
    /// "3 matching of 700 sessions" over five visible rows: the one line
    /// whose whole job is to be believed, contradicted by the rows beneath
    /// it.
    #[test]
    fn more_rows_than_matched_is_reported_as_incoherence() {
        let banner = count_banner(&filtered_listing(5, 3, 700, false));
        assert_eq!(
            banner.class, "banner truncation-banner filtered",
            "the wording that admits the counts are unsettled, not the confident one"
        );
        assert_eq!(banner.text, "showing 5 of 3 matching sessions (700 in all)");
        assert_eq!(banner.incoherence, Some(INCOHERENCE_NOTE));

        // The ordinary filter is untouched: fewer rows than matched is a walk
        // that stopped short, and rows equal to matched is a filter working.
        assert_eq!(
            count_banner(&filtered_listing(3, 3, 700, false)).incoherence,
            None
        );
        assert_eq!(
            count_banner(&filtered_listing(2, 3, 700, false)).incoherence,
            None
        );
    }

    /// Absence is evidence only from a reply that could have carried the
    /// missing rows.
    ///
    /// The truncation half is the one that bites on a real fleet, and it
    /// destroys the user's own work rather than merely misreporting: a walk
    /// stopped at a ceiling omits every session past the cutoff, and reading
    /// those omissions as departures retires the rename someone just made,
    /// closes the editor they have open, and drops the delete confirmation
    /// they are mid-decision on. All three are the client's own state, so
    /// nothing on the next read brings them back.
    #[test]
    fn absence_speaks_only_for_a_complete_unfiltered_walk() {
        assert!(
            absence_is_evidence(&listing(3, 3, false, false)),
            "a finished unfiltered walk carries every session there is"
        );
        assert!(
            !absence_is_evidence(&listing(500, 20_000, true, false)),
            "a walk stopped at a ceiling says nothing about what lay past it"
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
        // Incoherence arrives as truncation from the walk
        // (`api::fetch_sessions` sets `truncated: truncated || incoherent`),
        // which is what makes one check cover both: counts that disagree
        // with the rows are no basis for declaring anything gone.
        assert!(!absence_is_evidence(&listing(5, 3, true, true)));
    }

    /// A walk that ended cleanly while still SHORT of its own count speaks
    /// for nothing either — the case no flag on the reply reports.
    ///
    /// Short by one row, because one is all it takes and one is what the
    /// bug looks like in the field. The order the sidebar now offers is
    /// keyed on values that move while a walk is in progress (a session's
    /// activity stamp advances the moment it prints; its title changes when
    /// anyone renames it), so a live row can cross the cursor between two
    /// pages and be served by neither. The walk ends normally: no ceiling
    /// was hit, `next_cursor` is absent, `truncated` and `incoherent` are
    /// both false. Only the counts betray it.
    ///
    /// Believing such a reply is the most destructive reading in this file.
    /// The missing row is a LIVE session, so `commit_listing` would retire
    /// its rename, close its editor, drop its confirmation — and, if it is
    /// the selected one, tear the pane down and auto-open something else,
    /// all while the session is running perfectly well.
    #[test]
    fn an_underfilled_walk_is_not_evidence_of_departure() {
        assert!(
            !listing_is_complete(&listing(2, 3, false, false)),
            "two rows against a count of three is an incomplete walk however it ended"
        );
        assert!(
            !absence_is_evidence(&listing(2, 3, false, false)),
            "so the session it failed to serve must not be read as one that left"
        );
        // The boundary in both directions: exactly the counted rows is
        // complete, and MORE than counted is the incoherent case, which the
        // walk already reports but which must not be waved through here by
        // a `>=` comparison read on its own.
        assert!(listing_is_complete(&listing(3, 3, false, false)));
        assert!(!listing_is_complete(&SessionListing {
            incoherent: true,
            ..listing(4, 3, false, false)
        }));
        // The filtered reading: `matching` is the number a complete walk
        // would have carried, so it — not the fleet total — is what a
        // filtered walk is short of. A filtered listing has no absence
        // standing anyway, but the auto-select fallback asks the same
        // question and does consult filtered replies.
        assert!(listing_is_complete(&filtered_listing(3, 3, 700, false)));
        assert!(!listing_is_complete(&filtered_listing(2, 3, 700, false)));
        // And a helm that reports no `matching` at all falls back to the
        // fleet total, which is the same number for the walk it served (it
        // ignored the filter — see `api::matching_count`).
        assert!(listing_is_complete(&old_peer_filtered_listing(3, 3, false)));
        assert!(!listing_is_complete(&old_peer_filtered_listing(
            2, 3, false
        )));
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
    #[test]
    fn only_an_unfiltered_empty_fleet_gets_the_bare_no_sessions_line() {
        assert!(is_empty_fleet(&listing(0, 0, false, false)));
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
            !is_empty_fleet(&listing(0, 3, false, false)),
            "no rows against a non-zero total is a truncated walk, which the banner reports"
        );

        // The shortcut skips `count_banner` entirely, so it may only be taken
        // by a reply with nothing left to report. Each of these says
        // something the calmest sentence on the page would swallow.
        assert!(
            !is_empty_fleet(&listing(0, 0, true, false)),
            "a truncated walk with no rows could not read the list, which is not the same as \
             there being nothing in it"
        );
        assert!(
            !is_empty_fleet(&listing(0, 0, false, true)),
            "an incoherent reply is exactly what the banner's note exists for"
        );
        assert!(
            !is_empty_fleet(&SessionListing {
                matching: Some(4),
                ..listing(0, 0, false, false)
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
    #[test]
    fn a_filter_the_helm_ignored_is_said_out_loud_rather_than_counted() {
        let unanswered = old_peer_filtered_listing(700, 700, false);
        let banner = count_banner(&unanswered);
        assert_eq!(banner.class, "banner session-count filtered");
        assert_eq!(
            banner.text,
            "700 sessions — this helm does not support filtering, so the filter was ignored"
        );
        assert_eq!(banner.incoherence, None);

        // A walk that also stopped short keeps both facts: the shortfall in
        // the numbers, the ignored filter in the clause.
        let short = old_peer_filtered_listing(20, 700, true);
        let banner = count_banner(&short);
        assert_eq!(banner.class, "banner truncation-banner filtered");
        assert_eq!(
            banner.text,
            "showing 20 of 700 sessions — this helm does not support filtering, so the filter was \
             ignored"
        );
    }

    /// An ignored filter whose walk fell short of the FLEET says "showing N
    /// of M", even with nothing flagged.
    ///
    /// The rows an ignoring helm returns are the whole fleet, so the number
    /// they are a subset of is `total` — there is no matching count to
    /// measure against. Without that substitution the shortfall check has
    /// nothing to compare and the confident sentence wins: "700 sessions"
    /// printed over twenty rows, on a banner whose entire job is to say when
    /// the list on screen is not the list that exists.
    #[test]
    fn an_ignored_filters_shortfall_is_measured_against_the_fleet() {
        let cut_short = old_peer_filtered_listing(20, 700, false);
        let banner = count_banner(&cut_short);
        assert_eq!(
            banner.class, "banner truncation-banner filtered",
            "twenty rows out of a 700-session fleet is a walk that stopped, whatever the flags say"
        );
        assert_eq!(
            banner.text,
            "showing 20 of 700 sessions — this helm does not support filtering, so the filter was \
             ignored"
        );

        // A complete unfiltered walk under an ignored filter still reads as
        // complete: the rows ARE the fleet, and claiming otherwise would be
        // the same lie in the opposite direction.
        let complete = old_peer_filtered_listing(700, 700, false);
        assert_eq!(
            count_banner(&complete).class,
            "banner session-count filtered"
        );
    }

    /// More matched than exist is a contradiction, and the banner says so.
    ///
    /// `matching > total` cannot describe any view. It is what a list
    /// changing under a multi-page walk looks like when the two counts are
    /// read a moment apart, and it arrives with neither flag set — the
    /// helm's own incoherence check is about ROWS against the view, and
    /// this is one count against the other. Left alone the banner would
    /// print "40 matching of 3 sessions" in the confident wording, which is
    /// the count line contradicting itself in a single sentence.
    ///
    /// Pinned for the UNFILTERED default view as well, because that listing
    /// carries a real `matching` while printing the plain sentence: the
    /// contradiction is invisible in its wording, so a check keyed on
    /// `filtered` would let "3 sessions" stand above a reply that just said
    /// 40 of them matched.
    #[test]
    fn more_matched_than_exist_is_reported_as_incoherence() {
        let banner = count_banner(&filtered_listing(3, 40, 3, false));
        assert_eq!(banner.class, "banner truncation-banner filtered");
        assert_eq!(banner.text, "showing 3 of 40 matching sessions (3 in all)");
        assert_eq!(banner.incoherence, Some(INCOHERENCE_NOTE));

        let unfiltered = count_banner(&SessionListing {
            matching: Some(40),
            ..default_view_listing(3, 3)
        });
        assert_eq!(unfiltered.class, "banner truncation-banner");
        assert_eq!(
            unfiltered.text, "showing 3 of 3 sessions",
            "the unfiltered sentence has nowhere to print the impossible count, so the note \
             beside it is the whole report"
        );
        assert_eq!(unfiltered.incoherence, Some(INCOHERENCE_NOTE));

        // The ordinary case is untouched: matching below the view's total is
        // what every working filter looks like.
        assert_eq!(
            count_banner(&filtered_listing(3, 3, 700, false)).incoherence,
            None
        );
    }

    /// The no-match line follows the helm's COUNT, never the emptiness of
    /// this page's rows.
    ///
    /// Three ways to have no rows and only one of them means "your search
    /// found nothing". Saying it in the other two is worse than saying
    /// nothing: it contradicts the banner beside it (which is reporting how
    /// many matched, or that the filter never ran) and sends the user off to
    /// change a query that was working.
    #[test]
    fn the_no_match_line_follows_the_matching_count_rather_than_the_rows() {
        assert!(
            no_matches(&filtered_listing(0, 0, 700, false)),
            "the helm counted the matches and counted none"
        );
        assert!(
            !no_matches(&filtered_listing(0, 12, 700, true)),
            "twelve matched and this walk collected none of them: a truncation, not an empty search"
        );
        assert!(
            !no_matches(&old_peer_filtered_listing(0, 0, false)),
            "a helm that ignored the filter counted nothing, so it cannot be quoted as counting zero"
        );
        assert!(
            !no_matches(&listing(0, 0, false, false)),
            "and an unfiltered listing has no filter to report on"
        );
        assert!(
            !no_matches(&default_view_listing(0, 0)),
            "an empty default view is a list with nothing in it, not a search that found \
             nothing — nobody typed a query to be told about"
        );
        assert!(
            !no_matches(&filtered_listing(2, 0, 700, false)),
            "zero matches over visible rows is a contradiction the banner reports; the line would \
             argue with the rows"
        );
    }
}
