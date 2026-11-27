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
