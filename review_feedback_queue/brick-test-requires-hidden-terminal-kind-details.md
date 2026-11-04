# Tmux-only candidates cannot satisfy the required terminal-kind UI observation

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A healthy candidate that creates only tmux terminals cannot satisfy the brick test because the test requires a
terminal-kind detail that the product intentionally hides.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static contract
conflict, no runtime reproduction.

Source locations: 55f1e79f56ac6cf7b536badfc7c2ae59e9d74ca6:releasing/mac-vm-test/RECIPE.md:108-109.

new instructions unconditionally require reading host or tmux from session details in RECIPE.md and BRICK-TEST.md, also
emitted by guest_workflow.py:443-449,454-460,188-190. Yet migration.rs:217-218 intentionally omits that detail when the
supervisor creates no terminal hosts and all terminals are tmux. The same contract explicitly allows a candidate that
creates none to pass without Restart.

RECIPE.md:108-109 and :130 require reading the new local/remote terminal kind in session details; BRICK-TEST.md:396-398
and :414-415 make that UI comparison mandatory even though :419-420 explicitly permits candidates creating no terminal
hosts. guest_workflow.py:443-449,454-460 emits the same unconditional requirements to the native tester, and :188-190
also requires the old-session kind to match. In the same commit, migration.rs:217-218 returns None for all-tmux sessions
whose supervisor creates no hosts; SPEC.md Terminal experience :2221-2236 explicitly says such a host shows neither band
nor kinds, and SPEC_impl.md Terminal kinds on replies :2483-2502 confirms it. A concrete historical source is stable
v0.33.0 commit 9ce731850f060a394077a99b5839564926da982c: crates/farhelm-ui/src/session_view.rs:2947-3026 has no
terminal-kind detail and ls-tree contains neither the terminal-host crate nor migration.rs. This is a supported planner
branch, not a downgrade request: brick_test.py:145-168 permits candidate == latest and chooses an earlier stable as the
P0 baseline. No claim is made about the live current latest endpoint.

Possible coverage and caveats: none found. Coverage checks at tip ca9b0a19856dc136f63ad2d14f1076027edce1eb searched
SPEC.md, SPEC_impl.md, TODO.md Planned (empty :23-25), BUGS.md (all three sections read), plans/queue, queue INDEX and
TRIAGE_OUTCOMES.md for terminal-kind visibility, tmux-only candidates and brick-test contracts. Product acceptance of
hiding kinds establishes the conflict rather than accepting a broken test. plans/queue/new-terminal-button.md Acceptance
criteria :35-51 changes only the plus-button wording, not terminal-kind requirements. TRIAGE_OUTCOMES.md
retained-failed-tags-become-mandatory-upgrade-test-inputs.md :12224-12233 concerns unpublished tag inventory and does
not match this ordinary published-candidate UI requirement. No filter fully matches a persistent impossible acceptance
requirement; the older-upgrade filter concerns product session edge cases, not this harness contract. Persistence:
assigned instruction files and guest_workflow.py are unchanged through pinned tip; tip migration.rs:247-248 and
SPEC.md:2235-2238 retain intentional omission.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g199/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 55f1e79f56ac6cf7b536badfc7c2ae59e9d74ca6.. This
was a source-only review; runtime occurrence and frequency were not measured.
