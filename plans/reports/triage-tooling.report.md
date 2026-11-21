## What this was about

This plan resolved 40 triage records about how Farhelm is built, tested and documented. The triage directed us to fix
maintainer tooling when a documented command crashes, test or release evidence is lost or assigned to the wrong run, a
genuine result becomes a harness failure, or private data can enter a fixture. It also approved specific test and
wording corrections.

Twenty records resulted in fixes or corrections. Seventeen were closed under the triage criteria, two became follow-ups,
and one became moot when the old desktop smoke command was removed. Each archive entry records the finding, the decision
and its draft PR.

## Things you should know

- An agent screen capture now scrubs private proxy addresses even when their credentials contain punctuation or the
  address uses IPv6. The fixture check enforces the same privacy rule separately. Capture startup and shutdown preserve
  recovery information when cleanup cannot be confirmed. Capture launch and recovery use the same resolved executable
  path, including paths with spaces.
- A test launched by another recorded test can now keep its failed run under the normal evidence root after the parent
  removes its temporary directory. A worker run reports a failed evidence copy in its exit status. The command's
  streamed summary identifies the run even if the copy fails.
- An empty heading can no longer pull text from the archived flake ledger into the previous entry or invalidate its
  Cause. Both default ledger discovery and two explicit ledger paths are exercised, with separate counts for each class
  and for usable runs when one ledger is missing.
- The plans landing checks now start on Python 3.14. The eight remaining planning findings concern question routing,
  bookmark handoffs or how long plans wait. None established a command crash or test or release evidence loss under the
  triage decision.
- A release brick test now uses the longer checkpoint allowance for both the first attempt and its automatic retry after
  a pre-action provider refusal. For example, a 2,400-second allowance leaves 2,330 seconds after ten seconds of work
  and a 60-second delay. A remote update deferred before sampling now keeps its unavailable verdict even when the
  previous checkpoint saved continuity evidence.
- CentOS validation can install a missing compiler pin with older Rustup, and Mac devbuild can read that version's
  two-line toolchain output. The Podman test payload build has its own Cargo output inside the checkout, so another
  checkout sharing the ordinary Cargo cache cannot substitute its executable after the build. This uses a separate build
  cache for that payload.
- The screenshot notes now describe the current Linux hero refresh, the size floor for the Templates button crop, and
  the staged directory used by the template-search shot. The triage closed the capture diagnostics, readiness checks and
  regenerable demo preview rollback findings. It also closed the older installer advice and re-cut announcement
  navigation items. The source review inspected the published button, template-search and group-membership images.

The plan crosses into supervisor fixture validation for the Rust privacy guard, the planning system for the landing-test
loader, and the release workflow for the brick-test coordinator. The brick-test work builds on the landed guard-oracles
plan and covers retry scheduling and receipt ownership. The stable re-cut finding crosses the helm announcement link and
website release notes. The screenshot prose follows the landed targeted-capture work. The hook installation plan and
these fixture privacy checks use different code, and the test worker changes are separate from Tensorlake’s runner.

## Open questions and possible follow-ups

Both follow-ups are recorded under Near term / Plan follow-up in TODO.md.

- **Protect a test worker's checkout before staging.** A second request can still write to a worker's checkout while the
  first request is testing it, then get refused by the execution lock. It can also fetch an earlier request's artifacts
  from reused directories. The follow-up should protect the source from the first copy through execution, give each run
  its own evidence ownership, and preserve both cached builds and detached runs.
- **Keep deflake evidence tied to the selected revision's prerequisites.** The current sweep can ask an older recorder
  to test with system tmux even when that revision required a pin. Older revisions also have different test batteries.
  The follow-up should establish which historical bases the current tool supports and enforce their own prerequisites
  and batteries, or refuse incompatible bases before running tests.

## Pull requests

- Fixed scrubbing proxy credentials containing multiple at signs, fixture checks for private IPv6 proxies, and the
  fixture guard's handling of neighboring text with an at sign.
  [#4491](https://github.com/scode/farhelm/pull/4491/changes)
- Fixed capture executable path resolution, child cleanup after startup metadata fails, and retention of recovery files
  when stop fails. [#4494](https://github.com/scode/farhelm/pull/4494/changes)
- Fixed nested failed-run evidence retention and worker copy status, and corrected the runtime recorder guidance.
  Deferred ownership of the worker's checkout before staging.
  [#4496](https://github.com/scode/farhelm/pull/4496/changes)
- Fixed empty flake-ledger boundaries, class-count assertions, valid run-count assertions in partial reports, and CLI
  coverage of both ledger selection paths. [#4498](https://github.com/scode/farhelm/pull/4498/changes)
- Fixed the landing-test import. Recorded eight no-change dispositions covering deferrals carrying over to a replacement
  question, stale answers clearing a newer question, a PR closure failing to wake an executor, old cleanup moving a new
  holder's bookmarks, overdue protection aging out, passes counted before submission, history pages shifting when main
  moves, and passes counted while a dependent was held. [#4499](https://github.com/scode/farhelm/pull/4499/changes)
- Fixed the brick test's shortened retry budget and its reuse of stale continuity filenames. Recorded no-change
  dispositions for advance tmux advice on an older selected release and the release-notes anchor after a stable re-cut.
  [#4502](https://github.com/scode/farhelm/pull/4502/changes)
- Fixed the CentOS compiler bootstrap, Mac devbuild's multiline Rustup output, and packaging a different checkout's
  Cargo binary for Podman tests. Recorded no-change dispositions for the devbuild state path comparison and the retired
  Python environment’s ignore entry. [#4503](https://github.com/scode/farhelm/pull/4503/changes)
- Corrected one screenshot documentation record covering the Linux hero refresh, Templates button crop and
  template-search staging. Recorded no-change dispositions for the capture diagnostic, cleanup guard, drawn group
  placement, group wait budget and demo rollback. Discarded the retired desktop smoke path finding. Deferred enforcement
  of older deflake prerequisites. [#4506](https://github.com/scode/farhelm/pull/4506/changes)

## Checks

- `python3 -B scripts/test-capture-agent-screens.py` passed five synthetic cases for fixture privacy and nine after the
  capture lifecycle changes. Pinned
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^agent_kind::screen_fixtures::/) | test(/^agent_kind::status_line_fixtures::/)'`
  passed ten fixture checks. Runs `43b32793-2598-4132-8bea-a279f524bc49`, `8d10265d-076c-425b-a7aa-fdd337e6b8ee` and
  `3301ec65-1a7b-4f03-bba5-5de908a35d42`.
- `python3 -B scripts/test-record-test-run.py` passed 59 checks. `deno run -A <worker fixture>` passed three local
  stand-ins for worker and copy behavior. Runs `ed29fe1f-9441-4a9f-9ad6-f1012c361751` and
  `d37f7257-1013-4703-acbc-43676bf5f79c`.
- `python3 -B scripts/test-summarize-test-runs.py` passed six CLI checks and
  `python3 -B scripts/test-test-run-summary.py` passed eight classifier checks. Runs
  `7a1d3def-dde8-4fde-9bb4-0fddc6bbabbb` and `7449926a-716a-4fd1-b863-648e8278ee8b`.
- `python3.14 -B scripts/plans-landing/test-landing.py` and the same command on Python 3.15 each passed all 13 checks.
  Runs `eff70c32-f156-4b31-aae6-5898ab5c2fdb` and `d91a39d9-7c62-46ac-bdfd-2fb4b3a454ca`.
- `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` passed all 280 portable brick-test checks
  with simulated guests. Run `11e8d120-750a-4fc2-99e4-6beb050f2b71`. The final CLI wording was checked with
  `python3 -B releasing/mac-vm-test/brick_test.py test --help`.
- `deno run -A <build tooling fixture> <fixture directory>` passed three local stand-ins for the missing pin, both
  Rustup output forms and a competing Cargo output writer. Run `61f23e63-18e6-41c6-9d6d-fe971ea04d62`.
- Changed shell scripts passed `bash -n` and `shellcheck`. Changed Markdown, changelog fragments and instruction links
  passed `dprint check`, `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`. The
  instruction checker repeated an existing warning about an unrelated archived heading.
- The rebase reused tests of the code published in the original draft revisions `5f5da8ab4953` (#4491), `8dda8e473caf`
  (#4494), `8b6dc6216d42` (#4496), `397a56e04aaf` (#4498), `bb8ef2a978e0` (#4499), `90e04da51bd9` (#4502) and
  `e4e5c06fcca5` (#4503). The code and fixture diffs are identical after rebasing. Main's GUI session and terminal-host
  changes affect session UI, terminal behavior and its tests. The queue, archive and TODO updates intersected this work
  and were reconciled separately. Formatting, receipts and draft ancestry were checked again.
- The focused fixtures exercised the conditions behind the fixes and allowed controlled failures, so the plan skipped
  live workers, release validation, new native captures and real provisioning. The full product suite was outside the
  tooling scope. The documentation-only last PR used static checks.

## Review gate

Each of the eight drafts passed an independent source review and a cold wording read. Scope reviews kept the
screen-fixture privacy checks independent and the summary fix within its existing ledger parser. The worker staging and
historical deflake findings went through the complexity gate. The draft descriptions and archive receipts account for
all 40 records.

### Landing

#### What landed with it

The plan's eight PRs merged with a landing fix, #4511. No other plan landed in this round, and main had gained nothing
since the executor based the plan. A script checked that all 40 findings left the queue and the open ledger and that
each decision moved to the archive. An independent reviewer read the eight PRs against the triage decisions. Its
findings:

- **Privacy guards.** The fixture privacy checks in the capture script and in the Rust fixture test agree on every new
  case.
- **Release and brick-test tooling.** No guard is weakened.
- **Instruction files.** They match the code, and the validation guide and the full-test-run list are updated for every
  changed script.
- **Tools the landing relies on.** The recorder, the test worker's script and the landing scripts behave the same for an
  existing caller, with two exceptions. A recorder run started inside another recorder run now keeps its evidence
  instead of deleting it. And the script that runs a test on the test worker and copies its evidence back exits 1 when
  the remote run passed but the copy failed.

#### Fixes made while landing

One, in [#4511](https://github.com/scode/farhelm/pull/4511/changes). Capturing agent screens spends real vendor turns.
This plan's #4494 made an unconfirmed stop of the capture's terminal host raise an error, so a capture whose host could
not be confirmed stopped lost everything it had captured. The error escaped before the screens were copied into the
fixture tree, the scratch directory holding them was deleted, the remaining agents were skipped, and the command ended
in a traceback. Now the unconfirmed stop is reported, with the host's message naming the run directory it kept. The
screens already captured are written, and the run exits nonzero. A new test covers it.

#### Checks

The plan changes maintainer tooling (agent-screen capture and its privacy guards, the test recorder and its summaries,
the test worker's script, the landing scripts' tests, brick-test tooling, build scripts), one Rust fixture test,
instruction files, changelog notes, TODO.md and the review queue and triage ledger. Nothing that ships changed. The
landing ran each tool's own tests locally and the Rust parts on the separate arm64 Linux test worker.

On the plan as delivered:

- `python3 -B scripts/test-record-test-run.py`, `python3 -B scripts/test-summarize-test-runs.py`,
  `python3 -B scripts/test-capture-agent-screens.py`, `python3 -B scripts/plans-landing/test-landing.py`,
  `deno check scripts/plans-landing/idle-watch.ts`, `python3 -B deflake/test-deflake.py`,
  `python3 -B scripts/test-gui-e2e-smoke.py` and `python3 -B scripts/test-docs-shots.py`, locally under Python 3.15, run
  directly rather than through the recorder since they are the tools' own unit tests: passed.
- The brick-test harness's unit tests (`python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`),
  locally (recorder run `8f4f4429-c82a-4595-ac72-aa2f327e45f2`): passed.
- `cargo nextest run -p farhelm-supervisor --lib` selecting the agent screen and status-line fixture tests, on the test
  worker (recorder run `0e5276d8-3640-4769-b0c7-3ec6b01628b5`): passed.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker,
  not through the recorder: passed, with no errors in the output.
- `shellcheck` on the test worker's script and remote scripts and on the three build scripts, `bash -n` on the build
  scripts, `python3 -m py_compile` on the driver's prune script, `cargo fmt --all -- --check`, `dprint check`,
  `python3 scripts/check-agents-md.py` with its `--self-test`, and `python3 releasing/check-changelog.py format`,
  locally: passed.
- A script comparing the deleted feedback files, removed index lines and archived ledger entries: all 40 match.

After the landing fix (#4511):

- `python3 -B scripts/test-capture-agent-screens.py`, locally: all 10 passed, the new test among them.
- `python3 -B scripts/agent-tui.py self-test`, on the test worker with a freshly built CLI (recorder run
  `90d2ce17-d8fc-4624-a336-c4757d95e2af`): passed. It is a real start and stop with no vendor turns, which shows a
  normal stop still succeeds and cleans up now that an unconfirmed one fails. The instructions ask for it after changing
  the tool, and the executor ran only the mocked tests.
- `cargo fmt --all -- --check`, `dprint check` and `python3 releasing/check-changelog.py format`, locally: passed.
- `python3 -B scripts/test-test-run-summary.py` (the summary's classifier tests, for the flake-ledger fix), on the
  landed main under Python 3.15: passed. The landing tests passed under 3.15 too; the executor ran them under 3.14.

Skipped: the executor's two one-off stand-in fixtures for the evidence copy and the build tooling, whose behavior the
scripts' own tests above cover; a real agent-screen capture, which spends vendor turns and runs only when asked; the Mac
brick test itself; the workspace suite, since the only Rust change is a fixture test that ran above; CI, which runs only
on demand.

#### Things you should know that the report does not say

- Unlike what the report's opening implies, five of the outcomes were fixed where the triage decision said to discard
  them or leave the code alone. Their archive entries say only "Fixed":
  - a capture that failed while starting could leave an agent process running;
  - a test of the test-run summary's archive handling went around the summary command;
  - two of the summary's tests had assertions too loose to catch the problem they were for (two outcomes);
  - the summary could count a flake on the wrong side of its ledger's boundary, which arguably is evidence being
    misattributed.

  The changes are small and harmless, and reverting them would only be churn. Accepting them at review is enough.
- When the remote run passes but copying its evidence back fails, the test worker's script exits 1, the same status as a
  failed remote command. A caller has to read the error text to tell them apart. A status of its own would be clearer;
  it is not in TODO.md, so ask for an entry if you want it.
- The Podman test payload now builds in a folder of its own in each checkout and on the test worker, so the first Podman
  test run after this lands builds it from scratch.
- Older, not caused by this plan: neither privacy guard treats link-local addresses (169.254.0.0/16) or the
  100.64.0.0/10 range tailnets use as private, so a captured screen showing such an address would reach a committed
  fixture. It is not in TODO.md either; ask for an entry if you want it closed.
