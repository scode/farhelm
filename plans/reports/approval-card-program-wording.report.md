# Program names on Stop and Delete approval cards

## What this was about

When an agent asked to stop or delete a Bash or Zsh session, the approval card could say the action would end an agent.
The maintainer chose “shell” for those sessions and wanted Delete to say which processes the user would allow Farhelm to
stop.

Stop now names the program and terminal tabs in its heading, including in the waiting list. Delete names the scope of
consent for a session with nothing running, terminal tabs only, or both its main program and tabs. Its wording and
consent come from the same live session snapshot.

## Things you should know

- Bash and Zsh use “shell.” Agent sessions use “agent.” Plain commands and sessions whose program kind is unavailable
  use “main program.” A new client can read a card from an older helm, and an older client can read one from a new helm.
- If Stop has no cached description, Farhelm gives the supervisor two seconds to supply one. If that read fails or runs
  out of time, the card uses “main program” and the Stop request proceeds to its usual approval check.
- The supervisor enforces the original Delete consent at dispatch. For example, approval to close only the terminal tabs
  cannot also end a main program that has since restarted.

## Open questions and possible follow-ups

None from this plan.

## PRs

- [#4688](https://github.com/scode/farhelm/pull/4688/changes) Program wording on Stop and Delete approvals.

## Checks

- The focused proto, helm, helm-client and desktop approval selection passed all 284 tests at `7e6acfc8`
  (`01c5e0b2-b48d-45fb-8da1-2e49e5d5aee9`). The final rerun covers the optional Stop read deadline, late replies, Stop
  facts and the deliberate difference between the cached and live Delete snapshots.
- Existing Linux approval and agent Add host screenshots matched their references in both themes
  (`a626438d-aa04-4cd8-a021-3ae51637cc1f` at `5ea28739`). No reference updates were needed.
- `cargo fmt --all -- --check`, `dprint check` on the changed Markdown, `git diff --check`,
  `python -B scripts/check-test-sleeps.py` and `python3 releasing/check-changelog.py format` passed.
- The 10 focused checks at `5ea28739` (`d35fb34a-0944-4c52-b6e3-c2b4c86a84ea`) all passed. They covered the optional
  Stop read, expired and late replies, and the revised Stop and Delete assertions. The same revision passed strict
  `cargo clippy --all-targets -- -D warnings`.
- Draft head `8094a6d3` carries the tested patch on `1ea1227b`. The focused rerun covers the new deadline and its
  interaction with Stop. The initial 284-test run covers the other approval behavior. Main moved the permission-chip
  wording and the environment Stop and suspend action, away from the covered approval and transport paths. The selected
  screenshot fixtures and reference pictures are unchanged across the rebase.
- Executor validation targets the affected code. The full suite is pending at handoff for the landing monitor.

<details>
<summary>Recorded test commands</summary>

The commands ran through the test recorder. Each nextest run used four slots and zero retries.

```sh
cargo nextest run --locked --test-threads 4 --retries 0 -p farhelm-proto -p farhelm-helm -p farhelm-helm-client -p farhelm-gui --lib -E 'test(/^approvals::/) | (package(farhelm-helm) & test(/^agent_requests::/)) | (package(farhelm-helm-client) & test(/^collections::/))'
cargo nextest run --locked --test-threads 4 --retries 0 -p farhelm-helm -p farhelm-gui --lib -E 'test(/^client::tests::.*(listing|full_queue|writer_failure)/) | test(/agent_requests::tests::(a_stop_waits_for_approval|approval_cards_name_uncached|agent_delete_preserves|an_agent_delete_refusal)|approvals::model_tests::stop_and_delete_cards/)'
python3 scripts/check-gui-goldens.py --scene '*approval*' --scene host-add-dialog-agent
```

</details>

## Review

The independent source review passed after bounding Stop's optional read and tightening the Stop and Delete fixtures.
The transport test verifies that a reply arriving after the deadline leaves the same connection usable and the later
waiter receives its own result.
