# Reveal-wait documentation describes a retired asynchronous write

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the wording is inaccurate for current grid callers, but the earlier claim that f560058 newly
introduces that mismatch is not supported.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g092/l1`.

### g092/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e, with attribution corrected by complete-range caller evidence.
Original consequence preserved: the reveal-wait rationale tells future test authors to reason about an asynchronous
terminal-write completion although their grid attachment reveals synchronously. TLDR: the wording is inaccurate for
current grid callers, but the earlier claim that f560058 newly introduces that mismatch is not supported. Details:
helper lines 38-39 say exactly: '`revealed`, rather than `revealReason`, is the witness because the

- reason is recorded one asynchronous write completion before the reveal.' The same text exists at exclusive base
  6e423e30068b62c6cfa276c6d720ee35bb70387a. At base the ordinary run already creates host/grid terminals:
  e2e/suite-terminals.ts:4-8 and 16-20 explicitly separate ordinary product tests from the old-terminal run. Thus the
  rationale already described the wrong mechanism for ordinary grid callers. However, it was not wholly without an
  applicable client then: suite-terminals.ts:10-18 and 62-74 defines a separate tmux/xterm tagged run, and
  terminal-replay-rename.spec.ts:715-777 is an actual OLD_TERMINAL_TAG caller of waitForReplayReveal at :764 after
  injecting nonempty byte replay. Base terminal.js:5445 and 5467-5488 records the reason and calls writeBytes(joined, ()
  => reveal(true)); writeBytes:5362-5389 invokes term.write completion. Base config search at :107-110,171-175,211
  confirms explicit old-tag selection and propagation of tmux mode. Therefore 'all actual fixtures were already grid at
  base' is refuted; only the ordinary retained fixture population was. At 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513 the
  old run and old-tag callers are removed, leaving the helper's ordinary grid call sites
  (terminal-replay-rename.spec.ts:370,417,479,1072,1161,1416,1502). The actual suite-usage transition therefore belongs
  to 647f0227, before f560058 removes the remaining production byte mechanism; helper text itself is unchanged in both
  commits and at pinned tip. Source-confirmed current mismatch remains: terminal.js:3893-3915 calls reveal
  synchronously; :3774-3781 flushes and records reveal; grid-terminal.js:1997-2000 flush is paint(), with history
  explicitly allowed still pending at terminal.js:3774-3776. Suggested bucket: other, test documentation. Confidence:
  confirmed mismatch and caller history by source analysis; no execution or reproduction. proposed_drop: propose
  excluding l1 as an introduced defect in the assigned f560058 change because the exact ordinary-grid mismatch predates
  the complete range and is inherited by the retained helper callers; removal of the last byte referent alone does not
  prove new documentation harm. Preserve the contrary contextual fact for parent judgment: an actually applicable
  old-run caller existed at base and its removal at in-range 647f0227 makes this rationale stale for the suite as a
  whole. If that caller-population change qualifies as new staleness, attribute it to 647f0227 rather than f560058. This
  supersedes the prior proposed_drop:false on attribution grounds, not on production correctness or the absence of a
  runtime bug. No parent drop/lowering is assumed accepted. possible_cover: parent cites g089/l2's separate-location
  pre-base grid rationale; this supports using the same attribution standard, not automatic duplication or coverage.
  g181/c2 remains a distinct newly changed applied-history claim. Could-be-highest: no; the consequence is inaccurate
  fixture documentation only. No unresolved source premise remains for the requested base-versus-transition question;
  the remaining introduced-defect classification is expressly submitted to the parent with both supported perspectives.

Returned proposal (claims-060, event 11): **retain-verified**, suggested bucket **other**.

The reveal helper still explains a synchronous grid reveal using a retired asynchronous write callback. Keep the
source-confirmed documentation mismatch while the parent settles attribution: the ordinary-grid mismatch predates the
range, but removal of the last actually applicable old-run caller occurs inside it.

Original stronger consequence: The initial lead/finding alleged misleading synchronization documentation, not broken
reveal code, and initially attributed universal staleness to f560058. Latest record proposes dropping assigned-patch
attribution because ordinary-grid harm already existed before the range, while expressly preserving the contrary fact
that an actual old-run caller existed at base and was removed by in-range 647f0227. Do not erase that withdrawal
proposal or treat production correctness as refuting the documentation defect. History drainage is not what this helper
observes.

Unresolved premise or limitation: Parent classification of introduced-defect scope: is removal of the last old-run
caller enough to record newly universal staleness, despite ordinary retained callers already being misdescribed at the
exclusive base? Source facts on both sides are established; no new harm to an ordinary-grid caller is demonstrated.

Proposed basis: Retain the verified documentation mismatch with the attribution caveat for parent judgment. If retained
as in-range newly universal staleness, cite 647f0227's caller-population transition rather than claiming f560058 first
introduced ordinary-grid misinformation. No exact existing coverage was found.

Existing parent disposition (event 11): rejected. Parent read full claims060 and judges the stated attribution question.
The unchanged universal asynchronous-write rationale already misdescribes the retained ordinary-grid callers at the
exclusive base; their source, false explanation and user consequence persist unchanged. Removing the last old-run caller
to which that wording happened to apply makes it universally stale but adds no new false application or new promise for
a surviving caller. Unlike a new branch or contract applying old text to a new domain, this only removes a correctly
described branch. Preserve647f0227's removal and historical documentation mismatch, but no in-range introduced/worsened
consequence established for this exact helper. Correct production reveal is not the refutation. No security/user-work
loss or stronger consequence.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g092 record](records/review-through-00de0b6c/groups/g092.json). Record ID: `l1`.
