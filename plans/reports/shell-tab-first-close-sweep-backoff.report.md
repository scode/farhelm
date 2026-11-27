# Report: an exited tab's failed first cleanup now backs off

## What this was about

When a shell tab's shell exits, the supervisor closes the tab on its own: it sweeps up the tab's processes, ends the
tab's terminal host, sweeps again for anything that outlived the hangup, and removes the tab. #5087 made a cleanup that
fails along the way keep the tab and retry it later, waiting a second at first and doubling up to a minute between
tries, so a cleanup that keeps failing does not run constantly.

One path was left out. If the very first automatic cleanup of an exited tab failed at its first process sweep (before
ending the terminal host), the tab was not put on that schedule, and the supervisor tried again on every pass of its
cleanup loop, about every two seconds, for as long as the sweep kept failing. Each try holds the session against other
operations while it runs. The TODO entry recorded this gap as left over from #5087. When the plan was written you
decided to put that path on the existing schedule, with no cap on how long the retries go on, and to add a regression
test.

What is on main now:

- That first automatic cleanup, when its first sweep fails, puts the tab on the same retry schedule: a second, then
  doubling, at most a minute apart.
- A user's Close (or a session Stop) of a tab whose shell still runs is deliberately left as it was: if its first sweep
  fails, the tab is not handed to the automatic cleanup, so a failed Close never ends a running shell on its own a
  second later. You can press Close again.
- Two tests: one shows the automatic retry now waits for the backoff (with the old code it came about 90 ms after the
  failure, the test's cleanup loop running every 100 ms instead of the real two seconds), and one shows a failed Close
  of a live tab is left alone and that the automatic cleanup still closes the tab normally once its shell exits.
- The specification's paragraph on tab close retries now names this case, the unreleased changelog entry for #5087
  covers it, the supervisor's warning when an automatic cleanup fails no longer says the retry comes on the next pass,
  and the TODO entry is removed.

## Things you should know

- **Retries of this kind now skip one check.** Before, a later automatic try first checked whether the tab's terminal
  host could still be read, and left the tab to the supervisor's reconnect logic if not. A tab on the retry schedule
  skips that check and is cleaned up anyway, as the plan proposed. That is safe here because its shell is known to have
  exited, and it means such a tab no longer waits on a host that cannot be read.
- **A manual Close of a tab whose shell already exited** still gets one immediate automatic retry if its first sweep
  fails, before backing off. That case is rare (an exited tab is hidden from the tab strip) and bounded, so I left it.
- **No new changelog entry.** #5087's entry is unreleased, so it was amended rather than a second entry added.

## Open questions and possible follow-ups

None.

## PRs

- #5139 fix: back off when an exited tab's first cleanup sweep fails (the change, its tests, the spec sentence, the
  changelog amendment, the TODO removal).

## Checks

Run during this plan, through the test recorder:

- Run `21d540cb`, the inventory's "terminal tabs in terminal hosts" selection,
  `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(launch_file_names_round_trip_through_the_parser) | test(sweep_launch_dir_keeps_every_host_record) | test(/core::host_tabs::tests::/) | test(closing_a_tab_whose_runtime_reads_gone_confirms_its_process_first) | test(/^host_tabs::/)'`:
  24 passed, no test skipped for a missing system service. That covers opening, closing and automatically cleaning up
  terminal tabs against real terminal hosts, including both new tests, after the review fixes.
- Run `d33109fa`, the new exited-tab test with the old behavior temporarily put back: it failed as it should (the retry
  came about 90 ms after the failure). The temporary change was removed before committing.
- `cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test
  sleep check (the live-tab test's 1.5-second observation window carries its reason), `dprint check` on SPEC.md and
  TODO.md, and the changelog format check: all clean.

Reused: run `21d540cb` and the lint, format and sleep checks above ran on the change just before the final rebase, which
brought in only plan bookkeeping, so they still apply to what lands.

Earlier runs, superseded: `49cabee0` (the same selection, 24 passed, before the review fixes).

Skipped: the rest of the Rust suites and the end-to-end suites outside terminal tabs, since only the tab close path
changed.

## Review gate

A fresh-context Opus 5.5 review at high effort found no blocker. It confirmed that the automatic cleanup only ever
treats a tab this way when it reads the tab's shell as exited, that the retries lose nothing compared with the first
try, and that Stop, Delete, a manual Close and the tab strip all behave correctly with a tab on the schedule. Its one
should-fix was that the live-tab test could have passed with the automatic cleanup not running at all; it now ends by
letting the shell exit and checking the cleanup closes the tab. Its smaller points were fixed too: two code comments
that overstated the guarantee (they said the cleanup never closes a tab whose shell runs, which was never quite true,
and this change does not alter it), the stale warning text, one shared helper for the three tests that inject a failure,
two test descriptions narrowed, and the spec sentence. The fixes were not reviewed again. Run `21d540cb` ran after them.
