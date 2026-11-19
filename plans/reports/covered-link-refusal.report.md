## What this was about

A terminal link's hover panel shows where a link goes or what file Farhelm would download. A menu, dialog, or approval
card could cover part of the panel and the click could still go through. Even when Farhelm did refuse a click, the top
of an approval card could hide the explanation.

The planning decision was to refuse clicks when a surface that takes the pointer covers the panel, keep the old refusal
wording, and raise the explanation over approval cards. During implementation, a test showed that a popover can
disappear on press, so the file click preserves the refusal through release.

A covered destination panel now refuses the click, including opens through a tunnel and file downloads. The refusal says
the details are covered and asks you to move the pointer and hover again. While the pointer stays on the same link, the
explanation appears above approval cards.

## Things you should know

- Farhelm checks the whole panel as drawn, including its border. The pointer can be outside the covering surface and the
  panel can still be covered. If the panel also runs out of the window, the existing resize explanation wins.
- Display-only surfaces that let the pointer through, such as keyboard help and the launch pill, don't count as covers.
  An approval card still accepts clicks and keyboard input while the message is over it. An open confirmation has visual
  priority over the message.
- Moving within the same link can restart its hover while the explanation stays above the approval card. Leaving the
  link returns the explanation to its ordinary place. The click stays blocked even if a covering menu closes before the
  mouse button is released.

## Open questions and possible follow-ups

There is no decision outstanding.

## Pull requests

The three PRs were drafts at handoff. They are listed in landing order.

- Add the rendered-frame overlap check used to tell whether a panel is covered.
  [#4175](https://github.com/scode/farhelm/pull/4175/changes)
- Refuse covered web, tunnel, and file clicks, with wording for the kind of click.
  [#4180](https://github.com/scode/farhelm/pull/4180/changes)
- Make the explanation visible over approval cards and pin both appearances with screenshots.
  [#4186](https://github.com/scode/farhelm/pull/4186/changes)

## Checks

- The vendored GPUI test selection passed all five tests in recorder `f8ad2c83-668d-4cf5-a8cf-dd3561f63e5a`. It checks
  overlap, adjacency, clipped areas and hit areas that let the pointer through. The patch also reverse-applies cleanly.
- The reviewed stack passed 297 terminal, disclosure, file, layer and approval tests on Linux in
  `1e26e51e-c49f-45b7-8264-6930866c706e` with `cargo nextest run -p farhelm-gui --lib`, four slots and zero retries. The
  three targeted covered clicks also passed in `ba0f1681-0e03-424f-b295-550a2dd52322`. Earlier attempts exposed two test
  fixture mistakes. The tunnel test now grants consent before hover, and the file test's cover menu can actually close
  on the outside press. The test for Cmd-click models macOS on Linux.
- `python3 scripts/check-gui-goldens.py --update` captured 260 Linux reference images in
  `b38bed1a-0afa-447a-af5b-bf767b80b437`. The 258 existing references matched byte for byte. The two new images were
  inspected in light and dark, then published. A fresh fetch matched all 260 captures byte for byte.
- GUI Clippy with all targets and warnings denied, Rust formatting, dprint, whitespace, changelog format and the Rust
  test-delay check passed. The agent instructions check reported zero errors and two pre-existing heading warnings.
- The stack was rebased onto `99724fc9`. Main changed documentation screenshot tooling, TODO entries and plan
  bookkeeping. It changed none of the tested code or references. The final tip is `7ad077c6`. The existing evidence
  applies to the rebased stack.
- Skipped the full workspace and native macOS suites. The changes are in the shared desktop UI and its vendored hitbox
  check. The selected Linux tests exercise those paths and include the modeled Mac tunnel click.

## Review gate

All three drafts passed independent code and commit-message reviews. Review led to checking the full panel border,
retaining the press-time refusal across release, keeping one announced status, and letting a keyboard-taking overlay
such as a confirmation stay readable. It also caught a stale hover that could raise an explanation again after the
terminal left the screen. The scope recheck confirmed the new display slot uses the same layer manager as the cards.

### Landing

#### What landed with it

It landed in a round of three plans: covered-link-refusal, demo-video-capture and replace-with-checkout-hint. Since its
base, main gained the previous round's nine plans (among them session group dragging, model families, the session-limit
notice and compare-on-save checkout settings), bookkeeping and docs screenshot tooling. None of it changes the terminal,
its link panel, the file links or the approval cards; the previous round's concealed-link change alters which text forms
a link, not the link the refusal compares. The demo video plan's recording overlay, in the same round, takes no pointer
input, so it can neither refuse nor admit a click.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The patch to the vendored GPUI
core is the smallest that does the job: one method that asks whether anything drawn later covers a given hit area, using
GPUI's own hit test, with its own tests, and it is in step with its patch file.

#### Fixes made while landing

- **The raised refusal could cover the link's own panel and let the click through.** The plan's change that raises the
  "cannot open" explanation above approval cards kept it raised too long. The explanation stayed raised as long as the
  pointer was anywhere on the refused link, but moving one cell along the link reopens the panel, and the raised line
  then painted over the panel's top, its destination line. The cover check only sees surfaces that take clicks, which
  the line does not, so the next click opened the link with part of the panel hidden, which SPEC.md forbids. Two ways
  in: a menu covers the panel, the click is refused and the menu closes, the user moves one cell, and the next click
  opens; or an approval card covers it and is answered from the keyboard with the pointer still on the link. The old app
  avoided this on purpose. The line is now raised only while the link's panel is still withheld because something
  covered it, so the first move to another cell lowers it under the reopened panel. The test that asserted the old
  behavior now checks the new one, and the screenshot scene stages the withheld state.
- **SPEC_impl.md now records a known gap,** as the plan asked: surfaces that only draw (the quick-launch sequence pill,
  the one surface the old app's manual list of covering surfaces also missed; the launch pill; and a keyboard-focus
  tooltip) do not count as covering the panel, so a click through them is not refused.

#### Things you should know that the report does not say

- When the code was rewritten, several comments lost detail the old ones had: the old app's names, SPEC.md citations,
  and how the stacking order of overlays is computed. Two still describe the cover check as future work. I left these;
  possible follow-up.

#### Checks

The three plans change the desktop app's GUI crate, a patch to the vendored GPUI core, the demo video tooling and its
documents, AGENTS.md and the specs. The landing ran the lints, the GUI crate's whole suite, the vendored GPUI core's own
tests, the video tooling's tests, the app's end-to-end smoke and the reference screenshots (goldens). Builds ran
locally; the GUI suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `python3 -B scripts/test-readme-video.py`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings` and
  `python3 -B scripts/check-gui-lints.py`: passed.
- The vendored GPUI core's tests (`cargo test --manifest-path vendor/gpui-pre/Cargo.toml --features test-support` with
  the native-key, driver-record and new hit-test occlusion tests), locally: passed
  (`3130bce8-b268-4c31-977b-5bc1afe5a949`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`01c51026-2b44-41af-9830-f4c6eb0e081a`).
- The boat runs did not happen: every remote command on all four boats ended without reporting its result, and the
  harness deleted the boats as unsafe to reuse. That looked like a problem on the boat service's side; nothing ran.

On the fixed tree (the covered-link refusal fix and its test, a screenshot scene's staging, and documents), on four
fresh boats and locally:

- `cargo nextest run -p farhelm-gui`, from a locally built archive, split in three across boats: 1085, 1033 and 1022
  passed, 3140 in all, with no runtime skips (`71d71daf-a7e6-4aab-9dc6-39032f010643`,
  `176070d9-503b-4ea8-a89c-39c3f0062335`, `3d8609ce-34b9-431e-821c-e4ed78423b3d`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`409e2e23-c47f-4c6d-9d19-5355b3103ec2`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`29aca505-d814-43b8-b492-c29f1eea2d14`).
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 scripts/check-agents-md.py` and `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, once
  without and once with `--features agent`: passed.

Reused for the fixed tree: the vendored GPUI core's tests and the video tooling's tests, whose code did not change, and
the GUI lint check, the workspace and desktop clippy runs and the changelog format check, since the fixes changed only
the GUI crate (covered by its clippy runs above) and documents, and no lint configuration or changelog fragment.

Skipped:

- The rest of the Rust workspace and the desktop process's tests: the plans stay in the GUI crate and the vendored GPUI
  core.
- A full capture of the demo video: the executor captured the sample and handed it over; the landing changed none of the
  capture code.
- The Mac-only checks: no Mac-specific code changed.
