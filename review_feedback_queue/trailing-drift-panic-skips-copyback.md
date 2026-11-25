# A panic in a drift case skips copy-back of the agent's refreshed files without a note

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

If a live drift check for Grok or Antigravity hits an internal assertion while the agent is running, the cleanup that
copies the agent's refreshed login back into the maintainer's real configuration never runs, and nothing reports that it
was skipped. The maintainer can find themselves signed out of that vendor with no hint why.

## Details

Pre-existing. b784617a19 makes `with_drift_supervisor` the shared teardown for every category, and a later commit in the
range (39f269bc0) wrote a module doc promising that every unrecovered replaced file is named in a note. The same shape
existed in the pre-change `execute()` (the expect was inside the async block before `finish`).

Main reference `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs:856-880` (`run(...).await`, then teardown and
`config.finish`). `with_drift_supervisor_using` awaits `run(...)` and only then does `accepting.stop`, `state.close` and
`config.finish`. Rust panics unwind through the await: `state` (TestDir) and `config` drop, `TestDir::drop` ends the
hosts (`crates/farhelm-teststate/src/lib.rs:803-815`), and `config`'s tempdir is removed with the mirror, but
`PrivateConfig::finish` is never called, so neither `copy_back` nor `dropped_without_shutdown` runs. Panic sites
reachable inside `run`: `base.rs:100` (`.expect("accepted Turn 1 has resume argv")`, after a successful Turn 1), plus
any assert or expect in later category modules that reuse this fixture. The module doc of `private_config.rs` (lines
18-21) promises a note for every replaced file not copied back.

Verify by reading the cited lines. Fix: run `finish` from a drop guard (a small struct holding `&PrivateConfig` and a
"stopped" flag that runs `finish` in `Drop` unless disarmed), or wrap `run` in `AssertUnwindSafe(..).catch_unwind()`
(`futures::FutureExt`) and resume the panic after teardown.

Bucket: other (maintainer test tooling; a lost vendor login is recoverable). Not highest: maintainer tooling only.

Confidence: confirmed for the control flow; whether a panic actually happens after a vendor rewrote a file by rename is
situational.

Possible cover: none known.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g01 record f3; claude-opus-5-5
reviewer, source analysis only, nothing executed.
