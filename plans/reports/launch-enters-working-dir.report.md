# Launches enter the session folder

## Since the last delivery

PR #2363 is updated in place after the maintainer answered the landing block. The seven supervisor tests now keep their
intentional agent-launch failures while starting terminal tabs through the real helper. Directory-entry failures name
the folder; a missing agent program reports the program failure. Mac terminal tabs use the supervisor's own versioned
program for their helper.

## What this was about

When a session folder existed but could not be entered, tmux could silently start an agent or terminal shell in the home
folder. Farhelm still showed the selected folder, so unattended work could run somewhere the user had not chosen. The
maintainer required a launch to enter its session folder or fail clearly, and accepted ordinary symlink resolution.

Agent launches now enter that folder after checkout preparation and login-shell startup, and pass matching `PWD` to the
agent. Terminal tabs enter before starting their shell. The existing error paths report entry failure without starting
the requested program elsewhere.

## Things you should know

- An agent launch overrides a `cd` in shell startup files, as requested. A terminal tab retains ordinary shell behavior:
  its startup files may change directory after the shell starts in the session folder.
- Launch specifications written by older supervisors lack the optional directory field and keep their inherited
  directory behavior. New Create, retry, Restart and Replace attempts use the shared builder that fills it in. No
  supervisor protocol or database migration changes.
- The failure seen during landing was reproduced in one exact test, then all seven affected tests passed after repair.
  The earlier failed run remains failed evidence; the fresh passes are separate observations.
- Whether Mac privacy protection for Documents or Desktop routinely triggers the original problem remains unverified.
  This execution ran on Linux; no live installation was changed.

## Open questions and possible follow-ups

None required for this plan. Mac privacy behavior can be checked separately if it becomes a reported trigger.

## The PRs

- [#2363](https://github.com/scode/farhelm/pull/2363/changes), `fix: enforce the session folder at launch`, updates the
  existing draft with the original fix and the answered landing corrections.

## Checks run, reused and skipped

Fresh recorded runtime checks use pinned tmux 3.7c and nextest 0.9.148 with four runner slots and no retries.

- Reproduction: `cargo nextest run -p farhelm-supervisor --lib -E '<exact opened-tab scope test>'`, run
  `a0df684c-97b3-4d32-9134-b5bb821fa9d1`, failed because its configured placeholder could not execute the tab helper.
  That was before this round's repair.
- `cargo nextest run -p farhelm-supervisor --lib -E '<seven repaired tab fixtures and launch-helper contracts>'`, run
  `fb8ba128-147f-45fd-a73f-d8e40253e10e`: 17 passed, including every formerly failing test. Complete JUnit and retained
  output; no runtime substrate skip. A subsequent fixture-comment correction changes no tested behavior.
- `cargo nextest run -p farhelm --test e2e -E '<Create, startup-order, error and tab-containment cases>'`, run
  `6fa74a7a-cefe-4dd1-96ca-089d43dd5202`: nine product tests passed, plus the separate fixture-build setup. The
  permission-denial and systemd containment cases executed; no runtime skip, output truncation or incomplete report.
- `cargo nextest run -p farhelm-supervisor --lib -E '<shared builder, scope-prefix and versioned-program contracts>'`,
  run `4fc9a952-1661-44ad-a1f4-d0da5147966f`: four passed, complete report and output, no runtime skip.
- Both `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings` passed.
- Rust format, dprint for the changed Markdown, changelog format, agent-instruction consistency, and the test-sleep
  checker passed. The delay checker inspected 304 calls with no missing rationale.

The successful runtime commands cover 30 product tests. Final PR head `5244adda` preserves their executable behavior;
subsequent edits only clarified comments and specification wording, so no further runtime repetition was needed.

No previous runtime pass is needed as evidence for this delivery. The earlier failed landing run
`acae51f5-18dc-4dc0-af71-f9bda72e0a19` and the pre-repair reproduction remain recorded in the working log. The whole
supervisor or workspace battery was not repeated: the exact affected fixtures and launch boundaries provide the relevant
coverage. Browser, desktop, installer and provisioning suites were skipped because this change adds no behavior at those
boundaries. No manual Mac privacy test was run.

## Review gate

A fresh-context gpt-6.1-sol reviewer at high effort read the whole updated PR, the latest decision and the full test
authoring checklist. It reported no findings. The independent resume check also confirmed the existing PR and prior
work, and its complete upstream-diff audit found no additional functional conflict. No substantial departure from the
plan or new protocol was needed.
