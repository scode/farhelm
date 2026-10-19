## What this was about

This plan carries out three recorded decisions about agents composing launches and sending messages to another running
agent. A saved wrapper could be paired with a different agent type without another wrapper approval; a prompt card's
permission shield could change while the card waited; and an empty message could press Submit on text the user had
already left in the target agent's composer.

The first two behaviors are accepted specification rules. The third is refused: the prompt command now requires text
that is not empty after trimming whitespace, at the command line, the asking supervisor's relay, the helm and the target
supervisor. Accepted messages keep their exact bytes. New-session initial prompts retain their existing rules.

## Things you should know

`farhelm agent prompt` remains turned off. The helm still refuses every request before target lookup, approval or
supervisor contact. These changes do not re-enable it. Its specification NOTE and re-enable TODO now name only the
remaining stale-status gap described below.

A wrapper prefix is not tied to an agent type. Whoever composes the launch chooses both and is responsible for matching
them; a template's prefix stays user-configured when a later command-line flag changes the agent. The prompt card's
permission shield and its open-in-a-client notice are informational snapshots that may change while approval waits.

The finished agent-wrappers report still contains its original open decision. It was left unchanged because finished
plan reports are retained records; the current decision is now in SPEC.md and the corresponding TRIAGE_OUTCOMES.md
entry. All three feedback files and index entries are removed, and their outcome entries identify their draft PRs.

There is no storage or protocol change. The fix has a `kind: none` changelog fragment because the prompt command was
turned off before release and remains off.

## Open questions and possible follow-ups

Re-enabling agent-to-agent prompts still requires fixing or explicitly accepting the remaining status-sample gap: a
permission or trust question that appears after the latest status reading can receive the message and submit key as its
answer. This plan leaves that decision to the existing re-enable work.

## The PRs

- [#2346 — template wrapper responsibility](https://github.com/scode/farhelm/pull/2346/changes): specification and
  outcome cleanup.
- [#2347 — permissions shown on prompt cards](https://github.com/scode/farhelm/pull/2347/changes): snapshot rule and
  narrowing of the disabled-command checklist.
- [#2352 — blank prompt refusal](https://github.com/scode/farhelm/pull/2352/changes): command-specific validation,
  regressions and final checklist narrowing.

## Checks run, reused and skipped

Run for this stack:

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`: passed for the changed Rust paths and
  their callers.
- Through `scripts/record-test-run.py`,
  `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --lib -E '<eight named prompt contracts>'`:
  8 passed, run `cbcd694f-d355-4d08-823f-e845b2fc5b9a`. The selection covers initial-prompt preservation, command
  validation, enabled and disabled helm admission, and target refusal followed by literal-byte delivery. The supervisor
  tests use an owned raw-input recorder with pinned tmux, not a vendor agent. Four nextest slots, zero retries; other
  tests were selected out.
- Through the recorder, `cargo nextest run -p farhelm --test agent_cli -E 'test(prompt_)'`: 8 passed, run
  `ac1f21b3-71a2-4b99-aab3-3506dfda0a69`. This covers blank literal/file/stdin input, exact nonblank message bytes and
  existing initial-prompt CLI rules. Four slots, zero retries, tmux not used. Other tests were selected out.
- Retained output and complete manifests/JUnit were read for both runs: no runtime skips, truncated output or forced
  cleanup.
- `python -B scripts/check-test-sleeps.py` with the isolated interpreter from `docs/test-sleep-check.md`: passed for the
  Rust test changes.
- Targeted `dprint check` on SPEC.md, TODO.md, TRIAGE_OUTCOMES.md and the fragment;
  `python3 scripts/check-agents-md.py`; `python3 releasing/check-changelog.py format`: passed for the specification,
  feedback removals and fragment.

No results from an earlier task were reused. The final PR URL amendments changed only outcome bookkeeping, so the
recorded code checks remain applicable. Main moved only through two queue claims after the stack's base; its diff was
inspected and had no functional interaction, so no rebase or repeated runtime checks were needed.

The first two PRs ran document checks only. Browser, desktop, systemd, SSH and vendor-agent suites were skipped: this
stack changes specification and message admission, with the delivery boundary covered by the owned recorder; it changes
no UI, service setup or harness input sequence. End-to-end prompt delivery tests remain ignored while the command is
off. No live installation was changed.

## Review gate's outcome

Each PR passed its own independent fresh-context gpt-6.1-sol high review for correctness, design and idiomatic code or
prose. No findings remained. Each commit message also passed a fresh-context cold read; unclear wording was revised.

### Landing

Landed as #2346, #2347 and #2352, merged bottom-up, unchanged from what the report describes. The other plan in the same
landing round, link-hover-covers, lands after it.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained only plan bookkeeping. The other plan in this round, link-hover-covers, changes how
terminal file and web links refuse clicks when something covers their hover details, and when a desktop Cmd-click may
open a downloaded file. A fresh-context reviewer and I read the two against each other:

- They edit different sections of SPEC.md: this plan the agent-spawned sessions section, the other the terminal section.
  Neither contradicts the other's text.
- Both remove their own entries from the review feedback queue's index. That was the only conflict, and both sides'
  removals are kept.
- Only this plan edits TODO.md, and the two plans' TRIAGE_OUTCOMES.md entries are separate.

The reviewer also confirmed that, at the helm, the switch turning agent prompts off still refuses before the new
blank-message check, so a blank message that reaches the helm gets the same "turned off" refusal as any other (the
command line itself refuses a blank message before sending anything). A supervisor still on 0.30.0, which lacks the
blank check, is therefore never asked; once prompts are turned back on, the helm's own blank check still stops a blank
message before it reaches such a supervisor. New sessions' first messages keep their old rules, including allowing an
empty one.

#### Fixes made during the landing

None.

#### Possible follow-ups

- When agent prompts are turned back on: a message made only of invisible characters that do not count as whitespace (a
  zero-width space, for example) passes the new check, and its submit key would still submit whatever the user had typed
  in the target agent's input box. SPEC.md already says a message may mix with text in that box, so this is within the
  stated design, but the re-enable work may want to refuse such messages too.
- SPEC.md lists the places that refuse a blank message as the command line, the helm and the target session's
  supervisor. The asking session's supervisor, which relays the request, refuses it too, as the report says. Adding it
  to that sentence would make the list complete; nothing behaves differently either way.

#### Checks

Run now, on this stack rebased onto main with link-hover-covers on top (link-hover-covers changes no Rust code; its
browser and JavaScript checks from the same run are reported in its own landing notes):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the isolated test-sleep check, `dprint check` on every changed
  documentation, changelog, stylesheet, script and test file, and `python3 releasing/check-changelog.py format`: all
  passed.
- Through the test recorder with pinned nextest and tmux, four slots and no retries, the whole protocol, helm and
  supervisor libraries, the `farhelm` binary's own tests and the agent command-line test file: 2762 passed, run
  `13dd7643-487f-4153-b8a3-34e3a79e91cc`. The two tests nextest skipped are helpers that only run as child processes of
  other tests, and the output had no runtime `SKIPPED` marker. The executor had run selected tests only. The run's
  record was read after it finished and then lost when the machine rebooted before this plan was recorded; the result
  above is from that reading.
- Skipped: browser, desktop, systemd, SSH and vendor-agent suites, and the end-to-end prompt delivery tests (still
  ignored while prompts are off), for the report's reasons. `python3 scripts/check-agents-md.py` was skipped because the
  stack changes no repository instruction file.

Nothing in the executor's report became untrue at landing.
