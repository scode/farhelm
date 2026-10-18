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

### Landing

Landed on its own in round 87, as #2251, unchanged from what the report describes.

#### What landed since the PR was based, and whether it interacted

Only plan-queue bookkeeping reached main between this PR's base and its landing, and no other plan landed in this round.
The landing rebase was clean and the PR's diff is identical to the one the executor delivered. The landing monitor read
the change against main, and a fresh-context reviewer did the same independently. Neither found an interaction:

- The supervisor uses the new rule in one place only: to tell connected helms when to refresh the session list. It now
  tells them once, the first time it sees YOLO, and that is all the shield display needs.
- Restart, Restart with, Replace and the resume of an interrupted session all start a new run with no observed reading,
  and a supervisor restart starts fresh too, as the updated SPEC.md says. Rename keeps the reading, as SPEC_impl.md says
  it should.
- The hover still reads that YOLO was observed on the agent's status line. That wording stays true while the shield
  stays YOLO after the agent switches back to asking first, so it needed no change.
- SPEC.md has one paragraph about the observed shield, which this PR rewrites; the Claude and Muse docs pages remain
  accurate.

The observed shield first shipped in v0.29.0, so the changelog fragment is a new "changed" entry rather than an edit of
the original one.

#### Fixes made during the landing

None.

#### Possible follow-up

Minor, with no effect on behavior: a code comment about Claude's plan mode could be reworded, since plan mode and the
asking-first modes now have the same effect on the shield.

#### Checks

- Run now: none. The landing rebase changed no code.
- Reused: everything the report lists, at the revision it names, including recorder run
  `290ccec7-fc6c-4ef5-99be-6474c37b0943` (90 passed), the formatting and Clippy checks, and the website build. Nothing
  that reached main since then touches the code this PR changes.
- Skipped: everything the report skipped, for the reasons it gives.

Nothing in the executor's report became untrue at landing.
