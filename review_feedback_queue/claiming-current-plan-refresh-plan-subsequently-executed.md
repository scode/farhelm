# Claiming a current plan does not refresh the plan subsequently executed

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Plan execution can use stale instructions after claiming the current queue entry.

## Details

F268 — **definite** — `plans/AGENTS.md:372–375` — Claiming a current plan does not refresh the plan subsequently
executed

The prescribed workflow reads current remote queue state but executes plan contents from an earlier local fetch. It can
therefore claim the latest plan yet miss a new file or follow instructions superseded by maintainer decisions. After
confirming ownership, fetch and read the claimed plan and its decisions from a current committed snapshot before
choosing the implementation base.

## Evidence and triage context

- plans/AGENTS.md:178–182 fetches at the initial clean-main boundary.
- plans/AGENTS.md:363–372 performs remote status, claim, and re-check operations before reading the plan from
  main@origin, without requiring another fetch.
- scripts/plans-queue.py:14–18 explicitly leaves local checkout and jj state untouched; :1074–1078 reads remote head for
  the claim, and :1269–1272 reads remote head for status.
- plans/AGENTS.md:102–107 allows pending/blocked plan revisions and makes Decisions override conflicting plan text.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- The watcher cache item concerns an independently editable script and a different consequence. Neither TODO.md's
  Planned item nor the searched specifications, BUGS.md, queue, or triage ledger covers this stale execution snapshot.

Caveats:

- Requires a plan revision or answer between the initial fetch and successful claim. The concurrent schedule was not
  executed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_06_cor:p1:F2`.

- `automation_website_06_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
