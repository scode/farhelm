# Replacement history absence lacks a completed-delivery witness

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The replacement regression checks that old history is absent before proving history delivery has finished. The boundary
gap is confirmed, but a concrete false-pass counterpart for this small fixture remains unverified after investigation.

## Details

Missing premise: a concrete falsifying history-retention path for this small fixture, including the marker in old
scrollback and delivery/application ordering that lets the assertion pass. The fixture prints only startup lines, a
prompt and one marker echo (`fake_agent.rs:1659–1767`); the test neither requests its explicit spam output nor
establishes that the marker entered scrollback (`replace.spec.ts:580–598`). The source confirms the missing delivery
witness, but not an effective false pass from an existing broken producer.

Further static retrieval would repeat these facts. A controlled regression, retained executor/browser trace, or a
documented concrete competing implementation would settle the consequence; runtime and mutation experiments were outside
this review's charter. This is an investigated test-oracle uncertainty, not an observed flake or production history
leak.

Suggested bucket: **other**. Confidence: **unverified after investigation**. unverified after investigation

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:e2e/tests/replace.spec.ts:668-683.

the grid replacement test may miss wrongly retained old scrollback when absence is checked before history
delivery/application completes. Confirmed facts: replace.spec.ts:668-683 waits for a new component/open socket and READY
text, then termText immediately checks absence. helpers/term.ts:234-299,358-390 only reads applied model history/screen
and performs no replay/history drain wait. The test calls attachSession only for the source (593), not the replacement.
Source attachSession:404-422 does wait for ready, but that earlier source boundary cannot fence a later replacement.
terminal-readiness.ts:363-379 defines ready as matching open socket, revealed replay and focus, without historyPending,
so even borrowing that helper alone would not prove drained grid history. No hidden readiness helper was found around
the replacement assertion. Prior inspected grid_stream.rs:420-496 sends screen before history; terminal.js:4593-4597
deliberately reveals before all history is applied; grid-terminal.js:1799-1835,1890-1911 drains a first slice and
schedules the remainder. Read-only git diff from 9fcfe774c11b4bb2c57137f859f0e5545dbdc1a4 to the tip is empty for those
stream/client implementations and this test, establishing persistence. Additional fixture investigation: tip
fake_agent.rs:1659-1767 prints two startup lines and a prompt, then one echo for this marker; only explicit spam adds a
history-producing burst. replace.spec.ts:580-598 neither sends spam nor proves the marker moved into scrollback. Thus a
concrete false-pass counterpart still requires wrong history containing the marker plus a delivery/application ordering
that this small fixture has not established.

Possible coverage and caveats: none exact. SPEC_impl.md:1708-1736 accepts sliced rendering but does not accept false
validation. Ledger replacement-tests-overstate-immediate-list-no-delete-evidence.md:11964-11974 discards a different
Rust cache oracle needing a production seam. SPEC/SPEC_impl, TODO Planned (empty), plans/queue, BUGS, INDEX and ledger
searches found no exact replacement-grid-history-test coverage.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g193/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
assigned adaptation commit 9fcfe774c11b4bb2c57137f859f0e5545dbdc1a4 has identical relevant source.. This was a
source-only review; runtime occurrence and frequency were not measured.
