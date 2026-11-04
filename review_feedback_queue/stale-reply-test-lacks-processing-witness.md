# Dropped-reply regression can accept latest-in-batch behavior without proving stale-reply rejection

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The regression for cancelling a terminal-host Info request does not prove that a late reply cannot satisfy the next
request.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
static test-observability gap; no missing implementation premise for this limited claim.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/client/tests.rs:705-710.

at tip, tests.rs:693-698 establishes a cancelled sent request. The stand-in waits for the second request (:706), then
sends InfoReply1 and InfoReply2 immediately back to back (:707-708); only final result2 is asserted (:710). The exact
client processes every complete buffered frame before checking the request-completion predicate
(client.rs:1021-1030,1078-1099). InfoReply handling updates one latest slot (:1130 and :533-539). Consequently the
competing mechanism 'any latest reply is enough', with no request-number comparison in Replies::has (:542-543), gives
the same result2 whenever the two replies arrive in the same read batch. That mechanism would incorrectly return reply1
if only the stale reply were available. This test neither isolates that boundary nor asserts that the second future is
still pending after reply1 is processed. The current production comparison does check the correct request number
(:542-543), so no current product defect is asserted. Tokio1.53.2 io/util/write_all.rs:40-53 completes immediately when
poll_write is ready; the two tiny sends have no obligatory yield or handoff, and the test imposes no split-read barrier.
Batch arrival is a permitted input shape, not an observed or guaranteed kernel schedule; no runtime mutation survival or
flake frequency is claimed. Fix direction: send only the stale reply, witness its processing and the second request's
continued pending state, and only then release the fresh reply; a controlled buffered decoder test could make the
distinction deterministic.

tests.rs:693-698 confirms the first Info was sent and cancelled, but :705-708 reads the second request and then
immediately sends replies 1 and 2 back to back; :710 checks only final host_pid2. The client decodes buffered messages
as a batch, so a mechanism that simply overwrites a latest-reply slot can produce2 when both replies arrive in one
batch, even without preserving request identity after cancellation. The test does not establish that the second request
remains pending after consuming only reply1.

The regression for cancelling a terminal-host Info request does not prove that a late reply cannot satisfy the next
request. This is a test-oracle defect, not a current product stale-reply bug.

Possible coverage and caveats: none found. Required candidate-focused searches of SPEC.md,SPEC_impl.md,TODO.md
Planned,unlanded plans/queue,BUGS.md,queue INDEX/files and TRIAGE_OUTCOMES.md found no matching
trigger/consequence/scope. Pinned TODO Planned is empty at23-24. SPEC_impl.md Connecting1782-1798 and Testing6742-6809
impose/use the behavior rather than accepting a weak stale-reply oracle. TRIAGE_OUTCOMES.md9818-9835,
later-refresh-hide-stale-refresh-regression.md, was read in full and concerns fleet-refresh sequencing in
farhelm-helm/src/sessions_tests.rs, not the host client's per-kind cancelled Info replies. Similar keywords are not
coverage. The full previously read queue contains no matching cancelled-Info test; rare-diagnostic and safe-failure
filters do not cover an ordinary regression that cannot distinguish the stated behavior.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g107/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
originating assigned patch 1eef3d64e36cd31b56e5b7e815fa3cc183b55637.. This was a source-only review; runtime occurrence
and frequency were not measured.
