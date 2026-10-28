# A stale plan answer can clear a different question

Reviewed commit: d64ae3a81547729100529d210d3ea50434ff933d

## TLDR

A delayed answer can be attached to a plan's replacement question and make that plan eligible again, even when the new
question was reserved for the maintainer. Actual unauthorized execution is unestablished.

## Details

Suggested bucket: **other**. Confidence: **confirmed queue mutation; downstream authorization harm unverified after
investigation**. This is a preexisting answer-admission weakness newly relied on by the automatic answering workflow.

`plans/.agents/landing.md:168` says a concurrent change makes `queue answer` refuse. In fact,
`scripts/plans-queue.py:1025–1034` checks only that the slug is blocked. The request carries no expected question
identity; publication retries read current main (`1089–1121`). While a monitor drafts an answer to A, another review can
answer A, an executor can claim the plan and block on B, and the delayed answer to A can then be quoted beside B, remove
B and return the plan to pending. The parent revision has the same backend check. Bind submission to the question
actually read, including across publication retries.

The same transition accepts B when it is a huge question under `plans/.agents/landing.md:155–159`. The monitor never
classifies that replacement question. That loses B's blocked/manual-answer state, but does not establish permission to
perform its protected action: executors must reread Decisions and reconcile resumed work
(`plans/.agents/executing.md:67–87`), root `AGENTS.md:40–47` still requires actual authorization, and monitor answers
must identify their provenance and specific decisions (`landing.md:165–167`). These are instructions an executor must
follow, not mechanical authorization checks; they limit the established claim without proving harmful interpretation
impossible.

Investigation checked those consumers and the real layout answer at `plans/queue/steel-dialogs.md:524–545`. No concrete
A/B pair and supported downstream interpretation established a protected action occurring despite those requirements.
That is the missing premise for a security or catastrophic user-work consequence; it is not an impossibility proof. Such
a pair and consumer would warrant reassessing the bucket. No runtime reproduction ran.

Possible cover: `TRIAGE_OUTCOMES.md:20479–20495,20554–20566` excludes operational artifacts and a stale report-approval
case. The latter retires report tracking; this case changes question/answer pairing and execution eligibility. Exact
coverage is ambiguous and is preserved for triage. No exact Planned item, unlanded plan, BUGS entry or queue duplicate
was found. Proposed drop: none for the established queue defect.

Provenance: trailing-review run `6b8af8f0-1a32-459b-89e9-6cea539af957`, `block-answering/f2`; the potential
authorization extension received independent scrutiny from `admission-audit`, which agreed to retain the queue defect
and preserve the unestablished downstream consequence.
