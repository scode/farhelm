## What this was about

A terminal host could keep its program running after a temporary connection failure, while Farhelm left the agent
terminal or tab unavailable until the supervisor restarted. The session could show an unknown status, refuse attachment
and refuse operations even after the host was usable again. This plan extends the existing recovery path to agent
runtimes that fail after startup and to terminal tabs.

## Things you should know

Recoverable failures now retry while the supervisor runs, without stopping the host or its program. Retry delays start
at a second and grow to a minute. Repeated connection losses wait at least a full minute before each new recovery
episode starts, and successful recovery keeps the accumulated backoff. Tabs retain that history across an agent Restart,
keep their original order, and keep the process-group information needed to close their programs.

While a recovery attempt probes the host, Stop, Restart or Delete can wait for the host connection deadline, about five
seconds. An already open terminal uses the client's existing automatic reconnect and returns without reopening it,
provided automatic reconnect is enabled and another client has not taken control. The existing manual Reconnect control
remains available when automatic reconnect is disabled. This plan changes host recovery, not that client behavior.
Protocol, snapshot and identity failures that this release cannot resolve remain final for that running supervisor.

Recovery verifies that the session still owns the same terminal before replacing it, cannot install a replacement after
an orderly stop, and examines only that terminal's connection endpoint. A failed tab retry preserves the saved host
process identity through later attempts; losing the endpoint alone cannot authorize forgetting a process that may still
run. The normal tab close and cleanup retain responsibility for removing its saved launch record.

## Open questions and possible follow-ups

No product decision is needed. One agreed limitation remains: if an interrupted tab start cannot be resolved during
supervisor startup, Farhelm can still list that tab as unavailable and refuse normal Close until the supervisor next
starts and tries again. The plan explicitly leaves this interrupted-start case unchanged.

## The PRs

- [#2950](https://github.com/scode/farhelm/pull/2950/changes): recover temporarily unavailable agent terminals.
- [#2956](https://github.com/scode/farhelm/pull/2956/changes): recover temporarily unavailable terminal tabs.

## Checks run, reused and skipped

Run for this stack on Linux:

- Combined host recovery, tab and runtime lifecycle selection: 163 tests passed, plus the required fixture-build setup;
  recorder `9e3f55af-b9b1-4333-8eda-34f69911ee3e`. The selection ran supervisor host-terminal and tab units, host
  runtime units, real-host agent/tab recovery end-to-end tests, and the `terminal_host_runtime` integration target. It
  includes all 16 recovery regressions. Pinned nextest, four slots, zero retries and pinned tmux; retained output has no
  runtime `SKIPPED` marker. The 2,131 skipped cases were outside the selection, and the recorder reports complete
  evidence with untruncated output.
- Earlier focused `cargo nextest run -p farhelm-supervisor --lib -E 'test(readoption_)'` runs passed 7, 13 and 15 tests
  as the stack was developed; recorders `434c546e-ba37-40e1-b417-9efa3aac7329`, `0c4281f9-a3ae-4714-bc95-bffb2f003367`
  and `149b1247-934e-4b46-aae4-8fa1c693826d`. The final combined run covers the later corrections and supersedes those
  narrower checks for completion. No runtime test failed during this plan.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`, changed-file dprint
  and changelog format passed. The isolated test-delay checker inspected 403 delays with none missing a rationale. The
  instruction checker passed with zero errors and three existing citation warnings outside this change.

The combined child command was:

```sh
cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e --test terminal_host_runtime -E 'test(/service::core::tests::host_terminals::/) | test(/^service::core::host_tabs::/) | test(/^hosts::terminal::/) | test(/^host_terminals::/) | test(/^host_restarts::/) | test(/^host_tabs::/) | binary(terminal_host_runtime)'
```

No earlier-session evidence was reused. The final runtime run and source checks cover the product source pushed in both
drafts; committing it changed descriptions and identifiers, not the tested files. A fetch and full inspection of
subsequent main changes found only plan bookkeeping and a separate TODO addition, with no functional or specification
interaction requiring another rebase or runtime run.

The full workspace battery, browser, desktop and vendor suites were skipped: this change stays in the supervisor's
terminal-host recovery, with no browser assets, wire format, desktop lifecycle or vendored implementation change. The
selected real-host lifecycle and runtime checks cover its concrete integration risks. No macOS execution is claimed.

## Review gate outcome

Both PRs received the required independent gpt-6.1-sol high review. The first had no findings. Tab review found two
cleanup/scheduling interleavings and a later runtime-identity race; each was corrected with a regression, and the final
review has no findings. The required separate scope reassessment found the design stayed within the accepted outline and
added no unnecessary machinery. Commit and PR text passed separate fresh-context cold reads.

### Landing

Landed with no fixes of its own, together with stop-checkpoint-burst, which was stacked on top of it in the same round.
A terminal or terminal tab whose host became unavailable while the supervisor ran is now retried until the host can be
used again, instead of staying unavailable until the supervisor's next start.

- #2950: retrying agent terminals whose host became unavailable after they ran.
- #2956: the same for terminal tabs.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the grid terminal's link fix (#2949: no product code outside the browser, plus a
supervisor test and protocol comments) and four PRs that only added TODO entries and plans. None interacts with this
plan; two more such PRs (#2962, #2966) landed while the round's checks ran. Rebasing onto main conflicted only in
TODO.md, where #2949 had removed the entry next to the one this plan removes; the landing removed both, and the commits
are otherwise unchanged.

The interaction that mattered is with stop-checkpoint-burst, landed right after this plan: both rewrite the supervisor's
handling of terminal hosts at an orderly stop and when they recover, in the same files, and each executor tested without
the other. A fresh-context review read the two against each other and found no harmful interaction. An orderly stop
still keeps a retry from installing a terminal, for tabs as well as agent terminals. A terminal recovered by a retry is
measured from its host's real snapshot, so the stop's largest-first order and its skipping of small tails treat it
correctly. A stop cancelling a checkpoint never produces the "checkpoint lost" unavailability this plan now retries.
Each plan's new tests hold against the other's code, and the whole Rust workspace ran on the combined stack (below).

#### Things you should know

The whole workspace run had three failures that also failed, the same way, in one run on main without either plan, in
code neither plan changes (the checks below say what they cover).

#### Checks

This round landed readopt-unavailable-hosts and stop-checkpoint-burst, stacked in that order on main `e499b218`. On that
stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`, 403 delays, none without a reason), `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three warnings about
  older reports) and the workspace debug build: all passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, no retries, on a machine with rootless Podman, run `7670d6ed-f91a-4c68-a7ca-38594df84b0f`: 5412 passed, 69 left
  out by the test runner (tests marked ignored), 5 failed. "A restarted supervisor keeps a host tab attachable with its
  screen" is the test the landing fixed (stop-checkpoint-burst's notes). "Shallow pause resumes without reset or replay"
  is a recorded open flake, in terminal code neither plan touches. Three more failed the same way in one run on main
  `e499b218` without either plan (run `03a4af08-649b-48ca-88bb-5c5adf9d199d`, in which the tab test, as a control,
  passed), so they are not this round's. Two cover sessions whose tmux session or server died across a supervisor
  restart, which must stay listed, as exited, while attaching to them fails ("restart gap is decided per session" and
  "restart gap lists sessions without a terminal and attach fails"); in both, the listing no longer names tmux as the
  session's agent terminal where the test expects it. The third, a supervisor unit test, covers an OMP agent's report
  that arrives before its session's pane is known ("OMP publication gap parent report succeeds"), which was refused with
  "the session's foreground pane is unavailable". Each failed fast, within about a second, in both runs. They are in
  neither FLAKES.md nor TODO.md; the landing monitor told the maintainer in conversation, and finding the cause,
  starting with a narrow rerun, is a follow-up.
- After the landing's test fix: `cargo fmt --all -- --check`, `cargo clippy -p farhelm --all-targets -- -D warnings`,
  the test-delay check and `dprint check` passed, and the end-to-end `host_tabs::` and `host_restarts::` modules with
  the supervisor's `core::tests::host_terminals::` unit module
  (`cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E '...'`), run
  `a086105a-8f62-4573-ac14-924962a8644c`: all 83 passed.
- Reused: nothing. Not run: the browser, desktop and JavaScript tests (no user interface code changed) and the doctests
  (no doc examples changed).

Nothing in the executor's report became untrue at landing.
