### What this was about

Claude Code's screen can say that it is waiting for background agents while its input box is empty. Farhelm previously
treated that screen as idle, so a session doing work appeared inactive and did not receive the active-session treatment.

### Things you should know

The Claude screen reader now treats a working-line shape such as `✻ Waiting for 5 background agents to finish` as
anchored working when it appears above the ruled input box. The match requires a numeric count, the `background` word,
at least one noun, and the exact `to finish` ending. Footer hints and a task list without that announcement remain idle.
The reader documentation and `SPEC_impl.md` record this boundary.

A derived Claude 2.1.285 fixture covers the captured screen, including its shell and task hints. Tests cover spinner and
background-wait shapes, malformed and wrapped lines, the full fixture, and the finished-turn replacement that must
return to anchored idle. The Near term TODO entry was removed, and the fix has a changelog fragment.

### Open questions and possible follow-ups

None. The footer and task-list signals intentionally remain idle unless Claude also announces that it is waiting for the
background work to finish.

### The PRs

- [#1478](https://github.com/scode/farhelm/pull/1478/changes) — one draft PR,
  `fix: show Claude as working while it waits for background tasks`.

### Checks run, reused and skipped

- `cargo fmt --all -- --check` — passed.
- `dprint check` on the changed Markdown and changelog — passed.
- `python -B scripts/check-test-sleeps.py` with the pinned isolated parser environment — passed (274 delays inspected,
  none unannotated).
- `python3 releasing/check-changelog.py format` — passed.
- Recorded focused nextest run `cf162888-1740-48f8-8639-96adbc699d26` — passed; the three targeted Claude reader and
  fixture tests passed.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` — passed.
- An earlier focused run failed because the new fixture test used the wrong parsed scenario name; that test was
  corrected and the final recorded run above passed. The failed evidence remains retained privately.
- Full workspace tests, doctests, browser tests, and real-agent screen capture were skipped because the change is
  confined to the Claude screen reader, its fixtures, and documentation; the targeted reader, fixture, formatter, lint,
  and source-delay checks cover the affected behavior.

### Review gate outcome

A fresh-context Opus 5.5 reviewer at high effort found five concrete issues in the first implementation. All five were
addressed: the captured footer hints were restored, the unreachable newline guard and artificial test input were
removed, the rationale for the exact signal was documented, editorial elision was removed from the fixture, and
spinner-only naming was changed to working-line naming. The reviewer then had no further gate run; the final targeted
tests, Clippy, formatting, sleep checker, and changelog checks passed after those fixes.
