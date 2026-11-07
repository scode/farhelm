# Reboot tests release their concurrency slot before successor retirement

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The reboot tests do not keep their integration permit through successor retirement and terminal cleanup. Under the
explicit in-process/libtest use of that permit, another fixture can be admitted during the preceding fixture's teardown.
No permanent process leakage is established.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/restart_with_resume.rs:1781-1787`;
`crates/farhelm/tests/e2e/session_rename.rs:355-362`. References below use these paths' basenames unless another path or
revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The assigned test blobs are unchanged at pinned tip 407a49435278844248d573083fdea6e23fd96c02.
restart_with_resume.rs:1630-1631 declares state before slot, and :1785-1787 explicitly drops client, sup, then slot
while state is still live. The helper at :1301-1302 has the same declaration order and explicitly drops slot at :1582
while successor client/sup remain in scope. Its structured early return at :1546 drops later-declared successor
client/sup before slot, but state remains earlier-declared and therefore drops after slot.
crates/farhelm-teststate/src/lib.rs:741-764 ends hosts only during TestDir drop; the agent and terminal host do not end
merely because sup drops (SPEC.md Durability and resume:2843-2847). Terminal cleanup therefore starts after permit
release on these paths. This is eventual TestDir cleanup, with retention for later sweep on failure, not evidence of
permanent host leakage.

Additionally drop(client) only requests asynchronous transport shutdown: client.rs:1372-1390 sends shutdown, while
harness.rs:1252-1257 gives the spawned handle_connection task its own Arc<Supervisor>. Dropping the local sup handle
does not join that task. The existing retire_supervisor helper at harness.rs:1176-1197 instead waits for the strong
references to drain and for state-directory claim release. Thus a connection-owned supervisor/runtime can also outlive
the permit; the finding does not need a late host spawn or failed TestDir cleanup to hold. The parent in-process
SupervisorSeams default is Tmux (f42178f^ service/core.rs:1269-1288), so the real-process fixture's pre-existing host
default cited by g041/g041-l2 is a different constructor.

Consequence narrowed during synthesis: harness.rs:747-754 explicitly says the maintained runner starts a process per
test and SLOTS only caps harnesses inside one process, including explicit libtest diagnosis. This permit cannot
coordinate nextest tests, so no increased concurrency between maintained-runner tests is claimed. The established issue
is a violated in-process permit lifetime, conditional on that supported use. Keep the permit until successor connection
tasks are drained and terminal hosts are confirmed ended; do not equate eventual state-directory destruction with
cleanup before permit release.

Suggested bucket: other, limited test resource-bound correctness. Confidence: confirmed declaration/drop and async
ownership ordering; no observed timing/frequency claim. proposed_drop: false. possible_cover: none. Synthesis revision
preserves and narrows the prior consequence, adds connection-task ownership, and explicitly excludes permanent leakage
or nextest-wide admission effects.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g043/f1`,
`g045/g045-f2`. The retained append-only histories preserve all earlier claims and corrections. Reviewed source
references name their own revisions; the inclusive reviewed tip is the hash above. This review used source inspection
only, with no product runtime experiments or live-install changes.
