# Link refusal conceals a keyboard-opened Delete warning

Reviewed commit: 463cac1528b5c5e259404975380d321a3fff0412

## TLDR

A raised link refusal can hide a session's destructive Delete warning while keyboard opening and a fresh affirmative
Enter still work. The complete source path is established; actual overlap pixels and loss were not measured.

## Details

Suggested bucket: **highest**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location:
`crates/farhelm-ui/assets/app.css:1375-1403,1797-1847; crates/farhelm-ui/assets/session-action-shortcuts.js:7-53; crates/farhelm-ui/src/session_view.rs:2567-2622 at c1335cbd0b24063f2c4c0d3dd5dfba6749946dad`.
Locations and revisions in the assessment below identify the reviewed source; later main is outside this review's pinned
range.

Read g261/g261-synthesis-1.result.md:1-45 through a separate frozen synthesis manifest after latest-audit.json confirmed
all original mandatory ranges delivered. The producer independently establishes the same Delete keyboard route and
explicitly withdraws attribution-only whole-finding exclusion. Reconcile as ONE highest candidate with provenance
g261/f1 and a040/f1, not two issues. proposed_drop:false. Attribution-only historical statement remains valid: layer
ordering predates c1335cbd, which raises46 to51 and expands refusal triggers. Source-established facts: opaque refusal
sits over the destructive header warning band in the shared stacking context; shipped Delete keyboard chord opens the
nonmodal prompt without pointer departure; enabled affirmative answer takes focus and accepts a fresh Enter.
Snapshot-bound guards authorize the correct rendered status but do not check whether its warning is concealed. Exact
pixels, event occurrence frequency and actual user-work loss remain unmeasured; those measurements are distinct from an
unshown harmful input route, which is now supplied by shipped code. Suggested bucket highest, source-confirmed mechanism
conditional on its layout, no runtime reproduction. possible_cover remains SPEC2010-2012 and ledger15916-15931's exact
approval-card context exception, which does not cover destructive-header consent; follow-up code comment is not
acceptance. Generic dialog overlap allowance is not exact coverage and preserves destructive guards. Existing f1
retained; no withdrawal or separate finding added. Remaining work this turn: finish static synthesis of narrow other
exclusions, test boundaries and contract interactions; separate discovery remains for a later turn.

Provenance: `a040/f1`, `g261/f1`, `g136/f4`, `g145/f4`. These are reviewer-local identities, not additional findings.
Proposed drop: none for the retained remainder.
