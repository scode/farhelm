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
