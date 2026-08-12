//! Native heartbeat over the desktop webview's eval bridge
//! (PLAN_desktop_web_bug_triage.md's native-watchdog step).
//!
//! The console shim (`assets/client-log-shim.js`) covers a JS exception the
//! webview can still RUN code to report. It cannot cover the MT-5 class of
//! failure — the eval bridge itself going dead. Page JS may well keep
//! running then (MT-5 was native evals answering `Err`, not a dead JS
//! engine); what bricks is every UI path that depends on native-requested
//! eval, with nothing on the native side saying so. This module is the other half: a
//! periodic native-side probe that notices the bridge stopped answering
//! even when nothing on the JS side is left alive to notice it, and turns
//! that into one loud `tracing` line instead of a silent brick a user
//! eventually reports with nothing but "it's frozen."
//!
//! ## Why a fresh one-shot eval every tick, never a persistent channel
//!
//! This is the design chosen after reading dioxus-desktop 0.7.10's query
//! engine (the plan allowed either shape and demanded the read) (`query.rs`, `document.rs` in that crate): a [`dioxus::prelude::
//! document::eval`] handle is a lightweight, `Copy` reference into a
//! `generational_box` arena; the actual `Query` — its channels, its
//! `oneshot` result receiver — is owned by an `Owner` stored INSIDE the
//! query engine's own slab entry, not by anything our Rust code holds. That
//! entry is freed only when the JS side calls `.close()` on its half of the
//! channel and the browser's `FinalizationRegistry` later garbage-collects
//! the now-unreferenced object, which posts a `"drop"` IPC message back to
//! Rust (`native_eval.js`). If the webview never runs again — exactly the
//! condition this watchdog exists to detect — that `.close()` call never
//! happens, the GC callback never fires, and the slab entry is never freed.
//!
//! Two consequences follow, both verified by that source read rather than
//! assumed (the plan's own warning: MT-5 exists because an assumption here
//! was wrong once already):
//!
//! - **Dropping our side does not cancel anything on the webview side.**
//!   Wrapping one `eval().join()` in [`tokio::time::timeout`] and letting it
//!   elapse only drops OUR future; the webview keeps whatever script it was
//!   asked to run (harmless here, since the probe script returns
//!   immediately when the bridge is alive) and the slab entry lives on
//!   regardless.
//! - **A timed-out probe's slab entry is a real, permanent leak while the
//!   bridge stays dead.** Every tick that misses leaks one `Query` (two
//!   small channels plus a boxed trait object) until either the bridge
//!   recovers and its `.close()`/GC roundtrip eventually runs, or the
//!   process exits. At [`HEARTBEAT_INTERVAL`]'s cadence this is negligible
//!   for any session length a human would tolerate staring at a bricked
//!   window, and it is NOT bounded by anything other than "the app
//!   was restarted" — a documented cost of the log-only response
//!   (PLAN_desktop_web_bug_triage.md's decision), not a claim that nothing
//!   leaks.
//!
//! A persistent send/receive channel (one eval whose JS pushes a beat via
//! `dioxus.send`, with per-tick `recv` timeouts) WOULD survive a timed-out
//! receive — the handle is not consumed — and would bound the leak to one
//! entry. It was considered and not chosen, because its recovery detection
//! hangs off the dead context's own timer loop resuming: precisely the
//! thing an MT-5-class failure says nothing may assume. A fresh one-shot
//! eval per tick gives every tick an independent chance to observe
//! recovery, at the cost of the small, documented leak above.
//!
//! ## Why log-only
//!
//! No reload, no process exit — an explicit product decision recorded in
//! PLAN_desktop_web_bug_triage.md ("the user notices a bricked UI on their
//! own; the log's job is making the subsequent report instant"), not an
//! oversight. This module's output is one `tracing::error!` on death and
//! one quiet info line on recovery; between them it keeps probing
//! silently, and it never reloads or exits anything.

use std::time::Duration;

/// How often the watchdog probes the eval bridge.
///
/// Generous by design (PLAN_desktop_web_bug_triage.md's stated risk: "a busy
/// webview — giant terminal writes — may answer heartbeats slowly"). Wider
/// than any single probe's [`HEARTBEAT_TIMEOUT`] by a wide margin, so a
/// merely slow tick can never overlap the next one.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

/// How long one probe may take before it counts as a miss.
///
/// A few seconds: generous enough that ordinary event-loop backpressure
/// (a large paste, a burst of terminal output) does not read as a dead
/// bridge, while staying far below [`HEARTBEAT_INTERVAL`] so a miss is
/// resolved well before the next tick would otherwise fire anyway.
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);

/// The probe script. Trivial and synchronous on purpose — this measures
/// whether the bridge answers AT ALL, not how fast it can run real work, so
/// there is nothing here for a slow webview to be slow at beyond the IPC
/// round-trip itself.
const HEARTBEAT_SCRIPT: &str = "return true;";

/// What one heartbeat tick observed. Kept separate from [`Health`] (the
/// accumulated state across ticks) so [`transition`] can be a pure function
/// of "one fact plus prior state" — see that function's docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickOutcome {
    /// The probe script ran and returned before [`HEARTBEAT_TIMEOUT`].
    Answered,
    /// The probe timed out, or the eval channel itself errored — both read
    /// identically here: the bridge did not demonstrably answer this tick,
    /// and this module has no use for WHY.
    Missed,
}

/// The watchdog's accumulated view of the bridge's health, driven one tick
/// at a time by [`transition`].
///
/// Three states, not two, is what encodes "two consecutive misses" without
/// a separate counter: a single miss from `Healthy` only reaches `Suspect`,
/// and it takes a SECOND consecutive miss (from `Suspect`) to reach `Dead`.
/// An `Answered` tick from `Suspect` returns straight to `Healthy` with no
/// log at all — a lone slow tick is exactly the false positive
/// PLAN_desktop_web_bug_triage.md's known-risks section warns against, and
/// it must stay silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Health {
    Healthy,
    /// One miss observed; one more consecutive miss declares the bridge
    /// dead. Not itself logged — see the type's docs.
    Suspect,
    /// Two consecutive misses observed and already reported once via
    /// [`LogAction::Dead`]. Stays here, silently, through every further
    /// miss until an `Answered` tick reports recovery.
    Dead,
}

/// What [`transition`] says this tick's caller should log, if anything.
///
/// "Only the transitions log, never the ticks" (PLAN_desktop_web_bug_triage.
/// md) — and exactly two transitions log: `Suspect → Dead` (the second
/// consecutive miss) answers [`LogAction::Dead`], and `Dead → Healthy`
/// answers [`LogAction::Recovered`]. Every state-PRESERVING tick, and every
/// silent transition (`Healthy → Suspect`, `Suspect → Healthy`), answers
/// `None` — so a bricked webview produces exactly one `Dead` line for as
/// long as it stays bricked, never a line per 15-second tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogAction {
    None,
    /// The second of two consecutive misses just landed: log the one loud
    /// "bridge is dead" line.
    Dead,
    /// A tick answered after the bridge had been reported dead: log the one
    /// quiet recovery line.
    Recovered,
}

/// The watchdog's whole decision procedure, factored out as this module's
/// one testable seam (PLAN_desktop_web_bug_triage.md budgets this change at
/// most one seam, and the repo forbids sleep-based tests) — a pure function
/// of "what did this tick observe" and
/// "what did we believe before it," with no clock, no IO, and no `eval` of
/// its own. Everything ELSE in this module is real IO wiring around calling
/// this once per tick.
fn transition(state: Health, outcome: TickOutcome) -> (Health, LogAction) {
    match (state, outcome) {
        (Health::Healthy | Health::Suspect, TickOutcome::Answered) => {
            (Health::Healthy, LogAction::None)
        }
        (Health::Healthy, TickOutcome::Missed) => (Health::Suspect, LogAction::None),
        (Health::Suspect, TickOutcome::Missed) => (Health::Dead, LogAction::Dead),
        (Health::Dead, TickOutcome::Missed) => (Health::Dead, LogAction::None),
        (Health::Dead, TickOutcome::Answered) => (Health::Healthy, LogAction::Recovered),
    }
}

/// Run one fresh, one-shot heartbeat probe and report whether the bridge
/// answered in time.
///
/// See the module docs for why this issues a BRAND NEW `document::eval`
/// every call rather than reusing one across ticks, and for what it costs
/// (a leaked query slab entry) on a miss.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
async fn probe_once() -> TickOutcome {
    use dioxus::prelude::document;
    let eval = document::eval(HEARTBEAT_SCRIPT);
    match tokio::time::timeout(HEARTBEAT_TIMEOUT, eval.join::<bool>()).await {
        Ok(Ok(_)) => TickOutcome::Answered,
        // A quick `EvalError` and a timeout both mean "no demonstrated
        // answer this tick" for this module's purposes — see
        // `TickOutcome::Missed`'s docs.
        Ok(Err(_)) | Err(_) => TickOutcome::Missed,
    }
}

/// Start the eval-bridge heartbeat for the lifetime of the desktop app.
///
/// Hooked into `App`'s desktop branch in `lib.rs` next to
/// `desktop::use_foreground_on_launch`, the existing precedent for a
/// desktop-only, launch-once background task: a `use_hook` that spawns a
/// future and never restarts it, because `App` itself never remounts across
/// the app's lifetime (reauthentication remounts `DesktopBootstrapGate`
/// underneath it, not `App`).
///
/// The first tick is deliberately skipped rather than probed immediately —
/// firing a heartbeat before the webview and its document context have had
/// any chance to settle at launch would risk exactly the false-positive
/// "suspect" reading PLAN_desktop_web_bug_triage.md's known risks warn
/// about, for a window of time that tells nobody anything they need to
/// know this early.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn use_webview_watchdog() {
    use dioxus::prelude::*;
    use_hook(|| {
        spawn(async move {
            let mut state = Health::Healthy;
            let mut ticker = tokio::time::interval(HEARTBEAT_INTERVAL);
            // Delay, not the burst default: after a laptop resume or a
            // stalled runtime, replaying missed ticks back-to-back would
            // let two compressed timeouts satisfy the two-miss rule in
            // seconds — declaring a merely-waking webview dead, the exact
            // false positive the 15s spacing exists to prevent.
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            ticker.tick().await;
            loop {
                ticker.tick().await;
                let outcome = probe_once().await;
                let (next, action) = transition(state, outcome);
                state = next;
                match action {
                    LogAction::None => {}
                    LogAction::Dead => tracing::error!(
                        target: "webview_watchdog",
                        "webview eval bridge is not answering; the UI may be bricked (MT-5 class)"
                    ),
                    LogAction::Recovered => tracing::info!(
                        target: "webview_watchdog",
                        "webview eval bridge is answering again after being reported dead"
                    ),
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ordinary steady state: every answered tick stays healthy and logs
    /// nothing, regardless of how many ticks accumulate.
    #[test]
    fn answered_ticks_stay_healthy_and_silent() {
        let mut state = Health::Healthy;
        for _ in 0..5 {
            let (next, action) = transition(state, TickOutcome::Answered);
            assert_eq!(next, Health::Healthy);
            assert_eq!(action, LogAction::None);
            state = next;
        }
    }

    /// A single miss must not itself declare the bridge dead — that would
    /// make an ordinary slow tick indistinguishable from MT-5, exactly the
    /// false positive the two-miss rule exists to prevent.
    #[test]
    fn one_miss_is_only_suspect_and_logs_nothing() {
        let (state, action) = transition(Health::Healthy, TickOutcome::Missed);
        assert_eq!(state, Health::Suspect);
        assert_eq!(action, LogAction::None);
    }

    /// A miss that clears on the very next tick — one slow response, not a
    /// dead bridge — returns silently to healthy with no log line, so a busy
    /// webview never produces log noise for a transient stall.
    #[test]
    fn a_suspect_tick_that_then_answers_recovers_silently() {
        let (suspect, _) = transition(Health::Healthy, TickOutcome::Missed);
        let (state, action) = transition(suspect, TickOutcome::Answered);
        assert_eq!(state, Health::Healthy);
        assert_eq!(action, LogAction::None, "a lone recovered miss is not news");
    }

    /// Two CONSECUTIVE misses is the documented threshold: the second one
    /// transitions to `Dead` and is the exact tick that must log the one
    /// loud line.
    #[test]
    fn two_consecutive_misses_declare_death_exactly_once() {
        let (suspect, first_action) = transition(Health::Healthy, TickOutcome::Missed);
        assert_eq!(
            first_action,
            LogAction::None,
            "the first miss alone must stay silent"
        );
        let (dead, second_action) = transition(suspect, TickOutcome::Missed);
        assert_eq!(dead, Health::Dead);
        assert_eq!(second_action, LogAction::Dead);
    }

    /// Once dead, further misses must stay silent — "only the transitions
    /// log, never the ticks" is the whole point of the state machine, and a
    /// bricked webview left running overnight must not fill the log with a
    /// line every 15 seconds.
    #[test]
    fn repeated_misses_while_dead_never_log_again() {
        let mut state = Health::Dead;
        for _ in 0..10 {
            let (next, action) = transition(state, TickOutcome::Missed);
            assert_eq!(next, Health::Dead);
            assert_eq!(action, LogAction::None);
            state = next;
        }
    }

    /// Recovery from `Dead` must log exactly once, and the state machine
    /// must accept a SUBSEQUENT run of ordinary healthy ticks afterward
    /// without logging again — proving the machine is not stuck believing
    /// it is still reporting.
    #[test]
    fn recovery_from_dead_logs_once_then_falls_silent_again() {
        let (healthy, action) = transition(Health::Dead, TickOutcome::Answered);
        assert_eq!(healthy, Health::Healthy);
        assert_eq!(action, LogAction::Recovered);
        let (still_healthy, next_action) = transition(healthy, TickOutcome::Answered);
        assert_eq!(still_healthy, Health::Healthy);
        assert_eq!(next_action, LogAction::None);
    }

    /// A full, realistic timeline strung together end to end: healthy for a
    /// while, a stall that clears (no log), a real outage declared dead
    /// (one log), the outage persisting across several ticks (no further
    /// logs), and recovery (one more log) — asserting the TOTAL log count
    /// across the whole run is exactly two, which is the property that
    /// actually matters to a human reading the resulting log file.
    #[test]
    fn a_realistic_timeline_produces_exactly_two_log_lines() {
        let outcomes = [
            TickOutcome::Answered,
            TickOutcome::Answered,
            TickOutcome::Missed,   // suspect, silent
            TickOutcome::Answered, // recovers, silent
            TickOutcome::Missed,   // suspect, silent
            TickOutcome::Missed,   // dead, LOGS
            TickOutcome::Missed,   // still dead, silent
            TickOutcome::Missed,   // still dead, silent
            TickOutcome::Answered, // recovers, LOGS
            TickOutcome::Answered,
        ];
        let mut state = Health::Healthy;
        let mut logged = Vec::new();
        for outcome in outcomes {
            let (next, action) = transition(state, outcome);
            state = next;
            if action != LogAction::None {
                logged.push(action);
            }
        }
        assert_eq!(logged, vec![LogAction::Dead, LogAction::Recovered]);
        assert_eq!(state, Health::Healthy, "the timeline ends healthy");
    }
}
