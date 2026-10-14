# Potential stale target after a partial wrapped-link redraw — pre-existing

Independent drop decision: a fresh `gpt-6-astra` high reviewer rejected the pre-existing/specific-URL-policy exclusion.
The URL ledger decision is explicitly limited to non-security vendor issues and does not conclusively cover file-target
authorization. Suggested bucket: provisionally `highest`. The tooltip still discloses the old canonical target; this is
not demonstrated arbitrary execution, secret exfiltration, or a disclosure bypass. Actual wrapped-file redraw and
harmful reliance were not reproduced. Retain this trust-consequence question for triage.

Reviewed commit: 2e6fe8d09b747faec24382d95852bfb5d5688c90

## TLDR

Repainting only part of a wrapped link can leave its previous click target active.

## Details

Suggested bucket: `highest`. Confidence: confirmed for the existing invalidation predicate; likely for file-link
manifestation, with an actual partial wrapped-file redraw unverified. Source: dedicated `gpt-6-astra` high adversarial
review. Proposed drop: Drop from this commit's actionable findings because the responsible vendor code and file-link
range construction are unchanged from the first parent. Independent review is still required for this potentially
highest candidate..

crates/farhelm-ui/assets/vendor/xterm.js:1, Linkifier._handleNewLink's onRenderedViewportChange callback and
_clearCurrentLink, invalidate only when the repaint contains both link endpoints. A partial redraw therefore need not
invoke the changed hover identity checks in terminal-files.js:192–216. This is the existing bundled-library invalidation
defect, not introduced by this commit. Its potential misleading-target consequence warrants retaining the candidate for
independent drop review; the file-download manifestation was not reproduced.

Possible cover: TRIAGE_OUTCOMES.md:7680–7706,
repainting-part-wrapped-url-leaves-previous-target-outer-repaint-invalidation-predicate.md and the corresponding
inner-cached-link-clearing-predicate.md; reviewed SPEC_impl.md:759–762 accepts upgrading rather than working around
non-security vendor defects. The ledger establishes URL coverage, not necessarily every file-download consequence.

Status at frozen endpoint: Underlying vendor predicate persists unchanged at pinned main. Current SPEC_impl.md:774–777
retains the vendor policy; TODO.md:842–845 records checking upstream at the next upgrade. No fixing commit in the frozen
range.

Review coverage: full commit diff read: true. No commit-diff content remains unread. An oversized combined tool response
was truncated; required product code and policy content was subsequently reopened in bounded calls. The entire
13,089-line triage ledger was not read linearly; coverage was targeted searches and the cited entries. Unchanged
portions of large callers, tests and the minified vendor bundle outside the cited excerpts were not exhaustively read.
No Sol reports or new working-copy queue finding contents were read.

Runtime evidence: None, as requested by default. Findings are static review conclusions. No checkout files were created,
modified or deleted; no fetch, rebase or helper agents.
