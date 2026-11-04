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
