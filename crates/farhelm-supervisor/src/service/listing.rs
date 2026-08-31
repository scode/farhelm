//! `ListSessions`'s answer: the supervisor's whole session set in one
//! reply, ordered, cut at [`LIST_SESSIONS_CAP`], and described live.
//!
//! There is no page walk here, by contract. SPEC.md's Session list section
//! fixes the fleet this product is for at tens of sessions and says the
//! list is served WHOLE up to a fixed cap at every layer — no cursors, no
//! page sizes, no byte budgets. What this module owns is therefore small:
//! the order that decides WHICH rows survive the cap ([`list_order_key`]),
//! the cut itself ([`order_and_cut`]), and the live description of every
//! row that made it ([`list_all`]).
//!
//! [`list_all`] is the seam `handle_list_sessions` sits behind. It does the
//! whole of what a list request DOES — snapshot, order, cut, probe tmux,
//! witness exits, read the profile catalog — and none of how it answers:
//! no reply channel, no transport, no `req_id`. That is what lets the
//! listing be reasoned about, and tested, without a socket in sight.

use super::core::{SessionEntry, Supervisor};
use super::launch_artifacts::cleanup_launch_artifacts;
use super::status::{entry_info, observe_entry};
use crate::store::{LastOutcome, ProfileNames, Transition};
use anyhow::Context;
use farhelm_proto::{LIST_SESSIONS_CAP, SessionInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

/// The order a `SessionList` reply is in, and the order that decides
/// which rows survive the cap: creation time descending, with session id
/// ASCENDING as the tiebreak. Newest first, so a fleet past
/// [`LIST_SESSIONS_CAP`] loses its OLDEST sessions from view rather than
/// an arbitrary slice — the sessions a person is least likely to be
/// looking for.
///
/// The tiebreak DIRECTION is pinned by the protocol contract, not a free
/// choice of this build: `ControlMsg::SessionList`'s own docs
/// (`farhelm-proto`) specify "session id ascending as the tiebreak", so
/// this function's ordering must match that wording exactly, not merely
/// produce SOME fixed relative order for same-second creations.
///
/// `std::cmp::Reverse` on `created_at` is what turns "descending time,
/// ascending id" into a single ordinary ascending comparison other code
/// can sort by directly, rather than writing a hand-rolled
/// `Ordering::then_with` at every call site.
fn list_order_key(info: &SessionInfo) -> (std::cmp::Reverse<i64>, &str) {
    (std::cmp::Reverse(info.created_at), info.id.as_str())
}

/// Sort `items` into [`list_order_key`] order and keep the first
/// [`LIST_SESSIONS_CAP`]; the flag says whether anything was cut.
///
/// Generic over the item so the cut can be pinned on bare `SessionInfo`s
/// in tests while production runs it over `Arc<SessionEntry>`s; `info`
/// is how an item exposes the record the order reads. The flag is true
/// only when rows were actually dropped — a fleet that lands exactly on
/// the cap is complete, and saying otherwise would make every client
/// show a "could not read to the end" notice over a list it did read to
/// the end.
fn order_and_cut<T>(mut items: Vec<T>, info: impl Fn(&T) -> &SessionInfo) -> (Vec<T>, bool) {
    items.sort_by(|a, b| list_order_key(info(a)).cmp(&list_order_key(info(b))));
    let truncated = items.len() > LIST_SESSIONS_CAP;
    items.truncate(LIST_SESSIONS_CAP);
    (items, truncated)
}

/// What one `ListSessions` answers with, before it is put in a frame: the
/// rows that survived the cap, freshly described, and whether the cap
/// dropped any.
pub(crate) struct ListReply {
    pub(crate) sessions: Vec<SessionInfo>,
    pub(crate) truncated: bool,
}

/// Answer `ListSessions`: snapshot the session map, order and cap it, and
/// describe every entry that survived.
///
/// Two of this pass's reads are BATCHED — taken once for the whole list
/// rather than once per entry — and both are cut that way on purpose: one
/// tmux probe for liveness, and (only when some listed session names a
/// profile) one read of the profile catalog, from which every
/// source-profile existence is derived. That is a statement about those
/// two reads and not a count of everything this function does: the
/// observation loop below still reads a launch sentinel per entry that
/// has one, and commits its transitions in one batched write.
///
/// Every failure is an `Err` carrying the original error verbatim, and
/// every one of them fails the WHOLE request: an unreadable launch
/// sentinel or an unclassified tmux failure would otherwise leave this
/// list reporting an inference the unread file might contradict. Nothing
/// observed is committed before such a failure — see the observation loop
/// below.
pub(crate) async fn list_all(sup: &Supervisor) -> anyhow::Result<ListReply> {
    // The lock's own critical section is just the snapshot — clone every
    // `Arc<SessionEntry>` — nothing more: sorting and cutting the clone
    // happen AFTER the guard drops, so a request never holds
    // `sup.sessions` (and therefore blocks every OTHER connection's
    // create/list/stop/delete) for longer than the clone. No index over
    // the map, deliberately: the fleet is small enough that sorting a
    // snapshot per request is cheaper than the bookkeeping an index would
    // cost on every create and delete, and SPEC.md asks for none.
    //
    // The cut happens BEFORE any entry is status-annotated: the per-entry
    // observation below is not free, and past the cap it would be work
    // for rows the reply cannot carry.
    let snapshot: Vec<Arc<SessionEntry>> = {
        let sessions = sup.sessions.lock().await;
        sessions.values().cloned().collect()
    };
    let (entries, truncated) = order_and_cut(snapshot, |entry| &entry.info);
    // Before the reply is computed, so an identity claimed on
    // this very pass is reflected in the `restart_offer` it
    // carries rather than only in the next poll's. Cheap by
    // construction for the steady state (see `capture_pass`'s
    // cost envelope) and over EVERY session rather than this
    // reply's capped subset — see `Supervisor::capture_now` for
    // why the cap must not bind the ambiguity rule.
    //
    // KEPT after PLAN_M6_75.md item 1 gave the supervisor its own
    // ticker, deliberately: the ticker guarantees that capture
    // PROGRESSES with nobody connected, while this call is what
    // makes a REPLY fresh as of the request it answers. Proto v10
    // gives the supervisor edge no push, so a drain's reply is the
    // only way a client ever learns anything; the helm's
    // post-write wake exists precisely so a create is followed
    // immediately by a drain, and a drain answering from a sweep
    // that predates its own request would describe the world
    // before the write it is racing.
    //
    // That is why this is a `CaptureReason::Reply` pass and not
    // the old single-flight skip. The skip was wrong here in both
    // directions: it could reply from a PRE-COMMIT `restart_offer`
    // (somebody else's pass was in flight, so this one gave up),
    // and it did not actually make the two cadences free —
    // skipping only ever collapses passes that OVERLAP, and a
    // 2-second ticker beside a 3-second drain mostly does not.
    // Suppression on the TICKER side is what buys that back; see
    // `Supervisor::capture_pass_for` for the whole rule and its
    // real cost envelope.
    //
    // One sweep per `ListSessions`, and a `ListSessions` is the whole
    // list: with no pages there is nothing to multiply this by, and
    // the helm's 3-second drain pays exactly one sweep per host per
    // interval. That sweep is the dominant term in this path's cost.
    sup.capture_now().await;
    // ONE query for every session's liveness, not one per
    // session (`TmuxDriver::pane_states`'s own docs on why
    // that multiplies subprocess spawns under a polling UI) —
    // and skipped altogether when it could not possibly
    // change the answer: a terminal-less entry is decided
    // entirely by its recorded outcome (`session_status` never
    // consults the map for one), so a capped subset that is
    // ALL terminal-less (including the empty list) is fully
    // decidable without asking tmux anything. This matters
    // beyond just saving a subprocess spawn: it is what keeps
    // an authoritative "every session is a restart gap" (or
    // simply empty) listing from being turned into a spurious
    // `Internal` error by a private tmux server that happens
    // to ALSO be down for an unrelated reason.
    let pane_states = if entries.iter().any(|entry| entry.terminal.is_some()) {
        // An error here is reached only for a genuinely
        // UNCLASSIFIED tmux failure: `TmuxDriver::pane_states`
        // itself now tolerates a vanished private tmux server
        // (the whole reason a dead-tmux-server `ListSessions`
        // no longer lands here at all — see that method's
        // own docs for why an empty pane-states map is
        // honest, not fabricated, in that case).
        sup.tmux.pane_states().await?
    } else {
        HashMap::new()
    };
    // A list request is one of the places this supervisor
    // WITNESSES an exit (PLAN_M3.md item 2): the dead pane it
    // just found may be gone entirely by the next reboot,
    // taking its exit code with it, so the code is recorded
    // now — while tmux still has it — rather than recomputed
    // forever from a fact that expires. Every observation this
    // pass produces commits in ONE transaction, and the store
    // decides what each one means (`Transition::apply`), so a
    // stop running concurrently cannot have its annotation
    // erased by this list and this list cannot be misled by a
    // stale reading of its own.
    // A launch sentinel is READ regardless of whether this
    // supervisor `may_record()` (item 2 of the review-swarm
    // fix batch): a degraded supervisor (a handoff candidate,
    // or one whose boot-id read failed) still has standing to
    // REPORT what it can read, even though it must not WRITE a
    // conclusion it has no standing to store — the two halves
    // below are deliberately independent (`reply_status`
    // always reflects a sentinel this pass found; only
    // `observations` is gated on `may_record()`).
    //
    // A plain loop, not `observation()`'s pure `filter_map`
    // closure, because the sentinel check below is real I/O
    // (`read_launch_sentinel`) and therefore has to run
    // between two lock scopes rather than inside one
    // synchronous closure body: PLAN_M3.md item 3 wants the
    // SAME observation offered here that `reload_sessions`
    // offers — a non-terminal outcome whose pane is dead or
    // gone entirely gets its sentinel checked before falling
    // back to `observation()`'s plain exit inference, because
    // the sentinel outranks that inference exactly as surely
    // here as at reload, including (addition 18) for an entry
    // ALREADY recorded as an inferred `Interrupted` or
    // unannotated `Exited` — both are themselves only
    // inferences a sentinel is defined to beat.
    let mut observations: Vec<(String, i64, Transition)> = Vec::new();
    // This pass's sentinel finds, id to detail — used both to
    // gate post-commit file cleanup on the transition actually
    // landing, and (`reply_status`, below) to surface the
    // Error for THIS reply even when it could not be
    // committed durably this pass (`may_record()` false, or
    // the commit itself fails) — PLAN_M3.md item 3's
    // write-inability note: retain the file, retry
    // persistence on a later poll, but never let the reply
    // itself regress to a stale `Exited` in the meantime.
    let mut sentinel_hits: HashMap<String, String> = HashMap::new();
    for entry in &entries {
        // Loud propagation, not fall-through (item 1): the
        // WHOLE request fails rather than silently basing
        // this — or any other — entry's reply on an
        // inference the unreadable sentinel might
        // contradict. Nothing gathered so far this pass is
        // committed: this `?` short-circuits before
        // `transition_many` is ever called.
        let observed = observe_entry(sup, entry, &pane_states).await?;
        if observed.settled_error {
            cleanup_launch_artifacts(&sup.state_dir, &entry.info.id, entry.generation).await;
            continue;
        }
        if let Some(detail) = observed.sentinel {
            sentinel_hits.insert(entry.info.id.clone(), detail);
        }
        if let Some(transition) = observed.transition {
            observations.push((entry.info.id.clone(), entry.generation, transition));
        }
    }
    if !observations.is_empty() {
        match sup.store.transition_many(observations).await {
            Ok(committed) => {
                for entry in &entries {
                    if let Some(outcome) = committed.get(&entry.info.id) {
                        *entry.outcome.lock().expect("outcome mutex poisoned") = outcome.clone();
                    }
                }
                // Cleanup folded into this successful arm
                // (item 7), not a separate loop afterward: see
                // `reload_sessions`'s identical step for the
                // full lifecycle rationale (both files are
                // cosmetic once the durable outcome already
                // says what happened; a failed write must
                // leave them for the next pass to retry
                // against, hence gating on `committed` here
                // rather than on `sentinel_hits` alone).
                for entry in &entries {
                    if sentinel_hits.contains_key(&entry.info.id)
                        && matches!(
                            committed.get(&entry.info.id),
                            Some(LastOutcome::Error { .. })
                        )
                    {
                        cleanup_launch_artifacts(&sup.state_dir, &entry.info.id, entry.generation)
                            .await;
                    }
                }
            }
            // Logged, not fatal: the reply below is computed
            // from what this pass OBSERVED plus what is
            // durably recorded, both of which are still honest
            // when the write fails — and the next list retries.
            Err(e) => warn!(
                error = %format!("{e:#}"),
                "could not record observed session outcomes; \
                 the next list will retry"
            ),
        }
    }
    // ONE catalog read for the whole list (`farhelm_proto::SourceProfile`'s
    // note on per-snapshot lookup cost), and only when a session in it
    // actually names a profile — the overwhelmingly common fleet, where
    // every session is raw-created, pays nothing at all.
    //
    // A failed read FAILS the request rather than degrading to an empty
    // catalog, and the asymmetry is the point: an empty catalog is
    // indistinguishable from "every profile was deleted", so degrading
    // would make a transient database error render as a list of sessions
    // whose profiles are all gone. A failed list is retried in a second;
    // that lie is not correctable by anything.
    let profiles = if entries
        .iter()
        .any(|entry| entry.info.source_profile.is_some())
    {
        sup.store
            .profile_names()
            .await
            .context("reading the profile catalog to describe these sessions' source profiles")?
    } else {
        ProfileNames::new()
    };
    let sessions: Vec<SessionInfo> = entries
        .iter()
        .map(|entry| {
            entry_info(
                entry,
                &pane_states,
                sentinel_hits.get(&entry.info.id).map(String::as_str),
                &profiles,
            )
        })
        .collect();
    Ok(ListReply {
        sessions,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::{RestartOffer, SessionStatus};

    /// A minimal, distinct `SessionInfo` for the cut's own tests — distinct
    /// ids so a cut that drops the wrong entries (rather than merely the
    /// wrong COUNT) would still be caught.
    fn fake_session(id: &str, created_at: i64) -> SessionInfo {
        SessionInfo {
            parent: None,
            archived: false,
            id: id.to_string(),
            title: "x".to_string(),
            created_at,
            last_activity_at: created_at,
            creation_seq: None,
            cwd: "/tmp".to_string(),
            invocation: "agent".to_string(),
            status: SessionStatus::Running,
            annotation: None,
            restart_offer: RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        }
    }

    /// The reply's order is the protocol's: creation time descending, id
    /// ascending on ties. Pinned on the pure cut rather than through the
    /// handler because the same order decides what the cap drops, so a
    /// wrong tiebreak here would silently change WHICH sessions a fleet
    /// past the cap loses.
    #[test]
    fn the_order_is_newest_first_with_ids_ascending_on_ties() {
        let items = vec![
            fake_session("b", 10),
            fake_session("a", 10),
            fake_session("c", 30),
            fake_session("d", 20),
        ];
        let (ordered, truncated) = order_and_cut(items, |s| s);
        let ids: Vec<&str> = ordered.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["c", "d", "a", "b"]);
        assert!(!truncated, "nothing near the cap was cut");
    }

    /// The cap keeps the NEWEST rows and says so; a fleet that lands
    /// exactly on the cap is complete. Both halves matter to what a client
    /// shows: the flag is the only thing behind SPEC.md's "could not read
    /// to the end" notice, so a false positive at the boundary would put
    /// that notice over a list the client did read to the end, and a cut
    /// that kept the oldest rows would hide exactly the sessions a person
    /// is looking for.
    #[test]
    fn the_cap_keeps_the_newest_rows_and_flags_only_a_real_cut() {
        let at_cap: Vec<SessionInfo> = (0..LIST_SESSIONS_CAP)
            .map(|i| fake_session(&format!("s{i:04}"), i as i64))
            .collect();
        let (kept, truncated) = order_and_cut(at_cap, |s| s);
        assert_eq!(kept.len(), LIST_SESSIONS_CAP);
        assert!(!truncated, "landing exactly on the cap is not a cut");

        let past_cap: Vec<SessionInfo> = (0..=LIST_SESSIONS_CAP)
            .map(|i| fake_session(&format!("s{i:04}"), i as i64))
            .collect();
        let (kept, truncated) = order_and_cut(past_cap, |s| s);
        assert!(truncated, "one row past the cap is a cut");
        assert_eq!(kept.len(), LIST_SESSIONS_CAP);
        assert_eq!(
            kept.first().map(|s| s.created_at),
            Some(LIST_SESSIONS_CAP as i64),
            "the newest row survives"
        );
        assert!(
            kept.iter().all(|s| s.created_at > 0),
            "the oldest row is the one dropped"
        );
    }
}
