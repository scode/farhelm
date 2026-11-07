# The paused-terminal test overstates its stall-timeout coverage

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A migrated test now claims to cross the new terminal's stall timeout, but observes only eight of its configured sixty
seconds. Its inherited flood witness also does not establish output pressure. This is a new coverage-description error,
with no observed product stall.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/terminal_tabs.rs:2003-2078`. References below
use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Historical source F^ terminal_tabs.rs:2017-2049 already pauses first, sends the same FLOOD-%s command, performs eight
seconds of agent round trips and waits only for FLOOD-. It has no independent producer witness, and FLOOD- is literal
input echo there too. Therefore the missing producer premise and echoed recovery marker are inherited; do not attribute
them to the changed send_keys helper or new host backend. The first pass's marker-collision observation survives, but it
is not evidence that migration newly broke flood execution.

What changes in F: docs at tip 2006-2011 replace the old tmux pause-after explanation (F^:1957-1988) with a claim that
the host test runs longer than the stall window. The loop remains eight seconds (2058), while the host attachment's
stall_detach remains sixty (service/core.rs:207,460; grid_stream.rs:98-103, initial pass). This is a new incorrect claim
about coverage; before-expiry pause/resume isolation can be a legitimate test, but it does not cover expiry and cannot
both wait beyond expiry and expect the same attachment to resume successfully. Narrow the docs or add a separate
seam-controlled expiry case.

The observer is newly different: output waits now append grid screen/history row text rather than raw tmux bytes
(harness.rs:142-200,2694-2768). That permits repeated old rows, but this test uses unique agent-alive-N markers and
begins with no earlier flood, so no additional repeated-marker false pass is established beyond the inherited input-echo
collision. Peer g042-l3 concerns identical callback markers and should not be generalized to these unique round-trip
names. Active tab input uses ordinary send_keys after wait_for_shell; peer g044/f2's established stale-channel filter
route is not shown here.

Suggested bucket other; confidence confirmed source-level inherited oracle gap and new documentation mismatch, no
runtime or mutation experiment. proposed_drop false; no severity increase. possible_cover inherited earlier
implementation; commissioner decides range/queue handling. No product slowness, actual stalled viewer, missing flood
execution, or runtime false pass was observed. This revision narrows causality, preserving the initial observation
rather than erasing it.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g046/g046-f1`. The
retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their own
revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
