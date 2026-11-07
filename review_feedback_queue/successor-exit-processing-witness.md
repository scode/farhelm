# Wait for the replacement runtime's retained exit

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The retained-exit-code regression test relies on recovery replay finishing in the background before it checks the code.
A slow replay can make a correct restart fail this test; the failing schedule is unverified after investigation.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/session_lifecycle.rs:4534-4555`;
`crates/farhelm/tests/e2e/restart_under_concurrency.rs:484-489`;
`crates/farhelm/tests/e2e/session_lifecycle.rs:4542-4555`. References below use these paths' basenames unless another
path or revision is stated.

Recorder disposition: **unverified-after-investigation**. Triage priority: **other**. Source-confirmed conditional
behavior does not imply an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below
remain part of the finding.

At 204 the test waits for the original runtime to process exit 3, then drops that runtime through restart_supervisor
(207-213). At 224 it reads agent_exit_code from the new runtime and immediately requires Some(3) (225-229). The file is
unchanged at pinned tip 407a49435278844248d573083fdea6e23fd96c02. harness.rs:1439-1441 returns host.exit_code with no
wait, and 1252-1259 connects the replacement client using a handshake, not a replay fence. At tip
hosts/terminal.rs:1781-1785 initializes a reaped child as Running; 1916-1926 handles only the first queued journal
record before spawning the remaining replay. Its adopt contract at 1738-1739 explicitly does not await the rest. The
same introduced contract is at f42178f:hosts/terminal.rs:1736-1737; tip diff confirms the adoption behavior was
unchanged. service/core/host_handover.rs:636-640 confirms constructor adoptions return without catch-up. Constructor
capture_now (core.rs:6684-6686) applies hook reports (capture.rs:94-117), not terminal-replay readiness. SPEC_impl.md,
Terminal host / Across supervisor restarts, tip 2520-2524 (introduced 2509-2512), explicitly allows the exited child to
read running until its exit record replays. Await wait_for_dead_agent on the replacement supervisor before reading
retained; this preserves the no-listing boundary.

Checks performed: read the test at both pins, replacement/client helpers, host adoption and exit-code access,
constructor and capture pass, and the governing specification. No barrier guaranteeing the new exit record was processed
was found. The missing premise is an observed scheduler/journal-delivery interleaving that leaves replay unfinished
after constructor I/O, the client handshake and the getter's store query. Those operations usually provide replay time;
source establishes no explicit ordering guarantee. Runtime experiments are forbidden by the assigned brief, so a
controlled delayed-replay reproduction would establish how readily this test fails. Suggested bucket: other (maintainer
test reliability only). Confidence: likely, with the reachable failure schedule unverified after investigation; the
missing synchronization itself is confirmed. proposed_drop: false. possible_cover: none.

Later-turn synthesis; reviewed commit f42178f9402ce22c60530360edc10d6721ef5b16, unchanged test at pinned tip
407a49435278844248d573083fdea6e23fd96c02. This refines, rather than adds to, l2. TLDR: the test does not explicitly wait
for the replacement runtime to recover the child's retained exit. It can reject the expected code if that read occurs
before the replacement processes the exit; no such timing was observed or measured.

Guaranteed conditional ordering from source: a Child::Reaped adoption initializes the replacement status as
Liveness::Running { child: None } (tip hosts/terminal.rs:1781-1785). Adoption processes at most its first queued record
inline, then spawns the remaining runtime (1916-1926), and explicitly does not wait for the whole journal (1738-1739).
HostStatus::exit_code returns None in Running and the retained code only in Exited (865-872); HostTerminal::exit_code
reads that status directly (2527-2529). Therefore IF the replacement's exit record is not processed when
harness::agent_exit_code returns the status at 1439-1441, retained is None and boot_id_durable_outcome.rs:225-229 fails
its Some(3) requirement. That implication is definite. The original-runtime wait at :204 is not a successor-runtime
fence; restart_supervisor drops that runtime and reconstructs another (harness.rs:1181-1197,1213-1237). The client
handshake (1252-1259), constructor I/O, and the getter's asynchronous row lookup can give replay time, but none is the
named successor exit oracle.

Not established: that the finite journal and actual executor schedule of this particular fixture leave the replacement
behind after all that intervening I/O. A runtime that processes Exit inline as its first record or catches up before the
getter satisfies the test. Do not say adoption must return before Exit, that this test has been observed flaky, that it
always fails, or that a delayed replay was reproduced. The failing schedule remains unverified after investigation.
Checks here reread adoption initialization, first-record handling, exit-code access, replacement/client helpers, and the
existing exit wait; earlier checks covered constructor capture and SPEC_impl.md Terminal host / Across supervisor
restarts. No runtime tests are permitted by the brief. A controlled delayed-successor-replay test would resolve
reachability; the available source establishes a missing explicit barrier and the conditional consequence, not timing
frequency.

Fix direction: wait_for_dead_agent(&h.sup, h.state.path(), &exiting.id) on the replacement before reading retained. That
waits for its Exited status (harness.rs:1493-1510) without listing and preserves the intended no-durable-observation
boundary. Suggested bucket: other (test reliability only). Confidence: likely consequence with the exact missing
schedule above; confirmed missing explicit successor exit barrier. proposed_drop: false. possible_cover: none. No higher
consequence or independent new issue is claimed.

Interface synthesis in second discovery turn, not a new independent finding. Reviewed tip
407a49435278844248d573083fdea6e23fd96c02. Read latest g045/g045-d2 and own pinned source at
session_lifecycle.rs:4534-4555: restart_supervisor rebuilds, then the first successor listing immediately asserts
Exited. This is additional caller evidence for existing g040/l2 and g043/f6, not a new mechanism. Preserve l2's
confidence exactly: no explicit successor processed-Exit barrier is source-confirmed; the fixture's actual failure
schedule after constructor/client/list I/O remains unobserved and unverified after investigation. Do not infer a
measured flake from the extra caller. Peer g045/g045-d1 instead concerns a survival assertion accepting remembered PID
and old text; its distinct opposite false-pass consequence and missing attach-before-Exit premise are not refuted or
converted into l2's false-failure claim here. Suggested bucket other; no higher consequence. Confidence unchanged,
source fact versus conditional scheduling as in prior l2. proposed_drop false. possible_cover g045/g045-d2 and g043/f6,
same boundary; merge additional caller evidence. No new verified or investigated-unverified root from this synthesis.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g040/l2`, `g043/f6`,
`g045/g045-d2`, `g044/g045-d2`. The retained append-only histories preserve all earlier claims and corrections. Reviewed
source references name their own revisions; the inclusive reviewed tip is the hash above. This review used source
inspection only, with no product runtime experiments or live-install changes.
