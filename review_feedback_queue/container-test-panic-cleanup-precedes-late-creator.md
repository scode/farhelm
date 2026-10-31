# Container test panic cleanup may precede a late creator

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A failed test's cleanup can run before its pending creator finishes, but the complete directory/allocation schedule
remains unverified.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The inspected cleanup and creator lifetimes leave a conditional gap. Source checks did not settle the exact retained
directory lifetime and allocation ordering needed for an additional escaped test container. This is an investigated
private-fixture ownership concern, not an observed product leak or work-loss route.

Source locations: `crates/farhelm/tests/e2e/containerized_sessions.rs:334-347`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d; assigned callers were added by
f73589c5201a0408f2f1555c18a4efca6f6057be and 2d98a04e3e291bf2b05e9f9bdaf2a9fc2595ee11. TLDR: a readiness failure may
leave an owned test container running after panic cleanup; this has NOT been observed. Source basis: the existing
rm-only Drop (containerized_sessions.rs:14-22) does not serialize with or revoke the separate creator, and new callers
wait at :334-347 and :539-552. Creator reads the ticket into memory, ignores SIGHUP and retains its flock
(:container.rs:567-627). Required order: readiness/attach failure begins unwind; rm finishes while the session container
is absent; creator then publishes its marker while the state directory exists, obtains the checkout bind while that
directory exists, and completes initialization/running confirmation successfully. The marker is written before podman
run; files.rs:398-402,456-457 does not recreate its parent. Late inspection completion after state destruction therefore
fails safely. Every post-run failure attempts and confirms removal (container.rs:627-635,663-680); mere continued
creator lifetime proves no leak. The marker's later disappearance alone does not force rollback because
clear_initialization accepts NotFound (:240-244). Harness field/drop order keeps the state directory through tmux
diagnostics/shutdown and the earlier-declared checkout root through Harness destruction (harness.rs:883-903;
teststate/lib.rs:649-677). Verified shutdown calls kill-server and, if necessary, signals only the verified tmux peer,
not a container/creator sweep (teststate/tmux.rs:475-598); the testtrace wrapper only drops the Tokio runtime and
records unwind (testtrace/lib.rs:994-1025). These provide a source-level interval but do not establish actual Podman
storage-lock scheduling or a successful late allocation. Actual checks: traced marker staging/write and parent lifetime,
both fixture drop orders, tmux cleanup target, test wrapper runtime ownership, production rollback, and exact
queue/ledger text. Missing evidence: a supported ordinary Podman delay permitting that entire order. Source-only
authority cannot measure it; a controlled held-creator failure run must observe rm completion, marker/bind acquisition
before directory teardown, successful initialization and a final surviving owned UUID container. No tests or experiments
were run. Suggested bucket other; no established security or user-work-loss route. Confidence likely cleanup
coordination gap, suspected consequence unverified after investigation. proposed_drop none. possible_cover:
catalog-cleanup-can-start-untracked-test-session.md:12-18,33-78 and TRIAGE_OUTCOMES.md:20197-20209 concern four named
browser tests and GUI/key-mint creates, not these Podman fixtures. The quoted discard principle may inform parent
judgment but is not exact trigger/scope coverage. Its earlier cleanup record, TRIAGE_OUTCOMES.md:17199-17216, explicitly
limits repair to eight named browser blocks; :12468-12513 covers different named process fixtures. No exact existing
cleanup record, Planned item, unlanded plan, BUGS item or filter covers f2. Original attribution caveat remains: the
rm-only helper predates these assigned test callers.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g126/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
