# Codex conversations through launchers that stay running

## What this was about

A Codex session started through a launcher named `codex` that stayed running as Codex's parent lost all its conversation
reports. Farhelm could not learn which conversation the session was in, so Restart stayed unavailable.

The fix follows the Codex process whose conversation report Farhelm accepts first, while still checking that the report
comes through the supported hook and names a verified Codex transcript. Launchers above that process can stay running.
Later reports from nested Codex processes cannot replace its conversation. Restart uses the session's configured resume
command, so a launcher session resumes through its launcher.

## Things you should know

If a nested Codex reports first, Farhelm initially follows that conversation. A verified report from a Codex process
that started it corrects the choice, including that process's end-of-turn report. The session gets one notification per
launch explaining what an earlier Restart would have resumed and which conversation Farhelm now follows. The correction
and notification are saved together; a failed warning write leaves the earlier choice intact.

This rule applies to every Codex session. Sessions already running across the upgrade establish their reporting process
on a subsequent accepted report, so a nested conversation can briefly be followed until its parent reports. With several
nested Codex processes, a correction can move to an intermediate process; the notification does not promise that no
further correction can happen. This is the first-report tradeoff the plan requested.

The reporting-process choice survives a supervisor restart, but every relaunch clears it. If the followed Codex exits, a
fresh Codex in the same terminal can report its own conversation. A delayed descendant report that still records the
previous process in its ancestry remains refused, even if that process changed executable before exiting.

The specs and the Codex, agent wrappers, and agent-hook explanation pages describe these rules. The TODO entry is
removed and a fixed changelog fragment is included. No other agent's attribution rule changes.

## Open questions and possible follow-ups

There is no unresolved implementation decision. The original affected environment was not exercised. After this ships,
the maintainer should try a Codex session through its launcher: confirm conversation tracking, use `/clear`, and Restart
into the new conversation. The automated proof uses a synthetic native launcher and the shipped hook, supervisor,
storage, notification, and Restart paths.

## PRs

- [#1945 — capture codex conversations through launchers](https://github.com/scode/farhelm/pull/1945/changes), draft,
  bookmark `plan/codex-resident-launcher/01-pin-reporter`, head `aca2eade8626f8525732a83d89ebbb55e632564b`.

## Checks run, reused and skipped

Runtime checks used the pinned nextest and tmux with four slots and no retries. Selected-out tests are not counted as
coverage; the selected tests' retained output contains no runtime skip marker.

| Evidence                                                                                                                                                                      | Outcome and coverage                                                                                                                                                                                           |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo nextest run -p farhelm-supervisor --lib -E 'test(codex_pin) \| test(notification)'`, run `7044e65a-45b6-43b2-8049-99cefc6cb7b9`                                        | 16 passed after the review fixes: admission, full-chain dead-pin refusal, atomic warning rollback/reopen, and shared notification sequence, cap, recurrence and resolution.                                    |
| `cargo nextest run -p farhelm --test e2e -E 'test(codex_identity::)'`, run `473f70fe-1c80-4d9d-b4f1-af09b4638466`                                                             | Five passed after the review fixes: resident launcher capture and Restart, child-first correction by both callbacks, strict shell-descendant refusal, and the existing nested/clear/transcript/outage journey. |
| Supervisor storage, Codex locator and corridor selection, run `e1c9f498-3d55-44bb-bc5c-b57333c41d2c`                                                                          | 125 passed. Reused for unchanged migration, persistence and locator contracts; the subsequently extracted notification helper and changed admission paths were rechecked above.                                |
| `cargo clippy --all-targets -- -D warnings`                                                                                                                                   | Passed. Reused for unchanged workspace targets after the review fixes.                                                                                                                                         |
| `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`                                                         | Passed after the review fixes, including the shipped binary configuration without test seams.                                                                                                                  |
| `cargo fmt --all -- --check`, changed-file `dprint check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`, and diff whitespace check | Passed. The source checker inspected 295 deliberate delays and found zero missing explanations.                                                                                                                |
| `bun install --frozen-lockfile && bun run build`                                                                                                                              | Passed, including internal links. Reused because the website pages did not change during review fixes or rebase.                                                                                               |

The first focused run, `27fcaee7-2ee7-48ef-bfc7-adbe5fb42ccb`, failed on an absent-Codex diagnostic and two incomplete
new transcript fixtures. Those issues were fixed; narrow retry `9d05d752-3bad-4281-80ea-6b44c7f3756d` passed all three.
A separate runner-path mistake refused execution in `330db88c-4f8a-4703-8093-b615f3ce2b20`; the documented guarded setup
was then used. Failure evidence remains recorded in `farhelm-plan-codex-resident-launcher-log.md` beside the checkouts.
These were same-session development failures, not latent flakes.

The careful rebase read the planning PR and claims that landed while execution ran. They added two plans and their TODO
references, with no product code or authoritative-spec change. Rebase preserved the tested Rust, specs, website and
changelog files byte-identically, so those checks remain applicable. No extra runtime run followed from the rebase.

The full Rust battery, browser suites, desktop runtime, installer and release gates were skipped: the changed behavior
has direct process/store/Restart coverage, the notification wire format and UI are unchanged, and those broader suites
would add no identified missing evidence. No hosted CI or website deployment was triggered.

## Review gate outcome

A single fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, idiomatic Rust and test quality
against the full goal and test-authoring checklist. All three findings were accepted: a dead process whose executable
changed could lose its descendant fence; a correction could lose its warning after a shutdown; and shared contract text
still described the old rule. The fixes keep the complete ancestry fence, save adoption and warning atomically, and
correct the stale documentation. The same reviewer's follow-up found no remaining issues. Its source review is separate
from the observed runtime passes above. Commit and PR wording also passed the required cold read.
