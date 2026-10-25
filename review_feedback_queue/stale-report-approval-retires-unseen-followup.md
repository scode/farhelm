# A stale report approval can retire an unseen follow-up

Reviewed commit: 93dcb48e189d1b208f8b9cd9412db86d1c563ebd

## TLDR

If another review's follow-up lands while an older report page remains open, approving that old page can retire the
newer report's tracking. This pre-existing slug-only approval gap is preserved by the rewrite; no product-data loss is
claimed.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `plans/.agents/reviewing.md:30-36,48-54`. Locations and revisions in the assessment below identify the
reviewed source; later main is outside this review's pinned range.

Discovery-pass coverage revision; same original f2. Reviewed commit: 93dcb48e189d1b208f8b9cd9412db86d1c563ebd. TLDR: if
a follow-up lands while an earlier review page remains open, approval from the old page can remove the new report's
tracking before the maintainer reviews it. Exact source route: original reviewing.md:14-17 allows the role in any
separate session, :30-36 snapshots reports on the page, :48-54 acts on pasted approvals. scripts/plans-queue.py:868-877
Request carries slug but no report identity; :946-956 accepts any current landed row with that slug and deletes its
current plan/report; :1089-1121 reads/retries against current remote head without an expected report digest. A second
review session can request follow-up (:957-972), executors/monitor can land it again, then an old approval sees the new
landed row and succeeds. Pending/not-yet-relanded follow-up is correctly refused, so that is not this finding. The same
Request and approve transition remain at pinned tip 463cac1528b5c5e259404975380d321a3fff0412, same line numbers.
Port/clipboard retirement does not alter that backend precondition. Suggested remedy: bind approval to report content
identity at presentation and compare it on each publication attempt; stale approval should require a fresh review.
Suggested bucket: other, confined to maintainer review tracking; no established security, process or product data-loss
route. Confidence: confirmed source mechanism under the specified interleaving; occurrence unmeasured. Important scope
caveat: predecessor 93dcb48e189d1b208f8b9cd9412db86d1c563ebd^:plans/.agents/reviewing.md:24-37,48-54 already uses the
same approval protocol. This is a pre-existing contract gap retained by the assigned rewritten page instructions, not
introduced by those commits; parent must decide applicability to the patch-review charter. possible_cover: no exact
coverage found in pinned SPEC.md, SPEC_impl.md, TODO.md Planned, BUGS.md, queue INDEX/files, TRIAGE_OUTCOMES.md or
pending plan searches. The ledger's restart-parent-cancel.md (:6891-6907) is product Restart consent, and
plans-heading-splice.md (:9066-9082) is question-text parsing; neither matches this trigger/consequence. TODO's website
install-command guard (:383-389) is unrelated and outside Planned. FILTER has no clear match: this is a potentially
persistent removal of current report tracking, not a transient failed approval or imprecise diagnostic, and its trigger
need not be sub-second. proposed_drop: none on coverage; patch applicability caveat explicitly retained for parent
judgment. No tests, experiments, external replacement-skill audit or runtime evidence.

Provenance: `g234/f2`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
