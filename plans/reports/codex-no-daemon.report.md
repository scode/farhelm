### What this was about

Codex can run several terminal clients through one shared background server. That server keeps the environment of the
client that started it, so a Farhelm session could run commands with another session's identity and outside its own
process ownership. Farhelm's conversation-hook options happened to prevent this, but turning the hooks off removed that
protection.

Farhelm now passes `--no-daemon` whenever it fills Codex's `{farhelm_args}`, on start and resume, including when hooks
are disabled or skipped. The hook's status and diagnostic decision stay the same. The implementation and product
specifications, the Codex and Agent hook injection docs pages, and the changelog explain the behavior; the covered TODO
entry is removed.

### Things you should know

This requires Codex 0.156.0 or later. A custom command declared as Codex fails if it puts `{farhelm_args}` after `exec`,
passes `--remote`, or runs `codex agents` or `codex queue`. Farhelm does not inspect the command to detect these cases.
Those failures are accepted limitations recorded in the plan; no version probing or command parsing was added.

Existing agent launches and declared-Codex commands gain the flag at their next start, restart or resume. Processes
already running keep their current arguments. Legacy sessions created before Farhelm 0.22 from typed commands or
profiles keep their frozen launch behavior and can still attach to the shared server; recreating them gives them current
launch arguments.

The flag removes Codex's "Running without the shared background server" warning. Its separate hook-trust warning still
appears when the conversation hook is enabled. The source claims were verified against official Codex `rust-v0.163.0`,
commit `83086188`, and the 0.155.1/0.156.0 sources for the version boundary. No live shared server was inspected or
changed.

### Open questions and possible follow-ups

Codex typed manually inside a shell session does not receive Farhelm's generated arguments. Handling that was explicitly
outside this plan. There is no remaining decision needed for this change.

### PRs

- [#2820: isolate Codex commands from the shared server](https://github.com/scode/farhelm/pull/2820/changes), one draft
  PR, head `c4031943`.

### Checks run, reused and skipped

- Passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo clippy -p farhelm --bins -- -D warnings`.
- Passed recorder-backed supervisor tests selected by
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(agent_kind::) | test(farhelm_args_expand_at_the_placeholder_per_kind_phase_and_setting) | test(hook_skips_notify_except_for_the_users_own_opt_out)'`:
  92 passed, 1,274 not selected, run `66c8d1f9-6b3a-4db5-93f1-36ae744da559`. This covers start/resume, hooks on/off, the
  unavailable-executable hook seam, unchanged hook verdicts/logs, and the other agent integrations. Retained output
  contains no runtime `SKIPPED` message.
- Passed recorder-backed
  `cargo nextest run -p farhelm --test e2e -E 'test(=hook_identity::hooks_can_be_disabled_by_kind)'`, run
  `86e93b09-4cd5-4387-b81d-5cbeaa8c249e`: one test passed, fixture setup passed, 405 cases not selected, no runtime
  `SKIPPED` message. This is the single changed real-spawn argument check. Both Rust runs use the pinned nextest and
  tmux, four slots and zero retries.
- Passed `python -B scripts/check-test-sleeps.py` with the isolated pinned interpreter,
  `python3 releasing/check-changelog.py format`, `dprint check`, and `python3 scripts/check-agents-md.py`. The
  instructions check reports three existing unrelated citation warnings and zero errors.
- Passed the website's frozen dependency install and build: 33 pages, all internal links valid. It retains the existing
  empty-i18n/404 warnings. No preview was started because these are incidental docs edits in an unattended run.
- No earlier runtime evidence was reused. The checks cover the same file tree as `c4031943`; committing changed metadata
  only. Browser, desktop, installer and provisioning suites were skipped because no such behavior changed. The optional
  real-Codex probe and vendor screen-capture harness were skipped: upstream parser/source evidence is sufficient, and no
  vendor turn or shared-server mutation was needed.

### Review gate outcome

The single required fresh-context `gpt-6.1-sol` reviewer at high effort found no correctness, design, idiomaticity or
test-contract issue. It independently checked the cited Codex sources. The separate commit-message cold read passed. A
documentation pass covered every touched file. No scope departure or corrective review round was needed.
