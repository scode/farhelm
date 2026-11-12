//! What the terminal path's backpressure suite keeps now that every
//! terminal a test creates is a terminal host: the browser scrollback bound
//! (a static check of `terminal.js` against tmux's history limit), and
//! [`drain_for`], which other modules share.
//!
//! The byte stream's pause, stall and tmux catch-up tests needed tmux
//! terminals the supervisor created, and went with them. A grid stream's
//! pause, stall and history bounds are pinned by the supervisor's
//! `service::grid_stream` tests and the helm's terminal relay tests.

use crate::harness::*;

/// Drain an attachment into `seen` for `window`, returning any detach
/// reason that arrived.
///
/// Distinct from [`wait_for`] because its callers need to observe the
/// ABSENCE of something or to keep reading through a period with no
/// particular marker due — neither of which a needle-driven wait can
/// express.
pub(crate) async fn drain_for(
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    window: Duration,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            // The replay-complete marker (PLAN_M5.md item 4) is
            // presentation metadata no caller of this drain asserts on;
            // the catch-up boundary has its own coverage, helm-side, in
            // farhelm-helm's client.rs/lib.rs.
            Ok(Some(TermEvent::ReplayComplete)) => {}
            Ok(Some(TermEvent::Detached(detach))) => return Some(detach.reason),
            Ok(None) => return Some("closed".to_string()),
            Err(_) => return None,
        }
    }
}

/// The cross-language invariant PLAN_M2_5.md's honesty argument rests on:
/// the browser's scrollback capacity must sit between SPEC.md's promised
/// floor and tmux's actual history floor, never outside either end.
///
/// Why the UPPER bound matters: after a deep stall the catch-up replays at
/// most `HISTORY_LIMIT` lines. If xterm.js could retain MORE than that, a
/// user would watch scrollback they already had get truncated by the
/// recovery — visible, unexplained loss. Holding the browser at or below
/// the floor is what makes the catch-up's end state observably equivalent
/// to lossless slow delivery instead.
///
/// Why the LOWER bound matters, and why it is pinned HERE rather than left
/// implicit: SPEC.md's own product promise is "at least the current screen
/// plus 10,000 lines of scrollback" — a real minimum, not merely "whatever
/// happens to be at most `HISTORY_LIMIT`". Before this bound was added,
/// `scrollback: 0` (or any value far below the promised floor) satisfied
/// the upper-bound check just as well as a correct value, silently
/// defeating the whole product guarantee this test exists to protect.
///
/// Asserted by reading the UI asset directly, because nothing else
/// connects these numbers: they live in different languages, in different
/// crates, with no shared build step. A test that pinned only the Rust
/// constants would go green while the JavaScript drifted, which is
/// precisely the failure this exists to catch.
#[farhelm_testtrace::test]
fn browser_scrollback_stays_within_the_product_floor_and_the_tmux_history_ceiling() {
    /// SPEC.md: "the terminal retains, and replay covers, at least the
    /// current screen plus 10,000 lines of scrollback."
    const SPEC_MINIMUM_SCROLLBACK: u32 = 10_000;

    let terminal_js =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../farhelm-ui/assets/terminal.js");
    let source = std::fs::read_to_string(&terminal_js)
        .unwrap_or_else(|e| panic!("reading {}: {e}", terminal_js.display()));
    let (_, after) = source
        .split_once("scrollback:")
        .expect("terminal.js must configure an explicit xterm.js scrollback");
    let scrollback: u32 = after
        .trim_start()
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|digits| digits.parse().ok())
        .expect("terminal.js's scrollback must be a plain integer literal");
    assert!(
        scrollback <= farhelm_supervisor::tmux::HISTORY_LIMIT,
        "terminal.js keeps {scrollback} lines of scrollback but tmux only guarantees {} — a \
         post-stall catch-up would visibly truncate history the user already had",
        farhelm_supervisor::tmux::HISTORY_LIMIT
    );
    assert!(
        scrollback >= SPEC_MINIMUM_SCROLLBACK,
        "terminal.js keeps only {scrollback} lines of scrollback but SPEC.md promises at least \
         {SPEC_MINIMUM_SCROLLBACK} — this is a broken product promise, not merely a cosmetic gap"
    );
}
