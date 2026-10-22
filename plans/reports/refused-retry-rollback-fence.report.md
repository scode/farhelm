# Report: a refused create retry no longer deletes a restarted session

## What this was about

When a session create is interrupted (the supervisor crashed, or its reply was lost), the create is retried under the
same idempotency key, so the retry finishes or replays the original create instead of making a second session. If that
retry was refused, for example because the session's launch settings no longer validated, the supervisor deleted the
session record the interrupted attempt had left behind. It did that by session ID alone. If the user restarted the
session in that window, Restart had already started a new agent under the same record, and the stale refusal deleted it:
the restarted session dropped out of the session list, or Restart, finding its record gone partway through, undid its
own launch and stopped the new agent. The trailing review that found this traced it by reading the code; the exact
timing was never confirmed reachable, and I did not reproduce it.

You decided on 2026-10-21 to fix it, with a size gate of about 100 lines (over it, block and bring it back).

What landed: the refusal now deletes the record only while it is still the untouched first attempt, meaning its
idempotency key is still pending for this session and the session has not been relaunched (each Restart moves a
per-session launch counter up from zero). The check runs in the same database transaction as the delete. Restart moves
the counter in a transaction of its own, so whichever comes first, the other sees it. A Restart that comes second finds
the record gone and stops. No extra locking was needed.

## Things you should know

- When the check keeps the record, the refusal is not recorded against the idempotency key: the key stays pending,
  because the session still exists. The plan asked me to choose between that, recording the refusal while keeping the
  session (a later retry would then report a refusal for a live session), and returning the refusal with nothing
  recorded. I chose to leave the key pending. SPEC.md says a key lives as long as its session does and a retry returns
  the existing session, which only the pending choice keeps true. The reviewer agreed.
- The reply to such a refused retry is now an internal error saying the outcome was not recorded and the key can still
  be retried (its reason reads "skipped: the session was restarted or deleted meanwhile", alongside the original
  refusal), rather than the plain refusal. A plain refusal would have implied the key was used up. Whoever issued the
  create (the app, the helm or `farhelm agent create`) sees that error for that attempt; the restarted session itself is
  unaffected.
- The same check also keeps the record alone when the user deleted the session in that window; the reply then says so.
- Size: about 105 changed lines of code, tests and docs, at the edge of "about 100". A first version that moved the
  existing delete code into a helper came to 147 lines; I reshaped it to keep that code in place. I counted the result
  as within the gate.
- No change to SPEC.md or SPEC_impl.md was needed: neither describes this rollback.

## Open questions and possible follow-ups

- The reviewer found a related gap this change does not close, and which predates it. While a Restart is in its slow
  phase (the new agent not started yet), a retry under the same key can still decide the interrupted create never
  started and launch it itself, because that decision does not look at the launch counter either. That could start a
  second agent beside the restarting one. Leaving the key pending, as above, keeps that path open as it was before. My
  recommendation is a small follow-up that adds the same launch-counter check to the takeover. It is outside this
  outcome's agreed scope and would have pushed this PR over its gate.
- No test drives the whole refused retry through the supervisor to show that the restarted session stays listed and the
  reply is the "not recorded" error; the new test covers the database step, which is what the completion criteria ask
  for. A supervisor-level test would be a reasonable addition alongside the follow-up above.

## PRs

- [#2570](https://github.com/scode/farhelm/pull/2570/changes)
  `fix: keep a restarted session when its stale create retry is refused`: the guarded rollback, its test, the changelog
  fragment, and the triage bookkeeping (feedback file and index entry removed, Execution recorded).

## Checks

Run now:

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(a_refused_retry_rollback_spares_a_restarted_session) | test(the_rollback_delete_settles_only_what_it_is_given) | test(delete_session_removes_the_row_and_tolerates_a_missing_one) | test(a_retry_whose_directory_was_repointed_is_refused_rather_than_relaunched) | test(a_pre_launch_kinds_reservation_refuses_its_retry)'`
  through the test recorder: run `7b96e38f-6c1d-4b5f-ab70-86e65aeffec2`, 5 of 5 passed. The selection is the new test,
  the existing rollback tests, and two existing tests that drive a refused retry through the supervisor.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`: clean on the first version and again on the final
  one.
- `cargo fmt --all` (final version), `dprint check` on the changed files and
  `python3 releasing/check-changelog.py format` (first version; nothing they check changed after it): clean.

Reused: the recorder run above, for the final version. After the review, only one error message and comments changed,
and none of the selected tests checks that message.

Skipped: the workspace-wide Rust battery and browser tests. The change is confined to the supervisor's create rollback
and its database step, and the selection covers both.

## Review gate

One fresh-context reviewer (Opus 5.5, high effort, reviewing correctness, design and idiom, with the repository's
test-authoring checklist) found nothing blocking and judged the idempotency-key choice consistent with SPEC.md. I
applied its wording findings: the reply no longer claims the session was restarted when it may have been deleted, a
comment no longer promises more than the code does, and a doc comment now describes the return value accurately. Its two
follow-ups are listed above. It also suggested two optional style tweaks, which I left as they are.

### Landing

Landed as #2570, with the same content the report describes. It was the last of five plans in this landing round, after
triage-specs-oct21, cheap-ui-hardening-oct21, start-remedy-quoting and uninstall-mixed-spelling.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained popover-anchor-close and documentation commits, neither touching the supervisor. A
fresh-context reviewer and I read this plan against the supervisor work landed in recent rounds:

- create-reservation-retention (a create's retry key freed when its session is deleted): after a Delete, the new "not
  recorded" reply leaves the key unspent, which matches the rule that a retry after a Delete starts a fresh session.
  After a Restart, the refused retry changes nothing, and a later retry with the same key returns the restarted session.
- launch-attempt-owner (one owner for a failed launch's leftover files) and spare-launch-checkout (prepared checkouts):
  the new check runs before their cleanup and before the prepared-checkout guard, and changes neither.
- The other plans in the round do not touch the supervisor's create path.

#### Fixes made during the landing

None to the plan's code. The review-queue index conflicted with entries main had changed; the landing rebuilt it.

#### Possible follow-ups

- Found at landing, not confirmed, and older than this plan; nothing in the review queue or TODO.md tracks it: a stale
  retry can also be refused in a way that keeps its session listed as failed rather than removing it, for example when
  the folder for the fresh checkout it was creating is already taken, or when its launch failed before any agent
  started. That path has no check of its own for a Restart made in between, so a session the user just restarted could
  show as failed. It only runs once the session's other lifecycle operations have finished, so it matters only if a
  Restart can finish while its new agent is still starting, which nobody has shown. Possible follow-up: give that path
  the same check of the session's launch count, together with the takeover gap the report already raises.

#### Checks

Together the five plans change about 650 lines of code, tests and specification text (more with review-queue
bookkeeping) across the helm's connection errors and uninstall, the supervisor's create path and store, and the UI,
including how every menu remembers its focused item and the terminal's file-link script; that reach is why this round
ran the whole Rust workspace and the full browser suite. All of the following ran on the five plans stacked together on
this round's main, including the landing's conflict resolutions. After the run, the landing moved changelog text between
fragment files in cheap-ui-hardening-oct21's last PR (see its notes); that changed only changelog files:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 scripts/check-agents-md.py` (one warning,
  about a citation main already had), the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh` (which also builds the desktop app): all passed.
  `python3 releasing/check-changelog.py format` passed again after every changelog change.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  slots, no retries, run `461bfc4c-651d-4a70-8498-7a9b5ae524cf`: 4291 passed, none failed; 40 tests marked as ignored
  did not run.
- `cargo nextest run -p farhelm-ui --features desktop`, run `4073dcb5-a14f-42e5-bddd-8b007925da68`: 900 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `4086a6d6-96a5-4691-b05e-e50169546906`: 329 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `b5e88d8c-8a25-4e9b-be8f-ada4f9d48bc3`: 2475 passed, 14 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), 1 failed: on WebKit, "don't ask
  again on a refused YOLO replace marks the host, then replaces with the override", the open flake FLAKES.md recorded on
  2026-10-20, which the deflake sweep already excludes. It exercises a host's YOLO question during Replace, which none
  of the five plans touches; refused-retry-rollback-fence changes only the supervisor's handling of a stale create
  retry.
- An earlier start of these checks was stopped at its build step and discarded, because the landing changed the stack
  while it ran; nothing from it is counted.
- Reused: none. Skipped: the website build (no docs page changed), the doctests (no documentation example changed), the
  desktop smoke test (the desktop app crate is unchanged, and the menu marker fix is covered by the desktop-feature unit
  tests above, which mount real menus), the native Mac app (not runnable here) and the installer (unchanged).

The report's checks were the supervisor's store tests; the round ran the whole Rust workspace and the browser suite
above. Nothing in the executor's report became untrue at landing.
