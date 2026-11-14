# Intermediate removal of the vendored-xterm tag contract test

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Existing sessions that still use the old terminal lose the regression check that their wrapped
hyperlink warning reads the real destination correctly. A future vendored-library change can silently remove that
warning coverage.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g176/l1`.

### g176/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513.

TLDR: Existing sessions that still use the old terminal lose the regression check that their wrapped hyperlink warning
reads the real destination correctly. A future vendored-library change can silently remove that warning coverage.

Details: This commit deletes the seven-layout test beginning at parent e2e/tests/terminal-links.spec.ts:1082 (deletion
anchored at resulting line 1075). It was the test asserting real vendored-xterm getCell(...).extended.urlId values and
shared tags across Claude-style reopened ids, Codex cursor continuations and wraps. SPEC_impl.md:1911-1914 explicitly
requires it: "The field can change at a manual xterm upgrade; a browser test against the vendored bundle pins its
presence and shared tag for both agent layouts. Losing it falls back to today's row comparison, never a quieter
partial-match rule." Old terminals remain supported, per SPEC.md:3014 onward and SPEC_impl.md:1958-1966. The replacement
grid test at e2e/grid-client/grid-attach.spec.ts:1247-1277 supplies pre-tagged grid rows via showScreen and cannot read
the vendored-xterm field. crates/farhelm-ui/js-tests/terminal-links.test.js:477-496 constructs synthetic cells with the
field itself; its adapter tests at 511-541 cannot catch a change in the actual bundle either. Targeted pinned-tree
searches for urlId/extended.urlId/xtermLines and tagged-row checks found no retained real-bundle tag test. Preserve this
test by mounting the existing static old-terminal harness and feeding OSC output there; creating tmux terminals on a
current supervisor is unnecessary.

Suggested bucket: other (regression-test/spec-contract loss in maintainer tooling). Confidence: confirmed by source and
contract analysis only; no execution or reproduction. proposed_drop: false. possible_cover: none (forbidden coverage
metadata not read). Could-be-highest: no; no current security regression is established, and the existing fallback
remains explicitly specified. Caveat: new terminal tag/mismatch behavior retains separate grid and pure-function tests;
this finding concerns only the dependency contract of retained old-terminal support, not all wrapped-link coverage.

Synthesis turn 2 — range disposition: the original consequence and exact 647f0227 SPEC_impl.md:1911-1914 citation above
remain valid at that intermediate revision; no drop is accepted or proposed here. This is not a dependency test still
needed at pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. Later dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 retires
old terminals at supervisor startup; f560058da19e02716e6f943883a9508b447aff4e then removes the vendored xterm bundle and
terminal.js old-surface branches, plus terminal-links.js xtermLines/fromXtermRange/toXtermRange and private
extended.urlId reads (diff around original 576-631). Its terminal.js:13-35 mounts grids only. A pinned-tip source search
across assets, lib.rs, grid-client browser tests and JS tests found no xtermLines, extended.urlId, urlId,
vendor/xterm.js, writeBytes or term.write call. Therefore the future-xterm-upgrade consequence applies only while that
intermediate old renderer existed; the tip has no such dependency. Keep this as a confirmed intermediate
explicit-test-contract gap, suggested bucket other, not as an instruction to restore an obsolete dependency test on
current code. Confidence source/contract only, no execution; proposed_drop remains false; possible_cover none;
could-be-highest remains no. The later removal is source evolution, not a searched queue/coverage disposition.

Retirement premise source check completed in synthesis: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 core.rs:6426-6438
invokes OldTerminals::end_private_server while owning the state directory, then disables further tmux use; reload
contract at 6664-6681 gives old rows no terminal. old_terminals.rs:1-82 explains the startup-only ending/conversion and
accepted failure residuals. The subsequent actual adapter/bundle deletion, rather than the success of cleanup on every
machine, establishes that tip has no remaining xterm private-field dependency. No runtime cleanup result was assumed.

Returned proposal (claims-110, event 14): **needs-parent**, suggested bucket **other**.

The intermediate explicit dependency-test gap is established. Its subject disappears later within the reviewed range.
Preserve the historical finding, but do not ask current code to restore an obsolete xterm test. Parent must distinguish
recording an intermediate gap from recording an outstanding defect.

Original stronger consequence: Existing old sessions lost the regression check that wrapped hyperlink warnings read the
real destination correctly; a future vendored-library change could silently remove warning protection. No actual
misleading warning, opening bypass or work loss was demonstrated. The latest record preserves that intermediate
consequence and expressly rejects restoring an obsolete dependency test at tip.

Unresolved premise or limitation: No current xterm dependency or concrete shipped security regression remains
established. An instruction to restore this test on present code would need a surviving consumer that the tip search did
not find.

Proposed basis: Confirmed intermediate test-contract loss, followed by removal of its entire dependency within range.
Any parent dismissal of the historical warning-protection claim should preserve this distinction and receive the
independent check; this consultation does not authorize a drop.

Existing parent disposition (event 14): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g176 record](records/review-through-00de0b6c/groups/g176.json). Record ID: `l1`.
