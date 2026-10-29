## What this was about

Opening a session, host or restart menu focused its first command. Hovering another command added a second highlight,
leaving the pointer and keyboard pointing at different choices. The maintainer chose one shared current choice: mouse or
pen movement moves focus, and the fill follows focus alone.

Menus now follow that contract. Arrow keys continue from the hovered command, Enter runs it, and leaving the menu keeps
its focus and highlight. Disabled commands remain focusable without a fill and still refuse activation. The TODO entry
is removed, the specifications describe the behavior, and a fixed changelog fragment accompanies the change.

## Things you should know

The shared behavior covers session menus, host row and rail-avatar menus, restart and replace, sort, Help and Update,
and template number-key menus. The tunnel popover's separate hover feedback remains. Opening, wrapping, Home/End,
Escape/Tab focus return, touch behavior and menu placement retain their existing contracts.

The deliberately accepted opening-focus race remains: an opening request can overtake a hover that arrived earlier.
Hover help retains its pointer-rest delay and dismissal on leaving, even though the last hovered item stays focused.
Review found and fixed a further case where arrowing away and moving within the still-hovered item lost its help.

Computed-fill regressions directly cover session, host, restart/replace, sort and Help in Chromium and WebKit. Template
popup and shared keyboard behavior also have existing browser coverage; rail-avatar and Update variants were inspected
as callers of the same shared mechanism. WebKit is browser coverage for the desktop engine family, not a native app run.

## Open questions and possible follow-ups

No unresolved decisions. The launcher's suggestion lists and the quick switcher keep a separate selected choice and a
quieter neutral hover treatment. They use a different focus model and do not share the menu cause, so they remain
unchanged under the maintainer's scope decision.

## The PRs

- [#2803 — highlight one menu choice at a time](https://github.com/scode/farhelm/pull/2803/changes), draft, head
  `bdc02eea3bb9b08f0fb1be2d2415b1a6ce860991`.

## Checks run, reused and skipped

All runtime checks below used the test recorder. Final records have complete source evidence and output, normal cleanup,
and no skipped test cases.

- `node --test menu-focus.test.js tooltip.test.js`: 29 passed, run `550b3e73-f1cb-4bc3-8c5b-9463fca4ed3d`.
- `cargo nextest run -p farhelm-ui --lib`: 805 passed, four slots and no retries, run
  `7f2d0847-3520-4ef2-ad54-c78075b6ba33`.
- Selected browser cases in `menu-highlight`, `sidebar`, `header`, `sort`, `templates` and `tooltip`: 46 passed, 23 per
  engine, three workers and no retries, run `d6e76a43-d1bb-4ac2-a2e0-58a98bf90b9c`. These cover the new input behavior,
  queued-focus races, keyboard navigation, inset fills, template popup and tooltip/touch contracts.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`: passed, run `22d77b03-a0e0-40df-b2fe-83cac005a9d7`.
- Final `dx build --package farhelm-ui --platform web --release`: passed, run `f86e425c-e690-4aa3-bc8d-896a14271b43`.
  Reused the successful backend prerequisite `cargo build -p farhelm`, run `ff0a1d5c-3ae6-4475-af58-14265a2affee`;
  backend sources and dependencies did not change.
- Cargo formatting, changed Markdown formatting, changelog format, diff whitespace and the delay checker passed. The
  latter inspected 322 delays with no missing rationale. One attempt exhausted its time budget during compilation and
  was incomplete; the later check passed. The instruction-link checker reported zero errors and three existing warnings.

The original browser tests were also run against the pre-fix assets: all six required session/host/restart cases failed
on the intended extra-fill assertion in both engines, run `cf8c707c-43f9-4faa-b176-ad26f8b1d93c`. Pre-fix JS run
`2a72ec32-a29e-470d-b704-f7db1bab71a5` had five failures, including missing pointer focus. Separate regressions then
isolated the tooltip rest-delay failure in `3698670e-ad1e-4613-bbf0-1006e34b7c27` and the review finding's missing-help
failure in `9d57888e-1939-48db-b3e6-cf86ec54697c`. These expected failures remain retained as regression evidence.

The checks cover the final working changes on base `144bcea5`. The careful rebase onto `32299b8e` added only two
unrelated TODO entries, and the plan's interdiff was empty, so the per-command evidence remains applicable. The final
format and whitespace checks also passed after that rebase.

The full browser suite, native desktop smoke and backend runtime suites were skipped: the focused browser selection
covers the changed shared menu and tooltip paths, and no native shell or backend behavior changed. No visual review was
required by the plan; the fill is checked directly in both engines. No CI dispatch was requested or triggered.

## The review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort passed the final change with no unresolved findings. Its
single tooltip finding was reproduced, fixed through the existing pointer scheduling path, covered by unit and browser
regressions, and reviewed again. A separate cold reader confirmed the commit and PR text conveys the problem and reason
for moving focus with hover.
