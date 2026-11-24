# Reminder::frame uses a #[cfg(test)] branch in the render path for golden capture

Reviewed commit: 0ef2076df168067ba1fe16afdf3f90e923b99f1c

## TLDR

The pending-update reminder screenshot test relies on a test-only compile branch inside the rendering path, violating
the project's rule that tests and screenshot captures render through the same production code path that ships.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/update_readout/view/reminder.rs:80`.

In `crates/gui/farhelm-gui/src/update_readout/view/reminder.rs:30-31` and `80-83` (implemented in interface group
`g37_pending_upgrade_impl`, consumed by `crates/gui/farhelm-gui/src/goldens/desktop.rs:166-168` in
`g38_pending_upgrade_tests` via `VersionReadout::holding_reminder` in
`crates/gui/farhelm-gui/src/update_readout/view.rs:646-653`), `Reminder::held_frame` and the early return
`#[cfg(test)] if let Some(tick) = self.held_frame { return frame(tick); }` sit directly inside `Reminder::frame`, which
is called during `VersionReadout::toggle` (`view.rs:324`) and `VersionReadout::reminder_gap` prepaint
(`reminder.rs:210`).

`SPEC_impl.md` ("GUI architecture", "GPUI cookbook: traps") forbids `#[cfg(test)]` branches in a render path that change
what is drawn or done ("Rule: pass the dependency in, so the production path reads it the same way in every build and a
test hands it a different one ... In a render path it must not change what is drawn or done, or the frame a test checks
differs from the one the app draws"). The sibling golden hold mechanism `VersionReadout::step_hold`
(`crates/gui/farhelm-gui/src/update_readout/view.rs:138-143`, `253-256`) follows this rule by keeping
`step_hold: Option<u8>` and its read unconditional in all builds, gating only the test-only setter `holding_step` with
`#[cfg(test)]`.

How to verify: Inspect `reminder.rs:30-31`, `reminder.rs:76-85`, and contrast with `view.rs:138-143` and `SPEC_impl.md`
"GPUI cookbook: traps".

Fix: Remove `#[cfg(test)]` from `Reminder::held_frame` (`reminder.rs:30`) and from the
`if let Some(tick) = self.held_frame` check in `Reminder::frame` (`reminder.rs:80`), keeping `#[cfg(test)]` only on the
test-only constructor `VersionReadout::holding_reminder` (`view.rs:646`), matching `step_hold`/`holding_step`.

Reviewer metadata (g38_pending_upgrade_tests/f1): suggested bucket: other (violates an internal GUI testing/build-parity
rule in `SPEC_impl.md` with no security, data-loss, or user-visible runtime impact; in release builds `held_frame` is
absent/unused).; confidence: confirmed (verified directly from source at `0ef2076df168067ba1fe16afdf3f90e923b99f1c` and
`1b3fe49af86979aebc2719965682a17f8c6366ac`).; possible cover: none.

Additional source record at `0ef2076df168067ba1fe16afdf3f90e923b99f1c`,
`crates/gui/farhelm-gui/src/update_readout/view/reminder.rs:79` (g37_pending_upgrade_impl/f1):

In `crates/gui/farhelm-gui/src/update_readout/view/reminder.rs:30-31` and `reminder.rs:75-84`, `Reminder` defines
`#[cfg(test)] pub(super) held_frame: Option<u64>` and `Reminder::frame(&self, cx: &App)` branches on
`#[cfg(test)] if let Some(tick) = self.held_frame { return frame(tick); }` before consulting `self.burst`.
`Reminder::frame` is called during `VersionReadout::render` (`update_readout/view.rs:324`) and during
`VersionReadout::reminder_gap` prepaint (`reminder.rs:208`). `SPEC_impl.md`, "GPUI cookbook: traps" ("A `cfg(test)`
branch in production code tests a program that does not ship") states: "In a render path it must not change what is
drawn or done, or the frame a test checks differs from the one the app draws." The sibling spinner hold in
`crates/gui/farhelm-gui/src/update_readout/view.rs:133-139` and `:255` (`step_hold: Option<u8>`) follows that rule by
keeping the field and its read unconditional in every build and gating only the `holding_step` setter with
`#[cfg(test)]`. How to verify: inspect `reminder.rs:29-31`, `reminder.rs:75-84`, `view.rs:133-139`, and `SPEC_impl.md`
("GPUI cookbook: traps"). Fix: make `held_frame: Option<u64>` and its read in `Reminder::frame` unconditional (always
`None` in non-test builds, matching `step_hold`), leaving only `VersionReadout::holding_reminder` gated by
`#[cfg(test)]`.

Reviewer metadata (g37_pending_upgrade_impl/f1): suggested bucket: other (violates an explicit `SPEC_impl.md` GUI
render-path rule for test/golden fidelity, with no security, data-loss, or user-visible runtime impact in shipping
builds).; confidence: confirmed (directly verified in source at `0ef2076df168067ba1fe16afdf3f90e923b99f1c` and
`1b3fe49af86979aebc2719965682a17f8c6366ac`).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g37_pending_upgrade_impl/f1`,
`g38_pending_upgrade_tests/f1`.
