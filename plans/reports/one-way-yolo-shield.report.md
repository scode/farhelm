## What this was about

Farhelm could restore a safer-looking sidebar shield after an agent left a no-checks mode, then miss a later switch back
to bypass. The maintainer chose to keep observed YOLO evidence for the rest of the agent run, so a missed switch cannot
leave that reassuring shield in place.

The supervisor now latches its first definite no-checks reading. A later asking-first mode, an obscured status line, or
an unrecognized screen cannot clear it. The existing parser still recognizes the same Claude, Grok, Muse and Cursor
footers. The specification, agent guide and Cursor guide describe the new rule, and the TODO entry is removed.

## Things you should know

Restart, Replace and resume begin with the stored launch shield; restarting the host's supervisor also clears the
in-memory observation. The shield can therefore remain YOLO after the agent returns to asking first. This is the
requested conservative display, with the same best-effort detection limits as before.

Only the displayed shield changes. Admission, approval cards and permission inheritance still use the stored launch. The
hover wording stays unchanged. No persistent state, protocol change, new parser rule or setting was needed.

## Open questions and possible follow-ups

None identified within this plan's scope.

## The PRs

- [#2251](https://github.com/scode/farhelm/pull/2251/changes): keep observed YOLO shields for the agent run; draft head
  `6cadecd0f0d262f02ffd9cb5b70c35a0e988a3c1`.

## Checks run, reused and skipped

- Passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `python3 releasing/check-changelog.py format`.
- Passed `dprint check` on `SPEC.md`, `SPEC_impl.md`, `TODO.md`, the new fragment and the two changed guide pages.
- Passed `python -B scripts/check-test-sleeps.py` with the isolated pinned parser environment: 303 delays inspected,
  zero without rationale.
- Passed the website's `bun install --frozen-lockfile && bun run build`, including internal link validation.
- Passed recorder run `290ccec7-fc6c-4ef5-99be-6474c37b0943`: pinned nextest 0.9.148 and pinned tmux, four slots, zero
  retries, 90 passed, 1571 selected out. Child command:
  `cargo nextest run -p farhelm-supervisor -p farhelm-ui --lib -E 'test(service::ticker::) | test(agent_kind::screen_reader::) | test(agent_kind::status_line_fixtures::) | test(list::row::)'`.
  The retained output contains no runtime `SKIPPED` marker. This covers the latch/reset/reply regression, footer
  recognition and sidebar shield rendering.
- Runtime and Clippy evidence came from the reviewed working tree on base `c709436a`; it remains applicable to the draft
  head because the only later source edit corrected a normal doc comment to mention supervisor reload. The final
  formatting check ran after that correction. No main changes were rebased into this PR.
- Skipped browser, desktop runtime, full workspace and release gates: no parser or browser rendering behavior changed,
  and the selected supervisor and UI unit tests cover the changed state and its existing display. No CI dispatch was
  needed. No install or live-service action was taken.

## The review gate's outcome

A fresh-context gpt-6.1-sol reviewer at high effort found no behavioral, design, idiomaticity or test-authoring defects.
It found one comment that omitted supervisor restart as a reset condition; that sentence was corrected. The supplied
review included the full test-authoring checklist. The commit and PR wording passed a separate cold read after its
capitalization was corrected. No findings remain unresolved.
