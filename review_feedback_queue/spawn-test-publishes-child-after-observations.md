# Spawn test publishes its cleanup ID after observations

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A spawned test child can escape teardown registration when an intervening assertion fails.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The child exists before observation assertions, but its cleanup ID is registered afterward. The source-established gap
concerns test ownership, separately from the policy-restoration failure in the same harness.

Source locations: `e2e/tests/spawn.spec.ts:145-149,158-163`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Final discovery record, correcting test line numbers after numbered source inspection. Reviewed commit
d6645f771bee8ed03a66486ef8755834937d759d. TLDR: an observation failure after successful spawn can leave an owned child
session on the private stack, affecting later tests. parented remains undefined until childByTitle at spawn.spec.ts:149.
The test first successfully waits for SPAWNED-PARENTED at 145, then awaits /api/hosts, parses JSON and asserts
commands_without_asking (146-148). A failure there leaves a created child with no recorded cleanup ID. The fake agent
prints the marker only after the real spawn CLI succeeds (crates/farhelm-fixtures/src/fake_agent.rs:440-446). Finally
cleans only recorded child IDs and parent, then removes the working-directory root (spawn.spec.ts:158-163). No afterEach
sweep exists in spawn or the passive evidence fixture (helpers/evidence.ts:29-64). Parent cleanup sends only that
parent's stop/delete (helpers/fleet.ts:559-570); helm forwarding is keyed to that session
(crates/farhelm-helm/src/sessions.rs:3084-3104; client.rs:3427-3451), with supervisor dispatch preserving its ID
(service/handlers.rs:2848-2866). The private stack is reused across projects (e2e/playwright.config.ts:98-108), while
outer-run shutdown cleans its private tmux servers (e2e/start-stack.sh:262-300). No indefinite machine-wide leak or
product user-work loss is claimed. Record child cleanup ownership before unrelated assertions and provide failure
discovery for admitted spawns whose marker/lookup fails. A similar pre-lookup gap precedes unparented assignment at 132,
but the explicit post-success host-read interval suffices without assuming marker loss. This is pre-existing at assigned
commit bd97d312165a02c351903f6bdf2dffcbe9ef8f33; the casing edit did not introduce it. Suggested bucket: other.
Confidence: confirmed conditional source mechanism; occurrence unmeasured. proposed_drop: none. Coverage searches
covered SPEC.md/SPEC_impl.md, empty TODO Planned, BUGS.md, plans/queue, INDEX and matching queue/ledger material; no
exact coverage. catalog-cleanup-can-start-untracked-test-session.md concerns pending GUI creates after observation ends
in other files, and ledger spare-owner-tests-register-child-cleanup-after-readiness.md (17887-17905) covers two Rust
owner-death tests. possible_cover: broader intent of catalog cleanup's discard-unless-trivial decision at 20197-20210,
whose trigger and named scope differ. No applicable FILTER match for retained test-session state. Provenance: g066
separate discovery; original f2 and all revisions preserved.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g066/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
