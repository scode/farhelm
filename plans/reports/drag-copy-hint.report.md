## What this was about

Dragging over text in Codex's prompt box (the box you type into at the bottom) highlights it, but nothing reaches the
clipboard. Codex has mouse reporting on, so a plain drag goes to Codex rather than to Farhelm's terminal, which then
makes no selection of its own. Over its conversation Codex copies on release by itself, and Farhelm forwards that copy;
in the prompt box it only highlights, and copies only if you press Ctrl+C while the highlight is up, which it does not
tell you. The same happens with any program that takes the mouse and does not copy by itself (vim with mouse support on,
for example). The user is left with highlighted text, an unchanged clipboard and no idea why.

You decided on 2026-10-02 not to try to make that drag copy (Farhelm cannot, and sending Ctrl+C for the user would clear
or interrupt Codex when nothing is highlighted), and instead to show a hint: the program handles selection itself, so
use its own copy feature; and if the session is known to be Codex, say how to copy in Codex. A "Maybe later" TODO for a
launcher option that turns off Codex's full-screen mode was added when this plan was written and is untouched here.

This plan built that hint as one draft PR.

## Things you should know

- **When the notice appears.** After a plain left-button drag over a terminal whose program has mouse reporting on, if
  Farhelm made no selection and the program sends no clipboard write within about a second and a half. Not after a
  click, not after a drag the program copied itself (Codex's conversation area), not after an Option-drag (Shift-drag
  outside macOS), and not in a pane with no mouse reporting.
- **What it says.** In general: "This program handles mouse selection itself, so this drag did not copy anything. Use
  the program's own copy command. Or hold Shift while dragging to select and copy here." (Option instead of Shift on the
  platforms xterm treats as a Mac, which is the key that actually works there.) In a Codex session's agent terminal the
  first part is replaced by: "Codex handles mouse selection itself: to copy what you highlighted, press Ctrl+C while it
  is still highlighted (without a highlight, Ctrl+C clears your draft)." Shell tabs always get the general wording.
- **How it looks and behaves.** A small line in the terminal's top-right corner, below the pane's banner if one is
  showing, chosen so it does not cover Codex's prompt box at the bottom. It stays for six seconds, fades over one, never
  takes focus or blocks the mouse, and screen readers announce it politely. Each distinct wording shows at most once per
  page load.
- **What was checked on the real Codex.** On the installed Codex 0.160.0, in a private terminal with no prompt sent (no
  model turns used): a drag in the prompt box highlights and copies nothing; Ctrl+C while highlighted copies exactly the
  highlighted text and keeps the draft; Ctrl+C with nothing highlighted clears the draft; Ctrl+Insert does not copy, so
  the notice does not offer it. In Codex's non-full-screen Scrollback mode (chosen with `/tui`) mouse reporting is off,
  so the notice can never appear there, which is correct. Codex's saved settings were not changed.
- **How Farhelm knows a session is Codex.** The browser now reads each session's agent kind from the helm's session list
  (the helm already sent it). An agent kind this build does not know, or a helm too old to send it, simply gets the
  general wording; it can never break the session list. Because it uses the agent kind rather than how the session was
  launched, sessions started from a raw command or a profile get the Codex wording too.
- **Spec and TODO.** SPEC.md's Terminal experience section gains a sentence describing the notice (as guidance about the
  program, not a clipboard failure, so it does not conflict with clipboard errors staying silent), SPEC_impl.md gains a
  paragraph on how it is decided, the TODO entry "Copying text from Codex's prompt box does nothing" is removed, and the
  supervisor's map of per-agent code now names where the browser's per-agent facts live.
- **Changelog.** One `added` entry describing the notice for users.

## Open questions and possible follow-ups

- **Other agents' prompt boxes were not checked.** The TODO entry noted that whether other harnesses behave like Codex
  had not been checked; this plan did not check either. They get the general wording, which is accurate for any program
  that takes the mouse. If one of them has its own copy key worth naming, adding it is a one-line change in the same
  place as Codex's.
- **The review ran at this session's own effort, not an explicitly set high effort.** The plan asked for a reviewer at
  high effort; the agent mechanism used could set the model but not the effort, so the reviewer inherited this session's
  setting, which was not confirmed to be high. You can accept the review as it is (it found and got fixed eight issues,
  listed below), or ask for a follow-up re-review at explicitly high effort before landing.
- **Possible interaction with the stepped-animations PR (#1481, in review).** That PR adds a rule that animations which
  repeat forever must be stepped. This notice's fade runs once and draws nothing while it waits, so the rule does not
  apply and its check passes. If you would rather the rule cover one-shot fades as well, that is a decision for that
  plan.

## The PRs

1. [#1499](https://github.com/scode/farhelm/pull/1499/changes) — the drag-copy notice, with Codex's own instruction for
   Codex sessions.

## Checks

Run now (test runs through the repository's recorder; browser runs with the pinned tmux, one worker, no retries):

- `cd e2e && npx playwright test 'mouse-modes\.spec\.ts' 'terminal-clipboard\.spec\.ts'` on Chromium and WebKit, after
  the review fixes: run `ac2466b0`, 15 passed and 9 skipped. The two new tests (generic notice, Codex notice) passed on
  both engines. The skips are terminal-clipboard's documented WebKit skip (Playwright cannot grant WebKit clipboard
  permissions). The same selection before the review fixes: run `ae0c2db2`, same result.
- `cd crates/farhelm-ui/js-tests && node --test`: 169 passed.
- `cargo nextest run -p farhelm-ui --lib`: run `7aec4849`, 431 passed. Rust code changed afterwards only in
  documentation.
- `cargo check -p farhelm-ui --features desktop` and `scripts/check-desktop-assets.sh` (no new asset file; parity
  holds).
- `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings` on `farhelm-ui` and `farhelm-supervisor`;
  the test-sleep check (two new delays, both annotated observation windows);
  `python3 releasing/check-changelog.py
  format`; `dprint check` on the changed Markdown.

Reused: none.

Skipped, with the reason:

- The rest of the browser suite: the change touches only the terminal's mouse handling, the session view's pane markup
  and one stylesheet rule; the two specs above are the ones that drag in terminals under mouse reporting.
- Supervisor and helm tests: neither changed, apart from one documentation comment in the supervisor.
- Desktop runtime smoke test, installer and provisioning checks: nothing in those areas changed.
- `cargo clippy -p farhelm --bins`: the shipped binary's code did not change.

## Review gate

The PR got a fresh-context Opus 5.5 reviewer with an adversarial charter (assume defects and hunt for them: correctness
against the goal above, design fit, and idiomatic code), plus the repository's test-writing rules. It ran at this
session's own effort setting (see the open questions).

It reported eight findings, all fixed in the PR (grouped here into six lines):

- The most important was that the code inferred "no Option/Shift held" from "nothing was selected". That showed the
  notice to someone already holding the key (a drag inside one character cell) and hid it after an earlier forced
  selection. It now reads the key from the press the way xterm does.
- A notice firing into a session's terminal tab the user had just switched away from was used up unseen; it now waits
  for the next drag.
- The notice could cover the pane's banner; it now sits below it.
- A program asking to read the clipboard counted as copying; it no longer does.
- The browser tests now prove their premises (two findings): that the stub's copy arrived, and that the stub session
  really is Codex.
- Three comments no longer matched the code; they were fixed.

No finding was declined.
