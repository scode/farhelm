# A single repository's generation-cleanup error aborts startup cache and spare sweeps for all remaining repositories

Reviewed commit: 8dccd62785cf244c7b890ef261c677fb47d0cb44

## TLDR

When a host's supervisor starts up, if cleaning up an old repository cache directory for one repository fails (for
example, because a leftover Git process is still writing inside it), startup cleanup stops immediately and skips
cleaning up caches and prepared checkouts for every other repository on that host until the supervisor restarts again.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/repo_cache.rs:218`.

In `crates/farhelm-supervisor/src/repo_cache.rs:192-219`, `repo_cache::sweep` iterates over owners and permanent lock
files, running `spare_checkouts::sweep` (lines 208-212, which catches and logs per-repo errors) and then
`sweep_generations(&cache, &lock, paths, unused)?` (line 218). Commit 8dccd627 introduces per-generation cache
directories specifically because a Git child can outlive its shim's flock, and `retire()`
(`crates/farhelm-supervisor/src/repo_cache.rs:132-136`) already expects `remove_if_present` to fail in that case and
logs a warning so startup sweep can retry later. However, `sweep_generations`
(`crates/farhelm-supervisor/src/repo_cache.rs:282-284`) propagates any `remove_if_present(&path)?` error (such as
`ENOTEMPTY` when a surviving Git child creates a file while `remove_dir_all` walks an unselected generation) via `?`,
and `sweep` propagates it via `?` at line 218 (and `PreparationLock::open(&lock_path)?` / `lock.try_acquire()?` /
`lock.modified()?` at lines 193-199), aborting both the inner `locks` loop and the outer `owners` loop. Because
`repo_cache::sweep` is the only caller of `spare_checkouts::sweep`, one repository's obsolete-generation removal error
skips cache reclamation, 30-day cache eviction, and spare-checkout cleanup/eviction for all remaining repositories and
owners on the host (and skips remaining `paths` for the same repository). Verification: by source inspection of
`crates/farhelm-supervisor/src/repo_cache.rs:168-221` and `237-287`, plus a unit test with two repositories where an
unselected generation under the first repo fails removal (e.g. unwritable subdirectory) while the second repo has
expired/unselected debris. Fix: log and continue per-path inside `sweep_generations` and per-repository inside
`repo_cache::sweep` (matching how `spare_checkouts::sweep` is handled at lines 208-212), returning `Ok(())` or an
aggregate error only after visiting every repository and owner.

Reviewer metadata (g31_repo_cache_lock_redesign/f1): suggested bucket: other (delays cache and spare disk reclamation
until a later supervisor restart; sessions still start, no security impact or user data loss).; confidence: confirmed
(source analysis).; possible cover: SPEC_impl.md, "Owned checkout admission and lifetime", line 3670 ("A sweep failure
is logged and leaves unused caches for a later startup. Sessions can still start.") may partially cover leaving caches
for a later startup, though it does not explicitly state that a single repository's cleanup failure should abort
unrelated repositories' cache and spare sweeps..

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g31_repo_cache_lock_redesign/f1`.
