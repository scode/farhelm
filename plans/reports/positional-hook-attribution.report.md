# Attribute Grok and Codex reports to the process that ran the hook

## What this was about

Grok's installer points its launcher at a versioned executable. Farhelm used the executable name to recognize Grok's
conversation reports and rejected them on that install, so **Restart** could not resume the conversation. Codex depended
on its executable name too.

The maintainer chose the rule already used for Claude, Muse, and Antigravity. Determine which process ran the hook, then
tie the conversation to the process whose report was first accepted. A process above it in the same launch can correct
that choice by reporting its own conversation. Farhelm passes `--no-leader` when it starts Grok, which keeps Grok's
backend in the session. OMP keeps its separate launch check because another interactive OMP can send a report from
inside the session before the original process sends one.

## Things you should know

- A child can report first if the parent's first report is late or was lost. This includes a shell running a Codex hook
  before Farhelm has recorded a reporter. A later qualifying report from a recorded ancestor changes the selection and
  posts a correction notice. For Grok, that report must be a `SessionStart`. A corrected Grok selection needs a verified
  record before **Restart** can resume it. If the saved record is missing or mismatched at Restart, Farhelm keeps the
  selected conversation and withdraws the offer.
- The plan accepted an upgrade gap for Grok sessions that already had a conversation but no recorded reporter. A nested
  Grok's newer `SessionStart` can select a different conversation until the parent next reports its selection. Codex
  keeps any process and ancestors it already recorded. A child whose report still shows the recorded parent cannot
  replace it. Both Grok and Codex can release a pin for a process that has exited and is absent from the incoming
  report's ancestry. The new report still needs to pass its integration's checks. In particular, Grok's `--no-leader`
  check excludes the transient shell reporters that Codex can accept.
- The old Codex name rule could mistake the hook executable itself for the agent if someone named that executable
  `codex`. The shared rule starts above the hook invocation. The existing Codex integration tests continue to cover the
  supported native, wrapped and nested cases.

## Open questions and possible follow-ups

None identified.

## PRs

- Share the rules for retaining or replacing the saved reporting process.
  [#4724](https://github.com/scode/farhelm/pull/4724/changes)
- Restore conversation tracking and Restart for versioned Grok installs.
  [#4725](https://github.com/scode/farhelm/pull/4725/changes)
- Track a Codex conversation through renamed executables and handle a child-first shell report.
  [#4728](https://github.com/scode/farhelm/pull/4728/changes)
- Document the default for new integrations, OMP's exception, and accepted gaps.
  [#4729](https://github.com/scode/farhelm/pull/4729/changes)

## Checks run, reused and skipped

- `python3 -B scripts/run-harness-drift.py --harness grok --harness codex` passed at `d0262280` on Linux. Both updater
  checks reported already current. Grok 1.0.92 (run `aef9459f-366a-468c-ab43-34a529a32717`) and Codex 0.171.0 (run
  `89856f46-1765-4e55-8748-f238502e25bf`) each passed first report, Restart, recall, and a fresh accepted report for the
  resumed launch.
- The recorder ran pinned `cargo nextest run -p farhelm-supervisor --lib -E <selection>` with four slots and zero
  retries. These are the tested trees and exact selections.
  - `c3cfa107`, 138 passed with `test(/^procs::|::claude_pin::|::report_pin|::codex/)` (run
    `2f4c70e3-cc02-445d-b2a4-21aa9348f267`).
  - `c03b11cb`, 117 passed with `test(/^(procs|agent_kind::grok|service::notifications)::|::report_pin|::claude_pin::/)`
    (run `81807dc3-9852-49d2-b1b6-c2ec3102bd8b`).
  - `e7490f86`, 123 passed with
    `test(/^(procs|agent_kind::codex|agent_kind::grok|service::notifications)::|::claude_pin::|::report_pin/)` (run
    `ca100821-179a-449c-a35f-9d32e92fafd6`).
  - `20d93c69`, three passed with `test(a_renamed_codex_image_can_emit_the_hook) | test(/^service::notifications::/)`
    (run `acb50b8f-0724-4bf6-af57-70bdd8a12fc9`).
- After `cargo build --locked -p farhelm-fixtures`, recorded `cargo nextest run -p farhelm --test e2e -E <selection>`
  passed 35 at `c3cfa107` and all 36 at `20d93c69` with `test(/^(codex_identity|hook_identity)::/)` (runs
  `000ca7ad-313a-4d85-9e83-f683a4cbeb4d` and `792d2340-a9db-4914-ba22-ea10dd7b4af4`). At `c16022a5`,
  `test(/^hook_identity::/)` passed all 31 after strengthening the nested Grok case (run
  `5bc73882-cd52-466f-b8ef-ef97e41b1357`). The recorded output showed no runtime skips.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the annotated test delay check
  (`python -B scripts/check-test-sleeps.py` in the isolated pinned environment), and
  `python3 releasing/check-changelog.py format` passed. `dprint check` passed on the changed Markdown.
  `bun install --frozen-lockfile` and `bun run build` validated the docs website. The Codex, Grok, wrapper and hook
  pages were checked in that build.
- Reused the Clippy checks from `e7490f86` after the Codex follow-up changed diagnostic text, tests, and comments. Reran
  the targeted runtime checks for that follow-up. Reused the deterministic runs through `20d93c69` at `d0262280` because
  the later changes were a fixture's failure messages and documentation. Main advanced during validation and host
  auto-update landed during the report review. Its changes are to the helm's host connection and update flows, which are
  separate from local conversation reporting and Restart. The stack kept its tested base. Skipped the full workspace
  test suite because the change stays in hook admission and its docs. The recorded live checks ran on Linux.

## Review gate

All four PRs passed separate source and wording reviews. The Grok review strengthened the test so ordinary timestamp
ordering could not hide a missing parent pin. The Codex review corrected the notice for a shell reporter, preserved the
description of existing pins across upgrade, and tested a renamed executable below a resident wrapper. The final review
checked the documented default and OMP exception against the implementation.
