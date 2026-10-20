## What this was about

Opening or reconnecting a terminal tab while its session stopped, restarted or deleted could pause typing and unrelated
requests for every session on that host for up to two seconds. The maintainer chose to fix this within about 100 changed
lines.

Tab attachment now refuses immediately when the session is busy stopping, restarting or deleting. Other requests can
continue, and the terminal retries using its existing delay. Accepted attachments still keep the session protected until
the tab is resolved and control is transferred.

## Things you should know

The fix, regression test and comments total 99 changed lines, within the gate. The existing retry delay starts at half a
second and grows to 30-second probes, so a quick refusal does not create a retry loop. No process grace period changed.
The queued feedback was removed and its execution recorded. The working log follows the `farhelm-plan-<slug>-log.md`
rule beside the checkouts.

## Open questions and possible follow-ups

None. The plan stayed within the agreed scope and size.

## The PRs

- [#2409 — keep tab attachment from delaying host input](https://github.com/scode/farhelm/pull/2409/changes), draft.

## Checks run, reused and skipped

Run:

- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'supervisor tab attach, lifecycle claims and tab resolution' --concurrency '4 nextest slots; retries 0' --timeout 600 --tmux required -- cargo nextest run -p farhelm-supervisor --lib -E 'test(tab) | test(lifecycle)'`
  with pinned nextest 0.9.148 and the repository's pinned tmux: 51 passed, 1,117 selected out, no failures or runtime
  substrate skips reported. Recorder run `911232da-8d9d-4581-af61-f25e5357e703`; complete source, report and output
  evidence, with cleanup complete. This includes the updated paused-clock regression: the held session claim produces a
  conflict without advancing time or installing an input route, and becomes available after release.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` passed for the changed crate's production and test
  targets.
- `cargo fmt --all -- --check`,
  `dprint check TRIAGE_OUTCOMES.md releasing/changelog.d/tab-attach-no-wait.md review_feedback_queue/INDEX.md`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py` passed.
- `python -B scripts/check-test-sleeps.py` using the isolated pinned parser environment passed: 304 delays inspected,
  none missing a reason.

Reused for the final PR: the runtime run covered base `3755dab8` plus the final product and test diff. Only queue,
fragment and execution bookkeeping changed afterwards, so its coverage still applies. No failed runtime attempt was
replaced or omitted.

Skipped: browser and UI runtime tests because neither terminal retry policy nor UI code changed; the refusal already
uses the existing delayed reconnect path, which the independent reviewer checked. Full workspace tests and workspace
Clippy were not needed for this supervisor-only admission change; the focused tests and crate lint cover its behavior
and configuration change.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues in correctness, design, idiom,
test quality or the size gate, and checked the final bookkeeping. The PR message passed its cold read after clarifying
the explanation. A separate cold read found the report sufficient for a decision and free of public-repository hygiene
issues.

### Landing

Landed as #2409, with the same content the report describes, rebased onto this round's main (see the end of these
notes). It was the ninth of sixteen plans in this landing round, after the eight listed in the earlier plans' notes,
most recently replace-yolo-shared-question.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the eight plans earlier in this round, and documentation and
plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. This
plan changes only the supervisor, the host-side process that runs sessions. host-entry-session-ids changed how the helm
and the browser identify sessions, but the supervisor still uses its own session IDs, and none of the landed changes
touched the supervisor files this plan edits. Nothing left in the code refers to the waiting time this plan removes. The
change matches the specification's section on waiting between operations on one host. The only conflicts were in the
review feedback queue's index, where each plan removes its own entries; all removals are kept.

#### Fixes made during the landing

None.

#### Possible follow-ups

- Because a busy tab now refuses at once instead of waiting up to two seconds, a terminal that keeps reconnecting while
  its session stops or restarts retries sooner at first: roughly three or four attempts in the first five seconds
  instead of about two. The retry delay still grows to 30-second probes, as the report says. If the extra early attempts
  ever matter, a follow-up could start the delay higher after a busy refusal; I do not recommend one now. This was read
  from the code, not observed in a browser.

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

The report's checks ran on a main from before round 107's session-identity change, so they no longer describe what
landed; the combined runs above supersede them. Nothing else in the executor's report became untrue at landing.
