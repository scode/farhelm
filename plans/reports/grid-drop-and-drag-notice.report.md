## What this was about

New terminals refused dropped files and images, and gave no explanation when a program kept a mouse drag from selecting
text. The maintainer chose parity with old terminals, plus moving the existing hint away from the text just dragged
over. Both kinds now accept the same drops and show the same drag-copy guidance. The final PR removes both source TODO
entries.

## Things you should know

Files, images and text dropped onto a new agent terminal or shell tab use the existing upload and paste handling, with
the same payload precedence, directory refusal and disconnected-terminal refusal. The unsupported-drop switch and
message are removed.

After a plain drag goes to a program that uses the mouse and the program does not copy, new terminals show the existing
hint, including Codex's own instruction in its agent terminal. Wording, the 1.5-second program-copy grace, 30-second
lifetime and dismissal behavior are unchanged. A primary-selection write counts as the program copying even though the
browser does not put that selection on the system clipboard. Link drags withheld from the program and frozen takeover
screens never raise guidance.

In both kinds, the hint's bottom edge sits three terminal rows above the release point when it fits in the terminal and
window. Otherwise its top edge sits three rows below. A very short terminal keeps the previous placement clamped inside
the window, which can overlap the release row. Horizontal centering is unchanged. The gap follows the terminal's text
size.

The docs pages changed are `website/src/content/docs/docs/using/new-terminals.md` and
`website/src/content/docs/docs/using/work-in-a-session.mdx`. No preview was started during unattended execution. No
native drag handler, clipboard protocol, supervisor behavior or live installation changed.

## Open questions and possible follow-ups

The remaining manual check is a real Finder drag into the Mac app on new terminals: a file onto the agent terminal, a
file onto a shell tab, and a directory refusal. Browser tests synthesize drop data and do not establish native macOS
delivery. The existing desktop checklist now covers both terminal kinds.

The hint remains guidance, not a clipboard-success report. Existing clipboard limits can discard a program write before
the browser sees it, so such a write can still leave the hint visible. This plan changes neither those limits nor the
silent best-effort clipboard policy. No unresolved product decision blocks the stack.

## The PRs

- [#2965](https://github.com/scode/farhelm/pull/2965/changes): accept file drops in terminal hosts, head `2cc5471e`.
- [#2969](https://github.com/scode/farhelm/pull/2969/changes): show drag-copy guidance in terminal hosts and move it
  away from the release row, head `fcbae78f`.

Both are open drafts in one linear stack, one commit per PR. Neither was marked ready or merged by this executor.

## Checks run, reused and skipped

Runtime commands ran through `scripts/record-test-run.py` on Linux. Live browser runs used Chromium and WebKit, three
owned stacks, three worker slots, zero retries and pinned tmux 3.7c. Their complete reports passed policy checks,
retained untruncated output through EOF, and completed cleanup. Host-mode runs explicitly retained
`FARHELM_E2E_SUITE_TERMINALS=host` and confirmed the host-terminal premise. No live browser case was skipped.

- `cd e2e && npx playwright test 'terminal-attachments\.spec\.ts'`: 76 passed on old terminals, run
  `aab794bf-869a-4b2c-ac6b-2a86a53617eb`, and 76 on new terminals, run `b0246fd9-b33f-48c9-b319-6c8596ea6f29`. Each
  engine ran 38. This includes actual drop gestures on agents and tabs, precedence, bracketed paste, directories,
  interruptions and disconnected refusal. These PR1 results at `7bf4fcac` are reused for PR2 because upload, drop and
  paste behavior did not change; the added notice teardown is covered separately.
- `cd e2e && npx playwright test 'mouse-modes\.spec\.ts'`: six passed on old terminals, run
  `2712abb9-d360-4521-a4ba-cce5a13d988f`, and six on new terminals, run `a437c583-acc0-407c-bdec-d6cbcd981d15`, three
  per engine. This proves generic and Codex wording, program-copy suppression, click exclusion, placement, repeated
  display, dismissal and focus/input isolation, hidden-tab behavior and the actual 30-second lifetime.
- `cd e2e && npx playwright test --config grid-client/playwright.config.ts grid-attach.spec.ts --workers=2 --retries=0`:
  PR1 passed 68 cases, run `c7fab71d-d168-4d0b-88d1-690ebd6d953b`. PR2's expanded run
  `c95146a1-847b-44ae-9422-2169ac8588db` had 68 passes and four failures in two new fixtures, on both engines: a plain
  URL was incorrectly expected to show a hover label, and a frozen screen retaining mouse reporting was incorrectly
  expected to select without the forcing key. Both fixtures were corrected without changing product code. The exact two
  cases then passed in Chromium, run `00f9610a-a0cb-4b6f-8bc7-9436b29d09a2`, and all four passed on both engines, run
  `7cdecfc7-70a2-4126-882c-7ece68a326bf`, using `-g 'a withheld grid link drag|a dropped connection keeps'`. The other
  68 cases were unchanged. The failed run and traces remain retained, not represented as a full-suite pass or a latent
  flake. The static tests also prove primary-selection suppression without copying and frozen-screen silence.
- `cd crates/farhelm-ui/js-tests && node --test`: PR1 passed 508, run `8872efff-51d3-47e6-960d-1968672974bb`; PR2 passed
  512, run `0057cc40-ce6a-45df-8f95-b983d4a76291`. Exact row-gap, above/below/fallback, font scaling, viewport edges and
  notice lifecycle boundaries are covered. Later edits changed only static browser fixtures, comments and an unused
  import, preserving this unit coverage.
- `bun run build` in `website` passed after each docs change: 35 pages and all internal links valid. Selected Markdown
  `dprint check`, changelog format and whitespace checks passed. The isolated delay checker inspected 397 delays with
  zero missing rationales. The instruction checker had zero errors and three pre-existing stale-citation warnings. Asset
  JS, CSS and TypeScript have no configured dprint plugin; no formatter coverage is claimed for them.
- The CLI, fixture and Dioxus 0.7.10 web builds passed. Rust formatting passed for PR1's comment-only Rust edit. The
  initial missing Astro dependency and unrelated `dx` command were corrected before successful builds, with no source
  workaround or shared-tool replacement.

Main through `27531ca3` was inspected in full and the stack rebased onto it. Intervening changes were TODO and planning
bookkeeping only, with no executable or specification interaction. PR1's interdiff was empty; PR2's tested behavior was
preserved, so the command evidence above was reused without another runtime run. The final unused-import removal has no
execution effect.

Rust runtime, clippy, native desktop, installer, provisioning and the full browser battery were skipped: no Rust
behavior or native handling changed, and targeted tests cover the changed browser contracts. Native Finder delivery
remains the manual check above. No hosted CI was dispatched.

## Review gate outcome

Each PR received the required single fresh-context gpt-6.1-sol review at high effort. PR1 had no findings. PR2 found the
retained-mouse-mode fixture issue; the same reviewer confirmed its correction and the OSC-link fixture correction, with
no remaining findings. No scope expansion or weakened guarantee was required. Commit and PR wording passed separate
fresh-context cold reads; an invalid reader that opened the diff before its blind phase was replaced before use.
