# Old pending archives can move replacement folders

Reviewed commit: 463cac1528b5c5e259404975380d321a3fff0412

## TLDR

An interrupted archive written by older development code can move a replacement folder after device/inode reuse when
birth time is unavailable. Current permanent deletion refuses this birthless case; a proven older-release variant may be
filtered.

## Details

Suggested bucket: **high**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-supervisor/src/working_copies.rs:2115-2153`. Locations and revisions in the assessment
below identify the reviewed source; later main is outside this review's pinned range.

Later-turn synthesis at reviewed commit 463cac1528b5c5e259404975380d321a3fff0412; same defect as g112/f2 and a044's
g268/f1. Correction/clarification to earlier filter verdict: AGREE only for a journal actually carried across an upgrade
from an older Farhelm release, including the demonstrated 0.30.0 writer 8f5cf8dc4365a586daa2d2fd408bf8bea375f9a0.
AMBIGUOUS on excluding the whole pre-fix-journal finding. Static source shows the pre-fix implementation can journal
without birth-time refusal; it does not establish that all such journals were written by a released build. A development
build of pre-fix code can use the same writer; actual occurrence is unverified, and no source-enforced release-only gate
was shown. Retain the narrow development-build remainder conditionally as high, with the upgrade filter only possible
cover. Source mechanism remains confirmed conditional on a retained pending journal, interruption before rename, root
still matching, a replacement at the original path reusing the recorded device/inode, and birth time remaining
unavailable. Current recovery working_copies.rs:2115-2153 performs the move; current initial move :1703-1715 prevents
new continuously-birthless journals. Permanent deletion remains source-refuted while birth time remains unavailable
(:723-727 and checkout_trash.rs:137); no independent security consequence, process/data destruction, corrupt database or
broken installation was established. Suggested high for recoverable displacement, consistent with
TRIAGE_OUTCOMES.md:1573-1592. Confidence: source-established conditional mechanism, actual filesystem and pre-fix
development-build use unverified. possible_cover: FILTER.md 'Upgrades from older releases combined with another rare
condition' exactly covers released-writer instances, ambiguously development-build updates; no blanket coverage.
proposed_drop:false for the retained development-build remainder; proposed_drop:true only for the independently agreed
released-writer subcase. Preserve earlier possibly-highest permanent-loss claim and its scoped source refutation under
this same ID; no separate duplicate record.

Provenance: `a034/g112-older-archive`, `g112/f2`, `g139/f3`, `g188/f2`, `g268/f1`, `a044/g268-f1`. These are
reviewer-local identities, not additional findings. Proposed drop: none for the retained remainder.
