## What this was about

When the terminal made room for an exit or status line, it could lift the oldest retained text above the pane. Scrolling
all the way back could still leave the first line or the notice of lost output out of view. If a program exited on its
full-screen view, earlier terminal history could stay hidden and the program's old mouse mode could still interrupt
scrolling and selection.

The fix makes that output reachable by scrolling and selection. The final screen and the exit line stay together at the
bottom.

## Things you should know

- The maintainer chose to cover rows lifted by the red status line too, even while a program is running.
- After a full-screen program exits, its final screen stays below the earlier output. Select All includes both. When a
  line in the earlier output wrapped into the program's original screen, copying it with the final screen keeps a line
  break between the two. Links and word selection work in the restored history.
- A running full-screen program still owns the wheel. Once the program exits, the wheel and pointer can be used locally
  even if mouse reporting was active.

## Open questions and possible follow-ups

None.

## Pull requests

- Make the oldest retained row reachable when an exit or status line lifts the screen.
  [#4512](https://github.com/scode/farhelm/pull/4512/changes)
- Restore access to earlier output after a full-screen exit and keep pointer actions local after any exit.
  [#4516](https://github.com/scode/farhelm/pull/4516/changes)

## Checks

The final model, GUI and screenshot results below were reused after rebase because both patches and the test inputs
matched the tested versions. Main added two unrelated TODO entries, both of which remain.

- `cargo test --locked -j 4 -p farhelm-grid-model` passed 44 tests, including copying across the screen boundary and
  finding links in restored history. Recorder `c6ea409c-2728-483d-bab1-c5001ced6b59` at the reviewed source `349c3210`.
- `cargo test --locked -j 4 -p farhelm-gui --lib -- --exact --test-threads=1 <terminal cases>` passed 11 tests on Linux.
  They checked the oldest painted rows, page keys, wheel, scrollbar, Select All, drag selection, pointer motion after
  exit, and the live program's mouse behavior. Recorder `b341612c-1721-44e6-a086-7d9e671fbfe7` at `349c3210`.
- `python3 scripts/check-gui-goldens.py --scene terminal-status --jobs 2 -j 4` matched the light and dark references,
  including the exit line and red status line. Recorder `8a5420c7-3728-4b95-bfbd-ae1e4e40835a` at `349c3210`.
- The seven GUI tests on the first draft (`d888797b`) also checked a fitted live terminal at fractional scale, pointer
  coordinates on the lifted screen, and lift sizing. The first patch and those test inputs matched after rebase.
  Recorder `8d9ba791-f479-49b1-b648-a3b2e347f553`.
- Rust formatting and the Rust test-delay check passed on the final source. Markdown formatting, whitespace and
  changelog checks passed after the rebase.
- Full workspace, live-agent and macOS tests were skipped. The changes are in the desktop terminal's shared grid and
  input logic, which the selected Linux tests exercise without launching a real agent. The screenshot comparison used
  the repository's Linux renderer.

## Review gate

Each draft passed independent source review and a cold read of its title and commit message. Review of the second draft
found that word selection and links still treated the restored output as hidden. It also found that if the end of the
earlier output wrapped onto a line that was no longer shown, copying could join it to the first line of the final
full-screen view. Word selection and links now work in the same output users can scroll to. Copying and link recognition
keep the line break between earlier output and the final frame.

### Landing

#### What landed with it

The plan's two PRs merged with a landing fix, #4523, as the third of four plans landed together in this round, after the
helm and Podman triage plans and before the status-mark hover change. None of those, nor the per-agent Settings and hook
setup plan that landed on main since the executor based this one, touches the desktop terminal's code. The status change
also edits SPEC.md, in a different section.

An independent reviewer read both PRs against the plan and the specs. It found every acceptance criterion met and no
blocker. It also checked the one thing the second PR does beyond the plan: when an exited full-screen program's final
screen reveals the history above it, copying and link detection now stop at the boundary instead of joining the last
history line to the first line of the program's final screen. Revealing that history is what exposed the problem, the
fix is small and tested, and the reviewer judged it necessary.

#### Fixes made while landing

In [#4523](https://github.com/scode/farhelm/pull/4523/changes), text only:

- **SPEC.md.** The new sentence saying the wheel, clicks and drags act on the terminal once its program has exited had
  been added at the end of a long passage about the clipboard. It now follows the drag rule it qualifies. That put it
  right before the sentence on a program's own clipboard writes, which said they are "honored the same way" and would
  then have read as referring to the exited terminal; that sentence now says plainly that such a write also copies to
  the clipboard.
- **Changelog note.** It described only the reachable history. It now also says that after a program that used the mouse
  exits, a plain drag selects text and the wheel scrolls again.

#### Checks

The round landed four plans together: the helm triage, the Podman triage, the exited-terminal scrollback fix and the
status-mark hover change. The landing ran their checks once, on all four stacked on main. The Rust workspace suite, the
desktop app's tests, the lint runs and the end-to-end smoke test ran on a separate arm64 Linux test worker. The goldens,
the website build and the small checks ran locally on x86_64. The workspace run used four build jobs, as in the last
round, since runs with more had run out of memory on the worker; this round it had no memory or disk trouble.

On the four plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four build jobs, on the test worker (recorder run
  `8ba93f1d-d933-483b-a90a-eeac88311d3a`): all 8252 passed.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `890a2d33-0c71-410b-9498-3a792708a974`): all
  184 passed.
- `cargo clippy --all-targets -- -D warnings`, on the test worker, not through the recorder: failed on one lint in a new
  Podman test, fixed in #4522.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `c732c6cc-ba70-4b34-853a-fe7961b6d297`): every
  step passed but paste, which skips on Linux as it always does.
- `python3 scripts/check-gui-goldens.py`, the full comparison, locally (recorder run
  `2372fd39-11ef-4468-84d3-f1c70cdf9f7e`): failed only on the two pictures of the recipe Delete anyway question, which
  the Podman plan had captured before main's Settings sidebar changed. Fixed in #4522.
- `python3 -B scripts/test-gui-screenshot-batch.py`, locally: failed because that new scene was missing from the
  screenshot scene list. Fixed in #4522.
- `python3 -B scripts/test-check-gui-goldens.py` (recorder run `952eb6e8-4c50-4a08-bc89-870dd1358b6d`),
  `python3 -B scripts/gui-goldens.py --self-test`, `python3 -B scripts/test-docs-shots.py` (recorder run
  `6c988518-9a04-4428-b87d-f5a1d49ede50`), `python3 -B scripts/test-gui-agent.py` (recorder run
  `430497e6-e6fd-4d35-8648-4082a3b33d33`), the website build (`bun install --frozen-lockfile && bun run build`) with its
  link check, `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its isolated environment),
  `dprint check`, `python3 scripts/check-agents-md.py` and `python3 releasing/check-changelog.py format`, locally:
  passed.

After the landing fixes (#4521 to #4524):

- `python3 scripts/check-gui-goldens.py --update`, locally (recorder run `6d8fe2d8-8886-45e5-a8f7-c1902f41d10c`):
  re-rendered every picture and changed only the two Delete anyway pictures, which were then published.
- `python3 scripts/check-gui-goldens.py --scene 'settings-recipe-*' --scene 'session-row-*'`, locally (recorder run
  `9282ca4e-8230-435c-8d5d-20380af3477c`): passed.
- `cargo nextest run -p farhelm-helm --lib`, on the test worker (recorder run `2b0cfd85-3205-4a06-97c1-d028a7ae46db`):
  all 1801 passed.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and the same with `--features agent`, on the test
  worker: passed. They had not run on the first pass, which stopped at the failing lint run.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `cargo fmt --all -- --check`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

The rest of the first runs still applies after the fixes, which change only a Podman test's lint, a test comment, two
pictures and the scene list, a code comment on the helm's notices table, SPEC.md wording, the desktop parity notes and a
changelog note. Skipped: the Podman suite's opt-in real-engine and real-SSH variants, which neither triage plan needed
beyond the real Podman environment test, which ran in the workspace run on the worker (about two minutes, not skipped);
macOS, since no Mac was part of this landing and the pictures used the Linux renderer only; CI, which runs only on
demand.

#### Things you should know that the report does not say

- If the red status line or the exit line appears while you are already scrolled to the very top, the oldest line stays
  hidden behind that line until you scroll one more step. That follows the rule that a scrolled view stays on its line,
  and it is not a regression.
