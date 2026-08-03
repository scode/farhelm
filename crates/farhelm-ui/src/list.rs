//! The session list: `ListView` (the flat, polled listing plus its
//! stop/delete/create/rename actions), `SessionRow` (one row, including
//! the inline delete-confirmation prompt and the inline rename field), and
//! `CreateSessionForm` (the "new session" inline form). All three are
//! `ListView`'s own concern — none of them is meaningful mounted outside
//! it — so only `ListView` itself is `pub(crate)`; `SessionRow` and
//! `CreateSessionForm` stay private to this module. The rename FIELD is
//! the one exception: `rename::RenameForm` is shared with the session
//! view, since SPEC.md puts the same operation on both surfaces.

use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;

use crate::api::{
    POLL_INTERVAL_MS, SessionListing, create_session, delete_session, fetch_sessions,
    mint_intent_key, rename_session, stop_session,
};
use crate::rename::RenameForm;
use crate::{ApiBase, Session, SessionStatus};

/// The subset of `Session` `on_delete` actually needs, in `ListView` below:
/// the id the API call targets, plus `status` to decide whether this click
/// deletes immediately or opens the inline confirm prompt. No `title` —
/// unlike before this file's eval-based `window.confirm()` was replaced
/// with `SessionRow`'s in-page one, `on_delete` itself never builds any
/// confirm wording anymore; `confirm_message` (near `SessionRow`) computes
/// that straight from the row's own live `session.title`/`session.status`
/// on every render instead, so a title never needs to travel through this
/// type at all. Deliberately narrower than the whole `Session` — `on_delete`
/// has no legitimate reason to depend on `cwd` or `invocation` either, and a
/// dedicated type is what makes that impossible rather than merely
/// unlikely, unlike `on_open` (which keeps taking the whole `Session`: it
/// needs every field to populate `SessionView`).
#[derive(Debug, Clone)]
struct DeleteTarget {
    id: String,
    status: SessionStatus,
}

/// The flat session list: title, cwd, invocation, and a truthful status
/// badge per row, refetched on a timer; the "new session" form and the
/// per-row stop/delete actions (PLAN_M2.md step 8) live here too, since
/// both need to reach into the same poll loop — a create or a stop should
/// be reflected as soon as the next poll runs, not held behind an
/// optimistic local edit.
///
/// The poll loop lives in a `use_future` scoped to this component, so it
/// is cancelled for free when `App` switches to `SessionView` and this
/// component unmounts — "polling stops while a terminal is open"
/// (PLAN_M2.md) falls out of Dioxus's own task lifecycle rather than
/// needing an explicit stop signal.
#[component]
pub(crate) fn ListView(on_open: EventHandler<Session>) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut listing = use_signal(|| None::<Result<SessionListing, String>>);
    // Per-session, not one shared slot: a stop failing on session A must
    // not blank out session B's still-fresh success (or vice versa), and
    // a LATER success on any session must not silently erase an EARLIER
    // failure on a different one — which a single `Option<String>` would
    // do on every write regardless of which session it was about. Keyed
    // by session id so each row renders only its own entry.
    let mut errors = use_signal(HashMap::<String, String>::new);
    // Which sessions have a stop/delete in flight right now (also keyed
    // by id): both disables that row's buttons (so a second click can't
    // race the first) and is the re-entry guard the click handlers check
    // before doing anything — belt-and-suspenders, since a disabled
    // button should already stop the click from firing, but the DOM
    // update disabling it is not synchronous with the click handler
    // itself.
    let mut pending = use_signal(HashSet::<String>::new);
    // Which sessions are showing the inline "confirm delete?" prompt in
    // place of their normal stop/delete buttons — see `on_delete` below.
    // Deliberately a plain client-side set with no timeout and no
    // poll-driven reset: a listing refresh must leave an in-progress
    // confirmation alone (the user is mid-decision, not mid-poll), so
    // this is intentionally NOT derived from `listing` on every render.
    // The one reconciliation that does happen is in the poll loop below,
    // which drops an entry once its session is no longer in the listing
    // at all (deleted from elsewhere, say) — there is no row left for a
    // dangling entry to ever affect, so this is tidiness, not correctness.
    let mut confirming = use_signal(HashSet::<String>::new);
    // Which row, if any, has its rename field open (PLAN_M5.md item 6),
    // and the text being typed into it.
    //
    // One at a time, unlike `confirming`'s set, and that is the whole
    // interaction rather than a limitation: renaming is a focused edit the
    // user finishes or abandons, and a second open field would be an
    // invitation to type into two and lose track of which one Enter
    // submits. The draft lives HERE rather than in `RenameForm` for a
    // reason that has nothing to do with how many can be open: this
    // component re-renders for reasons the user did not cause, and one of
    // them (a failed listing poll swapping the rows for an error line)
    // unmounts the form entirely — a draft owned by the form would be
    // silently discarded with it. Seeded from the row's current title when
    // the field opens, which is also what keeps a poll carrying someone
    // else's rename from overwriting an edit in progress.
    let mut renaming = use_signal(|| None::<String>);
    let mut rename_draft = use_signal(String::new);
    // The optimistic rename corrections `apply_optimistic_renames` paints
    // over the server's listing, keyed by session id and carrying the poll
    // sequence number that bounds when the server could first have told
    // this view about it (`prune_optimistic_renames`). The tab strip's
    // scheme, applied to a title: without the number, a listing reply that
    // was already in flight when the rename landed would be
    // indistinguishable from the server disagreeing, and the row would flip
    // back to the old title for up to a full poll interval — a visible
    // wobble on the one operation whose entire point is that the new name
    // shows up at once.
    let mut renamed = use_signal(HashMap::<String, (String, u64)>::new);
    // How many listing polls this view has STARTED. A poll's own index is
    // the value it reads before incrementing, so an optimistic rename
    // recording the current value names the first poll GUARANTEED to have
    // started after the rename's response completed. That is a
    // conservative bound rather than a statement about the server: a poll
    // launched earlier can perfectly well observe the committed title,
    // since the write lands before the response is read. Conservative is
    // the safe direction — it can only keep a correction slightly longer
    // than strictly necessary, never retire one on a reply that could not
    // have seen it.
    let mut poll_sequence = use_signal(|| 0_u64);
    let mut show_create = use_signal(|| false);
    // Lifted out of `CreateSessionForm` rather than owned there: the
    // "new session" toggle button below needs to know whether a create is
    // in flight too, so it can refuse to unmount the form out from under
    // its own pending POST (see the toggle button's doc below).
    let submitting = use_signal(|| false);

    // Cloned once up front rather than moved into the poll loop below: a
    // `move ||` closure takes ownership of everything it captures, and
    // `on_stop`/`on_delete` need their own copy of `base` afterward.
    let poll_base = base.clone();
    use_future(move || {
        let base = poll_base.clone();
        async move {
            loop {
                // Read before incrementing, so `index` is this poll's own
                // position in the view's poll order — what tells an
                // optimistic rename whether this reply is late enough to
                // be evidence about it.
                let index = poll_sequence.peek().to_owned();
                poll_sequence += 1;
                let fetched = fetch_sessions(&base).await;
                // Drop any `confirming` entry whose session is gone from
                // this fetch entirely — the counterpart to the "a poll
                // refresh must not clear an in-progress confirmation"
                // rule just above: that rule protects a row that is
                // still LISTED, not one that has vanished (deleted from
                // another client while this one sat mid-confirmation, an
                // externally-imposed departure the `retain` below cannot
                // distinguish from the id simply never having existed).
                // Left off a failed fetch on purpose: an error reply
                // carries no session ids at all, and a transient fetch
                // failure is not evidence any session actually left.
                if let Ok(listing) = &fetched {
                    let live_ids: HashSet<&str> =
                        listing.sessions.iter().map(|s| s.id.as_str()).collect();
                    confirming
                        .write()
                        .retain(|id| live_ids.contains(id.as_str()));
                    // An open rename field for a session that has left the
                    // listing entirely goes with it, the same tidiness the
                    // `confirming` retain above performs — there is no row
                    // left for it to sit in.
                    let renaming_vanished = renaming
                        .read()
                        .as_ref()
                        .is_some_and(|id| !live_ids.contains(id.as_str()));
                    if renaming_vanished {
                        renaming.set(None);
                    }
                    // Same "only a successful fetch is evidence" rule as
                    // the two above, for the same reason: an error carries
                    // no titles at all, so it can neither confirm nor
                    // contradict an optimistic rename.
                    prune_optimistic_renames(&mut renamed.write(), &listing.sessions, index);
                }
                listing.set(Some(fetched));
                // Inlined rather than a shared `sleep_ms` helper: this is
                // the only call site, and `tokio::time::sleep` is
                // unavailable on wasm32 (no reactor in the browser) while
                // `gloo-timers`' `TimeoutFuture` only works on wasm32 (a
                // `wasm-bindgen` binding to `setTimeout`) — each target
                // gets the idiom that already fits it. The desktop build
                // runs inside the tokio multi-thread runtime
                // `dioxus-desktop` itself constructs (see its
                // `launch.rs`), so `tokio::time::sleep` needs no extra
                // setup there.
                #[cfg(target_arch = "wasm32")]
                gloo_timers::future::TimeoutFuture::new(POLL_INTERVAL_MS as u32).await;
                #[cfg(not(target_arch = "wasm32"))]
                tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
            }
        }
    });

    let stop_base = base.clone();
    // Takes the id directly, not the whole `Session`: nothing past the
    // insert-into-`pending` check below reads any other field, so a
    // `Session` clone (and a second, redundant id clone off of it) would
    // only be dead weight — see `SessionRow`'s call site for the mirrored
    // simplification on the caller's side.
    let on_stop = move |id: String| {
        // Cross-guard against `confirming`, not just `pending`: the
        // stop/delete buttons are only ABSENT from the DOM once a
        // rerender following `confirming.insert` has actually landed
        // (see `SessionRow`'s doc), so a stop click queued just ahead of
        // that rerender — a rapid synthetic double-click, say — could
        // otherwise still reach this handler for a row that is, or is
        // about to be, showing the confirm prompt. Refusing here keeps
        // the row's two lifecycle handlers from ever racing each other
        // for the same id: without it, a stop could slip `id` into
        // `pending` WHILE a delete confirmation is open, and the eventual
        // "confirm delete" click would then silently no-op — NOT because
        // of anything in `confirm_delete` itself, but because `do_delete`
        // (which it calls after removing `confirming`) has its OWN
        // `pending`-insert re-entry guard, which would find the id
        // already occupied by that stop and bail with no error at all.
        //
        // The same argument covers an open RENAME field, which replaces
        // the same buttons for the same reason.
        if confirming.read().contains(&id) || renaming.read().as_deref() == Some(id.as_str()) {
            return;
        }
        // Re-entry guard for the per-session in-flight set: a disabled
        // button should already stop this, but the click and the
        // re-render that disables it are not synchronous, so the handler
        // checks for itself too. `insert` returning `false` means an op
        // for this id was already running.
        if !pending.write().insert(id.clone()) {
            return;
        }
        let base = stop_base.clone();
        spawn(async move {
            // No optimistic flip (PLAN_M2.md design note): the row's
            // badge only ever reflects what the NEXT poll observes, so a
            // stop that silently failed can never leave the UI claiming a
            // session is exited when tmux still disagrees.
            let outcome = stop_session(&base, &id).await;
            match outcome.err() {
                Some(e) => {
                    errors.write().insert(id.clone(), format!("stop: {e}"));
                    pending.write().remove(&id);
                }
                None => {
                    errors.write().remove(&id);
                    // `pending` stays set across this extra fetch — not
                    // released until it completes: `on_delete`'s confirm
                    // wording is decided from the `status` the LATEST
                    // listing carries, and without this, an instant
                    // delete right after this stop would still see the
                    // stale pre-stop Alive (up to `POLL_INTERVAL_MS` old)
                    // and confirm with the wrong "is still running"
                    // wording for a session that just got stopped.
                    listing.set(Some(fetch_sessions(&base).await));
                    pending.write().remove(&id);
                }
            }
        });
    };

    let delete_base = base.clone();
    // The actual DELETE call, shared by both ways a delete can be
    // decided on: immediately for an Exited session, or after the user
    // hits "confirm delete" on the inline prompt for an Alive/Unknown
    // one (see `on_delete` and `confirm_delete` below, both of which
    // clone this closure rather than each reimplementing the request/
    // pending/error bookkeeping). Mirrors `on_stop`'s shape exactly,
    // `delete_session` and `errors`'/`pending`'s "delete:"-prefixed entry
    // in place of `on_stop`'s "stop:" one.
    let mut do_delete = move |id: String| {
        if !pending.write().insert(id.clone()) {
            return;
        }
        let base = delete_base.clone();
        spawn(async move {
            let outcome = delete_session(&base, &id).await;
            match outcome.err() {
                Some(e) => {
                    errors.write().insert(id.clone(), format!("delete: {e}"));
                    pending.write().remove(&id);
                }
                None => {
                    errors.write().remove(&id);
                    // Removed from the LOCAL listing immediately, before
                    // releasing `pending` — the one deliberate optimistic
                    // exception in this file (PLAN_M2.md's "no optimistic
                    // status flips" is about STATUS badges specifically;
                    // this is acting on a delete response already in
                    // hand, not guessing). Waiting for the next poll
                    // instead would leave the stale row's delete button
                    // re-enabled with nothing left server-side to delete
                    // — a second click in that window would 404 against
                    // an id that no longer exists, a confusing failure
                    // for an action that had already succeeded.
                    if let Some(Ok(current)) = listing.write().as_mut() {
                        current.sessions.retain(|s| s.id != id);
                    }
                    pending.write().remove(&id);
                }
            }
        });
    };

    // The delete button's initial click: decides whether this id needs
    // confirming at all, never itself calling the API.
    //
    // SPEC.md's "Lifecycle operations": delete confirms first only when
    // the agent might still be alive — an Exited session deletes
    // immediately (see its own arm below for the residual risk that
    // accepts). Alive and Unknown both confirm, entering the per-session
    // `confirming` state that `SessionRow` reads to swap its action area
    // (see that component's doc) — this closure itself does nothing more
    // than flip that flag; `confirm_delete` below is what a confirmed
    // click actually acts on.
    //
    // Refuses an id already in `pending` (the cross-guard mirroring
    // `on_stop`'s `confirming` check above): the delete button is only
    // disabled once a rerender following a `pending` insert lands, so a
    // rapid click queued just ahead of that rerender could otherwise
    // still reach this handler while, say, a stop is already in flight
    // for the same session. Refusing here is what keeps `confirming` from
    // ever being entered in that window — closing the door on the
    // opposite race `on_stop`'s guard closes. `do_delete`'s OWN
    // `pending`-insert guard (not `confirm_delete`'s `confirming`-removal
    // check, which exists for a different race entirely — see its own
    // doc below) would eventually refuse this same id too, but only AFTER
    // a confirm prompt had already opened for nothing; catching it here
    // means the prompt never opens in the first place.
    let mut do_delete_on_confirm = do_delete.clone();
    let on_delete = move |target: DeleteTarget| {
        if pending.read().contains(&target.id)
            || renaming.read().as_deref() == Some(target.id.as_str())
        {
            return;
        }
        match target.status {
            // Deliberately unconfirmed, a known residual: the AGENT
            // process has exited, but process-tree descendants it
            // spawned (a stray MCP server, a dev server) can outlive it,
            // and delete's process-tree sweep will kill whatever it
            // still finds. The UI has no way to know whether any such
            // descendant exists — only the supervisor's sweep does,
            // after the fact — so there is nothing concrete to report
            // here, and always confirming "just in case" would make
            // deleting routine, already-finished sessions needlessly
            // noisy. Revisit if M6.75's status work ever gives the UI a
            // basis for a sharper answer.
            // `Interrupted` joins `Exited` here for a stronger version of
            // the same argument: a host reboot is what produced this
            // status, and a reboot leaves no descendants at all — there
            // is not even the stray-MCP-server residual to accept. The
            // session's agent is definitively not running, so confirming
            // would be asking about a danger that cannot exist.
            // `Error` joins them for the strongest version yet: the login
            // shell and the launch shim DID run briefly (the shim is what
            // WRITES this very sentinel, from inside a real process), but
            // the AGENT'S OWN exec is what failed (PLAN_M3.md item 3) —
            // before it, before anything the agent itself might have
            // spawned. There is no lingering process tree to worry about,
            // not because nothing ever ran, but because the one thing
            // that could have left descendants never got the chance to.
            SessionStatus::Exited { .. }
            | SessionStatus::Interrupted
            | SessionStatus::Error { .. } => do_delete_on_confirm(target.id),
            // Unknown must not borrow Alive's "is still running" claim
            // it has no basis for — SPEC.md's no-guessing rule means an
            // unresolved status is presented as exactly that, uncertain,
            // never rounded up to a known-alive claim just because both
            // wordings end up confirming the same way. The DIFFERENT
            // wording itself lives in `SessionRow`, computed from
            // whatever `status` the row's own next render carries — not
            // captured here, since a status that changes while a
            // confirmation sits open (a session stopped from another
            // client, say) should be reflected in the prompt too.
            SessionStatus::Alive | SessionStatus::Unknown => {
                confirming.write().insert(target.id);
            }
        }
    };

    // The confirm-delete button's click, inside the inline prompt: the
    // exact same DELETE call an accepted `window.confirm()` used to
    // trigger before this rewrite, just reached from a different UI
    // widget. Clears `confirming` first so the row falls back to its
    // normal (busy/disabled) button layout the instant `do_delete`'s own
    // `pending` insert takes effect, rather than momentarily showing
    // both the prompt and a busy state.
    //
    // Proceeds ONLY when `remove` reports the id was actually present:
    // `HashSet::remove` returns `false` for an id already gone, which
    // happens whenever this confirmation was already resolved by
    // something else — `cancel_delete` running first (a queued confirm
    // click landing just after a cancel click, both fired in the same
    // burst), or a second confirm click racing the first's own removal.
    // Without this check, that second call would fall through to
    // `do_delete` regardless, which for the cancel-then-confirm race
    // would delete a session the user just told the UI to leave alone.
    let confirm_delete = move |id: String| {
        if !confirming.write().remove(&id) {
            return;
        }
        do_delete(id);
    };

    // The inline prompt's cancel button: just drops the flag. No API
    // call, no `pending` involvement — cancelling was never in flight to
    // begin with.
    let cancel_delete = move |id: String| {
        confirming.write().remove(&id);
    };

    // The rename button's click: opens this row's field, seeds the draft
    // from the title the row is showing right now, and never calls the API
    // — exactly as `on_delete` opens the confirm prompt. Refuses a row
    // with an operation already in flight or a confirmation already open,
    // the same cross-guard those two keep against each other and for the
    // same reason (the controls only disappear once a rerender lands, so a
    // click queued just ahead of one can still arrive here).
    //
    // Seeding HERE rather than in the form is what makes reopening start
    // from the current title while an edit already in progress is never
    // overwritten by a poll (see `renaming`/`rename_draft`).
    let on_rename_start = move |(id, title): (String, String)| {
        if pending.read().contains(&id) || confirming.read().contains(&id) {
            return;
        }
        rename_draft.set(title);
        renaming.set(Some(id));
    };

    // The rename field's submit. The title goes to the supervisor exactly
    // as typed (`api::rename_session`); everything decided here is what to
    // do with its answer.
    //
    // On success the reply — the session as the supervisor now describes
    // it, status re-probed and tabs rediscovered — is recorded as this
    // view's optimistic correction, so the new title paints without
    // waiting for a poll, and the field closes. On failure the field stays
    // open with what the user typed still in it (the same courtesy
    // `CreateSessionForm` extends to a failed create — a refused title is
    // usually one keystroke away from an accepted one) and the
    // supervisor's own words land in this row's error line, while the old
    // title stays everywhere it was.
    let rename_base = base.clone();
    let on_rename_submit = move |(id, title): (String, String)| {
        if !pending.write().insert(id.clone()) {
            return;
        }
        // This row's own previous failure, cleared by the retry that
        // supersedes it and by nothing else (see `errors`).
        errors.write().remove(&id);
        let base = rename_base.clone();
        spawn(async move {
            match rename_session(&base, &id, &title).await {
                Ok(session) => {
                    // The sequence number is read AFTER the reply, never
                    // before the request: it names the first poll
                    // GUARANTEED to have started after this response
                    // completed. A poll launched while the POST was still
                    // in flight MAY also observe the new title — the write
                    // lands before the reply is read — so this is a
                    // conservative bound, and conservative in the only
                    // safe direction (it can keep a correction a little
                    // longer, never retire it on a reply that could not
                    // have seen the rename).
                    let observed_from = poll_sequence.peek().to_owned();
                    renamed
                        .write()
                        .insert(id.clone(), (session.title.clone(), observed_from));
                    // Closed only if this row's field is still the open
                    // one. The form disables its own cancel while a
                    // request is in flight, so the user has to beat a
                    // rerender to get here — but if they do (cancel, then
                    // open another row's field), a blind `set(None)` would
                    // close a field they are typing in and throw the draft
                    // away.
                    if renaming.peek().as_deref() == Some(id.as_str()) {
                        renaming.set(None);
                    }
                }
                Err(e) => {
                    errors.write().insert(id.clone(), format!("rename: {e}"));
                }
            }
            pending.write().remove(&id);
        });
    };

    // Whether ANY row's open button should be disabled right now.
    // Opening a row navigates `App` away from `ListView` entirely (see
    // the module docs) — that unmounts this whole component, and with it
    // every task it owns, `spawn`ed or not: a create or a stop/delete
    // still in flight would have its eventual result silently discarded
    // instead of ever being acted on. This has to be a single flag
    // covering every row rather than a per-row one, because it is
    // `ListView` ITSELF that would unmount — every row's open action is
    // equally unsafe while ANYTHING is in flight, not just the row whose
    // own operation is running. A finer-grained rule (only the busy row's
    // own open button disabled, say) would need operations to be owned by
    // something that outlives this component instead, which is what M6.75's
    // live-push channel could plausibly provide; M2 has nothing of the
    // kind, so the global lock is what today's ownership model can
    // actually promise.
    let nav_locked = submitting() || !pending.read().is_empty();

    rsx! {
        div { class: "list-toolbar",
            button {
                r#type: "button",
                class: "btn new-session-button",
                // Disabled while a create is in flight: this is the
                // form's only cancel/close affordance, and toggling
                // `show_create` off would unmount `CreateSessionForm`
                // mid-POST — dropping the component drops its `spawn`ed
                // task's ability to ever act on the response, silently
                // losing track of whether the create actually happened.
                // Disabling the one control that can cause that unmount
                // is simpler and more robust than trying to keep a
                // detached task's result meaningful after the fact.
                disabled: submitting(),
                onclick: move |_| {
                    // Signal-level re-entry check, not just the
                    // `disabled` attribute above: the attribute's DOM
                    // update from a rerender is not synchronous with a
                    // click event, so a second click landing in that gap
                    // would still reach this handler even though the
                    // button already looked disabled.
                    if submitting() {
                        return;
                    }
                    show_create.set(!show_create());
                },
                "new session"
            }
        }
        if show_create() {
            CreateSessionForm {
                submitting,
                on_created: move |session| {
                    show_create.set(false);
                    on_open.call(session);
                },
            }
        }
        match &*listing.read() {
            None => rsx! { div { class: "status", "loading sessions…" } },
            Some(Err(e)) => rsx! {
                div { class: "status error", "failed to load sessions: {e}" }
            },
            Some(Ok(listing)) => rsx! {
                if listing.sessions.is_empty() && listing.total == 0 {
                    div { class: "status", "no sessions" }
                } else {
                    if listing.truncated {
                        // PLAN_M2.md acceptance 5: the cap and truncated
                        // flag exist to be shown, not just plumbed —
                        // silently presenting a partial list would look
                        // like a complete one.
                        div { class: "banner truncation-banner",
                            "showing {listing.sessions.len()} of {listing.total} sessions"
                        }
                    }
                    div { class: "session-list",
                        // The rows are the server's listing with this
                        // view's own just-landed renames painted over it,
                        // so a renamed session reads correctly EVERYWHERE
                        // the row shows its title — the row itself, the
                        // delete prompt that quotes it, the rename field
                        // if it is reopened, and the `Session` that
                        // `on_open` carries into the session view.
                        for session in apply_optimistic_renames(&listing.sessions, &renamed.read()) {
                            SessionRow {
                                key: "{session.id}",
                                error: errors.read().get(&session.id).cloned(),
                                busy: pending.read().contains(&session.id),
                                confirming: confirming.read().contains(&session.id),
                                renaming: renaming.read().as_deref() == Some(session.id.as_str()),
                                nav_disabled: nav_locked,
                                session,
                                on_open,
                                on_stop: on_stop.clone(),
                                on_delete: on_delete.clone(),
                                on_confirm_delete: confirm_delete.clone(),
                                on_cancel_delete: cancel_delete,
                                rename_draft,
                                on_rename_start,
                                on_rename_submit: on_rename_submit.clone(),
                                // Inlined rather than a named closure: it
                                // is one assignment, and the only rename
                                // state a cancel touches is which row is
                                // open — the draft is deliberately LEFT
                                // alone, since the next open reseeds it.
                                on_rename_cancel: move |_| renaming.set(None),
                            }
                        }
                    }
                }
            },
        }
    }
}

/// Inline create form (PLAN_M2.md step 8's "not a modal library" design
/// choice): working directory and agent command are required, title is
/// optional. Lives entirely inside `ListView` — there is no route or
/// signal for it beyond the `show_create` toggle that mounts/unmounts it.
///
/// `submitting` is owned by the CALLER (`ListView`), not this component:
/// `ListView`'s own "new session" toggle button needs to see it too, so it
/// can refuse to unmount this form while a create is still in flight —
/// dropping this component mid-`spawn` would strand the POST's eventual
/// response with nothing left to act on it. Lifting the flag up is
/// simpler than trying to keep a detached task meaningful after the fact.
///
/// `on_created` fires only on a successful POST, with the newly created
/// `Session` from the response body; `ListView` uses that to both close
/// the form and navigate straight into the new session's terminal
/// (SPEC.md: "creation launches the agent; you type your first prompt
/// into its terminal"). On failure the form stays mounted with its values
/// untouched and the error text rendered next to it — the fields are
/// plain `use_signal<String>`s rather than being reset or lifted into
/// `ListView`, so "form contents preserved" falls out of simply not
/// clearing them rather than needing a restore step. On success the
/// fields are left as-is too: `on_created` drives `ListView` to unmount
/// this whole component immediately (closing the form and navigating
/// away), so there is no one left to observe a reset — only the failure
/// path needs to leave the control usable again.
///
/// ## The intent key (PLAN_M3.md item 6)
///
/// One key per INTENDED create, reused across every retry of it. The
/// lifecycle is deliberately tied to the form's values rather than to its
/// mount: minted at first submit, kept across a failed submit (the retry
/// case the key exists for), and dropped the moment any field changes
/// (which makes the next submit a different intent). Both edges matter —
/// keeping it across an edit would send a request the server refuses as a
/// key reuse once the first attempt has a durable outcome, and dropping it
/// on failure would make a retry able to create a second session for the
/// same intent, which is the exact gap this closes.
///
/// The inputs are DISABLED while a create is in flight, which is what makes
/// that lifecycle a rule rather than a race: key generation is itself
/// asynchronous (`mint_intent_key` is an `await` on both renderers, even
/// though only the wasm build's half of it actually yields), so without it
/// a keystroke could land between minting a key and sending it, publishing
/// a key that belongs to values the user has already changed. Disabling was
/// chosen over reconciling generations afterwards because the form is inert
/// for that window anyway — the submit button and both navigation controls
/// are already disabled by the same flag.
///
/// A create from this form ALWAYS carries a key. If the key cannot be
/// generated the create is refused locally, with the failure shown like any
/// other: falling back to an unkeyed create would silently drop the one
/// protection this whole feature exists to provide, at exactly the moment
/// something is already wrong with the environment, and a user who retries
/// after a dropped reply would get a duplicate agent with nothing to
/// indicate why.
///
/// The server's key-reuse and already-deleted refusals need no handling of
/// their own here: they arrive as ordinary create failures (a 409 with the
/// supervisor's own message) and render in the same `.create-session-error`
/// line as every other one, which is what SPEC.md's "concrete, actionable
/// errors" asks for — the message names the key and what happened to it.
#[component]
fn CreateSessionForm(mut submitting: Signal<bool>, on_created: EventHandler<Session>) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut cwd = use_signal(String::new);
    let mut invocation = use_signal(String::new);
    let mut title = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    // This form's current intended create, if one has been submitted yet
    // (PLAN_M3.md item 6). Minted at FIRST SUBMIT and reused by every later
    // submit of the same values; cleared inline by every field's `oninput`,
    // because an edit makes the next submit a different intent. See this
    // component's own docs for both edges of that rule.
    let mut intent_key = use_signal(|| None::<String>);

    rsx! {
        form {
            class: "create-session-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                // Double-submission guard: covers concurrent clicks on
                // THIS mounted form (a double-click, or a stray repeat
                // event) — the control is inert for the whole round trip,
                // not just until the first click handler returns.
                //
                // The OTHER half — a retry after an ambiguous transport
                // failure (request sent, response lost) reaching the
                // supervisor a second time — is what `intent_key` closes,
                // and it cannot be closed here: only the server knows
                // whether the lost reply belonged to a session that
                // actually exists. This handler's job is merely to send
                // the SAME key for every retry of one intent.
                if submitting() {
                    return;
                }
                let base = base.clone();
                let cwd_value = cwd();
                let invocation_value = invocation();
                let title_value = title();
                // Snapshotted before disabling the form, not re-read
                // inside the task: either way is race-free (no edit can
                // land once `submitting` is true, below), but reading it
                // here keeps the "already have one" retry path free of an
                // await instead of calling `mint_intent_key` unconditionally.
                let needs_key = intent_key().is_none();
                submitting.set(true);
                error.set(None);
                spawn(async move {
                    if needs_key {
                        match mint_intent_key().await {
                            Ok(key) => intent_key.set(Some(key)),
                            Err(reason) => {
                                // No key, no create: see this component's
                                // docs on why an unkeyed create is not an
                                // acceptable degradation. The message says
                                // what failed rather than blaming the
                                // request, since nothing the user typed
                                // caused it.
                                error.set(Some(format!(
                                    "could not generate an idempotency key for this create, so \
                                     it was not sent (a retry could otherwise create a second \
                                     session): {reason}"
                                )));
                                submitting.set(false);
                                return;
                            }
                        }
                    }
                    let key = intent_key().expect("a key was just generated or already held");
                    match create_session(
                            &base,
                            &cwd_value,
                            &invocation_value,
                            &title_value,
                            &key,
                        )
                        .await
                    {
                        Ok(session) => on_created.call(session),
                        Err(e) => {
                            // The key deliberately SURVIVES a failure:
                            // this is exactly the case it exists for. A
                            // failure whose cause was an ambiguous
                            // transport error may have created a session
                            // the user cannot see, and resubmitting
                            // unchanged must reach that same session
                            // rather than launch a second agent. A user
                            // who instead fixes the form gets a new key
                            // from the fields' own `oninput`.
                            error.set(Some(e));
                            submitting.set(false);
                        }
                    }
                });
            },
            // Working directory and agent command are literal text that
            // gets EXECUTED, never prose — OS-level text mangling has no
            // way to tell the difference and "corrects" them anyway
            // (observed directly: WKWebView's autocorrect silently
            // substituting "claude" with "Claude" in place, with no
            // visible suggestion popup to catch and reject). A
            // capitalized command or a suggestion-popup keystroke
            // swallowed mid-path corrupts what actually runs. Title IS
            // ordinary prose, but the same opt-out applies to it too, for
            // a narrower reason: whatever the user types is what should
            // come back out verbatim (SPEC.md's "auto-generated when
            // omitted" is the only substitution this field ever gets, and
            // it happens server-side, deliberately, not as a silent
            // client-side "helpful" rewrite) — so every input here opts
            // out of every form of text mangling a browser might apply on
            // its own, for whichever of these two reasons applies to it.
            label {
                "working directory"
                input {
                    r#type: "text",
                    required: true,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{cwd}",
                    disabled: submitting(),
                    oninput: move |evt| {
                        cwd.set(evt.value());
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
            }
            label {
                "agent command"
                input {
                    r#type: "text",
                    required: true,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{invocation}",
                    disabled: submitting(),
                    oninput: move |evt| {
                        invocation.set(evt.value());
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
            }
            label {
                "title (optional)"
                input {
                    r#type: "text",
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{title}",
                    disabled: submitting(),
                    oninput: move |evt| {
                        title.set(evt.value());
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
            }
            button {
                r#type: "submit",
                class: "btn create-session-submit",
                disabled: submitting(),
                "create"
            }
            if let Some(err) = error.read().clone() {
                div { class: "create-session-error", "{err}" }
            }
        }
    }
}

/// One row: a plain `<div>` wrapper around three real `<button>`s (open,
/// stop, delete) rather than a `div` with `role`/`tabindex`/a hand-rolled
/// `onkeydown` — every action gets Enter- and Space-activation, focus
/// styling, and screen-reader semantics for free from being a real
/// button, and none of it needs reimplementing here (a hand-rolled div
/// also had a latent bug: Space on a focused element scrolls the page
/// unless the handler prevents default, which native button activation
/// never triggers in the first place).
///
/// The wrapper is a `div`, not a `button`, because M1's whole-row button
/// cannot host the stop/delete actions PLAN_M2.md step 8 adds: HTML
/// forbids interactive content nested inside a `<button>`, so a `<button
/// class="session-row">` containing further `<button>`s would be invalid
/// markup with undefined browser behavior. Splitting the open action into
/// its own sibling button (`.session-row-open`) keeps every action a real,
/// individually focusable button while satisfying that constraint — tab
/// order simply walks open → stop → delete across the row in the NORMAL
/// (not-confirming) state; see "Inline delete confirmation" below for how
/// that changes while a delete prompt is open.
///
/// `data-session-id` stays on the outer wrapper for Playwright to key off
/// of. No `stop_propagation` on the stop/delete buttons: the wrapper
/// `<div>` has no click handler of its own for a click to bubble into
/// (the open action lives on its own sibling button), so there is nothing
/// here for propagation to trigger by accident.
///
/// `error` is this row's own entry from `ListView`'s per-session error map
/// (`None` when its last action succeeded or none has run yet); `busy` is
/// whether a stop/delete for THIS session is currently in flight, which
/// disables both action buttons — both are `ListView`'s per-session state
/// (see its own docs for why a single shared error/in-flight slot would be
/// wrong), just narrowed to this one row by the caller before it gets here.
///
/// `nav_disabled`, unlike `error`/`busy`, is NOT this row's own state — it
/// is `ListView`'s single global "something is in flight somewhere" flag
/// (see `nav_locked` in `ListView`), applied identically to every row's
/// open button. Opening ANY row unmounts `ListView` (navigation replaces
/// it with `SessionView`), which would silently cancel whatever this row
/// OR any other row's in-flight create/stop/delete was doing — the whole
/// point of disabling it is to keep that unmount from happening at all
/// while anything still needs `ListView` to stay alive to finish.
///
/// ## Inline delete confirmation
///
/// `confirming` (also `ListView`'s per-session state, same discipline as
/// `error`/`busy`) swaps the stop/delete pair out for a prompt plus
/// **confirm delete**/**cancel** in place — there used to be a
/// `window.confirm()` call here instead; wry ships no native JS dialogs on
/// macOS's WKWebView (observed directly running the desktop build), which
/// made delete-on-a-live-session silently do nothing on that target. An
/// in-page prompt has no such platform dependency, so it replaces the
/// eval-based one everywhere, not just on desktop. While it is open, tab
/// order walks confirm delete → cancel; initial FOCUS lands directly on
/// cancel regardless of tab order (see below).
///
/// The `.session-row-open` button (title/cwd/invocation/badge) is given
/// the extra `confirming` class and hidden outright (`display: none` in
/// app.css) rather than merely staying `disabled`, which is what it did
/// before this fix (MT-8): `.session-row-main` lays out its children in
/// one non-wrapping flex row, and that button's own children each carry a
/// `min-width` floor (see `.session-title`/`.session-cwd`/
/// `.session-invocation`) that does not shrink to nothing just because
/// the OUTER flex algorithm hands the button a narrower slot to make room
/// for the confirm prompt's own elements. Past that floor the button's
/// content overflows its shrunk box — CSS flexbox does not clip
/// overflow by default — and renders on top of the confirm prompt sitting
/// immediately after it in the row, rather than being replaced by it.
/// Removing the button from layout entirely while confirming is open
/// sidesteps that interaction completely instead of trying to out-shrink
/// it: the confirm prompt already repeats the title (`.confirm-title`
/// below), so nothing the hidden button showed is lost information while
/// it is gone.
///
/// The prompt itself is TWO separate elements, not one combined sentence:
/// `.confirm-consequence` (from `confirm_consequence`, fixed wording with
/// no title in it at all) and `.confirm-title` (the title alone, quoted).
/// Both are ordinary Dioxus text interpolation, never `document::eval` —
/// a title containing quotes or JS-source-looking text (plausible, since a
/// supervisor over `--ssh` may be a different, possibly untrusted host)
/// renders as inert DOM text, never something parsed as script, which is
/// what makes the injection concern the old eval path needed
/// `serde_json`-encoding to guard against moot on this path. The SPLIT
/// exists for a different reason: a legal title can be tens of KB with no
/// whitespace at all, and app.css lets `.confirm-title` (only) shrink and
/// ellipsize under space pressure while `.confirm-consequence` never
/// shrinks — so the safety-critical "will be killed" half can never be
/// the one that gets clipped, which a single combined, single-ellipsized
/// string could not promise once the title ran long enough. Rendering the
/// consequence element FIRST is what makes "read the risk before the
/// title" the actual reading order, not just an incidental visual
/// side effect that a later DOM-order change could quietly undo.
///
/// ## Inline rename (PLAN_M5.md item 6)
///
/// `renaming` swaps the rename/stop/delete trio for the session's own
/// title plus `rename::RenameForm`, and hides the open button for the same
/// layout reason the confirm prompt does (see `open_class`). The two
/// states are mutually exclusive by construction — `ListView` refuses to
/// open either while the other is showing — so the branches below can be a
/// plain if/else chain rather than a composition of overlays.
///
/// Repeating the title is load-bearing, not decoration: the element that
/// normally shows it is out of layout while the field is open, so a
/// REFUSED rename would otherwise leave the rejected draft as the only
/// name on screen, when SPEC.md requires the old title to stay while the
/// supervisor's refusal is shown.
///
/// The draft itself is `ListView`'s (`rename_draft`), seeded when the
/// field opens; everything the submitted string then goes through is
/// `ListView`'s too. This component neither validates it nor decides what
/// a refusal means.
///
/// Focus-on-open uses the plain HTML `autofocus` attribute on the cancel
/// button (below), not Dioxus's `onmounted`/`set_focus` API: `set_focus`
/// returns a `Result` future that can fail (`MountedError`, e.g. on a
/// renderer that does not support it), and since focus-on-cancel is a
/// safety default — landing keyboard focus on the SAFE action before a
/// stray Enter/Space can reach anything — silently discarding that
/// `Result` would let the safety behavior vanish with nothing to show for
/// it. `autofocus` cannot fail the same way: it is applied by the browser
/// itself at parse/insert time as a plain attribute, with no fallible
/// async call in the UI's own control to get wrong or ignore. It reliably
/// fires exactly once per entry into `confirming` for the same reason
/// `onmounted` would have: the button is only ever created fresh inside
/// the `if confirming` branch below.
#[component]
fn SessionRow(
    session: Session,
    error: Option<String>,
    busy: bool,
    confirming: bool,
    renaming: bool,
    nav_disabled: bool,
    on_open: EventHandler<Session>,
    on_stop: EventHandler<String>,
    on_delete: EventHandler<DeleteTarget>,
    on_confirm_delete: EventHandler<String>,
    on_cancel_delete: EventHandler<String>,
    rename_draft: Signal<String>,
    on_rename_start: EventHandler<(String, String)>,
    on_rename_submit: EventHandler<(String, String)>,
    on_rename_cancel: EventHandler<()>,
) -> Element {
    let (badge_class, badge_text) = status_badge(&session.status, session.annotation.as_deref());
    let open_session = session.clone();
    let stop_id = session.id.clone();
    let delete_target = DeleteTarget {
        id: session.id.clone(),
        status: session.status.clone(),
    };
    let confirm_id = session.id.clone();
    let cancel_id = session.id.clone();
    let rename_start = (session.id.clone(), session.title.clone());
    let rename_submit_id = session.id.clone();
    // The open button is removed from layout by EITHER prompt — one
    // modifier class for both, since what the stylesheet needs to know is
    // "a prompt occupies this row", not which one. The reason is the one
    // MT-8 recorded: `.session-row-main` is a non-wrapping flex row whose
    // children have `min-width` floors, so anything that takes space
    // beside this button — the confirm prompt's elements, or the rename
    // field — is painted over by the button's own overflowing content
    // rather than laid out next to it.
    let open_class = if confirming || renaming {
        "session-row-open prompting"
    } else {
        "session-row-open"
    };

    rsx! {
        div {
            class: "session-row",
            "data-session-id": "{session.id}",
            // Two stacked rows, not one: the buttons need a plain flex
            // ROW (see `.session-row-main` in app.css), but a per-session
            // error line needs its own full-width row underneath rather
            // than squeezing in as a fourth flex item next to the
            // buttons — hence the extra wrapper rather than putting
            // everything directly under `.session-row`.
            div { class: "session-row-main",
                button {
                    r#type: "button",
                    // The modifier is what app.css hides (MT-8, see the
                    // "Inline delete confirmation" section of this
                    // component's doc above and `open_class` itself) —
                    // without it, this button's own title/cwd/invocation
                    // content overflows its flex-shrunk box and paints
                    // over whichever prompt is rendered right after it.
                    class: open_class,
                    // Disabled by ANY of the three locks: the global nav
                    // lock (any in-flight op anywhere), or this row's own
                    // confirmation or rename field being open — the
                    // simplest way to satisfy "cancel is the only way back
                    // to normal" (see the component doc above) is to make
                    // the open button inert for the whole time a prompt is
                    // showing, rather than giving it a second, competing
                    // meaning as an implicit cancel.
                    disabled: nav_disabled || confirming || renaming,
                    onclick: move |_| on_open.call(open_session.clone()),
                    span { class: "session-title", "{session.title}" }
                    span { class: "session-cwd", "{session.cwd}" }
                    span { class: "session-invocation", "{session.invocation}" }
                    span { class: "status-badge {badge_class}", "{badge_text}" }
                }
                if confirming {
                    // Called inline, not hoisted into a `let` above this
                    // `if`: this is the ONLY place either half of the
                    // prompt is ever shown, so computing them
                    // unconditionally on every render regardless of
                    // `confirming` would be wasted work on the common
                    // (not-confirming) case, and — since `confirm_consequence`
                    // is documented as never being CALLED outside this
                    // state (see its own doc) — computing it only here is
                    // what actually keeps that contract true rather than
                    // just asserted.
                    //
                    // Two elements, consequence first: see the component
                    // doc above for why an untruncatable consequence and
                    // a separately truncatable title, in THIS order, is
                    // what keeps a long title from ever clipping the
                    // safety-critical half.
                    span {
                        class: "confirm-consequence",
                        "{confirm_consequence(&session.status)}"
                    }
                    span { class: "confirm-title", "\"{session.title}\"" }
                    button {
                        r#type: "button",
                        class: "btn confirm-delete",
                        onclick: move |_| on_confirm_delete.call(confirm_id.clone()),
                        "confirm delete"
                    }
                    button {
                        r#type: "button",
                        class: "btn confirm-cancel",
                        // Safe default: land keyboard focus on cancel, not
                        // confirm, the instant this prompt appears — a
                        // stray Enter/Space right after the row's delete
                        // click (residual focus, a fast typist) then backs
                        // OUT of the destructive action instead of into
                        // it. Declarative `autofocus`, not `onmounted` +
                        // `set_focus`: see the component doc above for why
                        // the fallible, discardable-`Result` async API was
                        // rejected in favor of a plain HTML attribute that
                        // cannot silently fail to apply.
                        autofocus: true,
                        onclick: move |_| on_cancel_delete.call(cancel_id.clone()),
                        "cancel"
                    }
                } else if renaming {
                    // The field takes the action area over exactly as the
                    // confirm prompt does, rather than sitting beside the
                    // buttons: a text field needs real width, and the row
                    // has none to spare (see `open_class`).
                    //
                    // The AUTHORITATIVE title is repeated alongside it,
                    // and that is a requirement rather than a nicety: the
                    // open button that normally shows it is out of layout
                    // here, so without this element a refused rename would
                    // leave the rejected DRAFT as the only name on screen
                    // — the opposite of SPEC.md's "the old title stays"
                    // while the supervisor's refusal is shown. It shrinks
                    // and ellipsizes (app.css) so a legal multi-KB title
                    // cannot push the field or its buttons off the row.
                    span { class: "rename-current-title", "{session.title}" }
                    RenameForm {
                        draft: rename_draft,
                        busy,
                        on_submit: move |title| {
                            on_rename_submit.call((rename_submit_id.clone(), title))
                        },
                        on_cancel: move |_| on_rename_cancel.call(()),
                    }
                } else {
                    button {
                        r#type: "button",
                        class: "btn session-row-rename",
                        disabled: busy,
                        onclick: move |_| on_rename_start.call(rename_start.clone()),
                        "rename"
                    }
                    button {
                        r#type: "button",
                        class: "btn session-row-stop",
                        disabled: busy,
                        onclick: move |_| on_stop.call(stop_id.clone()),
                        "stop"
                    }
                    button {
                        r#type: "button",
                        class: "btn session-row-delete",
                        disabled: busy,
                        onclick: move |_| on_delete.call(delete_target.clone()),
                        "delete"
                    }
                }
            }
            if let Some(err) = &error {
                div { class: "action-error", "{err}" }
            }
        }
    }
}

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
fn apply_optimistic_renames(
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
fn prune_optimistic_renames(
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

/// Map a status — and, for an ended session, its annotation — to the
/// badge's CSS modifier class and display text. Kept as one function so
/// every case stays next to its siblings instead of drifting apart across
/// separate match arms in the render tree.
///
/// The annotation is a QUALIFIER on the exited status, never a
/// replacement for it: SPEC.md is explicit that "stopped" is not a
/// distinct status, so a user-stopped session reads "exited — stopped by
/// user (code 0)". An earlier version rendered the annotation alone, which
/// read as a fourth status word and quietly dropped the one fact every
/// row's badge is supposed to state. The annotation is ignored for every
/// other status — it describes how a run ENDED, and a live session has
/// not.
fn status_badge(status: &SessionStatus, annotation: Option<&str>) -> (&'static str, String) {
    match status {
        SessionStatus::Alive => ("alive", "alive".to_string()),
        SessionStatus::Exited { exit_code } => {
            let mut text = "exited".to_string();
            if let Some(annotation) = annotation {
                text.push_str(" — ");
                text.push_str(annotation);
            }
            if let Some(code) = exit_code {
                text.push_str(&format!(" (code {code})"));
            }
            ("exited", text)
        }
        SessionStatus::Interrupted => ("interrupted", "interrupted".to_string()),
        // The shim's exec-failure sentinel (PLAN_M3.md item 3): the agent
        // never ran at all, which is a different claim from `Exited`'s
        // "it ran and finished" — so it gets its own word and its own
        // red-family color (`app.css`'s `.status-badge.error`), the one
        // case in this match that IS reporting a failure. `detail` (the
        // shim's own errno/argv0 report) rides straight into the badge
        // text rather than being tucked behind a tooltip or a separate
        // element: it is usually short, and it is the one piece of
        // information that actually explains why the row needs attention.
        SessionStatus::Error { detail } => ("error", format!("error — {detail}")),
        SessionStatus::Unknown => ("unknown", "unknown".to_string()),
    }
}

/// The safety-critical half of the inline delete-confirmation prompt:
/// what deleting THIS session will actually do, worded so the risk reads
/// on its own without depending on the title. Rendered into its own
/// untruncatable DOM element (`SessionRow`'s `.confirm-consequence`, never
/// ellipsized) AHEAD of the title, which gets a separate, deliberately
/// truncatable element instead — a legal title can be tens of KB with no
/// whitespace at all, and the earlier single-string design (title
/// embedded mid-sentence, the whole thing ellipsized as one span) would
/// clip whichever half landed at the tail once a title ran long enough,
/// which for that wording was always this one: the actual consequence, is
/// still running and will be killed. Splitting the two apart, consequence
/// first, is what makes that unclippable regardless of title length.
///
/// Only ever OPENED from `Alive`/`Unknown` (see `on_delete`'s own match) —
/// but is written total over `SessionStatus` rather than partial, because
/// `confirming` is `ListView`'s own state, decoupled from any single
/// render: a session that was `Alive` when the user opened this prompt
/// can flip to `Exited` under it (stopped from another client, say)
/// before either button is clicked, and this function re-runs on every
/// render off whatever status the row's LATEST prop carries. The
/// `Exited`, `Interrupted`, AND `Error` arms are all that residual case's
/// fallback, not wordings SPEC.md's confirm-contract actually specifies —
/// and `Error` is not merely a defensive completeness case: a session
/// that was genuinely `Alive` when this prompt opened, whose agent then
/// turns out never to have execed at all (the launch shim's sentinel is
/// read only once the pane goes dead-or-absent — `service.rs`'s
/// dead-or-absent gate), can flip straight from `Alive` to `Error` under
/// an already-open prompt exactly like the `Exited` case above, just with
/// a narrower window.
///
/// `Interrupted`'s wording is deliberately NOT a killing warning
/// (PLAN_M3.md item 2): the status exists only because the HOST rebooted,
/// which took the agent and every descendant of it with it, so there is
/// nothing left for a delete to kill and claiming otherwise would be the
/// mirror image of the fabricated-liveness mistake `Unknown`'s wording
/// exists to avoid. What deleting actually costs is the session itself —
/// worth saying, because an interrupted session is the one case where the
/// record outlives everything it described and is all that is left to
/// lose (and, since restart landed, the only route back into that
/// conversation).
fn confirm_consequence(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Alive => "still running — deleting kills the agent:",
        SessionStatus::Unknown => {
            "status unknown — the agent may still be running and will be killed:"
        }
        SessionStatus::Exited { .. } => "delete anyway:",
        SessionStatus::Interrupted => {
            "interrupted by a host reboot — nothing left to kill; deleting discards the session:"
        }
        // `Error` never OPENS this prompt (see `on_delete`'s own match),
        // but a prompt already open for an `Alive` session CAN land here —
        // see this function's own docs — so this arm is reachable, not
        // merely a defensive completeness case.
        SessionStatus::Error { .. } => {
            "the agent never started — nothing to kill; deleting discards the session:"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Pins BOTH the badge's display text and its CSS modifier class per
    /// status — not just the text — since a class regression (e.g. an
    /// `Exited` row silently keeping the `alive` class) would only
    /// otherwise surface as a wrong-COLORED row in the browser, which no
    /// text-only assertion here would ever catch.
    #[test]
    fn status_badge_matches_text_and_class_for_each_status() {
        assert_eq!(
            status_badge(&SessionStatus::Alive, None),
            ("alive", "alive".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: Some(7) }, None),
            ("exited", "exited (code 7)".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: None }, None),
            ("exited", "exited".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Unknown, None),
            ("unknown", "unknown".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Interrupted, None),
            ("interrupted", "interrupted".to_string())
        );
        assert_eq!(
            status_badge(
                &SessionStatus::Error {
                    detail: "exec_failed argv0=/nope errno=2".to_string()
                },
                None
            ),
            (
                "error",
                "error — exec_failed argv0=/nope errno=2".to_string()
            ),
            "the shim's own recorded detail must reach the badge text, not just its class"
        );
    }

    /// SPEC.md: "'stopped' is not a distinct status" — a user-stopped
    /// session is an EXITED session carrying a qualifier, so the badge
    /// must still SAY exited and add the supervisor's own wording after
    /// it, with the exit code still visible when there is one. Rendering
    /// the annotation alone (an earlier shape of this) reads as a fourth
    /// status word and drops the one fact the badge exists to state. The
    /// `exited` CSS class is asserted alongside the text for the same
    /// reason: a stopped session must still LOOK like an ended one.
    ///
    /// The live-session case is the one a naive implementation gets
    /// wrong: an annotation describes how a run ENDED, so it must never
    /// leak onto a session that is running — which is exactly what a
    /// stopped-then-restarted session is.
    #[test]
    fn stop_annotation_qualifies_the_exited_badge_without_replacing_it() {
        assert_eq!(
            status_badge(
                &SessionStatus::Exited { exit_code: None },
                Some("stopped by user")
            ),
            ("exited", "exited — stopped by user".to_string())
        );
        assert_eq!(
            status_badge(
                &SessionStatus::Exited { exit_code: Some(0) },
                Some("stopped by user")
            ),
            ("exited", "exited — stopped by user (code 0)".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Alive, Some("stopped by user")),
            ("alive", "alive".to_string()),
            "an annotation must never describe a session that is running"
        );
    }

    /// Pins the exact two confirm-prompt wordings SPEC.md's no-guessing
    /// rule requires to stay distinct: `Alive` must claim the agent IS
    /// running, while `Unknown` must only ever admit uncertainty — a
    /// regression that quietly reused one string for both (or rounded
    /// `Unknown` up to `Alive`'s wording) is exactly what this guards
    /// against. Scoped to `confirm_consequence`'s own string-building
    /// alone — it says nothing about how `SessionRow` later renders the
    /// result, nor about the SEPARATE title element sitting next to it
    /// (both exercised by the Playwright suite instead, not by anything
    /// callable from this unit test).
    #[test]
    fn confirm_consequence_wording_differs_between_alive_and_unknown() {
        assert_eq!(
            confirm_consequence(&SessionStatus::Alive),
            "still running — deleting kills the agent:"
        );
        assert_eq!(
            confirm_consequence(&SessionStatus::Unknown),
            "status unknown — the agent may still be running and will be killed:"
        );
    }

    /// An interrupted session is NOT alive (a host reboot is what made it
    /// interrupted), so its consequence line must not claim anything will
    /// be killed — the same no-fabrication rule that keeps `Unknown` from
    /// borrowing `Alive`'s wording, applied in the opposite direction.
    /// Asserted as properties rather than as one exact string so the
    /// wording can be improved without the test having to be rewritten
    /// each time; what must not change is that it stops promising a kill
    /// and starts naming what deleting actually costs.
    #[test]
    fn interrupted_consequence_promises_no_kill() {
        let wording = confirm_consequence(&SessionStatus::Interrupted);
        assert!(
            !wording.contains("kills") && !wording.contains("will be killed"),
            "nothing survives a reboot for a delete to kill: {wording}"
        );
        assert!(
            wording.contains("reboot") && wording.contains("discard"),
            "the honest consequence is losing the session record itself: {wording}"
        );
    }

    /// Review-swarm fix batch item 22: `confirm_consequence`'s `Error` arm
    /// is reachable — not a defensive completeness case — via the exact
    /// same residual race `Interrupted`'s own test above exercises in
    /// prose: a confirm prompt opened while a session was `Alive` stays
    /// open under a LATER render whose status has since moved on, and
    /// `Error` is one of the statuses it can have moved to. The wording
    /// must match `Error`'s actual meaning (never started, not merely
    /// "finished"), not borrow `Interrupted`'s reboot-specific phrasing.
    #[test]
    fn error_consequence_promises_no_kill_and_names_no_reboot() {
        let wording = confirm_consequence(&SessionStatus::Error {
            detail: "exec_failed argv0=/nope errno=2".to_string(),
        });
        assert!(
            !wording.contains("kills") && !wording.contains("will be killed"),
            "an agent that never started leaves nothing for a delete to kill: {wording}"
        );
        assert!(
            !wording.contains("reboot"),
            "an exec failure is not a reboot; the wording must not borrow that framing: {wording}"
        );
        assert!(
            wording.contains("discard"),
            "the honest consequence is losing the session record itself: {wording}"
        );
    }
}
