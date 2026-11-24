# Ticker leaks dead animation subscribers without bound while reduced motion stays enabled

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When the user keeps the operating system's Reduce Motion setting turned on, Farhelm's GUI animation list accumulates
stale entries every time a cursor blink, pulsing indicator, or spinner starts and stops, leaking memory slowly over a
long desktop session because nothing prunes dead entries until Reduce Motion is turned back off.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/ticker.rs:421`.

In `crates/gui/farhelm-gui/src/ticker.rs`, `subscribe` (lines 421-437) pushes a new `Subscriber` (holding a `Weak<()>`
that also retains the `Rc` control block allocation) into `Ticker::animating` and calls `start_tick_loop(cx)`. When
`reduced_motion(cx)` is `true`, `start_tick_loop` (lines 458-473) returns immediately without running `tick` (lines
563-595), which is where `ticker.animating.retain(|s| s.alive.strong_count() > 0)` normally runs. `motion_changed`
(lines 441-455) explicitly leaves dead holders unpruned while reduced
(`/// While paused, nothing prunes dead holders until motion returns.`) and only prunes when `reduced` transitions back
to `false`. For users who keep OS Reduce Motion permanently enabled, motion never returns: every terminal focus/blur or
window switch (`crates/gui/farhelm-gui/src/terminal/attached.rs:1761`), session/host pulse transition
(`crates/gui/farhelm-gui/src/session_list.rs:977`, `session_pane.rs:790`, `hosts.rs:749`, etc.), and `Spinner::start`
(`crates/gui/farhelm-gui/src/components/spinner.rs:41`) appends a dead entry to `Ticker::animating` that persists for
the entire process lifetime. Verification: inspect `subscribe` (lines 421-437), `motion_changed` (lines 441-455), and
`start_tick_loop` (lines 458-473) in `crates/gui/farhelm-gui/src/ticker.rs`, or add a test that repeatedly creates and
drops `ticker::blink`/`ticker::animate` handles while `cx.set_reduce_motion(true)` and asserts `Ticker::animating.len()`
stays bounded. Fix: prune dead entries (`ticker.animating.retain(|subscriber| subscriber.alive.strong_count() > 0)`)
inside `subscribe` before pushing (at least whenever `!ticker.running`), and optionally also when `motion_changed`
pauses on `reduced == true`. (Revision reason: tightened exact line numbers at
1b3fe49af86979aebc2719965682a17f8c6366ac.)

Reviewer metadata (g27_reduced_motion_gpui_and_core/f1): suggested bucket: other (unbounded slow memory growth under
permanently enabled Reduce Motion; does not risk security or loss of user-owned work and grows slowly enough not to
materially degrade UX quickly).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g27_reduced_motion_gpui_and_core/f1`.
