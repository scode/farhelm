# Live motion resume test does not witness cursor blink resuming

Reviewed commit: 681a3909127d69e7087e82eba8efe551d2cffe35

## TLDR

The test checking that live Reduce Motion changes pause and then resume both the terminal cursor blink and the
running-session pulse does not actually verify that the cursor blink resumes blinking after motion is turned back on.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/ticker_tests.rs:621`.

In `crates/gui/farhelm-gui/src/ticker_tests.rs:561-630` (`a_live_motion_change_pauses_drawing_without_pausing_time`),
the test pauses at `ticks = 5` (`drawn == (false, 2)`), turns reduced motion on (`drawn == (true, 0)` at line 608 and
line 619), and then checks resumption at lines 621-629:

```rust
cx.update(|_, cx| cx.set_reduce_motion(false));
cx.run_until_parked();
advance(cx, Duration::from_millis(500));
assert_eq!(cx.update(|_, cx| ticker::ticks(cx)), paused + 5);
assert_eq!(
    drawn.get(),
    (true, 4),
    "both animations resume on the original handles"
);
```

At `ticks = paused + 5 = 10`, `blink.is_on(cx)` is `true` — the exact same boolean value (`true`) that `drawn.get().0`
held throughout the reduced-motion pause (lines 608 and 619). The intermediate redraw immediately after
`cx.set_reduce_motion(false); cx.run_until_parked();` (where `ticks == 5` makes `drawn.get() == (false, 2)`) is never
asserted, and because `MotionProbe` holds both `blink` and `pulse` on the same view, `pulse` wakes `MotionProbe` at
ticks 8 and 10 even if `blink`'s subscriber never woke it. Consequently, if `Blinking` stayed stuck at `true` or stopped
notifying after a reduced-motion toggle, line 625 would still pass.

How to verify: Inspect `crates/gui/farhelm-gui/src/ticker_tests.rs:608-629`; `drawn.get().0` is `true` at line 619 and
`true` at line 627 with no check of `false` in between.

Fix: Assert `assert_eq!(drawn.get(), (false, 2))` after `cx.set_reduce_motion(false); cx.run_until_parked();` before
advancing 500 ms (and/or advance another 500 ms or test a blink-only view so blink's own subscriber wakeup is
independently witnessed).

Reviewer metadata (g28_reduced_motion_tests_and_trash/f2): suggested bucket: other — Test-quality gap confined to unit
tests; no user-facing security, data-loss, or UX consequence.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g28_reduced_motion_tests_and_trash/f2`.
