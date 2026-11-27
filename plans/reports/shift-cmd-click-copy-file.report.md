# Report: Shift-Cmd-click a file path to copy its contents

## What this was about

A file path printed in a session's terminal could already be clicked to download the file into Downloads, or Cmd-clicked
(Ctrl-clicked on Linux) to download and open it. Getting a file's contents into the clipboard, say to paste a log or a
screenshot somewhere else, meant downloading it and then opening or copying it by hand. The TODO entry asked for a third
gesture: Shift-Cmd-click to copy the file to the clipboard.

When the plan was written you settled what that means: the file's contents only (not a file reference for Finder), an
explicit, loud list of formats ("Only a clear, loud list of specific formats that make sense"), a 10 MB cap, Linux text
only for now with no patch to the vendored GPUI, the hover hint saying "copy contents", and the status wording.

What is on main now:

- **The gesture.** Shift-Cmd-click on a Mac (Shift-Ctrl-click on Linux) on a file path whose hover card is showing
  copies the file's contents into the clipboard. Nothing is written to Downloads and nothing is opened. Plain click
  still downloads, and Cmd-click (Ctrl-click) still downloads and opens allowed types.
- **What can be copied.** Text (valid UTF-8 with no NUL byte, which includes SVG and logs with color codes) on both
  platforms, and on a Mac the image types GPUI can put on the clipboard: PNG, JPEG, WebP, GIF, BMP, TIFF and ICO. They
  are recognized by the file's extension and must start with that format's signature bytes. A `.png` whose contents are
  not a PNG is refused, never copied as text. Any other file is judged by its contents alone: if they are text, it
  copies as text whatever its extension (a `.pdf` that is really plain text copies); if not (a real PDF, an archive,
  AVIF, a program), it is refused with a reason.
- **The cap.** At most 10 MB (10,000,000 bytes). A file the hover card already showed as bigger is refused at once,
  without reading. The read itself also stops at 10 MB, in case the file grew after the hover.
- **The hover hint.** The card's last line adds "⇧⌘-click to copy contents" ("Shift-Ctrl-click to copy contents" on
  Linux) for files up to 10 MB, except images on Linux.
- **The status line.** The line at the top of the terminal says "Copying…", then "Copied to clipboard" (gone after five
  seconds, like a download's success) or "Copy failed:" with the reason (stays until the next click). A click refused
  because the hover card was clipped or covered says "cannot copy: …" instead of "cannot download: …".
- **Behind it.** The desktop app's helm client (its library for talking to the helm) gained a read of a file into
  memory, which goes through the helm's existing download route asked to hand the bytes back instead of saving them, so
  the helm, the supervisor and the protocol did not change. SPEC.md, the implementation notes, the "Work in a session"
  docs page (its download screenshot untouched) and the hover card's reference picture are updated, there is a changelog
  entry, and the TODO entry is removed.

## Things you should know

- **Linux copies text only.** An image there is refused before anything is read, with "only text can be copied here;
  images can be copied on macOS", and gets no copy hint. As you decided, this needs a change to GPUI's Linux clipboard,
  which was out of scope.
- **"Copied to clipboard" means Farhelm handed the contents over.** GPUI reports no clipboard failure, so a write the
  system drops still reads as copied. On Wayland that includes a write from a window that is not focused. Copying an
  empty file there also changes nothing on the clipboard, though it reads as copied. This follows SPEC.md's existing
  rule that clipboard writes are best-effort.
- **One modifier combination changed meaning.** Before, any combination that included Cmd (Ctrl on Linux) opened the
  file, including Shift-Cmd and Shift-Option-Cmd. Now Shift-Cmd copies, and Shift plus Cmd plus anything else (Option,
  say) only downloads. Copying takes exactly Shift and Cmd, so that a combination nobody meant as a copy never reads a
  file into the clipboard. Cmd with Option but without Shift still opens.
- **The newest copy wins.** Starting a copy cancels one still in flight, so a large file copied first can never land on
  the clipboard after a small one copied second.
- **A copy click clears any selection in the terminal.** Shift is also the key that extends a selection, and a click on
  a link only counts while nothing is selected, so without this an earlier selection would grow to the path and quietly
  swallow the copy. The selection is cleared when the button goes down with Shift and Cmd held. If you let go of Shift
  before releasing the button, the selection is gone and the click opens the file instead (it is a Cmd-click by then).
- **A file of unknown size gets no copy hint**, since the card cannot promise it fits, but the gesture still works on
  it, with the read's own 10 MB limit.
- **Under mouse reporting** (a full-screen program that takes mouse input), the copy click still reaches the program as
  a click, the way a download click on a file path already does.
- **Not tried on a Mac.** Every test ran on Linux. The tests set the platform themselves, so the Mac behavior (the
  modifier, the hint, images being accepted) is tested, but no real Mac clipboard received an image. That last step is
  GPUI's own clipboard code, which Farhelm hands a PNG, JPEG or other listed image to unchanged.
- The helm client part has a changelog fragment of kind "none", because it changes nothing visible on its own. The
  feature's fragment notes where the result goes beyond the release-note text you wrote into the plan (the Linux
  gesture, the 10 MB cap, the format list, Linux text only), so the release curation can fold that in.
- The "Work in a session" docs page gained a paragraph about the gesture. No preview was started, since this ran
  unattended.

## Open questions and possible follow-ups

- **Images on Linux.** Copying images there needs GPUI's Linux clipboard to write image data, which means a patch to the
  vendored GPUI. The options are to leave Linux at text only, as now, or to plan that patch as its own piece of work. I
  recommend leaving it until someone asks for it: text covers the common case, and a vendored patch has an upkeep cost
  on every GPUI update.

## PRs

- #5121 feat: let the helm client read a small file into memory (the in-memory, size-capped read of a file).
- #5124 feat: copy a terminal file's contents with Shift-Cmd-click (the gesture, format checks, hint, status line,
  SPEC.md, implementation notes, docs page, reference picture, changelog entry, TODO removal).

## Checks

Run now, on the stack as it lands (rebased onto current main; only a plans-queue commit landed in between):

- The helm client's whole test suite through the test recorder, run `5dcde16b`: 90 passed. It includes the new tests of
  the in-memory read: the request asks for no save and no open, bodies arrive exactly (both as the helm streams them,
  chunked, and with a length), a file of exactly the cap is read and one byte more is refused as too large, a 32 MiB
  file is cut off at the cap without being read to the end, refusals keep the helm's words, and a body that ends early
  is a lost connection. A test against a real helm process checks the request reaches the route.
- The desktop app's terminal tests through the test recorder, run `cc81b9ba`: 351 passed, the new ones among them: which
  modifier combination does what on each platform, which contents reach the clipboard, refusals before reading, the copy
  hint, a copy end to end with its status lines and failures, images on a Mac and on Linux, the newest copy winning, an
  earlier selection not swallowing the copy, and both "cannot copy" refusals.
- Run `eacc60af` was a deliberate check of the selection test: with the selection-clearing switched off, that test
  failed, as it should. It is not a flake; the change was put back.
- The desktop app's reference pictures, a full update through the test recorder, run `584f74b2`: only the file hover
  card's two pictures (light and dark) changed, and I looked at both. They are published to the store that holds these
  reference pictures, and #5124 records them.
- `cargo clippy -p farhelm-gui -p farhelm-helm-client --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the
  test sleep check, `dprint check` on the changed Markdown files, and the changelog format check.

Earlier runs, superseded: `b340118f` (the helm client before its review fixes, 90 passed) and `4d2e279c` (the terminal
tests before the review fixes, 350 passed).

Reused: none.

Skipped:

- The workspace-wide Rust run and the end-to-end suite: nothing outside the helm client and the desktop app's terminal
  code changed, and the helm's route is unchanged.
- The desktop app's end-to-end smoke: it cannot check the clipboard on Linux (the headless desktop never lets the app
  own it), and the click path it would drive is covered by the terminal tests above.
- The docs website build: the docs change is one paragraph of prose with no links or page structure.

## Review gate

Each PR had a fresh-context Opus 5.5 review at high effort. Neither found a blocker.

- #5121: one should-fix, which I fixed: its test of a read cut short used a different way of sending the file than the
  helm actually uses, so it now sends the file the helm's way, and one success test does too. Three small points (two
  overlong comment lines, an unexplained discarded value, changelog wording) were fixed as well.
- #5124: two should-fixes, both fixed. Two copies in flight could leave the older one on the clipboard, so the newest
  copy now cancels the older one, with a test. A test's description promised a step it did not run, and was corrected.
  Of the small points, these were fixed: a duplicated message, refusal reasons Farhelm writes itself now drawn as
  ordinary text (the host's words keep the treatment that makes hidden characters visible), three wording fixes in
  SPEC.md and the changelog entry, and a stronger assertion in the Linux test. Two were kept as they are, both described
  under "Things you should know": the selection is cleared when the button goes down, and Shift-Option-Cmd now
  downloads.

The fixes were not reviewed again; runs `5dcde16b` and `cc81b9ba` ran after them.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained only queue bookkeeping. In the same round, and right after it,
the plan shift-cmd-v-paste-upload landed: Cmd+Shift+V on a Mac (Ctrl+Alt+V on Linux) uploads the clipboard's text to the
session's host as a file and types its path. Both change the desktop app's terminal, SPEC.md and the "Work in a session"
docs page, but they meet nowhere: this plan's gesture is a mouse click on a file path, the other is a key chord, and no
code one plan changed is used by the other. Neither chord nor gesture is bound to anything else. Their SPEC.md and docs
paragraphs agree with each other. Both removed a TODO entry next to the other's, which was the round's one conflict;
both entries are gone.

An independent reviewer and I read the change. The read into memory stops at the 10 MB cap without holding more than
that, the image signature checks are right, every listed image format is one the Mac clipboard takes, and the newest
copy always wins. Nothing in it is more elaborate than the plan needs.

#### Fixes made while landing

None.

#### What the landing made untrue in the report

Nothing changed by the landing itself, but the review found one sentence of the report that was wrong from the start:

- **Under mouse reporting on Linux, the program does not get the copy click.** The report says that when a full-screen
  program takes mouse input, the copy click still reaches the program as a click. That holds on a Mac, where the file is
  copied and the program also gets the click. On Linux, Shift is the key that makes a click act in Farhelm instead of
  going to such a program (as a Shift-drag selects text there), so a Shift-Ctrl-click copies the file and the program
  sees no click at all. This follows the same convention as SPEC.md's Shift-drag rule; only the report's sentence is
  wrong for Linux.

#### Worth knowing

- **The hover card's hint line now wraps awkwardly.** I looked at the refreshed reference picture of the file hover
  card. For a small file that can also be opened, the hints no longer fit on the line with the file's size, so the size
  ("4.8 KB") stands alone and the next line starts with a stray "·" before "click to download". Possible follow-up:
  start the hints on their own line when they do not fit beside the size, or drop the leading separator when they wrap.
- **The copy hint is offered for files a copy will refuse.** The same picture shows "⇧⌘-click to copy contents" on an
  8.4 MB `report.pdf`. A real PDF is refused when clicked ("the file is not text or a supported image"). The executor
  chose to judge by the contents rather than keep a list of extensions that are never text, so that a `.pdf` that is
  really text still copies. That is defensible, but the hint now promises something most PDFs, archives and programs
  under 10 MB cannot do. Possible follow-up: leave the hint off for common binary extensions while still letting the
  click try.
- **Not described in SPEC.md: a copy click clears any selection in the terminal.** The report explains why (Shift would
  otherwise extend an earlier selection and swallow the copy). Possible follow-up: say so in SPEC.md's file-path rule.
- **A copy and a download share one status line**, and neither cancels the other, so whichever finishes last has the
  final word, as with two downloads. Nothing to do unless this confuses in practice.
- **The security-model docs page does not mention copying a file's contents.** Nothing on it became false; copying is
  the same transfer across the host boundary as a download, as SPEC.md now says.

#### Checks

Run once, on both of the round's plans stacked on main as it stood when the landing started (shift-cmd-click-copy-file,
then shift-cmd-v-paste-upload), so each plan's code was tested together with the other's. The SSH test worker was still
unreachable, as in the previous two rounds, so the slow suites ran on a boat sandbox (an x86_64 Ubuntu 24.04 VM), and
clippy ran locally:

- Locally, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent`, all with `-D warnings`: clean.
- On a boat, every test of the helm client (which the copy plan changed), the GUI crate and the desktop app (built once
  for `-p farhelm-helm-client -p farhelm-gui -p farhelm-desktop` and run through the recorder), recorded run
  bf7906bf-b4f3-4cdc-a81e-ad8ee912dd6b: 3,846 passed, 4 skipped (tests marked ignored), and no test that quietly passed
  for lack of systemd or SSH on the machine. That is wider than either executor's selection.
- On a boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  26fb6b9d-b461-481f-87f4-30b46b24ab50: 49 steps passed, with the paste step skipped as on every Linux run, so it says
  nothing about the clipboard; it covers the terminal, its keys and the Keyboard shortcuts overview around them.
- Locally, the full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run
  49fe77c7-3619-45d7-983b-16dc390362d0, and the test that checks the list of screens the screenshot comparison draws,
  recorded run 2364f0f1-04d5-4eb0-8e2b-1ceee283c607: passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format`,
  `dprint check` on the changed Markdown, and the docs website build (`bun run build`, which checks every link): passed.

Reused: nothing. Skipped: the other crates' tests and the supervisor's end-to-end suite, since neither plan changes the
helm, the supervisor or the protocol; any run on a Mac, since none was available to the landing, so the Mac-only parts
(for shift-cmd-click-copy-file, an image reaching the real Mac clipboard; for shift-cmd-v-paste-upload, the Mac menu
item and holding the chord down) are covered only by tests that set the platform themselves.
