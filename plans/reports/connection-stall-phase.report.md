## What this was about

When a host's supervisor connection stalled, Farhelm listed every possible step without saying which one was pending.
SSH's own explanation stayed in the helm log. You asked for five understandable stages, shown during an attempt and
after failure, in the existing host and unavailable-session detail.

Connection failures now distinguish SSH connection/login trouble, missing remote Farhelm, an absent supervisor, a
supervisor that does not answer, and unfinished setup after its answer. Live details show the step currently evidenced.
Recording the host's identity belongs to the final setup stage. The unavailable card's headline and explanation stay the
same. The shared twenty-second attempt budget, retry ladder, re-probe cadence and SSH options are unchanged. The last PR
removes the covered TODO entry.

## Things you should know

The remote helper emits one private progress line on stderr only after reaching its supervisor. It never writes progress
into the protocol stream. A new helm can diagnose older hosts, but an older helper emits no progress: an unmarked
timeout there remains ambiguous between a stalled SSH login and an older remote. Once this helm has observed the marker
for the current host configuration, later unmarked timeouts identify SSH connection/login. Changing its destination,
configured Farhelm program or state directory clears that evidence. It is kept only for this helm process, never across
restarts.

After a failure, the detail includes at most two recent stderr lines, bounded and control-escaped. SSH's natural exit
distinguishes connection/login failure, a missing or unexecutable Farhelm, and an absent supervisor. A signal or
unavailable exit status remains unknown rather than becoming a made-up exit code. An observed supervisor connection or
completed hello takes priority over contradictory exit evidence; malformed protocol responses keep their original
meaning.

Early connection loss allows at most two seconds to collect final stderr and the child's natural exit, within the same
twenty-second attempt budget. Deadline expiry adds no collection wait. Ordinary refusal replies do not incur this wait.
Normal absent-supervisor messages carry the exact start command. If configured paths are too long to include safely, the
detail explicitly gives descriptive guidance rather than a clipped command that appears ready to paste.

Live progress uses only known stage names; an older helm without a stage, or a future unknown stage, keeps the previous
generic detail. Prior failure text remains separately isolated from Farhelm's own words. Stage changes publish updates;
there is no progress timer or extra background publisher.

## Open questions and possible follow-ups

None required for the agreed behavior. Older remotes must be updated to report live progress. Previously observed
progress only refines a later timeout's diagnosis; it does not establish how far the current attempt has got.

## The PRs

- [#2502](https://github.com/scode/farhelm/pull/2502/changes): record attempt stages and publish the additive host-state
  field.
- [#2505](https://github.com/scode/farhelm/pull/2505/changes): classify SSH failures from bounded stderr, natural exit
  and remote progress.
- [#2507](https://github.com/scode/farhelm/pull/2507/changes): show live stages in shared host/session details and
  remove the covered TODO entry.

All three are open drafts in that linear order.

## Checks run, reused and skipped

Runtime checks used `scripts/record-test-run.py`. Rust selections used pinned nextest 0.9.148, four slots and zero
retries; SSH and process selections used the checksummed tmux 3.7c build. UI pure tests require no tmux. Browser
execution used Chromium and WebKit, three configured workers with private stacks, and zero retries. Successful runs
cited below have complete source and runner reports, retained output with EOF and no truncation, no runtime `SKIPPED`
messages and no cleanup limitation. Selection-filtered tests are separate from execution.

- PR1's manager, host API, shared HTTP fixture and refusal wording selection passed 124 tests in
  `b9078e84-a73a-4d8d-be48-1e2f2af0d857`. Its final lint correction removed only a redundant reference in assertion
  formatting; that unit reused the runtime evidence after inspection and reran Clippy and formatting. Final PR2 coverage
  below includes these connection contracts again.
- `cargo nextest run -p farhelm-helm --lib`, selecting manager, transport, SSH, hosts, HTTP contract tests and the
  provisioning progress-marker compatibility case: 163 passed in `4bd99bc3-8487-4251-9c3e-3f2d3fadf12e`. This covers the
  unchanged retry timing, local silent hello and final setup, marker framing, process/observation ownership, natural
  exit, bounded stderr, failure classification and configuration-scoped progress history.
- `cargo nextest run -p farhelm --test stdio_exit --test e2e`, selecting the stdio binary and host_connection module:
  six tests passed plus one fixture-build setup in `7cc3f08d-a9e1-4882-aa01-a4831a3ef7cd`. Both real SSH tests for an
  absent supervisor and missing executable ran, as did real local/SSH connections and helper stdout/stderr/exit
  contracts.
- `cargo nextest run -p farhelm-ui --lib`, selecting the hosts module, shared HTTP fixtures, stage decoding and strict
  required-field checks: 51 passed in `c897d172-27f1-47c9-b2c0-a82005aea61c`. This includes all five labels, retry
  numbering, unknown/absent-stage compatibility, peer isolation and matching unavailable-card/host details.
- `npx playwright test terminal-multihost.spec.ts -g host-list-phase-table`: two passed, one per engine, in
  `a51adfef-9534-401c-a2c8-6e7af71064c3`. This exercises the existing host row renderer with the live setup stage and
  previous failure text. The native build and pinned Dioxus 0.7.10 release web build passed before this run.
- `cargo check -p farhelm-ui --features desktop --tests` passed, including the changed desktop-only test constructor.
  `cargo fmt --all -- --check`, `git diff --check`, `dprint check` for the changed specification/TODO, and
  `python3 releasing/check-changelog.py format` passed. The isolated `python -B scripts/check-test-sleeps.py` passed
  with 309 delays inspected and zero missing rationale. Both `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings` passed on the combined stack, including the distinct shipped CLI
  configuration.

PR2's final source was captured above #2502 as fingerprint
`6e22f5e83f797885b54ce58856d75ab9d4d1ae8c76b8c9381c7fb23c5b01a4ab` and committed as `49f13994`. PR3 changes only
presentation, its tests and shared bookkeeping, so that connection-runtime evidence remains applicable. The six helper
and real-connection cases are reused after a test-only timing assertion/comment correction; their production and test
source stayed unchanged. The final browser source fingerprint is
`cd40bfd56b915874ea927f073af4262694f9967f3db45fddf7acf6f3df6bee7a` above `49f13994`, shared by the final 51-test UI run.
The final `38e963e4` differs only in commit metadata and a changelog sentence clarified to limit older-remote
uncertainty to timeouts. Production and test files are byte-identical, so these runtime, build and review results are
reused. Fragment format and diff checks passed again after that prose edit.

The first 163-test PR2 run, `f20257a7-a605-4de7-92d0-8fd57e72a3a6`, failed one newly written test that expected exactly
two seconds from a paused timer. Exact reproduction `db064207-203c-4075-982c-bd08b524557b` again observed 2.001 seconds.
The pinned Tokio timer rounds deadlines to a millisecond tick. The assertion now allows that one millisecond while
rejecting an early return or extended stderr wait; production timing is unchanged. The corrected exact case passed in
`71805b0c-6324-4489-a6e2-725f8a22a464`, then the final 163-test selection above passed. Both failures remain retained as
same-session development evidence, not latent flakes.

The full incoming main diff through `bbf81a85` was inspected. It changes uninstall refusals, their specification/docs
and shared TODO, plus queue/report bookkeeping; it does not interact with connection stages, helper transport, timing or
shared detail rendering. No rebase or extra runtime test was justified by that movement.

The full workspace/browser batteries, desktop runtime, installer, provisioning-container, website and JavaScript checks
were skipped: the focused manager/transport/helper/real-SSH/UI/browser coverage addresses the changed contracts; those
other subsystems did not change. No executable documentation example or instruction/heading/file move changed, so
doctests and the agent-instructions checker add no evidence. CI is manual-only and was not dispatched.

## The review gate's outcome

Each PR received its required single fresh gpt-6.1-sol/high correctness, design and idiom review, with the full goal and
test-authoring checklist. All passed. The PR2 follow-up accepted the timing assertion correction and caught a comment
that overstated what the fixture proved; that comment was corrected. No production findings or rejected findings remain,
and no material scope departure was needed. Commit/PR wording passed its separate fresh cold reads.
