# Plan deferral outlives the question it deferred

Reviewed commit: d64ae3a81547729100529d210d3ea50434ff933d

## TLDR

After the maintainer answers one large plan question, a later ordinary question from that plan can remain excluded from
automatic handling. Whether the operational-artifact exclusion covers this workflow defect is ambiguous.

## Details

Suggested bucket: **other**. Confidence: **confirmed instruction gap; actual monitor interpretation unmeasured**. This
concerns the maintained automation procedure, not a historical run's compliance or a product defect.

`plans/.agents/landing.md:155–159` records the slug of a question left for the maintainer. Lines `64–66,202–205` then
exclude that slug from automatic handling. The full landing workflow, log contract
(`plans/.agents/outside-files.md:25–32`) and reviewing flow (`plans/.agents/reviewing.md:78–88`) neither retire the mark
after an answer nor bind it to the original question. The supported answer, claim and block transitions reuse that slug
(`scripts/plans-queue.py:976–998,1025–1034`). A new ordinary question can therefore inherit an old deferral. A deferral
should identify the particular block or be retired when that block is answered.

Possible cover: `TRIAGE_OUTCOMES.md:20479–20495` excludes plans, reports and review/approval bookkeeping as operational
artifacts. Its application to stale report-approval tooling (`20554–20566`) means maintained automation is not
categorically exempt. It does not clearly settle this deferral consequence. No exact matching Planned item, unlanded
plan, BUGS entry, queue item or additional decision was found. Retain that coverage ambiguity for triage. Introduced by
this commit. Proposed drop: none. No security or user-work-loss consequence is established.

Provenance: trailing-review run `6b8af8f0-1a32-459b-89e9-6cea539af957`, `block-answering/f1`. Source review only.
