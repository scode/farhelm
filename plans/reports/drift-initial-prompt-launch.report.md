## What this was about

A launch prompt can be dropped, truncated at a newline, or left unsent in the composer. The drift suite had no case for
that first turn. This test sends a prompt whose first line starts with dashes and whose second line asks for a reply
that depends on the first. The drift runner now exposes the check for all eight agents that accept a launch prompt.

## Things you should know

Six installed CLIs passed. All six reopened the composer on Restart with their saved conversation command and without
the one-time prompt in their arguments. Claude, Codex, Grok, OMP and Antigravity also recalled the test word when
Farhelm opened the original conversation with a new launch prompt. Muse supports a fresh launch prompt and Restart.

OpenCode answered the fresh prompt. When it reopened the original conversation, it displayed the earlier answer and an
empty composer. Cursor stopped at its login screen before the test could exercise a prompt. Both are recorded in the
Plan follow-ups section of TODO.md.

## Open questions and possible follow-ups

- With a Cursor login, repeat the case on 2026.11.22-e9042b2 or a newer installed release. The fresh command is
  `cursor-agent -- "$prompt"`. Then find the ID for the conversation created by the test and check
  `cursor-agent --resume "$id" -- "$prompt"`.
- Repeat the OpenCode case on a release containing the upstream named-session prompt fix. The failing command on 1.19.6
  was `opencode --session "$id" "--prompt=$prompt"`. The tested version's TUI navigates into the saved conversation
  without forwarding the startup prompt.
  [OpenCode 1.19.6 startup code](https://github.com/anomalyco/opencode/blob/v1.19.6/packages/tui/src/app.tsx#L449-L469)

## PRs

- [PR #4787](https://github.com/scode/farhelm/pull/4787/changes) covers launch prompts in the drift suite and records
  the Cursor and OpenCode follow-ups.

## Checks run, reused and skipped

- Reused the local gate and recorded runs below from `952742b0a36163caeafb92966059d0f5ccdd9c6f`. The published draft has
  the identical tree. The fetched changes on main affected only the queue and other plans’ reports.
- `cargo fmt --all -- --check`, `dprint check -- TODO.md`, `git diff --check`,
  `cargo clippy --locked -p farhelm --all-targets -j 3 -- -D warnings` and `python -B scripts/check-test-sleeps.py`
  passed. `python3 -B scripts/run-harness-drift.py --list --category initial_prompt` listed eight cases.
- The recorded runner stand-ins, `python3 -B scripts/test-run-harness-drift.py`, passed 18 tests
  (`fc85cd53-0e9a-44b1-9bf0-11c181ab3885`). The pinned nextest selection
  `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'` passed 17 tests
  (`badd3024-d8a3-4814-80a4-2f452209e169`).
- `python3 -B scripts/run-harness-drift.py --no-upgrade --category initial_prompt` exited 1 because OpenCode's named
  turn failed. The case verdicts on the installed versions were as follows.

  | CLI          | Version            | Verdict                                                                     | Recorder run                           |
  | ------------ | ------------------ | --------------------------------------------------------------------------- | -------------------------------------- |
  | Antigravity  | 1.5.16             | Passed                                                                      | `9df80f32-ce33-4fbc-b06b-45679518061b` |
  | Claude Code  | 2.1.332            | Passed                                                                      | `95ee93bc-5313-4945-bb17-d19c69198137` |
  | Codex        | 0.172.0            | Passed                                                                      | `b6f46b0c-d0f3-4d3a-92da-cfd51cc6a3e7` |
  | Cursor Agent | 2026.11.22-e9042b2 | Could not run. The CLI required a vendor login before opening the composer. | `c284a57e-dbae-4535-b0b2-dcce098680bb` |
  | Grok         | 1.0.92             | Passed                                                                      | `5eadc430-6892-4fd3-aeae-010e19dc2bcf` |
  | Muse Code    | 1.4.25-R9361.1     | Passed                                                                      | `e53094be-7b5a-4e6b-9dc7-ee44a9c0efd6` |
  | OMP          | 19.1.1             | Passed                                                                      | `2eba8f41-0560-4a5f-af30-fd601f45ffab` |
  | OpenCode     | 1.19.6             | Failed. The fresh turn passed and the named launch produced no new turn.    | `6407a91c-f94b-4eee-87ea-dcdc2240fd96` |

- Skipped CLI upgrades as planned. The focused checks cover the changed drift suite, so broader suites and checks for
  production or release artifacts were skipped.

## Review

The independent source review found three gaps. The corrected tests check the literal multiline prompt and saved
conversation selector in the running process, and show separate errors when OpenCode session discovery fails. All three
findings were addressed and the source review gate cleared. The corrected live run reproduced the same Cursor and
OpenCode outcomes.
