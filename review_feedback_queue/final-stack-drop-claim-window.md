# Host restart-gap fixture can expose an inherited claim-drop race

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A very narrow setup race can make the converted restart-gap tab test fail even when tab opening behaves correctly.
Ordinary overlap between the test's supervisors is not this race: both share the same process's live ownership claim.
The possible consequence is a maintainer-test false failure only.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/tab_lifecycle_edges.rs:105-119`. References
below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **unverified-after-investigation**. Triage priority: **other**. Source-confirmed conditional
behavior does not imply an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below
remain part of the finding.

Correction of earlier wording, confirmed by g040's interface evidence and direct source inspection: core.rs:1490-1501
says ownership is per process/state-directory, and 1567-1575 upgrades and reuses a live Arc claim. If the predecessor's
connection still owns a live Supervisor/claim, replacement construction ordinarily reuses it and is not read-only. No
general concurrent-owner failure is alleged.

The separate window is the final Arc decrement: ownership's strong count is already zero while StateDirOwnership::drop
removes its weak registry entry and before its _lock field has closed (core.rs:1507-1537). A concurrent claim sees no
upgradable Arc and its try_lock can return WouldBlock (1560-1585). The source explicitly accepts this safe read-only
construction window. The test at tab_lifecycle_edges.rs:105-110 drops client and supervisor, ends terminals, then
constructs the successor without the retire_supervisor reference/lock barrier. end_all_terminals confirms terminal
termination, not predecessor claim release. Its default state_dir_claim_wait is zero. If the narrow claim-drop window
occurs, host_handover.rs:706-717 deliberately gives even an ended host row an unavailable handle without inspecting its
socket. core.rs:15387-15398 then answers host-unreadable Conflict; the test's 117-119 instead require restart advice.
This is a conditional source-level path, not a measured failure.

Exact pre-base provenance: exclusive base 8a8c9579e04299410652f0e4681975a7198dfc7b already has the same
Drop/weak-registry/try_lock implementation and accepted final-drop wording at core.rs:1400-1411,1434-1447,1470-1495. It
already has the unawaited drop(client); drop(sup); kill_tmux_server_and_wait; immediate reconstruct sequence at
tab_lifecycle_edges.rs:103-108. But that base test creates tmux terminals. Base core.rs:6996-7023 maps its absent pane
to no terminal irrespective of reconciliation writes; 7423 builds the entry with that result, and 15247-15248 returns
restart advice. Thus a claimless old fixture can still pass its assertion. F's switch to host terminals introduces the
unavailable-placeholder refusal as the observable failure while inheriting the unsynchronized teardown and ownership
window.

Suggested bucket: other. Confidence: likely; unverified after investigation for the exact scheduling premise that
successor claim runs after the old claim's final strong decrement but before its file-lock close, despite the
intervening terminal shutdown. No runtime trace or frequency is available; experiments were prohibited. A deterministic
final-drop gate or retained failure with owns_state_dir=false would settle that premise. Use retire_supervisor before
constructing the successor, or establish its ownership before asserting the tab refusal.

proposed_drop: false after synthesis. Prior proposed-drop basis was 'pre-existing rather than introduced' from the
inherited drop/drop/reconstruct sequence; its stronger pre-base citations are preserved above. That basis does establish
inherited mechanism, but does not refute the host conversion's newly observable failed assertion, so it is not by itself
a complete drop justification. possible_cover: none.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g045/g045-l4`. The
retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their own
revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
