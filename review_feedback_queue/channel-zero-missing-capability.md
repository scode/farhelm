# Channel-zero assertion also matches missing grid support

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The reserved-channel regression assertion can pass for a separate missing-capability refusal, even with a valid real
session.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/session_lifecycle.rs:1272`;
`crates/farhelm/tests/e2e/session_lifecycle.rs:1257-1283`. References below use these paths' basenames unless another
path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Later-turn synthesis verified the fixture premise: the assigned test calls harness then basic_session (1234-1244). The
harness constructs a real supervisor with default seams (harness.rs:2287-2288, 2647-2669), whose new_terminals is Host
(service/core.rs:1339-1363); basic_session creates the actual fake-agent session and waits for post-exec readiness
(harness.rs:1269-1284, 1796-1810). The test later successfully attaches this exact same session on channel 7 with
grid:true and explicitly asserts StreamKind::Grid (session_lifecycle.rs:1285-1308), independently confirming that its
terminal is a host, not an unknown id or mock.

With this fixture and a fresh raw connection, no relay/download reservation precheck applies (connection.rs:585-675).
The first request uses channel:0, grid:false (1257-1273). channels.rs:98-104 currently rejects channel zero;
handlers.rs:1422-1434 returns InvalidRequest with the reserved-channel wording before looking up the session. If the
zero check alone is removed from can_begin_stream, the otherwise valid, ready host session resolves at
handlers.rs:1471-1498 and Terminal::Host with grid:false returns InvalidRequest with ATTACH_NEEDS_GRID_CLIENT at
1530-1538, before any takeover, route installation, or stream exists. This competing refusal is expressly required by
SPEC_impl.md Terminal host. The test only matches req_id:1 and InvalidRequest (1276-1283), so both outcomes satisfy it.
The later valid channel-7 attach remains possible because the missing-grid refusal mutated no attachment.

Set grid:true for the channel-zero attack and assert its channel-specific diagnostic; a separate valid
nonzero/grid:false request can independently cover missing capability. This is a source-verified competing-refusal
analysis, not a runtime mutation experiment. It establishes a test gap, not shipped acceptance of channel zero.
Introducing and pinned-tip test/handler behavior are unchanged. Suggested bucket other; confidence confirmed;
proposed_drop false; possible_cover none.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g044/f5`,
`g041/g041-s4`, `g046/f5`. The retained append-only histories preserve all earlier claims and corrections. Reviewed
source references name their own revisions; the inclusive reviewed tip is the hash above. This review used source
inspection only, with no product runtime experiments or live-install changes.
