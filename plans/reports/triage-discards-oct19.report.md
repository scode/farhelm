## What this was about

The maintainer chose to discard four review findings on October 19. This plan closes their queue records; it changes no
Farhelm behavior.

- Keyboard traversal could skip an empty initial prompt. Triage judged this probably fixed by a later control added to
  the launcher, without browser confirmation, and chose to discard it.
- An old status publication could hide the session list's indication that the user stopped an agent, leaving it looking
  as though the agent exited on its own even after refresh. No loss of user work was established. The exact interleaving
  was not reproduced; triage discarded the proposed fix because it exceeded the agreed small-change threshold.
- An agent could ask to assign a quick-launch mark or number key to a template that already held it. If another client
  moved that shortcut to another template while approval waited, the request could succeed while its reply showed the
  current bindings, including that move. Triage judged this consistent with the assignment landing before the other
  client's move, rather than a failed operation reporting success.
- Before a repository is selected, an open launcher could show stale checkout-setup eligibility after an external
  configuration change. Triage accepted a display lag until the next interaction.

## Things you should know

All four feedback files and index entries were removed, and their original assessments and decisions remain in the
triage ledger with completion records. Discarding a finding does not establish that its reported behavior was fixed or
reproduced. The four outcomes share one PR, as agreed when scheduling this plan. No specification, product code, tests
or changelog fragment changed.

## Open questions and possible follow-ups

None identified within the agreed scope.

## PRs

- [#2423: close four discarded triage findings](https://github.com/scode/farhelm/pull/2423/changes) — one grouped
  bookkeeping PR, retained as a draft for the landing monitor.

## Checks run, reused and skipped

- `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` passed after formatting the new ledger lines.
- `python3 scripts/check-agents-md.py` passed with zero errors and warnings. The root instructions require it when
  Markdown files are removed.
- No checks were reused. Runtime Rust, JavaScript, browser, desktop, installer and provisioning tests were skipped
  because this change only removes queue records and marks their execution complete.

## Review gate outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no findings. The cold read of the commit
message passed. Each outcome has zero code, test or specification lines; no size gate was exceeded.

### Landing

Landed as #2423, with the same content the report describes, rebased onto this round's main (see the end of these
notes). It was the fourteenth of sixteen plans in this landing round, after the thirteen listed in the earlier plans'
notes, most recently tooling-checks-oct19.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the thirteen plans earlier in this round, and documentation
and plan bookkeeping. This plan only closes four review findings in the triage ledger and removes their queue files, and
nothing else in the round depends on those records. A fresh-context reviewer and I checked that none of the other plans
reopened or relied on the four discarded findings. The only conflicts were in the review feedback queue's index, where
each plan removes its own entries; all removals are kept.

#### Fixes made during the landing

None.

#### Checks

Every plan in this round was executed on a main from before round 107's session-identity change, so none had been tested
against it. That is why this round ran the whole Rust workspace and the full browser suite rather than a selection. All
of the following ran now, on all sixteen plans of this round stacked together on main, including the fixes made while
landing plans in this round:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and its `--self-test`,
  `python3 releasing/notes-review-page.py --self-test`, `scripts/check-desktop-assets.sh`, and the website build
  (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `40f2b98b-b160-4239-a653-44cd0bcdcf96`: 3948 passed.
  `cargo nextest run -p farhelm-ui --features desktop`, run `d288b68b-249f-4f21-9fe0-d4d6d1c4a738`: 717 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `f82f83f8-67c9-4e07-99ed-467c9830dc9f`: 317 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, run
  `81d76a96-912f-47ed-8e1f-57423edbeb32` (kept): 2183 passed and 14 skipped. Of the skips, 9 are the WebKit clipboard
  cases, because Playwright cannot grant WebKit clipboard permissions; 4 run only against a real, signed-in agent; and 1
  is a pixel capture that runs only on Chromium. One test failed: on WebKit, "model-only number key opens New with a
  temporary composition explanation" in `templates.spec.ts` timed out at its first step, waiting 60 seconds for the
  header's Templates button to settle before clicking it. No plan in this round changes that button or that test, and
  the file's other WebKit tests that open the same dialog passed.
  `npx playwright test --project=webkit-templates -g 'model-only number key opens New with a temporary composition explanation' --repeat-each=20`
  then passed all 20 (run `6c382916-77a1-4ecb-b4ee-a1dae8cac848`). The cause is probably machine load during the full
  run, not something this round broke, but that is not confirmed; on one observation it is not recorded as a flake.
- Skipped: the native Mac app (not runnable here) and the installer (unchanged).

This plan changes no code, so the runtime suites above only confirm the combined stack. The report was written on a main
from before round 107's session-identity change; nothing in it became untrue at landing.
