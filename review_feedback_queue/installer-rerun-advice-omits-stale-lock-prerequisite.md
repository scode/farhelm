# Installer recovery advice omits an old-lock prerequisite

Reviewed commit: 61dada0adec647c35fbb5b78a6039d059c5ea232

## TLDR

The install guide's blanket rerun recovery promise omits the manual removal an older unmarked stale installer lock may
require. Refusing that lock is accepted; the diagnostic-documentation filter's scope is ambiguous.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `website/src/content/docs/docs/get-started/install.md:26-27`. Locations and revisions in the assessment
below identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit: 61dada0adec647c35fbb5b78a6039d059c5ea232; affected guide and installer unchanged through pinned tip
463cac1528b5c5e259404975380d321a3fff0412. TLDR: after an interrupted installation, the guide promises a rerun finishes
and repairs it, although the reader may first need to remove a leftover lock as instructed by the refusal.
scripts/install.sh:700-708 says power loss skips cleanup and can retain that lock; 1147-1150 refuses the retained
directory. The payload repair at 1259-1268 lies after this gate. scripts/test-install-sh.sh:933-995 explicitly expects
the lock, first-rerun refusal, manual rmdir, then successful rerun (985-994); its damaged-version case at 806-828
establishes only the separate payload repair. docs/install_uninstall.md:85-87 already states the manual prerequisite.
Suggested bucket: other. Confidence: confirmed source mismatch; no runtime reproduction. Caveats: executable refusal
supplies ordinary recovery; no security consequence, bricked install or user-work loss established. possible_cover:
SPEC_impl.md:4430-4438, SPEC.md:517-534 and TRIAGE_OUTCOMES.md:3677-3701 accept the refusal and recovery prerequisite;
they do not accept an unconditional guide promise. The separate ledger payload-repair item at 18689-18707 does not
establish automatic lock recovery. Previous pinned coverage searches found no Planned, BUGS or queue duplicate; TODO.md
Planned is empty, and installer simplification at 550-565 is Maybe later. FILTER Rare, self-correcting glitches and
imprecise diagnostics remains ambiguous if documentation is diagnostic guidance: do not use accepted underlying lock
behavior as automatic acceptance of inaccurate prose, or attribute its persistent lock to the documentation.
proposed_drop: none; retain OTHER pending parent assessment of that filter boundary. Suggested correction: qualify the
guide with the lock refusal and its advice; no executable change requested.

Recording-time main has a newer installer with kernel locking and automatic cleanup of marked lock folders. Its
SPEC_impl.md side-by-side installation section still expressly refuses an older unmarked lock folder and says that only
the user can remove that leftover. The install guide is unchanged from the reviewed tip. The retained claim is this
narrow older-lock documentation caveat, not a claim that every interrupted new installer leaves a stale lock.

Provenance: `g307/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
