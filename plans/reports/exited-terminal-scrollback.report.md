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
