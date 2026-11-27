# Report: file hover cards say when a file is too large to copy

## What this was about

In the desktop app, resting the pointer on a file path printed in a terminal shows a hover card. Its third line has the
file's size and the actions you can take: "click to download", "⌘-click to open" where the file may be opened, and since
v0.46.0 "⇧⌘-click to copy contents" (Shift-Ctrl-click on Linux) for copying the file into the clipboard. A copy reads at
most 10 MB, so for a larger file the card left the copy hint out without a word. Someone who knew the shortcut could not
tell whether it was missing or whether the file was the problem. You asked for the card to say "too large to copy
contents" there instead, in the same plain muted style as the other hints, and to keep leaving it out for an image on
Linux (which can only copy text) and for a file whose size the host did not report.

What is on main now:

- A file over 10 MB (10,000,000 bytes, the same limit at which a copy click is refused) reads, for example, "87 MB ·
  click to download · too large to copy contents", after the opening hint when there is one. Files up to 10 MB keep the
  copy hint as before.
- On Linux an image gets neither hint at any size, and a file of unknown size gets neither on either platform.
- The specification's file hover paragraph says the same, and the two hover card screenshots the GUI tests compare
  against (dark and light) were updated: the 87 MB card now shows the new hint, and nothing else in them moved.
- A changelog fragment describes the change, and the TODO entry is removed.

## Things you should know

- **The user docs still mention only the copy hint.** The "Work in a session" page says "The hover shows the copy hint
  for files that fit." That is still true, so I left it alone, since the plan did not include the docs.
- **The release note leaves Linux out of its first paragraph.** The editorial guidance says to keep Linux desktop
  behavior out of public notes, so the entry names only Shift-Cmd-click. The Linux details are in the fragment's context
  paragraph for the curator. The review caught my first draft naming the Linux shortcut.

## Open questions and possible follow-ups

- If you want the docs page to mention the new hint, it is one sentence in "Work in a session" next to "The hover shows
  the copy hint for files that fit."

## PRs

- #5162 feat: say when a hovered file is too large to copy (the hint, its tests, the specification sentence, the two
  screenshots, the changelog fragment, the TODO removal).

## Checks

Run during this plan, through the test recorder:

- Run `ed3a0e15`, `cargo nextest run -p farhelm-gui --lib -E 'test(/^terminal::files::/)'`: 44 passed. These are the
  GUI's file hover, download and copy tests, among them the hint tests: the copy hint at exactly 10,000,000 bytes, the
  new hint one byte over on both platforms and after the opening hint, neither hint for a Linux image at or over the
  limit or for an unknown size, and the new hint for an image over the limit on a Mac. It ran after the review fixes.
- Run `bdd2af48`,
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^terminal::files::tests::(the_copy_hint_appears_only_where_a_copy_can_be_attempted|hints_offer_opening_only_where_the_helm_does_with_the_platforms_modifier)$/)'`,
  with the old behavior put back temporarily: the copy hint test failed on the missing "too large to copy contents",
  which shows it catches the change, and the opening hint test passed. The old behavior was then removed again.
- Run `84f61707`, `python3 scripts/check-gui-goldens.py --update`: the full screenshot update, which changed only the
  two file hover pictures (dark and light). I looked at both before publishing them.
- Run `19d7450b`, `python3 scripts/check-gui-goldens.py --scene 'terminal-file-disclosures'`: the file hover scene
  compared against the published pictures, passed.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test sleep check,
  `dprint check` on SPEC.md and TODO.md, and the changelog format check: all clean.

Reused: every check above covers the change as it lands. They all ran before the final rebase, but only plan bookkeeping
landed on main in between.

Earlier runs, superseded: `5563cbae` (the same 44 tests, all passed, before the review fixes renamed a variable and
reworded a comment).

Skipped: everything else, since only the hover card's hint, its tests and screenshots, the specification, the changelog
and TODO.md changed.

## Review gate

A fresh-context Opus 5.5 review at high effort found no blocker. It confirmed that every case in the plan's acceptance
criteria behaves as specified, and that the card says "too large" exactly when a copy click would be refused for its
size. Its one should-fix, the Linux shortcut in the release note's first paragraph, is fixed (see above). It had four
nits. Two are applied: a clearer name for the Linux image check, and a note on the screenshot scene that its medium and
large files must stay on either side of the 10 MB limit so the scene keeps showing both hints. One is the docs sentence
listed above. The last I left: one test spells out the file's path twice to give it an unknown size, which the reviewer
itself called correct and in the style of the test beside it. The fixes were not reviewed again. Run `ed3a0e15` ran
after them.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained one change: the desktop launcher's dead code for starting a fresh
managed checkout through **Replace with** was removed (delete-dead-replace-checkout-ui, #5160). It touches only the
launcher, not the terminal's file hover card. In the same round, after this plan, agent-hosts-tailscale-column landed
(#5161: `farhelm agent hosts` gains a `TAILSCALE` column), which does not touch the desktop app.

An independent reviewer and I read the change and found nothing to fix. The card says "too large to copy contents"
exactly when a copy click would be refused for the file's size. Only the file hover screenshots changed. I looked at the
new light one: the 87 MB card now ends "click to download · too large to copy contents", and the two smaller cards are
unchanged.

#### Fixes made while landing

- **A TODO.md conflict.** Both of this round's plans removed their own TODO entry, near each other, so stacking them
  conflicted there. I removed both entries; no code changed.

#### What the landing made untrue in the report

Nothing.

#### Worth knowing

- **The changelog fragment is filed as Changed.** A hint users newly see may belong under Added; release curation can
  settle that.

#### Checks

Run once, on the round's two plans stacked on main as it stood when the landing started (file-hover-too-large-hint, then
agent-hosts-tailscale-column), so each was tested with the other and with the previous round's change. One plan changes
the desktop app's file hover card and the other the command line's hosts table and the agent instructions. So the checks
cover the desktop app's GUI crate and its screenshots, the command line's and test fixtures' unit tests, the agent
command tests and the end-to-end agent listing tests, plus clippy over everything. All ran locally. The desktop smoke
test has no step with a file hover card, so no boat was used.

- `cargo clippy --all-targets`, and `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent`
  (the build with the desktop app's test-automation hooks): each reported one warning and nothing else. That warning is
  the unused ntfy settings method in the helm, which has been on main since #5120 and is outside this round, so these
  were run without `-D warnings`, which that warning makes fail.
  `cargo clippy -p farhelm-gui --all-targets -- -D warnings` (the GUI crate, which does not depend on the helm's code):
  clean.
- The GUI crate's whole default test suite (`cargo nextest run -p farhelm-gui`), recorded run
  e5a9ca08-8299-42f4-a2f5-d67476b22f70: 3,589 passed, none failed, and it skipped its 4 opt-in tests, among them the
  screenshot test the next item runs.
- The full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run 92e47951-51a1-4894-8f97-59f7d14d70a4:
  passed.
- The command line's and the test fixtures' unit tests and the agent command tests
  (`cargo nextest run -p farhelm -p farhelm-fixtures --bins --test agent_cli`), recorded run
  0092d61b-56fa-4cb6-8cba-7d1c3aab253c: 218 passed, none failed. An earlier attempt, recorded run
  4a11a467-f18b-428c-b603-f2c6493cb402, asked for library tests these binary crates do not have and ran nothing.
- The end-to-end agent listing tests on a real helm and supervisor
  (`cargo nextest run -p farhelm --test e2e -E 'test(/agent_listing_real_stack::/)'`), recorded run
  21070767-c81a-4118-8114-eba263fe6c87: 8 passed, none failed, and no test reported that it skipped for a missing system
  service.
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format` and `dprint check` on the changed
  Markdown: passed.

Reused: nothing. Skipped: the rest of the Rust workspace's tests, since nothing else changed; the desktop smoke test, as
above; the opt-in tests; and any run on a Mac, since none was available to the landing.
