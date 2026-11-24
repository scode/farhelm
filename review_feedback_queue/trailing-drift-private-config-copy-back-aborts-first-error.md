# PrivateConfig copy_back aborts on first error, leaving staged temp files and skipping remaining copy-backs and hook integrity check

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

If copying back one rewritten file during drift-test cleanup fails (for instance, because a symlinked dotfile resolves
to a read-only location or setting permissions/renaming fails), cleanup aborts immediately: any staged temporary file is
left behind in the maintainer's real directory, other rewritten files (such as a refreshed login token) are dropped
instead of copied back, and the final check that real hook files were untouched is skipped.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:247`.

In `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:217-253`, `Mirror::copy_back` iterates over
`&self.links` and uses `?` on `fs::canonicalize`, `fs::copy`, `fs::set_permissions`, and `fs::rename` (lines 238,
247-250). First, if `fs::set_permissions` or `fs::rename` fails after `fs::copy` created `staged`
(`.<name>.farhelm-drift-<pid>`), `staged` is not removed and remains orphaned in the maintainer's real directory.
Second, any single failure aborts the loop before later entries in `self.links` are copied back (dropping other
refreshed files when the tempdir drops) and also short-circuits `PrivateConfig::finish` (`private_config.rs:312-319`)
before `mirror.real_leaves_unchanged()` at line 316 runs (and before remaining mirrors in `self.mirrors` run). (Minor
related detail at lines 228-234: when `stamp.is_none()` because the original real entry was not a regular file, e.g., a
broken symlink or non-file entry that the vendor replaced with a regular file in the mirror, the note misleadingly
reports that "the real ... changed too".) Verification: inspect
`crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:217-253, 312-319`. Fix: remove `staged` on any
error after creation, handle per-file copy-back failures without short-circuiting other files (recording a note or
collecting errors after processing all links), and always run `mirror.real_leaves_unchanged()` for every mirror in
`PrivateConfig::finish`.

Reviewer metadata (g13_drift_private_hooks/f3): suggested bucket: other — confined to opt-in maintainer drift test
cleanup.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g13_drift_private_hooks/f3`.
