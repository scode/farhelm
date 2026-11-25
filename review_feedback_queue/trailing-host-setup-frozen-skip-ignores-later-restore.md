# Turning "Set up new hosts without asking" off during a host check no longer brings back the confirmation

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

If the user presses Add (or Re-run setup) while "Set up new hosts without asking" is on, then turns that switch off
while Farhelm is still checking the host (which can take a while, or wait on SSH sign-in), Farhelm installs on the host
anyway without showing the setup confirmation. Before this change, turning the switch off during the check brought the
confirmation back. The spec says turning the switch off restores the confirmation and that changes take effect
immediately.

## Details

Concerns 7c2cfe02db ("fix: keep host actions tied to what was shown", #4849). Locations:
`crates/gui/farhelm-gui/src/hosts/add.rs:1313` (`if !asks_before_setup && !self.from_agent`), `add.rs:948-954` (asks
read from `setup_frame` at the press), `add.rs:1124` (the sign-in replay keeps it);
`crates/gui/farhelm-gui/src/hosts/setup.rs:799-802` (the Re-run plan uses `asks_before_setup`), `hosts/menu.rs:419-421`
(Re-run captures `asks` at draw).

The commit freezes the setup-consent preference at press time and threads it (`asks_before_setup`) through probe,
sign-in replay and settle, replacing the live `setup::asks_first(cx)` read at settle. The commit message's goal is
one-directional: "a later preference change cannot skip the confirmation already required for that attempt". But the
frozen value is used in both directions: a press with asks=false (skip on) followed by the user turning the switch off
(asks=true) before the plan arrives still takes the `!asks_before_setup` branch at `add.rs:1313` and submits setup with
no confirmation; the Re-run path does the same through `setup.rs:799-802`. SPEC.md, Settings Hosts section (around lines
2644-2646): "Turning one off restores that confirmation; ... Changes take effect immediately in the current client." The
probe window is not instantaneous: it can wait on SSH sign-in (the replay at `add.rs:1124` keeps the stale value), so
the user has a real opportunity to change the shared preference (here or from another client) and expect it to apply.

Verify by reading `settled` in `add.rs` (the `Outcome` provisionable arm) and the SSH arm of `rerun` in `setup.rs`:
neither consults `asks_first(cx)` any more. Fix: ask if either the press-time or the live choice asks, that is
`asks_before_setup || setup::asks_first(cx)` at settle (and in the Re-run SSH arm), which keeps the commit's intent (a
later skip cannot drop a required question) while honoring a later restore. Add a test for the reverse direction (press
with skip on, turn it off during the probe, expect the confirmation).

Bucket: other. Farhelm installs itself on a host the user explicitly pressed Add or Re-run for, under a skip preference
that was on at the press; the user's later withdrawal of blanket consent is ignored for that one attempt. No data loss
or security boundary is crossed beyond what the user had authorized at press time. Not highest: the install targets the
destination the user typed and pressed Add for, while their own skip preference was on; nothing beyond that press is
acted on.

Confidence: confirmed from source.

Possible cover: none known. SPEC.md's Settings Hosts paragraph ("Turning one off restores that confirmation") is the
contract this departs from; SPEC_impl.md, "GUI: Dioxus" (answers bind to what was drawn), motivates press-time binding
but concerns what a press acts on, not ignoring a later restored confirmation.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g22 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
