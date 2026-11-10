## What this was about

In Hosts, answering an old Suspend question could still pause a sandbox after Cancel. An old Cancel, Close or Escape
could also close a newer question. The existing confirmation rule requires each answer to act only on the question it
was drawn for, as host Remove already does. This plan applies that rule to the Hosts-panel Suspend question.

## Things you should know

Cancel, Close, Escape and Suspend now consume only their own question's opening. An answer from a cancelled, replaced or
already-answered question does nothing, including leaving a newer question and its focus isolation alone. A current
Suspend answer still closes the question and starts the same request; it holds no new operation lock. The question still
opens only after the existing activity check needs confirmation.

The session toolbar's separate Suspend question is outside this plan. The fix, helper regression and explanatory
comments total 80 added/deleted lines under the size gate, excluding outcome bookkeeping and the changelog fragment. The
regression covers cancellation, reopening for the same host, replacement by another host and duplicate answers.

## Open questions and possible follow-ups

No new follow-up identified. The separately queued session-toolbar question remains unchanged.

## The PRs

[PR #3299](https://github.com/scode/farhelm/pull/3299/changes) — ignore stale host Suspend answers; draft head
`368d33c2ba8421651a1f79bd061ec983ba6fd7e4`.

## Checks run, reused and skipped

Run during implementation:

- Recorded `cargo nextest run -p farhelm-ui --lib -E 'test(/^hosts::/) | test(/^ops::/)'`: 114 passed, including the new
  Suspend regression; 824 tests outside the filter were not run. Run `5dadc8b1-364b-48dc-9554-a19e74c17818` used
  controlled nextest 0.9.148, four global slots, zero retries and tmux mode `none`. The source identity, JUnit report
  and captured output are complete, with no runtime `SKIPPED` lines, omitted output or truncation. This selection covers
  the changed question handlers and the shared confirmation mechanism.
- The first recorded attempt, `6b1cefed-086b-495d-afa7-3186ddae88f6`, failed compilation before executing tests: Dioxus
  rejected a local binding inside the rendered conditional. Moving the binding before the render fixed it. That failed
  record is retained privately; it is not counted as a test pass or a latent flake.
- `cargo fmt --all -- --check` and `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed, checking the
  changed Rust source and every UI target.
- `python -B scripts/check-test-sleeps.py` with the isolated parser interpreter passed: 429 delays checked, zero
  unannotated. This checks the changed test against the repository's delay rules.
- `python3 releasing/check-changelog.py format` passed for 38 fragments. Targeted `dprint check` and whitespace checks
  passed for the outcome graduation and queue index.
- After the rebase, `python3 scripts/check-agents-md.py` passed with zero errors and one existing archive-citation
  warning, checking references affected by removing the feedback file.

Reused for the delivered head: the successful UI tests, formatting, Clippy, sleep check and code review above covered
base `b29bdbf8` plus the final working diff (fingerprint
`47f14fc4a9c0efda2821620d9e59a768af57147de21a2de2ef1f0d95fca99138`). The rebase onto `37b6fe23` added an unrelated
supervisor repository-cache fix and planning bookkeeping. Its conflicts were confined to shared outcome bookkeeping and
were resolved by preserving both outcomes. A direct comparison confirms the tested UI source and changelog fragment are
unchanged at the delivered head. Later main changes through `fc454545` are planning bookkeeping only.

Skipped: browser tests, because the existing ordinary Suspend test requires a new web build and stack but does not
exercise queued stale answers; the focused handler regression covers that distinction directly, and no dialog markup,
focus script or timing mechanism changed. Full workspace, desktop and cloud tests add no identified coverage for this
bounded UI change. Hosted CI was not dispatched; it runs only on demand. The live installation was not used.

## Review gate outcome

The required single fresh-context GPT-6.1 Sol reviewer at high effort passed with no actionable findings. It received
the full plan and the test-authoring checklist. No review findings were rejected. A separate blind commit-message reader
understood the cancellation and replacement problem and found the wording consistent with the change.
