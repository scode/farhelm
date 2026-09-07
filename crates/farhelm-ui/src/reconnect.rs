//! Terminal auto-reconnect (PLAN_M6.md item 7): the ladder, the phases,
//! and the words the terminal surface says while it is climbing them.
//!
//! The mechanism — noticing a dead socket, remounting an island, arming a
//! heartbeat — is terminal.js's, for the same reason the paste/drop
//! mechanism is (see `attachments`'s header): it lives on the DOM and
//! WebSocket side of a boundary neither renderer can reach across. What is
//! computed HERE is everything that is a decision rather than a mechanism:
//! how long to wait before each attempt, when the active window is spent
//! and background probing takes over, and what the user reads while either
//! is happening. Everything here is DATA — the numbers and the sentences,
//! serialized once into the page — because the state machine that consumes
//! it has to live where the sockets are, and a second copy of it on this
//! side would be a model of the real one rather than the real one. What
//! that leaves testable in Rust is exactly what belongs to Rust: the
//! values shipped, and the wording a user reads.
//!
//! ## What reconnects, and what deliberately does not
//!
//! Transport loss, in three shapes (PLAN_M6.md item 7): a socket that
//! CLOSES with no explanation from the server; a socket that silently
//! stops carrying anything while still looking open — the laptop-wake case
//! this milestone exists to fix, where a NAT or sleep timeout killed the
//! connection without either end noticing, and what the heartbeat below is
//! for; and loss the server DOES explain because it happened upstream of
//! the browser — the helm losing its own connection to a supervisor, or a
//! host that has gone away, both of which arrive as detach notices and are
//! no less a dropped connection for having been narrated. All three enter
//! the same visible retry flow.
//!
//! Two DECISIONS are carved out, and terminal.js enforces them by matching
//! the reasons the server sends (its `decisionDetach`). The reasoning is
//! worth keeping next to the numbers:
//!
//! - A takeover detach keeps its take-control surface. A displaced client
//!   that bounced back would fight the new owner, and the fight would be
//!   visible as the two clients' takeovers alternating.
//! - A stall detach keeps its banner. The stalled client's wedge is the
//!   reason it was detached, and reconnecting into the same wedge helps
//!   nobody; the user acts first.
//!
//! Every OTHER detach notice is infrastructure failing — the helm losing
//! its supervisor, a host that went away — which is transport loss one
//! layer up and recovers like any other. Vetoing on those instead (the
//! first version of this did) would have left a terminal dead behind a
//! banner for exactly the overnight outage SPEC.md promises it survives.
//! A navigation-caused close is not a failure at all and never reaches
//! this ladder either.
//!
//! One more rule belongs to the same family and is enforced on the wire
//! rather than here: an automatic attempt attaches with
//! `ControlMsg::Attach::if_unowned`, so a recovery can never take the
//! session from a client that attached while this one was away. The
//! refusal comes back as the ordinary takeover notice, which lands the
//! recovering view in the take-control state it was already in.

use serde_json::json;

/// The active-retry ladder, in milliseconds — PLAN_M6.md item 7's
/// "0.5s, 1s, 2s, 4s, 8s, 15s — about thirty seconds", verbatim.
///
/// Doubling from half a second is what makes the common case invisible: a
/// socket dropped by a wifi handover or a helm restart is usually back on
/// the first or second rung, so the user sees a blink rather than a
/// dialog. The ladder then backs off rather than hammering, and its total
/// (~30s) is the window worth spending on "this will fix itself in a
/// moment" before admitting that it might not.
pub(crate) const RETRY_LADDER_MS: [u32; 6] = [500, 1_000, 2_000, 4_000, 8_000, 15_000];

/// How often a terminal re-probes once the active window above is spent.
///
/// The same two-regime shape SPEC.md's Errors section already gives host
/// connections — "bounded retries followed by periodic low-frequency
/// re-probing, so a host that comes back overnight resurfaces by itself" —
/// applied to one terminal's socket. Thirty seconds is cheap enough to run
/// forever in a background tab and short enough that a user who fixes
/// their network does not sit wondering whether anything is still trying.
///
/// Deliberately unbounded in count: there is no failure state to reach.
/// The manual control exists for impatience, not for necessity.
pub(crate) const PROBE_INTERVAL_MS: u32 = 30_000;

/// How long a terminal's socket may carry NOTHING before the heartbeat
/// asks whether it is still alive.
///
/// Idle-gated, which is what makes it free: a terminal with output flowing
/// re-arms this on every frame and never sends anything at all. Only a
/// quiet socket is ever probed, and quiet is exactly the state in which
/// "the connection died and nobody noticed" is indistinguishable from "the
/// agent is thinking".
///
/// Fifteen seconds is far longer than any gap the transport itself
/// introduces and short enough that a user who wakes their laptop and
/// starts typing gets an answer before they conclude the app is broken.
pub(crate) const HEARTBEAT_IDLE_MS: u32 = 15_000;

/// How long the heartbeat waits for the helm's answer before declaring the
/// socket dead.
///
/// Generous on purpose. Expiring too eagerly costs a needless reattach —
/// cheap, but it re-runs a replay and briefly hides a working terminal —
/// while waiting only means a genuinely dead socket is noticed a few
/// seconds later. Ten seconds clears any plausible scheduling delay in a
/// loaded browser tab or a helm under load.
pub(crate) const HEARTBEAT_TIMEOUT_MS: u32 = 10_000;

/// The two properties those numbers have to keep, checked at COMPILE time
/// rather than by a test — both are statements about constants, so a
/// violation should never get as far as being runnable (the same discipline
/// `tabs::MAX_MOUNTED_TAB_ISLANDS` keeps).
///
/// The ORDERING is the real contract: with a timeout longer than the idle
/// gap, a second probe could be armed while the first answer is still
/// outstanding, and the socket would then be declared dead by whichever
/// answer happened to be missing. The combined latency is a bound rather
/// than a value because it is what the user experiences — a wedged terminal
/// noticed within about half a minute of its last byte, which is the whole
/// point of the check.
const _: () = assert!(HEARTBEAT_TIMEOUT_MS <= HEARTBEAT_IDLE_MS);
const _: () = assert!(HEARTBEAT_IDLE_MS + HEARTBEAT_TIMEOUT_MS <= 30_000);

/// What the surface says while an attempt is IN FLIGHT during the active
/// window.
///
/// It names the ordinary thing that is happening and that it is automatic,
/// because the single most valuable piece of information here is that the
/// user does not have to do anything. `{n}`/`{of}` are substituted by
/// terminal.js.
const ATTEMPTING_TEXT: &str = "connection lost — reconnecting (attempt {n} of {of})…";

/// What the surface says BETWEEN attempts during the active window.
///
/// The countdown is stated as it was scheduled rather than ticked down: a
/// per-second repaint of a terminal overlay buys nothing a user is
/// watching for, and the number's job is to say "soon, and by itself".
const WAITING_TEXT: &str = "connection lost — reconnecting in {seconds}s (attempt {n} of {of})…";

/// What the surface says while a background probe is in flight.
///
/// No attempt number: past the active window the count is a number nobody
/// is counting, and a rising one would read as escalating failure rather
/// than as patience.
const PROBING_TEXT: &str = "connection lost — checking whether this terminal is reachable again…";

/// What the surface says between background probes.
///
/// The promise it makes is the one that matters overnight: nothing more is
/// required of the user, and this terminal reattaches itself when the
/// connection comes back.
const PROBING_WAITING_TEXT: &str = "connection lost — retrying every {seconds}s; this terminal \
                                    reattaches on its own once the connection is back";

/// What the surface says when automatic recovery is OFF because the helm
/// is not the build this page was made for.
///
/// It names the cause, because the alternative — a terminal that simply
/// stops trying, with a button and no explanation — is the silent
/// degradation SPEC.md's version rule forbids. Both remedies are offered
/// in the order they should be tried: the button gets this terminal back
/// now, and the reload gets the whole page onto the matching build.
const MANUAL_ONLY_TEXT: &str = "connection lost — this page and the helm report different builds, \
                                so this terminal will not reconnect on its own; press reconnect \
                                now, or reload the page";

/// The manual control's label.
///
/// Present from the FIRST failure onward (PLAN_M6.md item 7, user decision
/// 2026-08-04), not only once the ladder is spent: a user who knows their
/// VPN just came back should never have to wait out a backoff they can see
/// on screen.
const MANUAL_TEXT: &str = "reconnect now";

/// Everything terminal.js needs to run the reconnect flow, as the JSON
/// `farhelmTerm.sync()` takes.
///
/// One object per session view, alongside the attachment policy and for
/// the same reasons: the values are identical for every island, and the
/// wording a user reads when their terminal drops is reviewable Rust prose
/// rather than a string buried in a script file.
///
/// The `{n}`/`{of}`/`{seconds}` placeholders are substituted by
/// terminal.js's `fillTemplate`, the same one-pass substitution the
/// attachment messages use.
///
/// `capable` is whether the helm answering this page is the build this
/// bundle was made for (`skew::helm_is_current`), and it gates both
/// UNATTENDED behaviors at once — the heartbeat and automatic attempts —
/// because both depend on the far end honoring something this milestone
/// added.
///
/// The heartbeat needs `ping`, which an older helm ignores by its own
/// tolerance contract: the probe would go unanswered on a perfectly
/// healthy socket and be read as death, every fifteen seconds, forever.
///
/// Automatic attempts need `if_unowned` — the field that makes an
/// unattended attach refuse rather than displace. A helm that predates it
/// drops it silently and performs the displacing attach, so a page talking
/// to one would take sessions from other clients on its own initiative,
/// which is the failure PROTOCOL_VERSION 9 exists to make impossible
/// between helm and supervisor and this gate makes impossible between
/// browser and helm. There is no handshake on this edge to refuse at, so
/// the build stamp is the handshake: no stamp, or a stamp that disagrees,
/// means unattended behavior is off.
///
/// What stays ON regardless is the MANUAL control. Pressing "reconnect
/// now" is a user asking to take the session, which is the ordinary
/// displacing attach every client has always made — nothing about it
/// depends on the far end understanding anything new.
pub(crate) fn reconnect_policy(capable: bool) -> serde_json::Value {
    json!({
        "delaysMs": RETRY_LADDER_MS,
        "probeIntervalMs": PROBE_INTERVAL_MS,
        "auto": capable,
        "heartbeat": capable.then(|| json!({
            "idleMs": HEARTBEAT_IDLE_MS,
            "timeoutMs": HEARTBEAT_TIMEOUT_MS,
        })),
        "text": {
            "attempting": ATTEMPTING_TEXT,
            "waiting": WAITING_TEXT,
            "probing": PROBING_TEXT,
            "probingWaiting": PROBING_WAITING_TEXT,
            "manual": MANUAL_TEXT,
            "manualOnly": MANUAL_ONLY_TEXT,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ladder is PLAN_M6.md item 7's, written out as literals rather
    /// than derived: it is a user-visible cadence settled by a decision, so
    /// a test that recomputed it from the same constant would pass for any
    /// rung someone quietly retuned.
    ///
    /// The total matters as much as the rungs. "About thirty seconds" is
    /// what makes the active window a window — long enough to cover a wifi
    /// handover or a helm restart, short enough that a user staring at a
    /// dead terminal is told the truth about it quickly.
    #[farhelm_testtrace::test]
    fn the_active_ladder_is_the_planned_six_rungs_totalling_about_thirty_seconds() {
        assert_eq!(RETRY_LADDER_MS, [500, 1_000, 2_000, 4_000, 8_000, 15_000]);
        let total: u32 = RETRY_LADDER_MS.iter().sum();
        assert_eq!(total, 30_500, "the active window is ~30s of real waiting");
    }

    /// An INCAPABLE helm switches both unattended behaviors off in the one
    /// place that decides them, and leaves the manual wording present —
    /// the surface still has to explain itself, and still has to offer the
    /// control that works regardless.
    #[farhelm_testtrace::test]
    fn an_incapable_helm_withholds_every_unattended_behavior() {
        let policy = reconnect_policy(false);
        assert_eq!(policy["auto"], json!(false));
        assert_eq!(
            policy["heartbeat"],
            json!(null),
            "no timings at all: a probe an older helm ignores would be read as death"
        );
        assert!(
            policy["delaysMs"].is_array() && policy["text"]["manual"].is_string(),
            "the ladder and the manual control still ship — a user may still ask"
        );
    }

    /// The policy is a WIRE contract with terminal.js, so its keys are
    /// asserted by name: a renamed field would not fail to compile, it
    /// would silently leave the page running its own fallbacks — or, for
    /// the text keys, painting a literal `undefined` over a dead terminal.
    #[farhelm_testtrace::test]
    fn the_policy_carries_every_key_the_page_reads() {
        let policy = reconnect_policy(true);
        assert_eq!(
            policy["delaysMs"],
            json!([500, 1_000, 2_000, 4_000, 8_000, 15_000])
        );
        assert_eq!(policy["probeIntervalMs"], json!(PROBE_INTERVAL_MS));
        assert_eq!(policy["auto"], json!(true));
        assert_eq!(policy["heartbeat"]["idleMs"], json!(HEARTBEAT_IDLE_MS));
        assert_eq!(
            policy["heartbeat"]["timeoutMs"],
            json!(HEARTBEAT_TIMEOUT_MS)
        );
        for key in [
            "attempting",
            "waiting",
            "probing",
            "probingWaiting",
            "manual",
            "manualOnly",
        ] {
            assert!(
                policy["text"][key].as_str().is_some_and(|t| !t.is_empty()),
                "the page paints text[{key}] onto a terminal that just died"
            );
        }
    }

    /// The two regimes must READ differently, because they mean different
    /// things to the person watching: one says "wait a moment", the other
    /// says "this may be a while, and you still do not have to do
    /// anything". A shared sentence would collapse the phase distinction
    /// SPEC.md's Errors section requires be visible.
    #[farhelm_testtrace::test]
    fn the_two_regimes_are_worded_apart() {
        for active in [ATTEMPTING_TEXT, WAITING_TEXT] {
            assert!(
                active.contains("{n}") && active.contains("{of}"),
                "the active window names which attempt this is: {active}"
            );
        }
        for probing in [PROBING_TEXT, PROBING_WAITING_TEXT] {
            assert!(
                !probing.contains("{n}"),
                "a rising count past the window reads as escalating failure: {probing}"
            );
        }
        assert!(
            WAITING_TEXT.contains("{seconds}") && PROBING_WAITING_TEXT.contains("{seconds}"),
            "a wait states how long it is"
        );
        assert!(
            PROBING_WAITING_TEXT.contains("on its own"),
            "the overnight promise is that nothing more is asked of the user"
        );
    }
}
