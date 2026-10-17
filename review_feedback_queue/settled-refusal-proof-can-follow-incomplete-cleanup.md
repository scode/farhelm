# Settled-refusal proof can follow incomplete process cleanup

Reviewed commit: e870329ce9be321493a2fc03de4f3cc67549fd2f

## TLDR

A failed replacement can be labelled definitely unaccepted even though cleanup did not establish that all its processes
stopped. A later Replace can then start fresh beside surviving work. The concrete rollback scenarios already have
recorded decisions; whether those decisions fully cover the new retry signal remains unresolved.

## Details

Suggested bucket: conservatively `highest`, conditional on surviving work overlapping a fresh attempt and affecting user
data. Confidence: predicate/rollback mismatch confirmed statically; surviving descendants, successful later retry and
data loss were not reproduced. Provenance: a separate Astra high exclusion auditor rejected the repeat reviewer's
blanket safety refutation. Proposed drop: existing fault/rollback coverage; the independent verdict was ambiguous, so
this record retains that question under the recording rules. No uncovered single-fault execution was established.

The supervisor proof accepts a matching fingerprint and lifetime, a `Failed` reservation and an absent session row
(`crates/farhelm-supervisor/src/service/core.rs:7391–7406`). These conditions do not themselves prove process cleanup:

- A rollback uses `ScopeKillFailure::Warn` (`core.rs:9835–9843`), which can accept an unconfirmed scope kill after a
  clean portable sweep (`crates/farhelm-supervisor/src/service/sweep.rs:1405–1419`), then removes the ordinary session
  (`core.rs:9887–9897`).
- After database confirmation fails, a process-sweep error only enriches the returned error (`core.rs:10062–10078`).
  Successful tmux termination still allows abandonment (`:10111–10134`), settling `Failed` and deleting the row
  (`:14122–14129`; `crates/farhelm-supervisor/src/store.rs:5596–5613`).

The resulting proof passes through `service/handlers.rs:489–506`, the helm's Create-only restriction
(`crates/farhelm-helm/src/client.rs:2573–2576`), and the HTTP marker to plain Replace's key retirement. The UI does not
launch automatically: another affirmative Replace action is needed. A flag on a failed Delete cannot retire the create's
identity; that separate restriction was independently confirmed.

Possible cover: substantial. `TRIAGE_OUTCOMES.md`, “create-rollback-orphans-unconfirmed-scope.md” and
“failed-confirmation-discard-session-despite-failed-process.md”, record the two underlying orphaning mechanisms and
their deferral. TODO.md's related entry is Maybe later, not Planned; the ledger is the relevant coverage source.
SPEC.md:1149–1160 explicitly accepts coinciding independent rare faults leaving processes running beside a relaunch.
Those rules may fully cover the concrete combinations above. The first pass excluded the database-confirmation/sweep
combination on that exact basis after independent review. Astra's auditor could not establish that every route covered
by the new proof necessarily meets that exception and retained the ambiguity. This is an outcome-screening difference,
not evidence that a new unsupported single-fault regression exists.

Triage should resolve that coverage boundary before commissioning changes. Do not use the current specification or code
comments as proof that every failed-create rollback stopped all work. Static review only; no fault injection or runtime
tests. The referenced code persists at pinned tip afc828776596f2e6639d16d4203ccc8b3d0f5862.
