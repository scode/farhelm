# An ambiguous claim can be orphaned by the next pick

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An uncertain plan claim could be stranded when the executor claims another.

## Details

F267 — **possible** — `plans/AGENTS.md:366–368` — An ambiguous claim can be orphaned by the next pick

The workflow allows another pick while an earlier claim remains unresolved but keeps only one current-claim recovery
record. If the first claim becomes visible later, it can remain assigned while execution proceeds elsewhere, blocking
other executors. Late visibility was not induced. Resolve each uncertain claim before another, or retain and reconcile
an explicit set of unresolved claims.

## Evidence and triage context

- plans/AGENTS.md:118–122 defines the executor log around its current claim; :346–351 recovers that recorded claim.
- plans/AGENTS.md:364–368 writes the new slug/id before claiming, then instructs picking again while an earlier claim
  remains uncertain.
- scripts/plans-queue.py:1118–1134 attempts the remote update and can still raise Failure when verification fails; :71
  and :1544–1546 map that failure to exit 3.
- scripts/plans-queue.py:1292–1306 reports an ordinarily dated abandoned claim only after more than 24 hours and
  sufficient log inactivity.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/watcher-revision-cache.md:14–26 concerns caching a wake verdict against another revision. It
  does not cover loss of an unresolved claim's recovery record. review_feedback_queue/plans-heading-splice.md:14–23
  concerns Markdown section corruption, not claim reconciliation.

Caveats:

- No remote failure or stale read was induced. An executor could retain extra history voluntarily, but the prescribed
  flow does not require reconciling that history before another claim.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_06_cor:p1:F1`.

- `automation_website_06_cor:p1:F1`: confidence as filed: possible; suggested bucket as filed: other.
