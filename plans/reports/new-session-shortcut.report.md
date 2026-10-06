## What this was about

The Mac desktop app had no keyboard shortcut for New. The maintainer limited this plan to that app: Cmd+N opens the
session launcher, using the same host and folder prefill as clicking New, even when a terminal has focus. The web UI
keeps the browser's new-window shortcut.

## Things you should know

The shortcut was not exercised at runtime in the Mac desktop app before delivery. This run used Linux; compilation and
review establish the code's shape, not native key delivery. The new bullet in docs/manual-mac-checklist.md is the
verification still needed on a Mac, including terminal input suppression, prefill, dialogs and a busy list.

Cmd+N requires Command without Shift, Alt or Control. An open modal or disabled New prevents opening; repeated key
events cannot toggle the form closed while it is appearing. Matching uses the layout's letter rather than a physical
key, as the plan prescribed. The Linux desktop build installs no listener.

The TODO entry is removed. No native menu command was added.

## Open questions and possible follow-ups

No decision is required to finish this plan. A native menu item showing the shortcut remains an optional follow-up,
outside the requested scope. Native keyboard-layout behavior remains unverified with the rest of the Mac runtime.

## The PRs

[#1675](https://github.com/scode/farhelm/pull/1675/changes) — Cmd+N opens the session launcher in the Mac desktop app;
draft, based on main.

## Checks run, reused and skipped

- Run: cargo fmt --all -- --check, dprint check on SPEC.md, TODO.md and docs/manual-mac-checklist.md, and python3
  releasing/check-changelog.py format; all passed on the final diff.
- Run: cargo check -p farhelm-ui --features desktop -j 2 and cargo clippy -p farhelm-ui --features desktop --lib -j 2 --
  -D warnings; both passed. These check the Mac caller's types on Linux because its platform gate uses cfg!.
- Reused: the compile and Clippy results from before the final review edits. Subsequent changes added an auto-repeat
  guard inside the existing JavaScript string and explanatory comments, and changed Markdown. No Rust type or platform
  configuration changed. Rebase onto newer main changed only another plan's queue claim, preserving that evidence.
- Skipped: native Mac runtime, unavailable on the executing host; manual checklist now records the exact checks. Browser
  tests were not relevant because no browser behavior changed. No Rust or JavaScript runtime suite was selected for this
  desktop key handler; no new tests were added. The test-delay checker was not applicable because tests and test helpers
  were unchanged. Desktop asset parity needs no rerun because no asset was added or changed.

## Review gate outcome

Independent reviews by Opus 5.5 high (reported model claude-opus-5-5) and gpt-6-astra high passed on the final diff. The
Opus review identified an auto-repeat race, now fixed, and documentation/wording improvements, now applied. Its
speculative non-Latin layout fallback was not added: it was unverified and would depart from the plan's deliberate
character-based key matching. Both final review artifacts report no findings. The commit and PR title also passed a
fresh-context wording cold read.
