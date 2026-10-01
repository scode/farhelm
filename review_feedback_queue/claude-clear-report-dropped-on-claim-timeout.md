# A Claude `/clear` report can be dropped, leaving the pre-`/clear` conversation as the Resume target

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

After `/clear` in a Claude session, Claude's hook tells the supervisor the new conversation's ID. If that report arrives
while something else is briefly holding the session's capture lock for a second or more (a stalled database read on a
busy disk, or a second Claude report still being processed), it is refused. Nothing resends it, and the scan is not
allowed to change a hook-reported ID. The session keeps offering the conversation the user cleared, and a Resume restart
reopens it. This lasts until Claude's next `SessionStart` event (a compaction, another `/clear`, or a resume inside
Claude); a short session may never get one.

## Details

Source: gap-filling review pass, 2026-09-30. A gap reviewer left an ordering lead untraced; a first trace refuted the
lead as stated but raised this related case; a second, independent trace confirmed it.

Reviewer's confidence: likely. Unverified premise: that a Claude session's capture claim is actually held for 1 s or
more on a real host. No reproduction was run; every other step of the chain is confirmed in code.

Reviewer's bucket suggestion: highest (same consequence class as `claude-scan-claims-foreign-record.md` and
`claude-resume-template-selector-collision.md`; the trigger is rare).

Possible cover for triage to check: SPEC.md "Healthy local filesystems" is a weak candidate: it accepts hangs and I/O
errors, but not ordinary fsync latency on a busy but healthy disk. SPEC_impl.md:1553-1563 fixes "at most a second of
waiting on the report path" but does not accept losing the report. SPEC.md:1060-1070 says "a legitimate reported
`/clear` switches the current identity". No FILTER.md filter applies (durable wrong-conversation Resume on a first-class
harness); TODO.md `## Planned` and TRIAGE_OUTCOMES.md contain nothing related.

- **The refusal.** The report path waits for the capture claim for at most `CAPTURE_CLAIM_WAIT` = 1 s
  (`crates/farhelm-supervisor/src/service/core.rs:13842`). On timeout it returns `Conflict` without writing
  (core.rs:14026-14036, `claim_before` at core.rs:1735-1762). The hook makes exactly one round trip and never retries
  (`crates/farhelm/src/hook.rs:650-700`).
- **Nothing converges it.**
  - Claude's injected settings hook only `SessionStart` (`crates/farhelm-supervisor/src/agent_kind/mod.rs:1528-1540`),
    so no later prompt or stop event re-announces the current ID.
  - The report-only refresh pass copies the stored record into memory and nothing else
    (`crates/farhelm-supervisor/src/service/capture.rs:820-890`). For Claude, `refresh_reported_capture_claimed` returns
    `Ok(true)` without doing anything (core.rs:6149-6153; `refreshes_reported_capture` is false for Claude at
    agent_kind/mod.rs:2415-2423).
  - The Claude scan write is write-once, `captured_conversation IS NULL`
    (`crates/farhelm-supervisor/src/store.rs:4903-4906`), so a hook-reported row can never be replaced by a scan.
  - The doc comments at core.rs:1731-1733 ("the reporter retries on its next lifecycle event, and the refresh pass
    converges the row meanwhile") and core.rs:13840-13841 are false for Claude (and for Pi and Goose).
- **Who else holds the claim for a Claude session.** The report-only refresh loop (capture.rs:831-841, run by every
  capture pass: the 2 s ticker or the 3 s drain, capture.rs:490-515 and :968), which holds it across one `store.session`
  read; and a concurrent Claude report for the same session, which holds it across the tmux lookup, the process walk and
  the write (`service/core/vendor/claude.rs:48-62`, core.rs:14211-14253, store.rs:4967-4991). Lifecycle paths return
  before claiming for Claude (core.rs:6119-6135); scan passes use their own lock.
- **How the hold reaches 1 s.** The database is a single connection behind a std mutex on the blocking pool
  (`crates/farhelm-supervisor/src/db.rs:126-143`), with SQLite's default journal and fsync settings (store.rs:87). An
  fsync stall under heavy writeback (a Rust build on the same disk, say) can keep one row read waiting a second or more
  while the refresh loop sits on that session, and the `/clear` report has to arrive exactly then. The other route is
  two `SessionStart` reports within a second (`/clear` right after startup, or twice quickly) while the first is slowed.
- **Why the bound is what loses it.** Without the bound the report would still land late: its handler runs inline on the
  connection and is not cancelled when the hook disconnects (`service/connection.rs:564`).

Verify: hold `capture_locks` for a Claude session id past 1 s, send a `SessionStart` report with a new ID, and assert
`Conflict`; then release the claim, run a capture pass, and assert the row and the Resume offer still name the old ID.

Fix shape: on the legacy overwrite path keep the reply bounded but not the admission. On timeout, let a detached task
keep the same queued wait and apply the write when it acquires the claim (the tokio mutex is FIFO, so arrival order is
preserved). Or remove the bound on the legacy path, since its peers only ever hold the claim briefly. Correct the two
doc comments either way.
