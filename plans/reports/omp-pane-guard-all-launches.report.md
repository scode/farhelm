### Since the last review

PR #1779 is amended in place; no PR was added or dropped. The original exact-launcher test at the terminal's top process
is replaced by the agreed unreadable-arguments rule. The unrealistic npm positive control and non-exact-launcher refusal
expectations are replaced by readable npm-style and Bun-wrapper controls. The specification and changelog now describe
that narrower rule.

### What this was about

An OMP session launched through a custom Bun or npm command could offer a nested OMP conversation as the session's own
Resume target if the terminal's top Bun or Node process had unreadable arguments. Without those arguments, Farhelm could
miss an outer OMP and take a reporting child for the session's agent. This is a conditional code-path gap: no live
nested OMP reproduction established the required separately interactive child loading Farhelm's reporter.

Triage initially chose a small guard requiring the top process to match the launch method's expected launcher. Landing
found that npm rewrites its command line, so that check could remove Resume from real npm/npx launches on macOS. The
maintainer chose option 1: refuse only unreadable top-process arguments, for every OMP launch program, and keep the
change small.

### Things you should know

The implementation uses one arguments-availability condition in the existing guard. A Bun or Node top process above the
reporting OMP refuses when its arguments cannot be read, including when they exceed the read budget. Readable arguments
need no expected-launcher match. The reporting OMP must still be recognized, and recognized nested OMP runtimes and
unrecognized programs between the top process and OMP still refuse.

The latest every-program decision also changes installed `omp` behavior: a readable Bun or Node process at the
terminal's top may now allow Resume for the reporting OMP below it, subject to the other checks. Main previously refused
that case. The tests, specification and changelog disclose this. The exception applies only to the terminal's top
process; an intervening nested OMP or an unrecognized intervening program still prevents Resume.

### Open questions and possible follow-ups

No maintainer decision remains. The older documentation claim that npx launchers are recognized is tracked separately in
TODO.md and is outside this plan. This change does not establish that live npm/npx launches on macOS receive Resume: the
tests supply process-chain shapes, and no live macOS or nested OMP integration was run. A readable top process may still
be an arbitrary wrapper; that is the agreed boundary, rather than proof of an expected launcher.

### PRs

- [#1779 — refuse OMP Resume when top-process arguments are unreadable](https://github.com/scode/farhelm/pull/1779/changes),
  draft, based on main; pushed commit `83a0238522e104f7900bda85d9d45e67c208c489`.

### Checks run, reused and skipped

- Run now: `cargo nextest run -p farhelm-supervisor --lib -E 'test(procs::tests::) & test(omp)'` through the recorder,
  run `73f45de2-6b43-4266-b147-d94b86a41c86`, passed all 13 selected tests with four slots and zero retries. The 1066
  skipped entries were selection exclusions; no selected test reported a runtime skip. The run covered the amended
  seven-file tree on main `1ea4f69e`, before the final commit description was assigned. It covers unreadable Bun/Node
  panes across installed/Bun/npm launches, readable rewritten npm and arbitrary Bun panes, and existing runtime/corridor
  controls.
- Run now: supervisor all-targets Clippy with warnings denied, workspace Rust formatting, dprint on the changed
  specification/ledger/feedback index, changelog format lint (32 fragments), and the isolated-interpreter test-delay
  check (278 delays, zero missing rationales) passed.
- The original round's 79 process tests are historical evidence, not reused as a verdict for the changed rule. Current
  focused execution covers that rule. No failed runtime run occurred in this resumed round.
- The careful rebase audit read the full intervening main diff: host appearance, managed-checkout naming/trash and
  protocol 44, remote uninstall, stop-expiry fallback, Claude compaction and bookkeeping. None changes the
  process-chain, arguments or OMP admission contracts used here. The later OMP documentation TODO and queue-only changes
  likewise have no runtime interaction; current focused evidence remains applicable.
- Full workspace, browser, desktop, installer and release suites were skipped because the change is confined to
  process-chain attribution and has targeted tests and crate lint coverage. No affected browser integration or broader
  uncovered interaction was identified. Hosted CI was not dispatched; this repository does not start CI for draft PR
  updates. Live npm/macOS and nested-OMP lifecycle coverage remain the explicit limits above.

### Review gate outcome

The required fresh source reviewer, requested as native GPT-6.1-sol at high effort, read the complete amended diff, the
plan and latest Decisions, and the full test-authoring checklist. It found no actionable correctness, design, Rust
idiom, test-contract or documentation issue and no unnecessary complexity. It specifically confirmed the installed-omp
readable-pane delta follows the latest decision. The executor inspected the diff and retained test results
independently; the reviewer ran no tests.

Implementation and investigation stayed local. The required VCS wording cold read passed; its readers were requested as
native GPT-6.1-sol at medium effort. The resume and report cold reads used fresh native agents with the executing model
inherited by request. A separate documentation pass covered every touched file. Actual native model attribution and
usage counters are unavailable; requested model names are not proof of runtime attribution. Private delegation evidence
is retained under session UUID `d0e361cc-2cf7-49dd-a656-7f0f6a284fbf`.
