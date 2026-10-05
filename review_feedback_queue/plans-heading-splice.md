# Legal Markdown headings can strand part of a plan question

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Legal Markdown headings can strand part of a plan question.

## Details

`F47 / COR-PLANS-HEADING-SPLICE` — **definite** — `scripts/plans-queue.py:408` — Legal Markdown headings can strand part
of a plan question

When an executor asks a question about a plan, the queue script inserts the question under `## Blocked`. Answering it
later removes that whole section and archives the question with the maintainer's decision. The input guard is meant to
prevent headings inside the question from ending that section early, but it only recognizes heading lines beginning with
hashes and a space in column zero.

Legal Markdown such as an indented `## Options` heading, or `Options` followed by a line of hyphens, passes that guard.
The script then formats the complete plan with dprint, which can normalize those forms into a column-zero level-two
heading. The section-removal code treats that heading as the end of `Blocked`, so answering archives only the question's
prefix and leaves the remaining question or options as ordinary plan text. The queue transition can succeed while the
plan retains material that should have been removed.

Validate the formatted Markdown or use heading rules that cover these legal forms, and check either refusal or
preservation of the whole question through the block-and-answer round trip. The original review reports two read-only
formatter invocations confirming normalization; no remote transition was exercised. Proposed bucket: other. No possible
cover was identified.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **other**.

Originating reviewers and passes: `auto_edges p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Two read-only formatterinvocations confirmed normalization; no remote transition.

## Filed reviewer metadata

- `auto_edges p1`: confidence as filed: definite / source and pinned formatter confirmed. Severity: durable plan-section
  corruption. Suggested bucket: other. Suggested bucket as filed: other.
