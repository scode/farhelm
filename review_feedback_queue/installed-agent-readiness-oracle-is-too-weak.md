# Launch-only drift checks do not require evidence of an interactive screen

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Launch-only checks can pass on identifying text without establishing that the agent is interactive.

## Details

The contract is docs/harness-drift-tests.md:17–21 and :34–38 at pinned head 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e.
crates/farhelm/tests/e2e/real_agent_capture/drift.rs:156–175 includes a product name as a standalone permissible marker
for each launch-only profile. Terminal::ready at :450–469 accepts any single marker or any trust phrase from :304–315.
It verifies terminal-child liveness and refuses certain error messages at :420–445, with no positive requirement for a
composer or the choices of a current trust menu. The launch-only path returns that readiness as success at :650–655. Use
a profile-specific rendered composer cue or an anchored current trust menu, retaining the no-submission behavior. The
code proves that a banner or phrase can satisfy the test. The pinned captures do not establish an actual false pass for
an installed Cursor or OpenCode version. Evidence to establish one would be an attributable startup frame containing a
permissive marker while the child is alive, before either promised screen appears.

Confidence: confirmed by inspection of the test contract and passing predicate. Vendor false-pass reachability remains
unverified.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `driftRunner/l14`.
