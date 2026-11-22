# The × dismissal follows the session ID into a new launch

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

Dismissing a hook hint for one run can hide the hint on a later run of the same session.

## Details

**other.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b
crates/gui/farhelm-gui/src/session_pane/hook_band.rs:25-42,59-75,131-137,163-177

Clicking × for one uncaptured run can silence the hint for a later uncaptured run of the same session. This is an
occurrence identity defect in a local action. There is no request or delivery outcome. The click stores (session key,
agent); follow retains the entry while that row stays eligible and also when the row or GUI store is absent; draw hides
any matching pair. At bb31959c2f22c96bbbb72da2e60eee4a0b288a6b,
crates/farhelm-supervisor/src/service/core.rs:12796-12835 holds the old row and status during relaunch, then
:13216-13235,13259-13268,13278-13334 publishes the new generation under the same ID. The new run can remain uncaptured
with setup off. A named Restart with is available on this path even when plain Restart is unavailable (pinned
SPEC.md:2104-2123. SPEC_impl.md:8632-8637). The hint predicate also admits the initial Unknown status
(crates/farhelm-proto/src/lib.rs:927-936), so the application need never observe an ineligible row between runs. Pinned
SPEC.md:3098-3101 limits × to this occurrence and SPEC_impl.md:4525-4528 keys the explanation to the launch result. A
GUI reconnect also retains the same window-owned band (main_view.rs:99-101,142; session_pane.rs:377-380,392-394), a
secondary lifetime mismatch under SPEC.md:5500-5513. Identify the run when storing the local dismissal and scope
ephemeral state to the GUI session. The reviewer suggested other. Confidence confirmed by source with the stated
condition that the new run still has no accepted report. Hook-contract independently identified the later-run case, as
reported by the commissioner.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-session/HS-01`, with
`hook-supervisor-checks/HSC-08`, `hook-contract/hook-hint-occurrence`.
