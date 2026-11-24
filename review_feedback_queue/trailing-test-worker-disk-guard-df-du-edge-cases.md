# Test-worker disk guard mishandles negative `df` readings, post-cleanup `df`/`du` failures, and empty-dir action reporting

Reviewed commit: ac5a5004c041e85247889dac7533c83a1b8a408e

## TLDR

On a full ext4 test worker, or if `df`/`du` fails after cleanup (for example when a run is stopped mid-cleanup), the
worker's disk guard can misread negative available space as positive free GB, wipe a healthy tree on a transient second
`df` failure, or falsely emit `FULL BY OTHER WORKLOADS` and page the maintainer.

## Details

**other.** Source inspection. Main reference `scripts/test-worker/remote/env.sh:38-84`.

Four edge-case defects in `scripts/test-worker/remote/env.sh:38-84`:

1. `disk_free_gb()` (line 38) runs `df --output=avail -BG . 2>/dev/null | tail -1 | tr -dc 0-9`. On ext4 when usage
   enters the 5% root-reserved blocks, GNU coreutils `df` prints a negative availability such as `-5G`. Stripping `-`
   with `tr -dc 0-9` turns `-5G` into `5` (misreporting free space as `5 GB free`, and on a >1.1 TB disk where reserved
   space overflow can exceed 55 GB, turning `-60G` into `60` and skipping cleanup and refusal entirely). Clamp negative
   `df` values to `0` instead of stripping `-`.
2. Lines 56-59 guard against an empty initial `free` reading ('Without a reading, cleaning could wipe a healthy tree and
   blaming others would be a guess'), but lines 65 and 73 re-read `free=$(disk_free_gb)` without checking
   `[ -z "$free" ]`. If `df` fails (or is killed by `SIGTERM` during `test-worker.sh stop`), line 67
   (`[ "${free:-0}" -lt 55 ]`) treats empty as `0` and deletes all build output, and line 76 treats empty as `0` and
   prints `disk: FULL BY OTHER WORKLOADS` (with empty `$(disk_used_gb)`) and returns 1.
3. At line 77, leftover build output is detected only via `[ "${own:-0}" -gt 0 ]`. If `rm -rf` fails to delete a build
   directory and `du` also returns empty or `0`, `keep_disk_free()` misclassifies the failure as
   `FULL BY OTHER WORKLOADS` instead of `this checkout's build output could not all be removed`. Check `own_build_dirs`
   directly to detect surviving build directories.
4. At lines 61-75, when `own_build_dirs` is empty on a fresh checkout with `< 55 GB` free, both blocks set `action`
   despite removing nothing, printing `(its build output was removed first)`.

Reviewer metadata (g12_tooling_drift_upgrade_and_worker/l4): suggested bucket: other (maintainer test-worker tooling
diagnostics/edge-case handling; cannot delete files outside `own_build_dirs` and does not affect shipped Farhelm).;
confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g12_tooling_drift_upgrade_and_worker/l4`.
