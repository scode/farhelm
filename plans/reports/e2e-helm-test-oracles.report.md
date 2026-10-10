## What this was about

Ten tests could report success without observing the regression they claimed to catch. You chose to fix these test gaps
together, with a complexity gate that leaves any substantially larger change for triage.

## Things you should know

The three Stop cases protect distinct cleanup promises: Stop finds a surviving background process after its agent and
terminal pane have died; follows a session-marked parent to a child that does not carry that marker; and catches
descendants that keep forking during shutdown. These cases disable systemd scopes and check the saved launch record, so
a scope kill cannot substitute for the portable cleanup being tested.

The interrupted-installer case proves staging is removed when interruption happens after cleanup traps are installed.
Host provisioning checks that Farhelm and tmux are installed with their own distinct executable bytes. The upload case
requires the stall deadline to expire while empty chunks are still arriving, and the agent-relay case requires
concurrent replies arriving in reverse order to remain matched to the right requests.

Two listing cases protect session visibility: a successful list poller must continue through Stop while the Stop
annotation remains visible, and a stale listing begun before Create must not erase the newly created session. The latter
holds the next listing back so a later correct refresh cannot hide a stale publication. The final case sends a malformed
supervisor reply and requires the helm to promptly fail pending requests and close that connection rather than leave
requests hung.

All ten fixes stayed within the existing tests and fixture facilities. No product behavior or specification changed. The
tests now establish the relevant fixture readiness, traffic, reply ordering, or successful concurrent observation before
accepting a result.

## Open questions and possible follow-ups

None. No outcome tripped its complexity gate or was dropped.

## PRs

- [#1818 — expose false passes in helm and end-to-end tests](https://github.com/scode/farhelm/pull/1818/changes), one
  draft PR for all ten outcomes.

## Checks run, reused and skipped

All runtime commands used `python3 scripts/record-test-run.py`. Rust selections used
`cargo nextest run -p farhelm-helm --lib -E '<exact changed cases>'` and
`cargo nextest run -p farhelm --test e2e -E '<exact changed cases>'`; complete selectors are retained with the run IDs
below. The installer command was `bash scripts/test-install-sh.sh`. Positive runs used `--kind development`; temporary
regression runs used `--kind repetition` with the same corresponding selection.

The exact three helm tests passed on Linux: runs `f28fc675-77a1-4ec2-bfe0-73a199df7d86` (two cases) and
`7d50adb8-562d-41cc-9864-4cb0259259b7` (the stale-refresh case). The first command used the wrong module name for the
stale-refresh filter; the separate corrected selection closed that coverage gap.

The six changed end-to-end cases passed with pinned tmux and nextest, four global slots and zero retries: run
`fdee24f9-9378-427e-a85b-a14b8d282e43`. All selected cases executed; the three process-cleanup cases asserted actual
unscoped launches. The installer fixture suite passed 516 checks with zero failures: run
`b035cf1f-ff23-41d6-aacb-109620062613`.

An initial Rust setup attempt, run `5096ae5c-fdc6-44fb-b935-f8d408b2bcc8`, failed before compilation because the
container's default Rust tool directory was unwritable. The corrected child invocation supplied the mounted tool
directories. This was a setup failure, with no test result. The evidence is retained privately.

Temporary regressions were rejected at the intended assertions: wrong tmux executable bytes, a stale refresh erasing the
new session, and an ignored malformed frame leaving a request hung (run `055e118b-af3e-41f0-a2af-193eeae345b3`, three
expected failures); empty chunks extending the upload deadline (run `57ed1664-6646-496a-8866-f88231df184a`, one expected
failure); and SIGTERM bypassing installer cleanup (run `56c4b5b4-f512-46df-90d6-0dee6bf379c9`, exactly one failure for
leftover staging). The installer mutation run performed one extra optional installed-forwarder ShellCheck check, which
passed. An initial payload mutation used an unimported enum name and failed compilation before tests (run
`639314e8-eb1d-4a9b-88b3-fd265ef54ff3`); it was corrected for the three-case run. These deliberate failures are retained
privately as negative controls, not product failures.

Every temporary product edit was restored byte-for-byte. The positive runs above cover the unchanged test diff over main
`fdd03c9a` and remain applicable after that restoration and completion-record edits. Additional destructive Stop
mutations were omitted because disabling process cleanup would create unmanaged survivors or a fork storm; the tests
directly assert saved unscoped launches. The relay test observes request arrival and reversed answer arrival, and the
listing observer must complete all 200 successful polls; no new product mutation machinery was added for those
contracts.

`cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, Bash syntax, ShellCheck on the changed
installer test, and formatting of the ledger and feedback index passed. The isolated source-only delay checker inspected
283 delays with zero missing rationales.

Every intervening main diff through `054c3798` was read. They changed queue claims, landing states and delivered reports
only. The conflict-free rebase preserved executable code, specifications and fixtures, so the successful commands and
source review remain applicable. The later PR URL edits affect completion records only and passed formatting.

The full workspace runtime suite and browser suites were skipped: these changes affect ten specific Rust and installer
test oracles, and the focused cases exercise them directly. No browser integration behavior changed. No macOS execution
was claimed.

## Review gate

A fresh gpt-6.1-sol reviewer at high effort found no actionable issues in the nine changed test/helper files, with the
complete test-authoring checklist supplied. Its source-only review examined all ten distinguishing assertions and the
shared listing hold's ordering. Runtime results and final bookkeeping were assessed separately by the executor. The
wording cold read and independent resume reconciliation also passed.

Implementation stayed local under no-workhorse mode. Native tools did not expose actual model identity, effort or usage
counters; the named models are requested routes. Private routing evidence is retained under session UUID
`5c9810ab-dcb4-4a62-9a79-bfb239f2d810` and the plan's working log beside the checkouts.
