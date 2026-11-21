# First-turn drift checks do not bind the response line to the submitted prompt

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The first-turn check can credit text that was already visible before its prompt was submitted.

## Details

The contract at docs/harness-drift-tests.md:17–21 in pinned head 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e requires a
real response to the submitted prompt. crates/farhelm/tests/e2e/real_agent_capture/drift.rs:650–659 waits for the prompt
echo, sends Enter, and begins observing without a pre-submit output baseline. :514–550 records any whole OK line in the
visible grid, including on its first poll, and accepts it with a conversation capture, Resume offer, and hook
acknowledgement. Those companion checks need not be evidence of a completed turn: the existing Claude audit documents a
startup report without typing in crates/farhelm/tests/e2e/real_agent_capture.rs:169–180 and :489–499, and its observer
checks both Resume and acknowledgement at :262–282. Bind the response witness to this submission, for example by
requiring a newly emitted response containing a per-turn marker checked outside the echoed prompt. Source inspection
establishes the missing freshness check. The test starts a private terminal and submits no earlier test turn, and no
pinned capture shows a matching pre-submit OK that stays on screen once the prompt is sent. That frame, attributable to
a tracked launch with usable hook evidence, is the missing premise for an observed false pass.

Confidence: confirmed by inspection of the test contract and passing predicate. Vendor false-pass reachability remains
unverified.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `driftRunner/l15`.
