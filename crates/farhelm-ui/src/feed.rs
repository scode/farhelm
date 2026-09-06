//! The invalidation feed (PLAN_M6_75.md item 6): the one channel that
//! replaced all four of this UI's periodic loops.
//!
//! The helm's `/api/events` says "something changed" and nothing else — a
//! revision number per notification, never a session, never a host, never a
//! diff (farhelm-helm's `events.rs`). A client that receives one re-reads
//! whatever its current surface needs through the readers it already has.
//! That is why this module owns no data: it owns a COUNTER and a health
//! verdict, and the pages own their own reads.
//!
//! ## Why this lives at App level
//!
//! The channel must outlive selection changes, which is why neither pane
//! can own it. [`FleetFeed`] is mounted beside the build-skew notice,
//! above the two-pane shell, and it holds the subscription for the whole
//! life of the page. Consumers subscribe to its counter through
//! [`use_feed_reader`] and unsubscribe by unmounting; under the sidebar
//! layout the list and a session view are commonly BOTH mounted, so one
//! notification fans out to both readers — supported by design. The cost
//! is one extra feed CONSUMER, not one request: the list consumer's
//! refresh issues both a listing and a hosts read, so a notification with
//! a session selected typically costs three requests in total.
//!
//! ## The socket is JavaScript's; the policy is Rust's
//!
//! Same division as `reconnect`/terminal.js, for the same reason: a
//! WebSocket lives on a side of the boundary neither renderer can reach
//! across, while the numbers and the rules are decisions worth reviewing and
//! unit-testing. So `assets/events.js` opens the socket, reconnects it, and
//! reports what happened; everything here decides what those reports MEAN.
//!
//! The recovery ladder is `reconnect`'s, deliberately reused rather than
//! re-tuned: a dropped feed and a dropped terminal are the same transport
//! failure, and two ladders would be two things to keep in step. What the
//! feed pointedly does NOT borrow is the WORDING — there is no banner for a
//! feed that is down, because the fallback below keeps the page correct
//! while it is, and announcing the loss of a mechanism the user never asked
//! for would be alarming noise about nothing they can act on.
//!
//! ## The fallback, and the two failures it deliberately tells apart
//!
//! This is the load-bearing rule of the module, and the two halves pull in
//! opposite directions:
//!
//! - **The feed is unhealthy, stamps MATCH.** The helm is the build this
//!   bundle was made for; only the socket is gone. Polling is then the
//!   documented fallback and runs at `api::POLL_INTERVAL_MS`, because the
//!   data it reads is data this bundle understands.
//! - **Build SKEW.** Feed and fallback BOTH stop. Polling a helm on another
//!   build is precisely the unattended behavior SPEC_impl.md's withdrawal
//!   rule exists to revoke — this milestone's vocabulary is what a listing
//!   carries, so a stale bundle polling a newer helm re-reads rows it cannot
//!   decode, three seconds at a time, forever. The reload prompt (`skew`) is
//!   the way forward, and it is already on screen.
//!
//! The trigger for the withdrawal is a LATCHED mismatch
//! (`skew::build_skew_detected`), not `skew::helm_is_current`. The
//! difference matters exactly once, at startup: no reply has been seen yet,
//! so `helm_is_current` is false — and gating on it would stop the first
//! read from ever happening, leaving a page that polls nothing and knows
//! nothing. A helm that reports no stamp at all still latches
//! [`skew::Skew::Silent`] on its first reply, so the withdrawal arrives one
//! request later rather than never.
//!
//! The withdrawal has to bind a subscription that may not exist yet — the
//! socket is opened from a task and withdrawn from an effect, with nothing
//! ordering the two — which is why it writes a latch the island reads rather
//! than only calling `stop()`. See the effect in [`FleetFeed`], and
//! `events.js`'s `withdrawn`. The feed's consumers stand down on the same
//! latch ([`use_feed_reader`]), since a notification already in hand is one
//! neither half of that handshake can take back.
//!
//! ## The handshake, and the race it closes
//!
//! On (re)subscription the helm sends the CURRENT revision immediately.
//! [`FeedState::note_revision`] is what makes that useful: it bumps the
//! notice counter — which is what drives a re-read — in the SAME operation
//! that marks the feed healthy, so there is no arrangement of those two
//! writes that switches the fallback off without a re-read having been
//! ASKED FOR.
//!
//! The race that closes is real and otherwise silent: a client whose socket
//! died falls back to polling, and a mutation landing between its last poll
//! and its resubscription is invisible to both mechanisms — the poll is
//! over, and the bump happened before the subscription existed. The
//! handshake re-read is the one guaranteed look that covers that window.
//!
//! What is NOT claimed, and the distinction is worth stating because the
//! rule above reads stronger than it is: nothing here knows whether a
//! re-read SUCCEEDED. The notice is emitted before any read starts and this
//! module never hears how one ended, so a fetch that fails against a
//! perfectly healthy socket would leave a surface stale with no second
//! notification owed and the fallback switched off. That gap is closed one
//! layer up, by `reader`: a notice records a demand on the surface, that
//! demand survives a failed read, and the surface's own reader retries until
//! an answer arrives. The honest division of labour is that this module
//! promises a re-read is TRIGGERED and `reader` promises one eventually
//! LANDS.

use dioxus::prelude::*;
use serde::Deserialize;

use std::sync::atomic::{AtomicU64, Ordering};

use crate::ApiBase;
use crate::api::POLL_INTERVAL_MS;
use crate::reader;
use crate::reconnect::{PROBE_INTERVAL_MS, RETRY_LADDER_MS};
use crate::skew;

/// The helm's feed endpoint, as a path appended to the API base.
///
/// Kept here rather than in `api` because it is the ONE route this UI does
/// not reach through `api::send` — it is a WebSocket, opened by
/// `assets/events.js`, and so it carries no build stamp of its own. The
/// stamp still governs it: the withdrawal above is driven by whatever the
/// ordinary REST reads observed.
const EVENTS_PATH: &str = "/api/events";

/// How long a socket that has OPENED may stay silent before the island
/// treats it as dead (`assets/events.js` arms this per attempt).
///
/// The helm answers every (re)subscription with the current revision
/// immediately, so silence here is not slowness — it is an upgrade that was
/// accepted by something that then stopped serving it: a helm wedged mid
/// shutdown, a proxy that completed the handshake and never forwarded a
/// frame.
///
/// What the deadline is FOR is the recovery ladder, not the fallback poll.
/// The page is not left blind by such a socket: this module marks the feed
/// healthy only on a delivered frame ([`FeedState::note_revision`]), so a
/// subscription that has never spoken leaves `healthy` false and the
/// fallback is already reading. What is lost instead is the FEED — the
/// island sits on an open socket with its ladder suspended, so nothing ever
/// reconnects, and a page that could have been getting push notifications
/// stays on a three-second poll for as long as it is open. The deadline
/// turns that into an ordinary outage: report it, climb the ladder, and get
/// the subscription back.
///
/// Ten seconds is chosen to be uninteresting: far longer than any real
/// handshake over a slow link, and short enough that a wedged proxy costs a
/// few polls rather than the whole session's worth of push delivery.
const HANDSHAKE_DEADLINE_MS: u64 = 10_000;

/// The next subscription's identity.
///
/// One number per `subscribe`, and its whole job is to make the CLEANUP
/// specific. The subscription task can outlive the component that owns it by
/// however long an eval takes to reach the island, so a remount can install
/// a replacement while a dead task's cleanup is still in flight — and a
/// cleanup that said only "end the subscription" would end the wrong one,
/// leaving a page with no feed and nothing to bring it back.
///
/// Monotonic and never reused, which is all the identity needs to be: it is
/// compared for equality against the live subscription's own copy and means
/// nothing to anyone else. `Relaxed` because there is no other state being
/// published with it — and in the browser there is only one thread anyway.
static NEXT_SUBSCRIPTION: AtomicU64 = AtomicU64::new(1);

/// What this page currently knows about the socket carrying the fleet's
/// revision notifications.
///
/// A value type with the transitions on it, rather than loose signals, so
/// the one rule that matters can be stated and tested in one place: a
/// notification bumps the counter that drives re-reads and marks the feed
/// healthy in the same operation (see [`FeedState::note_revision`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct FeedState {
    /// How many notifications have arrived, handshakes included. The pages'
    /// re-read trigger, and monotonic by construction so a consumer can
    /// record which ones it has already acted on.
    ///
    /// A COUNT rather than the helm's own revision number, deliberately. A
    /// re-subscription's handshake can legitimately repeat a revision this
    /// page has already seen, and that repeat is exactly the message the
    /// fallback handover hangs on — a change-detecting client would discard
    /// it as "nothing new" and lose the one guaranteed look at the window
    /// its socket was down for. The revision itself is therefore not kept at
    /// all: nothing here may compare one against its predecessor, and a
    /// field nobody reads is an invitation to start.
    pub(crate) notices: u64,
    /// Whether the feed is currently carrying changes. The fallback poll's
    /// gate, and nothing else — see [`fallback_polls`].
    pub(crate) healthy: bool,
}

impl FeedState {
    /// A revision notification arrived — a handshake or a later bump, which
    /// are deliberately indistinguishable (farhelm-helm's `events.rs` refuses
    /// to mark them apart, because the client's correct answer to either is
    /// "re-read").
    ///
    /// Bumping `notices` and setting `healthy` in ONE operation is the
    /// handshake discipline in code: there is no arrangement of these two
    /// writes that leaves the feed healthy without a re-read having been
    /// triggered, so the fallback can never be switched off across a window
    /// nothing was asked to look at. Whether that look SUCCEEDS is `reader`'s
    /// problem, not this module's — see the header.
    fn note_revision(&mut self) {
        self.notices += 1;
        self.healthy = true;
    }

    /// The feed stopped carrying changes: the socket went away
    /// (`events.js` reports this before it starts climbing its ladder), or
    /// the subscription itself ended.
    ///
    /// One transition for both, because the page's answer is the same and
    /// the difference is in what happens NEXT rather than in what is true
    /// now: a dropped socket is the island's to reconnect, while an ended
    /// subscription simply never comes back (see the channel's `Err` arm in
    /// [`FleetFeed`]). Recording "how it ended" separately would be a field
    /// nothing reads — the fallback gate asks only whether the feed is
    /// carrying changes.
    fn note_feed_down(&mut self) {
        self.healthy = false;
    }
}

/// This page's feed state — a global for the same reason `skew`'s verdict is
/// one: it is written from one place (the subscription task) and read from
/// several unrelated components, and threading a signal through the view
/// tree to reach them would put the plumbing everywhere and the meaning
/// nowhere.
pub(crate) static FLEET_FEED: GlobalSignal<FeedState> = Signal::global(FeedState::default);

/// Whether the documented poll fallback should be reading right now.
///
/// The whole rule in one expression, split out so it can be exercised
/// without a Dioxus runtime — see the module docs for why the two `false`
/// answers mean opposite things: under skew the page is standing down, while
/// under a healthy feed it simply has a better source.
pub(crate) fn fallback_polls(build_skew: bool, feed_healthy: bool) -> bool {
    !build_skew && !feed_healthy
}

/// [`fallback_polls`] against the live signals, for a poll loop.
///
/// `peek` on both, deliberately: this is read from inside a spawned task
/// rather than during a render, and a tracked read there would subscribe the
/// component that happens to own the task to changes it re-checks on its own
/// schedule anyway.
pub(crate) fn fallback_polls_now() -> bool {
    fallback_polls(skew::build_skew_detected_now(), FLEET_FEED.peek().healthy)
}

/// Wait one fallback poll interval.
///
/// Shared by both pages' fallback loops. The renderer-specific half lives in
/// [`reader::sleep_ms`] rather than here, so this file states the CADENCE
/// and that module states how a wait is performed at all — the same sleep
/// the read retries use.
pub(crate) async fn fallback_sleep() {
    reader::sleep_ms(POLL_INTERVAL_MS).await;
}

/// Re-read whenever the feed says something changed.
///
/// A hook: call it unconditionally, once, in the page component that owns
/// the reads. `reread` is invoked once per notification and must not perform
/// a read, since an effect is not a place to await anything — both pages ASK
/// their surface readers instead (`reader::request_read`), which is also what
/// keeps a burst of notifications from becoming a burst of walks.
///
/// The count already acted on is seeded from the CURRENT one at mount rather
/// than from zero, and that seeding is what keeps a page from double-reading
/// the moment it opens: the feed outlives navigation, so a page mounted
/// after fifty notifications would otherwise treat all fifty as unhandled
/// and fire a re-read on top of the initial read it performs anyway.
///
/// A task spawned from inside `reread` belongs to the CALLING component's
/// scope, not to the feed's: Dioxus runs an effect's callback with its
/// owning scope on the stack, so the reads a consumer starts here are torn
/// down with that consumer. That is what scopes re-reads to mounted
/// consumers as a property of the lifecycle rather than something anyone
/// has to remember — and under the two-pane shell there are commonly TWO
/// such consumers alive at once (the persistent list and the selected
/// session's view), each rereading its own surfaces per notification.
///
/// What this hook does NOT promise is that the re-read succeeds. A
/// notification is spent once `reread` has been called, and this module
/// never hears how the read ended; recovering a failed one is the surface
/// reader's job (see the module header).
pub(crate) fn use_feed_reader(mut reread: impl FnMut() + 'static) {
    let mut acted_on = use_signal(|| FLEET_FEED.peek().notices);
    use_effect(move || {
        // The ONE tracked read in this closure. Everything else peeks, so
        // the effect re-runs when a notification lands and for no other
        // reason.
        let notices = FLEET_FEED.read().notices;
        if notices == *acted_on.peek() {
            return;
        }
        acted_on.set(notices);
        // Checked HERE, immediately before the re-read, and not only where
        // the socket is withdrawn: an effect run is queued work, so a
        // notification that arrived before the mismatch latched can still
        // reach this point after it. Withdrawing the socket cannot catch
        // that one — the notice is already in hand — and re-reading on it
        // would send a stale bundle off to decode a newer helm's rows,
        // which is the exact behavior SPEC_impl.md's withdrawal rule
        // revokes. The notice is marked acted-on regardless: under skew
        // this page is standing down for good, so there is nothing to come
        // back and serve it later.
        if skew::build_skew_detected_now() {
            return;
        }
        reread();
    });
}

/// Everything `events.js` needs to run the subscription, as the JSON
/// `farhelmEvents.subscribe()` takes.
///
/// The same shape and the same bargain as `reconnect::reconnect_policy`: the
/// numbers are Rust's, reviewable and pinned by tests here, and the loop
/// that consumes them lives where the socket does. The two regimes are
/// `reconnect`'s own — a short active ladder for the ordinary blip, then
/// unbounded low-frequency probing so a feed that comes back overnight
/// resubscribes by itself. The handshake deadline joins them for the third
/// failure a ladder alone cannot see (see [`HANDSHAKE_DEADLINE_MS`]).
pub(crate) fn feed_policy() -> serde_json::Value {
    serde_json::json!({
        "path": EVENTS_PATH,
        "delaysMs": RETRY_LADDER_MS,
        "probeIntervalMs": PROBE_INTERVAL_MS,
        "handshakeMs": HANDSHAKE_DEADLINE_MS,
    })
}

/// One report from `events.js`.
///
/// Tolerant of a vocabulary this build does not know, on the same terms as
/// `HostPhase`: an unrecognized report must cost nothing at all, because the
/// alternative is a deserialization error that this module cannot tell apart
/// from the bridge dying — and that would retire the feed permanently over a
/// message it merely did not understand.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FeedReport {
    /// A revision notification: the handshake, or a later bump.
    ///
    /// The number is decoded but deliberately never stored (see
    /// [`FeedState::notices`]); it is kept in the type as the WIRE contract
    /// with `events.js`, which the tests below assert against literal JSON.
    Revision { revision: u64 },
    /// The socket is gone; `events.js` is climbing its ladder.
    Down,
    #[serde(other)]
    Unrecognized,
}

/// Applies one report. Split from the task purely so the state machine can
/// be driven in a test without a runtime or a socket.
fn apply(state: &mut FeedState, report: FeedReport) {
    match report {
        FeedReport::Revision { .. } => state.note_revision(),
        FeedReport::Down => state.note_feed_down(),
        // Nothing to do, and specifically NOT a health change: a message
        // this build cannot read says nothing about whether the socket is up.
        FeedReport::Unrecognized => {}
    }
}

/// The subscription itself: mounted once, at App level, renders nothing.
///
/// A component rather than a bare hook because it needs a place in the tree
/// that outlives both pages, and `App` is that place. It paints no DOM of
/// its own — everything it produces is state other components read.
///
/// ## Why the snippet parks instead of returning
///
/// Dioxus wraps an evaluated snippet in an async function and calls
/// `dioxus.close()` the moment that function RESOLVES (see
/// `dioxus-web`'s `PROMISE_WRAPPER` and `dioxus-desktop`'s query wrapper).
/// A snippet that registered a callback and returned would therefore close
/// the channel out from under the very callback it just installed. So
/// `farhelmEvents.subscribe` returns a promise that settles only when the
/// subscription is superseded or stopped, and the snippet awaits it — the
/// snippet's lifetime IS the subscription's.
#[component]
pub(crate) fn FleetFeed() -> Element {
    let base = use_context::<ApiBase>().0;

    use_future(move || {
        let base = base.clone();
        async move {
            // Every value crosses the boundary through serde_json rather
            // than string interpolation, exactly as the terminal specs do:
            // the API base is configuration this process was started with,
            // and building JavaScript by concatenation is a habit worth not
            // having on the one origin that can reach the helm's API.
            let base_js = serde_json::to_string(&base).expect("a string is serializable");
            let policy = feed_policy();
            // This subscription's identity, minted per attempt (see
            // `NEXT_SUBSCRIPTION`). It travels to the island so that the
            // cleanup below can name what it is ending instead of ending
            // whatever happens to be live.
            let token = NEXT_SUBSCRIPTION.fetch_add(1, Ordering::Relaxed);
            // The wait for the island is unbounded, exactly like
            // terminal.js's `waitForIsland`, and for the same reason:
            // registration order is not execution order, and there is no
            // sensible thing to do if the asset never arrives except keep
            // waiting — the page is meanwhile on its fallback poll, which is
            // the honest behavior for a feed that never came up.
            let js = format!(
                r#"var send = function (report) {{ dioxus.send(report); }};
                   while (!window.farhelmEvents) {{
                       await new Promise(function (r) {{ setTimeout(r, 50); }});
                   }}
                   await window.farhelmEvents.subscribe({base_js}, {policy}, send, {token});"#
            );
            let mut channel = document::eval(&js);
            loop {
                match channel.recv::<FeedReport>().await {
                    Ok(report) => apply(&mut FLEET_FEED.write(), report),
                    // The channel is gone, which `events.js` never causes on
                    // its own — it reports a dead socket and keeps climbing
                    // its ladder rather than ending the subscription. So
                    // this arm is one of exactly two things, and neither has
                    // anything left to retry against:
                    //
                    // - The page WITHDREW the feed. Under a latched build
                    //   mismatch the effect below stops the island, which
                    //   settles the parked promise, which ends the snippet
                    //   and closes this channel. That is the withdrawal
                    //   working, not a failure — and the fallback poll stays
                    //   off because `fallback_polls` refuses it under skew,
                    //   not because of anything decided here.
                    // - The BRIDGE died. Nothing can reach the island at
                    //   all — the failure mode manual testing found on wry's
                    //   macOS webview, where every eval resolves immediately
                    //   (see `api::mint_lease` for that history). There is no
                    //   recovery short of a remount, and the page falls back
                    //   to polling exactly as it did before this milestone.
                    //
                    // Told apart nowhere, deliberately: the state that
                    // follows is identical, and a flag recording which one
                    // it was would have no reader. What makes both permanent
                    // is this `return` — the task ends, so nothing will ever
                    // mark the feed healthy again.
                    Err(_) => {
                        FLEET_FEED.write().note_feed_down();
                        // The socket outlives this task on any renderer that
                        // keeps evaluating after a channel closes, and a
                        // subscription nobody is listening to is worse than
                        // none: it holds a helm-side subscriber, and its
                        // handshake timer goes on climbing a ladder whose
                        // reports reach a channel that is gone. Released
                        // rather than STOPPED, and the difference is the
                        // withdrawal latch — this is a bridge that died, not
                        // a page that stood down, so a later mount must be
                        // free to subscribe again (see `events.js`).
                        //
                        // Named, not blanket: this task can outlive its own
                        // component, and a remount installs a fresh
                        // subscription while the old cleanup may still be on
                        // its way through the eval queue. An unscoped release
                        // would then tear down the REPLACEMENT — a page that
                        // silently loses its feed for the rest of its life
                        // because an earlier one died.
                        document::eval(&format!(
                            "if (window.farhelmEvents) {{ window.farhelmEvents.release({token}); }}"
                        ));
                        return;
                    }
                }
            }
        }
    });

    // The withdrawal, applied to the socket. A latched build mismatch stops
    // the feed as well as the fallback — see the module docs for why
    // polling a helm on another build is exactly what the skew gate exists
    // to revoke, and why the same argument covers a socket whose
    // notifications would send this bundle re-reading rows it cannot decode.
    //
    // Stopping the island resolves the parked subscribe promise, which ends
    // the snippet, which closes the channel, which lands the loop above on
    // its `Err` arm — so the state ends up unhealthy through the ordinary
    // path rather than through a second one written for this case.
    //
    // The LATCH is what makes this correct in both orderings, and the
    // ordering is genuinely undecided: the subscription waits for the island
    // in a task while this runs from an effect, and a mismatch can latch on
    // the very first reply — before `events.js` has been executed at all. A
    // withdrawal that only called `stop()` would then find nothing to stop,
    // and the subscription would open a moment later on a page that had
    // already stood down: a withdrawn client holding a socket whose every
    // notification sends it re-reading rows it cannot decode. Writing the
    // global FIRST closes that, because `events.js` reads it at registration
    // and again at every `subscribe` (see that file's `withdrawn`); calling
    // `stop()` after it covers the other order, where the island is already
    // there and holding a live socket.
    //
    // One-shot rather than a snippet that waits for the island, and
    // deliberately: an eval that parks has to be kept alive to keep running,
    // and an effect is not a place to hold a future. A plain assignment
    // needs neither.
    use_effect(move || {
        if skew::build_skew_detected() {
            document::eval(
                "window.farhelmFeedWithdrawn = true;
                 if (window.farhelmEvents) { window.farhelmEvents.stop(); }",
            );
        }
    });

    rsx! {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The withdrawal rule, as a table over both inputs.
    ///
    /// The two `false` answers are the point and they mean opposite things:
    /// a healthy feed does not poll because it has something better, and a
    /// skewed page does not poll because it has been told to stand down. A
    /// refactor that collapsed the two conditions into one boolean would
    /// pass every other test in this crate and quietly reintroduce a stale
    /// bundle hammering a helm it cannot read.
    #[farhelm_testtrace::test]
    fn the_fallback_polls_only_for_a_dead_feed_on_a_matching_build() {
        assert!(
            fallback_polls(false, false),
            "stamps match and the feed is down: this is the documented fallback"
        );
        assert!(
            !fallback_polls(false, true),
            "a healthy feed is the whole reason the loops were removed"
        );
        assert!(
            !fallback_polls(true, false),
            "under skew the page stands down rather than polling a helm it cannot decode"
        );
        assert!(
            !fallback_polls(true, true),
            "and skew wins even if a socket is somehow still up"
        );
    }

    /// The handshake invariant: the feed cannot become healthy without a
    /// re-read having been triggered.
    ///
    /// This is the whole fallback handover in one assertion. A mutation
    /// landing between the fallback's last poll and the resubscription is
    /// invisible to both mechanisms — the poll is over, the bump predates
    /// the subscription — so the handshake's re-read is the only thing that
    /// covers that window. An implementation that set `healthy` on connect
    /// and only bumped `notices` on a CHANGED revision would look correct,
    /// pass a two-client test, and lose exactly that mutation.
    ///
    /// Note what is NOT claimed: that the re-read landed. This module never
    /// learns how a read ended (`reader` owns that half), so the guarantee
    /// stated here is exactly the guarantee the code can keep.
    #[farhelm_testtrace::test]
    fn a_handshake_re_read_is_always_triggered_before_the_feed_is_trusted() {
        let mut state = FeedState::default();
        state.note_feed_down();
        let before = state.notices;

        // A handshake carrying a revision this page has ALREADY seen — the
        // case a change-detecting client would discard. Indistinguishable
        // here by construction, since the number is not kept at all.
        state.note_revision();
        assert!(state.healthy);
        assert_eq!(
            state.notices,
            before + 1,
            "a repeated revision is still a re-read: the handshake's job is the window, not the \
             number"
        );
    }

    /// A dropped socket puts the page back on its fallback, and a
    /// re-handshake takes it off again — the ordinary feed-death cycle.
    #[farhelm_testtrace::test]
    fn a_dropped_socket_hands_over_to_the_fallback_and_back() {
        let mut state = FeedState::default();
        apply(&mut state, FeedReport::Revision { revision: 7 });
        assert!(!fallback_polls(false, state.healthy));

        apply(&mut state, FeedReport::Down);
        assert!(
            fallback_polls(false, state.healthy),
            "a dead feed on a matching build is exactly when polling is right"
        );

        // The SAME revision the page already saw, which a resubscription's
        // handshake legitimately repeats: it still counts as a notice, which
        // is what hands the page back off the fallback.
        apply(&mut state, FeedReport::Revision { revision: 7 });
        assert_eq!(state.notices, 2);
        assert!(!fallback_polls(false, state.healthy));
    }

    /// A report this build does not understand must cost nothing — least of
    /// all the feed's health.
    ///
    /// The failure it guards against is a future `events.js` gaining a
    /// report kind (a diagnostic, say) and every older bundle treating it as
    /// a decode failure. A decode failure is indistinguishable from the
    /// bridge dying, which retires the feed PERMANENTLY — a disproportionate
    /// answer to a message that merely was not understood.
    #[farhelm_testtrace::test]
    fn an_unrecognized_report_changes_nothing() {
        let mut state = FeedState::default();
        apply(&mut state, FeedReport::Revision { revision: 3 });
        let before = state;

        let decoded: FeedReport =
            serde_json::from_value(serde_json::json!({ "kind": "weather", "outlook": "fine" }))
                .expect("an unknown kind decodes rather than failing");
        assert_eq!(decoded, FeedReport::Unrecognized);
        apply(&mut state, decoded);
        assert_eq!(state, before);
    }

    /// The two reports `events.js` actually sends must decode from the exact
    /// JSON it builds.
    ///
    /// A wire contract with a file the compiler never sees, which is why it
    /// is asserted against literal JSON rather than round-tripped: renaming
    /// a field on either side would otherwise fail silently, and silently
    /// here means a page that polls forever while a perfectly good socket
    /// delivers notifications nobody reads.
    #[farhelm_testtrace::test]
    fn the_reports_the_island_sends_decode_as_written() {
        assert_eq!(
            serde_json::from_value::<FeedReport>(
                serde_json::json!({ "kind": "revision", "revision": 12 })
            )
            .expect("the island's revision report"),
            FeedReport::Revision { revision: 12 }
        );
        assert_eq!(
            serde_json::from_value::<FeedReport>(serde_json::json!({ "kind": "down" }))
                .expect("the island's socket-loss report"),
            FeedReport::Down
        );
    }

    /// A subscription that ENDS leaves the page on its fallback, whether it
    /// ended because the bridge died or because the page withdrew the feed.
    ///
    /// One transition for both is the design (see `note_feed_down`), and the
    /// half worth pinning is that it is not recoverable from here: nothing
    /// this state machine can be told afterwards makes the feed healthy
    /// again, because the task that would have told it has returned. The
    /// permanence is the task's, not a flag's.
    #[farhelm_testtrace::test]
    fn a_subscription_that_ends_leaves_the_page_polling_for_good() {
        let mut state = FeedState::default();
        apply(&mut state, FeedReport::Revision { revision: 1 });
        state.note_feed_down();
        assert!(fallback_polls(false, state.healthy));
        // And under skew — the withdrawal case — the page does not poll
        // either, which is the whole point of standing down rather than a
        // second thing to arrange.
        assert!(!fallback_polls(true, state.healthy));
    }

    /// The policy is a WIRE contract with `events.js`, so its keys are
    /// asserted by name: a renamed field would not fail to compile, it would
    /// leave the island running whatever defaults it invented — most likely
    /// a tight reconnect loop against a helm that is down, or (for the
    /// deadline) a socket that parks forever on a helm that stopped talking.
    #[farhelm_testtrace::test]
    fn the_policy_carries_every_key_the_island_reads() {
        let policy = feed_policy();
        assert_eq!(policy["path"], serde_json::json!(EVENTS_PATH));
        assert_eq!(
            policy["delaysMs"],
            serde_json::json!([500, 1_000, 2_000, 4_000, 8_000, 15_000]),
            "the feed climbs the terminal ladder rather than one of its own"
        );
        assert_eq!(policy["probeIntervalMs"], serde_json::json!(30_000));
        assert_eq!(
            policy["handshakeMs"],
            serde_json::json!(10_000),
            "an opened socket that never greets must be given up on, not waited on forever"
        );
    }
}
