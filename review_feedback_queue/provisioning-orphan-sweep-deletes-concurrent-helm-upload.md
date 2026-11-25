# Provisioning orphan-temporary sweep can delete a concurrent helm's active upload

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

If two Farhelm helms set up or update the same remote host at the same time, one helm's cleanup of leftover staging
files can delete the file the other is still uploading or about to install. That setup/update fails with a visible error
and can be retried; nothing already installed is harmed.

## Details

**other.** Source inspection; commissioner confirmed the sweep code and checked the spec's ownership section (no
acceptance of cross-helm races). Established consequence is a visible, retryable run failure — fail-closed, no data loss
or security effect. Unverified premise (kept from the reviewer): two helm processes provisioning the same destination
concurrently — per-helm registries/locks verified, cross-helm coordination not found but not exhaustively proven absent.
It would move to high if a single-helm flow were shown to run two sweeps against the same destination concurrently
despite the busy/lock machinery.

`crates/farhelm-helm/src/provisioning/backend.rs:726-804` (`cleanup_orphaned_temporaries`) and `orphan_cleanup_script`
(1732-1740) delete every regular file matching `.{destination basename}.farhelm-tmp-<canonical uuid>` in the
destination's parent — prefix + UUID shape only, with no age/ownership/liveness check (local round-trip equality at
762-770; remote anchored grep at 1736). Called from `install_source` (1226) and `upload_source` (1414). Temporary names
carry a per-plan UUID nonce (`plan.rs:706-710`, minted `service.rs:1850-1855`), so two concurrent runs on the same
destination use different nonces that both match. Within one helm the per-host provisioning lock serializes runs (the
comment at 1408-1413 fixes the within-run ordering hazard); across helm processes nothing coordinates — each helm has
its own registry and locks, and the same destination can be registered in two helms. The outcome is fail-closed: the
victim's next `sha256sum <tmp`, chmod, or mv fails (digest recheck at 1352-1360 / 1521-1533), so no wrong-bytes install
is possible and the installed destination never matches the sweep prefix.

SPEC.md "Ownership during cleanup and provisioning" (~5857-5956) addresses permissions and user-writable install
directories but does not accept or address concurrent provisioning from two helms. FILTER.md "Rare, self-correcting
glitches" considered: the trigger is rare and the whole user-visible consequence is a safely-failing retryable
operation, but the sweep's rm is a destructive action on a non-orphaned file, which the filter's exclusion list names
("a destructive or state-changing action applied to the wrong target"); the match being unclear, it does not match —
recorded. Triage may still accept the race as a design gap (the product assumes one owning helm per supervisor).

Verify: register one SSH host in two helm instances and start Update on both; the later sweep removes the earlier run's
temporary and that run fails at "verifying the uploaded payload" or "finishing the atomic payload install". Fix shape:
an abandonment signal for the sweep (mtime grace is awkward since transfers have no overall bound), tolerate a vanished
temporary by re-uploading once, or record the race as an explicit spec acceptance.

Reviewer metadata (area 5 chunk A, provisioning backend first half): suggested bucket: other; confidence: likely
(cross-helm concurrency premise unverified); proposed_drop: none; possible_cover: none known. Commissioner validation:
sweep mechanism confirmed at the cited lines; coverage searched (queue, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md,
plans/queue) — no match.
