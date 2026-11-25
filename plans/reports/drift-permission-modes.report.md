# Permission-mode drift tests

## What this was about

Farhelm's real-agent drift suite exercised one launch choice per harness. It could miss a vendor dropping a permission
flag, a YOLO label changing, or a Custom permission choice losing its arguments on Restart. The maintainer wanted that
coverage without prompting tools to trigger permission dialogs. Cursor's retired Agent's own setting was dropped from
the plan.

## Things you should know

- The opt-in category covers Claude, Codex, Antigravity, Grok, Muse, OMP, Goose and Cursor. It checks the actual launch
  words and requires the installed CLI to reach its message input. Claude, Grok, Muse and Cursor also check whether
  Farhelm sees the live YOLO label. Goose checks Approve, Smart approve and Chat, and OMP also checks Approve.
- For agents whose conversations Farhelm can reopen a Custom choice completes one text turn, then checks that the saved
  Restart command carries the literal flags and the new process reaches its composer. Cursor's Custom case only checks
  launch. The tests use the installed CLI versions and disposable Farhelm sessions.
- Six harnesses passed, one failed a command check, and one needed sign-in. Restart could fail to reopen the original
  Antigravity conversation when the user ends Custom permission flags with `--`. The live 1.5.16 test captured that
  conversation, then found the saved `--conversation` selector after the delimiter, where it would be treated as an
  operand. The test stopped before Restart. The same harness passed YOLO and Custom with `--settings` alone. Cursor
  2026.11.22-e9042b2 reached sign-in in both YOLO and Custom, leaving message readiness and the YOLO label unverified.

## Open questions and possible follow-ups

- The Antigravity fix belongs in a separate change. Moving the selector before the delimiter has to account for existing
  saved launch commands and the prefix used to edit workspace trust. A new TODO entry records the reproduction and the
  required ordering.
- The existing Cursor follow-up now includes the two permission cases. They need an authenticated CLI to verify composer
  readiness and the YOLO label.

## The PRs

- Add live permission-mode and Custom Restart coverage. [#4784](https://github.com/scode/farhelm/pull/4784/changes)

## Checks run, reused and skipped

- `python3 -B scripts/test-run-harness-drift.py` passed all 18 stand-in tests, recorder
  `2e76ea51-53f3-47c2-906b-5180001fe599`. The run used base `0ec5e7dd` with the test change. The runner and its
  stand-ins stayed unchanged. The broader `test(/^real_agent_capture::drift::/)` nextest selection passed 17 cases at
  `532bb3bb`, recorder `4c7e4fc3-fb49-4a0c-96e3-a78a7caa7fff`. Its base, model, hook and private-configuration cases
  still apply because the changes after that run were confined to the permission category and TODO. At `042ff0d3`, the
  two permission guards passed under
  `cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::permission_modes::/)'`, recorder
  `ad8613f0-da9d-4b98-848f-6a1b48a01d74`.
- `python3 -B scripts/run-harness-drift.py --list --category permission_modes` found exactly eight cases. Rust and
  Markdown formatting, the test-delay check and `cargo clippy -p farhelm --all-targets -- -D warnings` passed.
- `python3 -B scripts/run-harness-drift.py --no-upgrade --category permission_modes` at `042ff0d3` attempted all eight
  cases. The runner exited nonzero on the Antigravity ordering defect.

| Harness     | Version            | Verdict                    | Recorder run                           |
| ----------- | ------------------ | -------------------------- | -------------------------------------- |
| Claude      | 2.1.332            | Passed                     | `a29f0feb-38f2-4737-b2fb-532a675d6357` |
| Codex       | 0.172.0            | Passed                     | `c7d542a1-0d24-405d-a271-60489983ecf9` |
| Goose       | 1.60.2             | Passed                     | `9a3dbc91-9bae-4b0b-a565-2a99f2e5e828` |
| Grok        | 1.0.92             | Passed                     | `f9e3a04e-c4b5-4cce-b5bd-dfa20530fbf7` |
| Muse        | 1.4.25             | Passed                     | `5a25f325-97f7-4054-98b4-ed8698b9d979` |
| OMP         | 19.1.1             | Passed                     | `7c78a6f9-bf88-4e49-9b20-4df7eaf9a968` |
| Antigravity | 1.5.16             | Failed on Custom `--`      | `6f6559ef-ebb4-4fc3-bff9-23aeecb55d49` |
| Cursor      | 2026.11.22-e9042b2 | Could not run past sign-in | `a8f3a2b4-4fbf-4976-8b59-5256fd83bf2e` |

- The initial Antigravity ordering failure is retained as `1f231a5a-f983-46f8-aa2a-300586a6d526`. The initial Claude run
  `884678c6-e545-4654-b2c0-4956b749d4cb` recorded a rendered reply with a space between the two letters the test
  expected. The final check requests one exact word and verifies that the prompt echo cannot count as the reply. Claude
  then passed.
- The full workspace run was skipped because the diff changes an opt-in category, documentation and TODO entries. The
  selected unit, runner and live checks cover those changes.

## Review outcome

The source review gate cleared after corrections. The independent review found that a trust prompt could count as ready,
that several Custom flags matched the defaults, and that the Antigravity test's original word comparison would also
reject a valid placement of the selector. The category now waits for the composer, uses distinct Custom inputs and
checks the selector and delimiter independently. The remaining Antigravity failure is a product follow-up. The commit
wording passed its cold read.

### Landing

#### What landed with it

Two plans landed this round, both adding a category to the opt-in live drift tests: drift-reasoning-effort first, then
this one, through [#4784](https://github.com/scode/farhelm/pull/4784/changes) and the landing fix
[#4792](https://github.com/scode/farhelm/pull/4792/changes). They touch the same files only where each registers its
category and records its own follow-ups in TODO.md, so stacking them needed nothing but keeping both sides. Main also
gained three docs changes while they landed (two reorganizations of TODO.md around Tensorlake, and a triage of the
review queue), none of which touches the drift tests.

The reasoning effort plan's landing fix turned Claude's background updater off for every drift case, this category
included, so a Claude launch and its Restart here now always test the same Claude version.

#### Fixes made while landing

#4792 changes only the drift tests and their docs:

- **Per-agent choices now name every agent.** The category picked the agents whose YOLO label Farhelm reads from the
  screen by naming four of them, and handled Antigravity's `--` (which ends its options) by comparing against
  Antigravity by name. The repository's code rules forbid that, because an agent added later silently gets the default.
  Each now lists every agent. The list of agents whose YOLO label is read repeats one the supervisor keeps, which the
  test cannot use directly, so its comment says to update it when the supervisor learns to read another agent's label.
- **The docs paragraph moved.** It sat between the model selection category's paragraph and that category's Codex note,
  which then read as if it applied to every category. It now follows the other categories' paragraphs.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. Both plans landed this
round (drift-reasoning-effort and drift-permission-modes) change only the opt-in live drift tests and their docs, so the
landing ran the drift tests that need no vendor CLI and the checks that inspect the changed files, once on the two plans
stacked together with their landing fixes, which is what main now holds.

On the stacked plans with the landing fixes (`3ac3fd12`):

- `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`, on the test worker
  (recorder run `b68d5437-426c-4166-a747-29c27d4df8a8`): all 20 passed, with no runtime skip. They include each new
  category's checks that every harness offering the feature has a case.
- `python3 -B scripts/run-harness-drift.py --list --category all`, on the test worker: it built the drift tests and
  listed ten base, nine model selection, eight permission modes and seven reasoning effort cases, starting no agent.
- `cargo clippy --all-targets -- -D warnings`, on the test worker: passed.
- `python3 -B scripts/test-run-harness-drift.py`, locally (recorder run `148edde1-d9c4-45da-a873-6d2ee6294170`): all 18
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 scripts/check-agents-md.py` (no errors; one warning already on main), locally: passed.

The same set had also passed on an earlier revision of the landing fixes (`441426b3`, drift tests in recorder run
`c4b82231-1bd2-43e9-a2dd-8c2647f4cff1`).

Reused: the executors' live runs of the new categories against the installed agents, recorded in the reports. The
landing fixes change how the per-agent choices are written, not what any agent is launched with, with one exception:
Claude now starts with its background updater turned off in every category, not only the reasoning effort one. That was
already how the reasoning effort runs in the report launched Claude.

Skipped: the live drift cases themselves, since they spend vendor startups or turns, need the vendor logins and are
opt-in by design. That leaves Claude's updater setting unexercised live in the other categories; the launch with it is
the same one the reasoning effort runs used, and the drift tests above check how the launch command is built. Also
skipped: the workspace-wide Rust suite and the desktop checks, since no shipped code changed, and CI, which runs only on
demand.

#### What the report says that is no longer true

Nothing. The review found the report accurate.

#### Things you should know that the report does not say

- **The Antigravity Restart problem the report describes is a bug users can hit today.** A user who launches Antigravity
  with Custom permission flags ending in a bare `--` gets a Restart that does not reopen the conversation, because the
  conversation option lands after the `--`. The trigger is uncommon, and what Antigravity does with the misplaced option
  instead was not observed: the test stops before Restart. The fix is recorded as a TODO follow-up, not made here. Until
  it lands, this category's Antigravity case reports FAILED on every run where Antigravity is installed and signed in,
  including in the "run all tests" flow, which since the model selection plan's landing runs every drift category. The
  TODO entry already covers that failure, so a run need not record it again.
