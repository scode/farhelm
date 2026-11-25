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
