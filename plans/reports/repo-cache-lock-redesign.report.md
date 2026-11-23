# Repository cache recovery

## What this was about

A session can fail during checkout if an interrupted Git operation leaves the host's repository cache stuck. Clearing
the locks directly could interfere with Git processes that survived the interruption. The queue decision approved a
separate path for each replacement because those processes can still reopen files at the original path.

If preparing the selected cache fails, Farhelm tries once in a fresh directory. It switches to the replacement after Git
fetches successfully. A failed retry leaves the previous cache selected, so an unavailable remote does not cost the host
the objects it already has. Checkouts and prepared spare checkouts can continue using their copied Git objects after the
cache is removed.

## Things you should know

- The first checkout for each repository after upgrade fetches a fresh cache before retiring the legacy one. It can take
  longer even if the legacy cache was healthy. If the fetch fails, Farhelm keeps the legacy cache on disk.
- Startup removes unfinished replacements and expired caches while respecting the per-repository lock. This includes the
  earlier fix for an expired legacy cache left as a plain file. Background Git cleanup leaves replacement to the
  checkout path.

## Open questions and possible follow-ups

None.

## PRs

- [#4697](https://github.com/scode/farhelm/pull/4697/changes) recovers checkouts and migrates older caches, including
  the startup cleanup fix from the earlier round.

## Checks run, reused and skipped

- On `8a78e870`, 126 selected supervisor tests passed with no runtime skips in recorded run
  `c1832d5b-ab2f-4c49-ac1d-e7e090134599`, using
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^repo_cache::|^launch::|^spare_checkouts::|^spare_build::|^service::spare_builder::/)'`.
  The cases cover recovering from packed and loose ref locks, preserving the cache on an unavailable remote, independent
  checkout objects, startup reclaim skipping held locks, and background consolidation taking the same lock and
  compacting packs.
- On `8a78e870`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo fmt --all -- --check`, and the test-delay checker passed. The tests and Rust checks still apply after rebasing
  onto `afe81073`. The incoming Codex credential changes affect agent arguments and environment. The cache patch,
  checkout preparation inputs and Git child setup stayed the same.
- Changed Markdown formatting, changelog format and `git diff --check` passed again after the rebase.
- The full workspace runtime suite was skipped because the selected tests cover the affected behavior. Runtime
  validation ran locally on Linux, so it does not cover macOS filesystems or the Git shipped with macOS.

## Review gate

The required fresh source review found two issues. An interruption during expiry could leave a half-deleted legacy cache
at its original path, and Git failure diagnostics could omit the directory Git opened. Both were corrected and rechecked
by the source reviewer. A separate cold read checked the commit and PR wording.
