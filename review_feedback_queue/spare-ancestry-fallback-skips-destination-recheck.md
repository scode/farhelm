# An ancestry refusal skips the later destination recheck

Reviewed commit: 949942421d12b46449164a7c11e20a0494e3ac10

## TLDR

A prepared-checkout ancestry refusal returns before the post-fetch destination ownership check, and fallback cloning can
use a replacement empty folder. No existing-file loss is shown; the scope of the accepted check/move window is
ambiguous.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-supervisor/src/spare_take.rs:128-135`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit 949942421d12b46449164a7c11e20a0494e3ac10. TLDR: if the user moves the allocated checkout folder and puts
another empty folder at its path during the visible fetch, a rewound/diverged branch or ancestry timeout causes an
ordinary clone into the replacement rather than the changed-destination refusal. Source mechanism confirmed:
Candidate::claim checks device/inode and emptiness at spare_take.rs:289-296,323; the new ancestry refusal at 128-135
returns before verify_destination at 137-139; launch.rs:1809-1814 calls clone_with_repo_cache on false;
launch.rs:1952-1962,1968-2025 uses preparation.cwd with no revalidation. Its verification callback is || Ok(()).
Existing ordinary clone into an allocated empty directory and the fallback tests at launch.rs:5402-5439 establish that
empty directory contents, not retained allocation identity, govern this fallback. The changed rewind test keeps its
destination untouched and does not exercise replacement during fetch. Suggested bucket other: wrong-folder mutation is
established, but no route to losing pre-existing files/security impact is established; normal nonempty targets fail
clone. Confidence: confirmed source mechanism, contract applicability ambiguous. Runtime frequency unmeasured; no
experiment performed. Existing deleted-branch refusal also bypasses the check; this patch adds rewind/divergence/timeout
routes. possible_cover: SPEC.md:3570-3582 (same reviewed revision) excludes replacing a folder inside the guard's
check-then-move window. It may cover this after-initial-check replacement, although the take's promised post-fetch check
(SPEC_impl.md:2492-2498,2512-2515) has not run and fallback clones rather than moving entries. The pinned-tip ledger
archive-rename-move-replacement-directory.md at TRIAGE_OUTCOMES.md:12790-12802 concerns the microsecond
validation-to-rename gap, not a whole credential-bearing fetch before the later destination check. The .farhelm-spares
acceptance at TRIAGE_OUTCOMES.md:17404-17441 concerns the source namespace, not this allocated target outside it. TODO
Planned is empty; BUGS has no matching trigger/consequence; queue INDEX has no corresponding target replacement item;
matching unlanded-plan search found none. FILTER rare safe failure does not cover successful mutation of the wrong
directory. proposed_drop: none from this reviewer; preserve the ambiguous check-then-move acceptance for parent audit.
This event qualifies the initial claim of an unconditional spec violation; the code path remains confirmed.

Provenance: `g269/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
