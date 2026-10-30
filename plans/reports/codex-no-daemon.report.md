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

### Landing

Landed as delivered, with no fixes at landing. Farhelm now starts Codex with `--no-daemon` on every start and resume,
whether conversation hooks are on or off. Without it, Codex can hand a session's commands to one shared background
server, which keeps the environment of whichever terminal started it, so a session could run commands with another
session's identity and outside the session's own process tree, where Farhelm does not track or stop them. Until now only
Farhelm's conversation hook arguments happened to prevent that, and turning hooks off removed the protection.

- #2820: the flag on every Codex launch, its tests, the specification text, the Codex and hook-injection docs pages, a
  changelog entry, and removal of its TODO entry.

#### What landed since the stack was based, and whether it interacted

Only host-short-names (the sidebar's host captions) and plan bookkeeping landed since; nothing interacted.

#### Things you should know

- Codex older than 0.156.0 no longer starts under Farhelm, since it does not know the flag. The plan accepted this with
  no version check; it fits the two-week agent version support window in SPEC.md, and SPEC.md, SPEC_impl.md, the Codex
  docs page and the changelog entry say so.
- A custom command declared as Codex fails if it places `{farhelm_args}` after `exec`, passes `--remote`, or runs a
  Codex subcommand such as `codex agents` or `codex queue`; the plan accepted that too. The docs list those four as if
  complete, but the review's reading of the Codex source suggests other subcommands (`codex review`, `codex cloud`)
  would also reject the flag.
- Sessions already running keep their current arguments until their next start, restart or resume. Sessions created
  before Farhelm 0.22 from typed commands or profiles keep their old launch and can still use the shared server.
- Follow-ups the review found, none of them blocking: the agent-screen capture tools and the instructions for trying a
  real agent in a terminal still start Codex without `--no-daemon`; the Custom commands and Agent wrappers docs pages
  still describe `{farhelm_args}` as only about conversation tracking, though for Codex dropping them now also loses the
  session's own processes; and the hook-injection page still lists Codex versions it was verified on (0.149.0, 0.149.1,
  0.155.1) that can no longer start.

#### Checks

This round landed only this plan, stacked on main `08af8c01`. On the stack as delivered:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`), `dprint check`, `python3 releasing/check-changelog.py format`, the docs
  website build (`bun run build` in `website/`) and the workspace debug build: all passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, no retries, run `f0b300d3-7298-4a62-acbe-29bcb28a82ac`: 4555 passed, 41 skipped by design, 2 failed, both known
  flakes in code this plan does not touch ("shallow pause resumes without reset or replay" and "a hung save releases
  waiters at their deadline"). The end-to-end Codex launch, identity and hook tests ran and passed, and no test reported
  itself skipped at runtime.
- Skipped: the browser suite (the browser tests' fake agent accepts any extra argument and no spec checks a Codex
  command line), the UI's JavaScript and desktop tests (no UI change), and the native Mac app (not runnable on Linux).
  No real Codex was started. Reused: nothing from the executor's runs.

The report's checks were the executor's selected runs, which included only one of the end-to-end Codex launch tests;
this landing ran the whole Rust workspace, which covers the rest. Nothing in the executor's report became untrue at
landing.
