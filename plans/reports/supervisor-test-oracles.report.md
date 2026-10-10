## What this was about

Eight supervisor tests could pass without proving the behavior they claimed to protect: orderly connection shutdown,
terminal recovery, conversation switching, duplicate session creation, rejected creation requests, and session restart.
These were defective tests; no current product failure was exposed. Their observations could accept a broken operation
or never establish the conditions needed to exercise it.

The maintainer asked for all eight small test repairs in one draft PR, with unexpectedly complex outcomes left for
triage. All eight assessments still held, and all eight repairs stayed within the named tests and existing observation
facilities.

## Things you should know

When a connection shuts down, queued messages must drain and the task sending them must finish naturally. Its test now
rejects forced cancellation as proof of success. When recovering terminal output after a delayed tmux filter response,
the replay test now establishes that exactly one response is still outstanding; consuming it beforehand would avoid the
recovery condition and miss output being mistaken for that response.

Oh My Pi (OMP) conversation transitions now use distinct conversation identities and files. Each transition checks that
the supervisor recorded the new conversation, so retaining the old one cannot pass. For concurrent duplicate-create
requests, the test waits until the second request reaches the held lock, then verifies exclusion and eventual handoff.
Retrying creation of an already-ended session must preserve its exited status and original exit code; keeping the same
session identity alone no longer counts as correct replay.

Requests with an invalid duplicate-create key or an oversized resume command must be refused without recording that key
as a creation attempt. The tests now reject records in any state, including permanently failed attempts that their old
checks overlooked. For a valid session restart, the preservation test first requires an accepted restart and a stored
advance of the session's run number (its generation). Only then does it check that the conversation and its origin
information survived; a refused restart that left everything untouched can no longer pass.

These are test corrections; no product code or new test seams ship. All eight feedback files and their complete index
entries are removed, and all eight ledger entries reference the same draft PR. No outcome was discarded or deferred, and
no strengthened test exposed a failure against unchanged product code.

## Open questions and possible follow-ups

None requiring a decision. Validation ran on Linux with pinned tmux and a live Bun runtime for the OMP fixture. No macOS
execution is claimed.

## PRs

- [#1816 — reject false passes in supervisor regressions](https://github.com/scode/farhelm/pull/1816/changes); one draft
  PR for all eight outcomes.

## Checks run, reused and skipped

All runtime checks below used the test recorder, four nextest slots and zero retries. UUIDs identify retained evidence.

The initial and restored runs used this command. The temporary-regression run used the same command and selection with
`--kind repetition` in place of `--kind development`:

```sh
python3 scripts/record-test-run.py --runner nextest \
  --kind development --tmux required \
  --selection 'eight supervisor test oracle repairs' \
  --concurrency '4 nextest slots; retries 0' -- \
  cargo nextest run -p farhelm-supervisor --lib \
  -E 'test(=service::connection::tests::a_signaled_writer_drains_queued_frames_without_the_drain_window) | test(=tmux::stream::tests::a_late_pane_filter_does_not_desynchronize_the_catch_up_replay) | test(=service::core::tests::omp_proven_transitions_report_in_order) | test(=service::core::tests::intent_locks_exclude_hand_off_and_prune) | test(=service::core::tests::a_pending_reservation_whose_session_ended_replays_it) | test(=service::handlers::tests::a_degenerate_intent_key_is_rejected_before_anything_is_stored) | test(=service::handlers::tests::an_oversized_resume_command_is_refused_before_anything_is_stored) | test(=store::tests::relaunch_preserves_capture_and_provenance_together)'
```

- Initial exact eight-test selection: 8/8 passed, run `f6054d0c-9392-4cea-b057-b030f4e4e698`. Retained output had no
  runtime `SKIPPED` messages; OMP and tmux coverage executed.
- Temporary regression proof: all eight tests failed at the intended distinguishing observable, run
  `aa45d391-83f2-44c3-8348-7d6eb0a6e5fb`. The mutations omitted writer closure and reply settlement, kept an old
  conversation, bypassed lock sharing, changed the ended replay's stored outcome, persisted refused keys, and refused a
  valid restart. The ended-replay proof changed durable state under the same identity; it did not launch a second real
  agent. All temporary edits were restored exactly to the reviewed source, and the expected failure evidence is
  retained.
- Restored exact eight-test selection: 8/8 passed, run `d778f4c5-e4de-43ee-86bf-37b9b4902f8b`, with no runtime `SKIPPED`
  messages.
- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`: passed.
- Isolated `python -B scripts/check-test-sleeps.py`: passed, 282 delays inspected and zero missing rationales.
  `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`: passed.

The recorder refused extra nextest display and policy arguments before starting a command; the corrected invocation used
its supported selection arguments and retained profile. This was an invocation correction, not a test failure.

The checks cover the reviewed source on base `8c57e40a`. Main then changed only plan claims and another plan's
delivery/report; all intervening diffs were read. The rebase onto `d8d43e77` preserved the tested code and its
dependencies, so the successful commands' evidence remains applicable. Subsequent ledger URL bookkeeping passed Markdown
formatting again; it requires no additional runtime tests.

Full workspace, browser, desktop, installer and release checks were skipped: this change ships only the eight tests and
bookkeeping, and focused execution plus deliberate regression proofs cover their changed contracts. No doctest or
generated artifact changed. Hosted CI was not dispatched. No changelog fragment is required for this test-only change.

## Review gate outcome

The prescribed fresh reader requested on gpt-6.1-sol high inspected all eight changed tests, the full diff, supporting
code, relevant specifications, the plan and ledger criteria, and the full test-authoring checklist. It found no source
defects and confirmed that each assertion distinguishes its named regression. Runtime results were verified separately
by the executor. Implementation and investigation stayed local under no-workhorse mode.

Commit and PR wording received a fresh cold read requested on gpt-6.1-sol medium. The reader understood the test
false-pass correction and found no contradicted claims or convention violations. The title is concise and the PR body is
empty because the diff is self-explanatory.

Actual native model identity and usage counters were unavailable. Requested routes are retained as requests in private
routing session `a63f6b2c-45f7-4830-bf84-ea76de63472d`; they are not asserted as measured runtime attribution. The
executor leaves the PR in draft and does not merge it.

### Landing

Landed on 2026-10-10 (UTC) as #1816. The rebase met only the review queue index.

#### Review before merging

No findings. Each reworked test now observes what its finding said it missed: the writer task's own completion, a
genuinely outstanding reply, a fresh conversation per transition, the lock contender's arrival, the exact stored
outcome, reservations in any state, and the restart's claimed generation.

#### Checks

These six plans were landed together, stacked in landing order on main after gui-text-safety, ui-interaction-fixes and
helm-cli-fixes merged; browser-terminal-test-oracles landed after them in the same round.

- Run now, on the six stacked: `cargo fmt --all -- --check`, `dprint check`, the changelog lint, the plans-queue tests,
  the test-recorder tests, the installer tests (517 checks), `sh -n scripts/install.sh`,
  `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`; the supervisor, helm,
  UI, protocol and CLI unit tests with the agent-relay, upload, boot-outcome, hook-identity, restart-with-resume and
  session-lifecycle end-to-end tests, through the recorder with pinned tmux 3.7c, four slots and no retries (run
  `8bc6001f`, 3064 of 3064); the UI JavaScript tests (230 of 230); and on Chromium and WebKit with one worker and no
  retries the notifications, sidebar-resize, sidebar, terminal-attachments, terminal-create-idempotency,
  terminal-multihost, yolo-guard, templates and quick-switcher specs (run `77ec2061`, 472 passed). `shellcheck` reports
  the same warnings for `scripts/build-private-tmux.sh` and `scripts/desktop-smoke.sh` on main as with these changes.
- These plans were claimed while an earlier round was still landing, which delayed their merge; the landing instructions
  were changed the same day (#1829) so that a round's plans are fixed when it starts.

Nothing in the report above was made untrue by the landing.
