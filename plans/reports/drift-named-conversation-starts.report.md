## What this was about

Farhelm can open a conversation by its ID or session file when you start an agent. The existing live tests checked
ordinary Restart, which resumes the conversation Farhelm already knows about. They could miss a vendor changing how it
accepts a conversation explicitly selected at launch.

The new `named_conversation` category seeds a random label into an owned test conversation and checks that selecting its
ID or file opens the conversation that can recall the label. It checks that the reporting agents capture the selected
conversation, and exercises both absolute and relative file names for Pi and OMP. The Goose test starts with a different
captured conversation, selects the seeded destination via Restart with, and checks what happened to the old capture
separately from whether the agent recalls the destination.

## Things you should know

Eight installed agents passed the live check. Cursor requested login before the seed turn, so its named recall remains
unverified. Goose cleared the old capture and recalled the explicitly selected conversation. The selected conversation
held a previously installed reporter, which registered the destination during launch. This made plain Restart available
again and failed the live check.

## Open questions and possible follow-ups

Goose stores Farhelm's reporter in each conversation it creates with Farhelm. The reporter in the named destination can
start again during named Restart after Farhelm clears the old capture. The test reports the failure. The TODO follow-up
covers how named Create and named Restart handle that reporter while preserving ordinary Goose Restart. The selector is
a one-time choice, so the documented behavior leaves plain Restart unavailable after the named launch.

An authenticated Cursor installation can verify the wired case. Cursor's `create-chat` allocated a test conversation ID,
but the interactive CLI asked for login before the seed turn.

## PRs

- [#4786](https://github.com/scode/farhelm/pull/4786/changes) adds the named-conversation drift category and records the
  Goose follow-up.

## Checks run, reused and skipped

- Through the recorder, `python3 -B scripts/test-run-harness-drift.py` passed 18 stand-in checks at `263bb1b6` (run
  `72eca9b4-bbff-4922-a3b3-4f61a9aa4450`). The runner and its stand-ins stayed unchanged.
- Through the recorder, `cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'` passed all
  18 selected tests at `736f7b56` (run `c41d37c5-ec7e-4e92-984a-3a8cff107d2e`). Listings with
  `python3 -B scripts/run-harness-drift.py --list --category named_conversation` found all 10 cases.
- `cargo clippy -p farhelm --all-targets -- -D warnings` and `cargo fmt --all -- --check` passed on `d3e96eb2`, the
  final Rust source. `dprint check TODO.md docs/harness-drift-tests.md` passed on the final Markdown.
  `python -B scripts/check-test-sleeps.py` passed on `736f7b56`. The subsequent test edit only moved when the ignored
  Goose case returns its failure, so the ordinary drift and delay checks still apply. The rebase added queue and report
  bookkeeping to the base with an identical test patch.
- At `736f7b56`, `python3 -B scripts/run-harness-drift.py --no-upgrade --category named_conversation` attempted all 10
  cases and exited 1 on the Goose failure. The table gives each case's recorder ID. The last Goose-only change preserves
  the recall check if the reporter registers the destination before the first observation.
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category named_conversation --harness goose` reproduced the
  same post-recall failure on `b07db0fb` (run `0cfbb4c2-6cfd-4310-90ea-cc74e7a053a8`).

| CLI         | Installed version  | Live result                                           | Recorder run                           |
| ----------- | ------------------ | ----------------------------------------------------- | -------------------------------------- |
| Antigravity | 1.5.16             | Passed                                                | `0d80842a-c4ba-4b6b-a916-a4affc13f51a` |
| Claude      | 2.1.332            | Passed                                                | `f76adb52-4300-4767-a9e0-8ec931cadaf6` |
| Codex       | 0.172.0            | Passed                                                | `e59e3ce3-bcd6-40d3-8dae-3174e2a8bd9d` |
| Cursor      | 2026.11.22-e9042b2 | Could not run past login                              | `272320c0-3abd-4e3e-9b40-ebbfadb58b81` |
| Goose       | 1.60.2             | Failed after recall because plain Restart was offered | `e97c50f1-8da4-4081-82f2-87c2c832a187` |
| Grok        | 1.0.92             | Passed                                                | `cd5d150e-0a6b-4b3b-bd96-f358feb45c3e` |
| Muse        | 1.4.25-R9361.1     | Passed                                                | `20715c0a-541b-42a4-8f0b-aea532e12e42` |
| OMP         | 19.1.1             | Passed, absolute and relative files                   | `6fd193ed-749a-469c-8d16-1a9cdc093fea` |
| OpenCode    | 1.19.6             | Passed                                                | `a1a66e9d-8b37-4990-9a7e-a375f0503b2f` |
| Pi          | 1.6.0              | Passed, absolute and relative files                   | `9df26ea9-6edf-4ff4-8775-c85f44fa35f5` |

- Vendor upgrades, unrelated live drift categories and the full workspace suite were skipped. This plan asked for checks
  of named starts on the installed versions and changed only opt-in tests, their documentation and TODO.md.

## Review gate

A fresh-context Sol high reviewer checked the code and its follow-up changes. The review caught that Goose could pass
recall against its ordinary Restart target and that a later credential error could conceal a failed recall. The test now
targets a different conversation and treats a credential refusal after a successful seed as a failure. A separate cold
reader reviewed the commit and PR title. The review gate cleared after the corrections.
