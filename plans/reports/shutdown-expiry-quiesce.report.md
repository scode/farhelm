## What this was about

When an update, restart or other planned stop exceeded the supervisor's stop budget, it could close terminal-output
connections before asking tmux to stop sending output. Waiting for an attachment lock could consume the entire budget.
Abrupt closure of output-bearing clients has crashed the private tmux server on tmux 3.6 and 3.7b, ending all sessions
on the host; whether that defect exists on the pinned tmux 3.7c remains unverified. The maintainer chose a small
best-effort fix and a limit on further defensive complexity without newer crash evidence.

## Things you should know

The existing ten-second orderly-stop budget remains. If it expires, the supervisor makes one independent attempt to list
the private server's control clients and disable their output, with at most two additional seconds before returning. It
reuses the acknowledged no-output command already used for orderly cleanup. It does not kill clients, retry or verify
the roster afterwards. Failure ends the attempt, is logged, and still permits exit; expiry still counts as an incomplete
orderly stop.

This can quiet existing clients even when the attachment lock used the original budget, but it does not guarantee that
every connection closes safely. The test proves that a real, live, output-enabled control client becomes no-output while
remaining alive. It does not reproduce the historical crash or establish queued-output protection on 3.7c.

The implementation specification now preserves the existing safe-teardown discipline while requiring an observed abort
on tmux 3.7c or later before this class of rare bypass justifies significant additional complexity. Code and test
changes total 144 changed Rust lines, within the roughly 150-line bound. The feedback item is removed and its triage
outcome records this draft PR.

## Open questions and possible follow-ups

None required to complete this plan. An observed abrupt-close abort on tmux 3.7c or later would reopen the agreed limit
on defensive work. This run did not establish one.

## PRs

- [PR #1780](https://github.com/scode/farhelm/pull/1780/changes): bounded output quiet-down on planned-stop expiry and
  the specification limit, in one commit on bookmark `plan/shutdown-expiry-quiesce/01-shutdown-expiry-quiesce`. Created
  as a draft; this executor did not mark it ready or merge it.

## Checks run, reused and skipped

- Ran the direct real-client proof through the recorder:
  `cargo nextest run -p farhelm-supervisor -E 'test(=tmux::tests::stop_expiry_quiets_a_live_control_client_without_killing_it)'`.
  One passed, no runtime skip; run `858fe933-afd9-4461-8d05-c64c5fe62770`. The recorder verified pinned tmux 3.7c and
  nextest 0.9.143, four global slots and zero retries.
- Ran three focused orderly-stop regressions through the recorder:
  `cargo nextest run -p farhelm-supervisor -E 'test(=service::teardown::tests::a_stop_waits_for_live_sinks_and_refuses_new_ones) | test(=service::teardown::tests::a_stop_waits_for_the_reaper_a_released_sink_leaves_behind) | test(=service::teardown::tests::the_stop_budget_covers_the_attachments_lock)'`.
  Three passed, no runtime skip; run `88c36968-c1d6-48d1-9e15-0542099e73ed`, with the same verified substrate and runner
  policy.
- Ran `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`; both passed,
  covering the test-enabled and shipped supervisor configurations. `cargo fmt --all -- --check`, changed-Markdown
  `dprint check`, the isolated `python -B scripts/check-test-sleeps.py` and
  `python3 releasing/check-changelog.py format` passed. The delay check found zero unexplained delays.
- Reused runtime and Clippy evidence from the change based on main `c39871e9` after reading the complete intervening
  main diff and rebasing onto `9a8c9d64`. The only intervening change delivered another plan's queue state and report,
  with no code, dependencies or behavior contract changed. Formatting the specification and adding the ledger's PR URL
  preserved the tested Rust behavior; the ledger Markdown check was repeated.
- Skipped broad Rust, browser, desktop, installer and release tests: the focused flag transition and existing
  orderly-stop lifetime and lock regressions cover the changed mechanism. No UI, transport, installer or release
  behavior changed. No crash reproduction was required by the agreed small-fix scope.

## Review gate outcome

The prescribed fresh-context gpt-6.1-sol high reviewer found no actionable correctness, design, idiomatic-code or
test-authoring issue. It independently confirmed the 144-line bound, the whole-attempt two-second deadline, and the real
client's live/output-enabled premise and retained no-output state. The wording cold reader recovered the motivation and
best-effort limitation without contradicted claims or convention violations. A separate documentation pass covered every
touched code file. Requested native model selections are known; actual model attribution and usage counters were not
exposed. Review artifacts and evidence remain private.
