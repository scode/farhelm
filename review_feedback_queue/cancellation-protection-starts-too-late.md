# Cancellation protection starts too late

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Cancelling a connection supervisor before its first poll leaves its worker unmanaged.

## Details

F170 — **definite** — `crates/farhelm-helm/src/manager.rs:1767` — Cancellation protection starts too late

The abort guard is constructed inside the supervising future rather than before that future is spawned. Cancellation
before its first poll drops the captured worker handle without aborting it. Once the start gate is open, the worker can
keep connecting or refreshing without a manager handle; closed notification senders do not stop it. Construct the guard
first and move it into the future.

## Evidence and triage context

- crates/farhelm-helm/src/manager.rs:1754–1769: the actor is spawned before the supervising future constructs
  AbortOnDrop.
- crates/farhelm-helm/src/manager.rs:1272–1284: dropping a JoinHandle does not abort its task; AbortOnDrop supplies the
  abort.
- crates/farhelm-helm/src/manager.rs:1668–1669: actor start gates are released before reconciliation returns.
- crates/farhelm-helm/src/manager.rs:2855–2867,2907–2915: Stop and shutdown abort the supervising task.
- crates/farhelm-helm/src/manager.rs:3020–3023: a closed nudge channel parks rather than terminates the actor.
- crates/farhelm-helm/src/manager.rs:4168–4206: identity-less refreshes do not revalidate registry existence.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"matched": null, "possible": "review_feedback_queue/hostnotfound-refresh-keeps-serving.md:15–25 concerns missing
  registry revalidation after deletion. It does not cover failure to deliver cancellation, including shutdown."}

Caveats:

- Requires cancellation before the supervising task's first poll.
- No runtime reproduction.
- An orphan removed from the manager map cannot pass agent_requests.rs:702–707's origin check; continued authorization
  was not established.
- Requires cancellation before the supervising future's first poll.
- No runtime reproduction was performed.
- crates/farhelm-helm/src/agent_requests.rs:702–707 rejects upcalls whose originating connection is no longer published;
  continued authorization is not established.
- Same-actor persistence after an identity-less deletion is related context, not the same editable cancellation region.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_03:p1:F1`.

- `gap_helm_connections_sec_03:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
