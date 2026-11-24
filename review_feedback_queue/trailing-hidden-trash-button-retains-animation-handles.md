# Hidden Trash button retains bob and wiggle handles across reduced-motion toggles and keeps the 10 Hz ticker running

Reviewed commit: 681a3909127d69e7087e82eba8efe551d2cffe35

## TLDR

If the Trash button stops being shown (because the user opens the Hosts panel or marks sessions) while its archive
wiggle or emptying bob is running, Farhelm does not release that animation handle while the button is hidden. The app
keeps its 10-times-a-second animation timer running in the background while the button is not on screen, and if Reduce
Motion is turned on and then off again while the button stays hidden, an interrupted archive wiggle can resume late when
the user returns to the session list.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/trash.rs:764`.

In `crates/gui/farhelm-gui/src/trash.rs:755-785`, `TrashButton::render` is the only place that creates or clears
`self.bob` (lines 766-768) and `self.wiggle` (lines 775-785). `SessionList` holds `trash: Entity<TrashButton>` for the
life of the window (`crates/gui/farhelm-gui/src/session_list.rs:375,499`), and omits it from the rendered tree while the
Hosts panel is shown (`crates/gui/farhelm-gui/src/session_list.rs:1068-1079`) or while sessions are marked
(`crates/gui/farhelm-gui/src/session_list.rs:1298-1300`). When `TrashButton` is hidden mid-animation:

1. With live Reduce Motion (added in this commit): if a wiggle starts (`self.wiggle = Some(...)`), the button is hidden,
   `cx.set_reduce_motion(true)` runs, and later `cx.set_reduce_motion(false)` runs before `TrashButton` renders again,
   `TrashButton::render` never ran while reduced motion was on, so `self.wiggle` was never dropped. Because
   `ticker::motion_changed` (`crates/gui/farhelm-gui/src/ticker.rs:441-455`) cancelled the tick loop while reduced
   motion was active, `self.wiggle`'s `TickCounter.count` stayed frozen below 9 ticks; when motion returns and the
   button becomes visible again, the stale interrupted wiggle resumes mid-swing, violating the stated intent at
   `crates/gui/farhelm-gui/src/trash.rs:773-774` and `SPEC.md`, "Managed checkouts" ("Reduced motion skips both parts").
2. Even without toggling Reduce Motion, hiding `TrashButton` while `self.wiggle` or `self.bob` is `Some(...)` leaves its
   `ticker::AnimationTicks` (`TickCounter` + `_animating: Animating`) live indefinitely until `TrashButton` renders
   again, keeping the ticker's 10 Hz `tick` loop running in an active window even after the 900 ms wiggle has elapsed
   (`crates/gui/farhelm-gui/src/ticker.rs:248-277,563-594`; compare
   `crates/gui/farhelm-gui/src/trash/cue/tests.rs:233`).

How to verify: Start a confirmed archive wiggle (or emptying bob) on `TrashButton`, click `hosts-panel-toggle` (or mark
a session) after 1 tick, then either (a) toggle `cx.set_reduce_motion(true)`, advance time, toggle
`cx.set_reduce_motion(false)`, return to the list and observe `wiggling(cx)` / `turn(cx)` still non-empty, or (b)
advance 2 seconds while Hosts stays open and observe `ticker::ticks(cx)` continuing to advance at 10 Hz.

Fix: Release `self.bob` and `self.wiggle` when `TrashButton` stops being drawn (for instance via `cx.observe_release` on
the keyed `checkout-trash-shown` entity created in `render`, or an explicit hide hook from `SessionList`), and observe
`cx.observe_reduce_motion` on `TrashButton` (or clear `wiggle`/`bob` when `reduced_motion` turns on) so a hidden button
cannot retain an interrupted wiggle across a reduced-motion toggle.

Reviewer metadata (g28_reduced_motion_tests_and_trash/f1): suggested bucket: other — Minor correctness and idle-timer
waste when hiding the Trash button mid-animation; no security, data-loss, or material UX degradation consequence.;
confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g28_reduced_motion_tests_and_trash/f1`.
