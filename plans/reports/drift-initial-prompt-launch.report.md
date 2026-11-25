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

### Landing

#### What landed with it

Two plans landed this round, both adding a category to the opt-in live drift tests: this one, through
[#4787](https://github.com/scode/farhelm/pull/4787/changes) and the landing fix
[#4795](https://github.com/scode/farhelm/pull/4795/changes), and then drift-named-conversation-starts. Both were written
before the previous round's drift plans landed, so they were rebased onto those. The only overlap was in TODO.md (each
plan's planned entry removed, every follow-up kept) and the drift docs. Nothing in what landed changes how this category
launches an agent, except that Claude now starts with its background updater turned off, as every drift case does since
the previous round.

#### Fixes made while landing

#4795 changes only the drift docs and TODO.md:

- **The drift docs now describe this category.** It was the only recent category without a paragraph. The paragraph says
  what prompt it sends, which agents' Restart it checks, and when it reopens the first conversation with a second
  prompt.
- **The Cursor follow-up asked for something the test cannot do.** It said to "find the test conversation's ID", but
  Cursor lists its conversations only in an interactive picker, so the step that reopens the first conversation can only
  report COULD NOT RUN for Cursor. The entry now points at `cursor-agent create-chat`, which returns a chat ID before
  the first turn and which the named conversation category already uses.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. Both plans landed this
round (drift-initial-prompt-launch and drift-named-conversation-starts) change only the opt-in live drift tests and
their docs, so the landing ran the drift tests that need no vendor CLI and the checks that inspect the changed files, on
the two plans rebased onto main and stacked together with their landing fixes.

On that stack (`7aa0e277`), whose code is what main now holds:

- `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`, on the test worker
  (recorder run `f8d59b12-6df5-4ede-a0d6-94282b1541db`): all 23 passed, with no runtime skip.
- `python3 -B scripts/run-harness-drift.py --list --category all`, on the test worker: it built the drift tests and
  listed eight launch prompt and ten named conversation cases beside the other categories', starting no agent.
- `cargo clippy --all-targets -- -D warnings`, on the test worker: passed.
- `python3 -B scripts/test-run-harness-drift.py`, locally (recorder run `ca051ad2-c296-422c-86db-1a96e90a14c8`): all 18
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 scripts/check-agents-md.py` (no errors; one warning, about an old citation in the triage archive, already on
  main), locally: passed.

After that, only the drift docs changed (the launch prompt paragraph was made more precise), and `dprint check`,
`check-agents-md` and the formatter passed again on the final stack.

Reused: the executors' live runs of the two categories against the installed agents, recorded in the reports. They ran
before the previous round turned off Claude's background updater for every drift case, so this round's Claude launches
now carry that setting where the reported runs did not; the reasoning effort category's live runs already launched
Claude that way.

Skipped: the live drift cases themselves, since they spend vendor turns, need the vendor logins and are opt-in by
design; the workspace-wide Rust suite and the desktop checks, since no shipped code changed; and CI, which runs only on
demand.

#### What the report says that is no longer true

The report's first follow-up says to find the ID of the conversation the test created and resume Cursor with it. The
test has no way to find that ID, so the TODO.md entry, as corrected above, now says to get one from
`cursor-agent create-chat` instead. Otherwise the landing's own review found the report accurate.

#### Things you should know that the report does not say

- **OpenCode's conversation lookup exists twice, and the two copies disagree.** This category and the named conversation
  category each find the test's OpenCode conversation in OpenCode's session list, by the test's folder. This one
  compares the folder's path without resolving symbolic links and reports COULD NOT RUN when no conversation, or more
  than one, matches; the other resolves the links and reports FAILED. An OpenCode release that records folders
  differently could therefore look like a missing prerequisite here and a failure there. Sharing one lookup is a small
  follow-up; no TODO entry records it yet.
- **A Restart that answered the prompt again would not be caught on screen.** The check that Restart did not answer the
  launch prompt a second time reads the screen right after startup, before any second answer could appear. What does
  protect against a re-sent prompt is the check that the restarted process's arguments no longer hold it, which is
  enough for a prompt Farhelm passes on the command line, as all of these agents' launch prompts are.
