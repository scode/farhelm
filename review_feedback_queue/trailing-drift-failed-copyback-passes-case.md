# A failed copy-back of a refreshed vendor login no longer fails the drift case

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When a live agent drift check ends and the agent had refreshed its login inside the test's private copy of its settings,
the check copies the new login back to the maintainer's real settings. If that copy fails (disk full, permissions, a
broken dotfiles link), the maintainer is left with a stale login and may be signed out of that agent, yet the drift
run's summary reports the case as passed. The only trace is one line in the scrolling console output.

## Details

Concerns 39f269bc0b.

Locations: `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:298-325` (`copy_back` turns a
`replace_real` error into a note) and `:595-623` (`PrivateConfig::finish` returns `Ok(notes)` when the hook check
passes); `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs:873-879` (notes are only printed as `drift: ...`),
`:885` (`after_cleanup`) and `:906-926` (`run_case` writes the verdict from the case result alone).

Before this commit `Mirror::copy_back` used `?` on canonicalize, copy, set_permissions and rename, so a failed copy-back
made `PrivateConfig::finish` return `Err`, and `after_cleanup` (`drift/mod.rs:885-903`) turned the case into "failed"
with the error as its detail. The commit made `copy_back` infallible (`fn copy_back(&self) -> Vec<String>`), pushing a
note per failed file ("... copying it back failed: ...; the case's copy was dropped"), and `finish` returns `Ok(notes)`
unless the real hook-file check fails. `with_drift_supervisor_using` prints each note with `eprintln!("drift: {note}")`
and returns the run's own result, so `run_case` writes `{"verdict": "passed"}` and the summary table of
`scripts/run-harness-drift.py` shows PASSED for a case that just lost a refreshed vendor credential. The commit's stated
goal (keep copying the remaining files, do not skip the hook check) does not require downgrading the failure:
`copy_back` can keep iterating and still let `finish` report an error after the loop.

Verify by reading `copy_back` and `finish` at the tip, and the pre-commit version via
`git show 39f269bc0b~1:crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs`. Fix: have `copy_back`
return notes and failures separately (or mark failure notes), and have `finish` return `Err` listing them after the hook
check (the hook-check error first, as now), so `after_cleanup` fails the case. Keep "real one changed too" as a note if
that is the intended non-failure; the same question applies to that pre-existing note if the maintainer wants a lost
refreshed login to be visible in the verdict.

Bucket: other. The consequence is a vendor login that may need to be redone, inside maintainer test tooling; the bucket
rules keep harm confined to maintainer tooling out of highest unless it is catastrophic, and a re-login is recreatable.
Not highest: it affects only the opt-in drift suite run by a maintainer, and the lost item is recoverable by signing in
again.

Confidence: confirmed from source.

Possible cover: `docs/harness-drift-tests.md` lines 116-120 say the case "prints a drift: note naming any it could not",
which describes the note but not whether the verdict should fail; the module doc at `private_config.rs:18-22` likewise
says "names it in a note otherwise, including when its copy-back fails". The commit may have intended note-only
behavior; if so, this is a deliberate visibility tradeoff for the maintainer to confirm.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g02 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
