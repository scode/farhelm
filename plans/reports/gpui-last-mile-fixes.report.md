## What this was about

The desktop launcher treated a suspended sandbox or container like any other disconnected host. The refusal left you
without a way to tell why the launch was refused or what to do next. The session card used generic wording while a
provider was suspending or resuming a host. Clone hid some characters in a copied folder or session name. This plan
closes those gaps and records the disposition of the remaining launcher cases from the browser suite.

## Things you should know

- The released browser app resumed the host as part of Launch. On desktop, Launch and quick launch now name the host and
  ask you to resume it in Hosts. A resumable container can show “Stopped” in Hosts while the launch refusal says the
  host “is suspended”. As agreed for this plan, automatic resume during launch stays a separate Near term item.
- The unavailable session card names Tensorlake or the container host while it waits for the provider. Clone spells out
  hidden characters in copied fields, including the numbered default name for a managed checkout. A launch with the copy
  still untouched sends the original characters.
- The desktop checks cover checkout discovery and host selection, refusal help, focus during launch completion, and
  destinations restored from templates. Replace with can retry the same draft after a missing-folder refusal. If its
  source disappears before the reply, success opens the replacement when the source was open. A refusal shows the error
  and falls back to the last visited session still present.

## Open questions and possible follow-ups

- The browser launcher rechecked its checkout setup when settings changed elsewhere while it was open. The desktop
  launcher lacks that refresh trigger. Restoring it would change behavior and give the old external-change case a
  desktop counterpart.
- A test with replies that can finish out of order could verify that a launch completes on another host before Suspend
  finishes, and that checkout setup on a newly selected host settles before its old preview. The ordered fake helm lets
  the desktop tests verify that both later requests are sent.
- A test with a real host and folder could verify that New, Clone and Replace with recover when the original path is
  repaired. The Replace with test scripts a successful retry with the same draft. The shared New and Clone create test
  retries after editing the folder.

## PRs

- Launcher and quick-launch advice for suspended hosts. [#4056](https://github.com/scode/farhelm/pull/4056/changes)
- Provider wording on the unavailable session card. [#4058](https://github.com/scode/farhelm/pull/4058/changes)
- Spelled-out hidden characters in Clone fields. [#4059](https://github.com/scode/farhelm/pull/4059/changes)
- Checkout composer and host picker tests. [#4060](https://github.com/scode/farhelm/pull/4060/changes)
- Restart with hover, footer and template refusal tests. [#4061](https://github.com/scode/farhelm/pull/4061/changes)
- Launcher tabs, prompts and agent choices tests. [#4062](https://github.com/scode/farhelm/pull/4062/changes)
- Focus and selection during launch completion. [#4063](https://github.com/scode/farhelm/pull/4063/changes)
- Launcher search, keyboard navigation and recents. [#4064](https://github.com/scode/farhelm/pull/4064/changes)
- Launcher placement and host availability tests. [#4065](https://github.com/scode/farhelm/pull/4065/changes)

## Checks run, reused and skipped

- On the final stack at `086342d2`, `cargo fmt --all -- --check`,
  `cargo clippy -p farhelm-gui --all-targets -- -D warnings` and 19 tests passed. The run used
  `cargo nextest run -p farhelm-gui -E <launcher and environment cases>` through `scripts/record-test-run.py` with
  selection `last-mile combined launcher and environment paths` and recorder run `517ffe2e-a03b-46a3-b1fa-fb4f19bcb53b`.
  This includes the three #4065 cases for restored template placement, the selected host disappearing and a create sent
  during Suspend. It also covers the earlier suspension advice, Clone text, refusal, focus and destination interactions.
- The same worker commands passed each earlier draft's focused selection. The pre-rebase checks remain applicable
  because the rebase preserved each patch and main changed only plan records, documentation, capture scripts and a
  separate test runner. These are the per-draft recorder runs and their covered revisions:

  | Draft | Revision   | Passed | Recorder run                           |
  | ----- | ---------- | -----: | -------------------------------------- |
  | #4056 | `a41bbc3d` |      2 | `87a224c6-cc82-45ee-9201-df1f6a36ffff` |
  | #4058 | `dceed4bf` |      3 | `c836c0cb-bb27-48d8-aba8-36c92b12e752` |
  | #4059 | `a15af27b` |      4 | `2ab9af56-69f6-4eeb-9204-11c24401ebfe` |
  | #4060 | `e80e339`  |      6 | `1289b16e-9e67-4b51-9e6b-5c1315b0c4c1` |
  | #4061 | `56329c50` |      5 | `85953613-f102-4e3d-acdd-73c6700969e0` |
  | #4062 | `f78f339c` |      4 | `82851a96-6fb6-4a61-8449-816f01b0ca6f` |
  | #4063 | `bd994131` |      7 | `5afc97e8-c5f3-4014-a4ee-e0dda85d9f5b` |
  | #4064 | `4241cbe4` |     14 | `12c74642-c36d-4028-8743-7a3fcb1b6032` |

  #4056 had a later doc comment and parity wording edit with no effect on runtime. The isolated Clone and checkout
  selections were reused because the intervening changes covered separate launch paths. The final stack check exercises
  the overlapping destination and refusal paths together.
- `dprint check` on changed Markdown, `git diff --check`, `python3 releasing/check-changelog.py format` and the
  source-only `python -B scripts/check-test-sleeps.py` passed. `python3 scripts/check-agents-md.py` reported one
  existing archive citation warning and zero errors.
- Native text input owns composing Enter. The model picker tests that Enter only chooses a model through the shared text
  input rule. The window tests check where focus stays after Clone or Replace with. Live terminal input is covered
  separately.
- Skipped the whole-workspace and native Mac suites. The changes use the shared desktop launcher and session-card paths,
  exercised by the focused GPUI tests on Linux. No native Mac integration or capture code changed.

## Review gate

Passed. A fresh GPT-6.2 Sol reviewer at high reasoning reviewed each draft, following the maintainer's override of the
plan's GPT-6.1 Sol reviewer. The refusal now keeps right-to-left host names from reordering the surrounding advice. The
reviews also caught a second place that could display Clone's copied name raw and tightened the tests for checkout
discovery, missing-folder retry, keyboard focus and environment selectors. The reviewer follow-ups confirmed the fixes.
The commit messages and draft titles also passed separate cold reads.
