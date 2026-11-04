# Byte-stream stall test does not establish its blocked-send premise

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The byte-terminal regression can pass without testing teardown during a blocked browser send.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed missing fixture
witness, not an observed false pass on a supported system and not a production bug.

Source locations: 3097bab8d322564f38f3f075103ebd184d106fe3:crates/farhelm-helm/src/terminal.rs:1481-1495,1549-1573; tip
ca9b0a19856dc136f63ad2d14f1076027edce1eb:1610-1624,1678-1702.

the helper sends 200 64-KiB frames before injecting Detached, but observes neither a pending send nor a bound on the
server's buffering. Its own documentation explicitly acknowledges the unobserved premise. The grid branch instead waits
for PauseOutput. At tip rest_harness.rs:877-888 creates a plain listener without controlling the server send buffer,
while connect_with_receive_buffer at 1676-1702 only requests the client's receive buffer; it supplies no blocked-send
oracle. The two-Detach oracle corrects the older overflow/completion ambiguity but does not establish this earlier
premise.

Possible coverage and caveats: tip TRIAGE_OUTCOMES.md:11580-11595 gives the broad Fix, skip hard ones policy;
11907-11950 specifically discards old queue-overflow/independent-cleanup false passes because distinguishing handler
completion then needed a production seam. Neither section explicitly accepts this distinct blocked-send premise. The
broad policy could still apply if its repair requires a new test-only production hook; static inspection of the current
harness establishes absence of a witness, not necessity of such a hook, so coverage remains ambiguous rather than
treated as blanket exclusion. TODO Planned remains empty (tip TODO.md:21-26); focused searches of specifications, BUGS,
plans/queue, queue and ledger found no exact duplicate acceptance.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g038/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 3097bab8d322564f38f3f075103ebd184d106fe3;
persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence and frequency
were not measured.
