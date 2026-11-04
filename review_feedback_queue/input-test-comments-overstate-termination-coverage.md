# Input-test comments overstate reader-termination and co-arrival coverage

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The test comments claim stronger coverage and transport guarantees than the tests establish.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed.

Source locations: 3097bab8d322564f38f3f075103ebd184d106fe3:crates/farhelm-helm/src/rest_harness.rs:1780-1800;
crates/farhelm-helm/src/client.rs:6781-6784; crates/farhelm-helm/src/terminal.rs:2191-2193.

the assigned send_frames helper concatenates two complete messages and calls write_all, ignoring errors
(rest_harness.rs:1787-1800). The caller says this makes the following message already present when the first is read
(terminal.rs:2191-2196), then observes detach/close and absence of forwarded data (2198-2241). However send_grid_input
removes the attachment on the oversized message and refuses all subsequent messages for its channel
(client.rs:4135-4150); that refusal also occurs if the browser-reader loop continues instead of breaking at
terminal.rs:808-813. The companion direct test separately proves this client refusal with an actual later call and
positive Resize witness (client.rs:6817-6848). Thus the WebSocket test's observable cannot distinguish reader
termination from continued reads refused by the client. Additionally the pinned Tokio 1.53.2 WriteAll future loops over
poll_write and may suspend or fail after a prefix (dependency src/io/util/write_all.rs:40-50), so its one write_all call
does not establish atomic receiver co-arrival.

The test comments claim stronger coverage and transport guarantees than the tests establish. The specified
input-rejection behavior is covered and production behaves correctly; this finding is narrowed to misleading maintainer
documentation, not a demand for extra lifecycle coverage. Reconciliation: g038's actual turn2-synthesis.reply and
current synthesis progress correctly distinguish the required no-forwarding/detach/close behavior from atomic receiver
co-arrival. SPEC_impl.md at tip:3015-3026 requires ending the attachment on oversized input, not proving which reader
task completes first. The WebSocket test's own specification at reviewed terminal.rs:2162-2174 (tip:2291-2303) likewise
requires detach/close/no forwarded data, which its observations cover. The stronger assertion is in the companion client
test docstring: reviewed client.rs:6781-6784 (tip:6783-6786) says the WebSocket test shows the handler stops reading. In
fact the client removes the channel and rejects subsequent binary messages independently (client.rs:4135-4150), and
outbound completion aborts and joins inbound (reviewed terminal.rs:865-901), so the same external observations can hold
without the immediate reader break at 808-813. The assigned helper docstring at reviewed rest_harness.rs:1780-1786 and
its caller comment at terminal.rs:2191-2193 also imply one write makes the following message already present when the
first is read. Pinned Tokio 1.53.2 src/io/util/write_all.rs:40-50 loops over poll_write and can suspend or fail after
prefix progress; the helper ignores the error. These comments should describe adjacent-frame construction and actual
rejection/closure observations, without promising deterministic co-arrival or a reader-termination oracle.

Possible coverage and caveats: tip TRIAGE_OUTCOMES.md:11580-11595 conditionally discards test repairs requiring a new
production-only test hook. It does not require an exclusion here: the needed correction is the claims in test/helper
comments, which is a contained documentation change with no hook. No exact accepted claim or matching
queue/Planned/plan/BUGS/ledger item was found in the prior focused searches. FILTER.md's imprecise diagnostic filter
concerns error/log diagnostics rather than these code-documentation guarantees.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g036/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 3097bab8d322564f38f3f075103ebd184d106fe3;
persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence and frequency
were not measured.
