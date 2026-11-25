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

### Landing

#### What landed with it

Two plans landed this round, both adding a category to the opt-in live drift tests: drift-initial-prompt-launch first,
then this one, through [#4786](https://github.com/scode/farhelm/pull/4786/changes) and the landing fix
[#4796](https://github.com/scode/farhelm/pull/4796/changes). Both were written before the previous round's drift plans
landed, so they were rebased onto those. The overlap was in TODO.md (each plan's planned entry removed, every follow-up
kept), the drift docs, and the list of categories in the drift module, where both register theirs. This plan also moved
the shared check for an agent's reply into a helper that both categories and the older ones now use, with the same
behavior. Claude now starts with its background updater turned off, as every drift case does since the previous round. A
planning change for the triage outcomes also reached main meanwhile, touching no code.

#### Fixes made while landing

#4796 changes only the drift tests, their docs and TODO.md:

- **Per-agent choices now name every agent.** The category chose the check for an agent without conversation capture
  through a catch-all branch, and decided how to find the conversation ID by comparing against Cursor by name, so any
  other agent would have used OpenCode's session list. Only Cursor and OpenCode reach that code today, so nothing
  misbehaved, but the repository's code rules forbid both patterns, because an agent added later silently gets the
  default. Each now lists every agent.
- **Cursor's unverified case now has a follow-up.** The Cursor case stopped at Cursor's login screen before its seed
  turn, and no TODO entry said it still needs a signed-in run. The existing Cursor login entry now includes it.
- **The docs said the seeded conversations live in the test folders.** Each agent keeps them in its own conversation
  store, like any other conversation; the docs now say so.

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
Claude that way, so the reuse holds. Neither category's Claude case has run live with it.

Skipped: the live drift cases themselves, since they spend vendor turns, need the vendor logins and are opt-in by
design; the workspace-wide Rust suite and the desktop checks, since no shipped code changed; and CI, which runs only on
demand.

#### What the report says that is no longer true

Nothing. The landing's own review found the report accurate.

#### Things you should know that the report does not say

- **The Goose failure is a mismatch between Farhelm's documented behavior and what users see.** After a Goose session is
  restarted onto a different conversation chosen by its ID, plain Restart becomes available again, though Farhelm says
  Restart is unavailable after such a one-time choice. The cause is Farhelm's conversation reporter, which Goose saved
  inside that conversation when Farhelm created it and which registers the chosen conversation again at launch. From the
  report, that offered Restart resumes the conversation the user chose, so nothing appears to be lost; what is wrong is
  that Farhelm offers what it said it would not. The fix could go either way (stop the reporter, or change the
  documented behavior), and the TODO follow-up records the problem for both named Restart and the untested case of
  creating a new session on a chosen conversation. Until it is settled, this category's Goose case reports FAILED on
  runs where Goose 1.60.2 is installed and signed in, including in the "run all tests" flow; the TODO entry already
  covers that failure.
- **The seeded test conversations stay in each agent's conversation history.** Each case creates a short conversation
  with a random label from a temporary folder, and the agent keeps it in its own store afterwards, as every drift
  category's turns already do.
- **OpenCode's conversation lookup exists twice, and the two copies disagree**, as the launch prompt plan's landing
  notes describe: the same OpenCode behavior could read as FAILED here and as COULD NOT RUN there. No TODO entry records
  sharing it yet.
