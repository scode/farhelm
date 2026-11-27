# Report: paste clipboard text as an uploaded file

## What this was about

Pasting a large block of text into a session's terminal sends all of it to the agent as one giant paste into its input
box. Your TODO entry asked for a second paste that instead puts the clipboard text in a file on the session's host and
pastes that file's path. During planning you settled the details:

- the file goes in the session's existing attachments, the same place pasted screenshots go;
- the chord is Cmd+Shift+V on a Mac and Ctrl+Alt+V on Linux, because Ctrl+Shift+V is already ordinary paste there;
- the text is uploaded exactly as copied, as `pasted-<n>.txt`, numbered together with pasted screenshots;
- files and images on the clipboard upload as an ordinary paste uploads them, and an empty clipboard does nothing;
- holding the chord uploads once;
- it is listed in the Mac Edit menu, in the Keyboard shortcuts dialog and in the docs.

One PR builds it (#5123). Nothing on the helm or the supervisor changed: the new chord uses the same upload path that
pasted screenshots already use, so it works the same on local, SSH and sandbox hosts.

### What changed for someone using Farhelm

- **Cmd+Shift+V on a Mac, Ctrl+Alt+V on Linux**, in a focused terminal, uploads the clipboard text to the session's host
  and types the file's path at the prompt, followed by a space, the same way a pasted screenshot's path is typed. The
  text itself never reaches the program.
- **The Mac Edit menu** has a new **Paste via File Upload** item below Paste, which shows the chord as ⇧⌘V. It is
  enabled only while a terminal has focus, and disabled in a text field.
- **The Keyboard shortcuts dialog** lists **Paste via file upload** at the end of its Terminal group, shown as ⌘+Shift+V
  on a Mac and Ctrl+Alt+V on Linux. It is the only terminal key the dialog lists; Paste, Copy, Select All and the page
  keys are still left out, and SPEC.md now says so.
- **A terminal that is not connected** refuses the chord with the usual "This terminal is not connected, so nothing was
  attached" line, rather than dropping the text silently. An ordinary paste of text into such a terminal still does
  nothing, as before.
- **Docs:** the Keyboard shortcuts page lists the chord in its terminal keys table, and the "Copy, paste, and links"
  section of the page on working in a session describes it.

## Things you should know

- **Behind the "Session unavailable" card, the chord does nothing and shows nothing.** The plan asked for the refusal
  line there too. But a terminal covered by that card takes no keys or menu commands at all, and putting the card up
  cancels a paste still waiting for the clipboard. So the chord, like ordinary paste, simply does nothing there. I kept
  that rather than making the covered terminal take this one command just to refuse it, since focus has already moved
  off a covered terminal and the card itself says why it is unavailable.
- **Linux programs lose Ctrl+Alt+V in the terminal.** Before, that chord went to the program (Emacs, for one, uses it to
  scroll the other window). Now Farhelm takes it. The changelog fragment says so.
- **The file is not temporary.** The plan's release note calls it a temporary file. It is an ordinary attachment: it
  stays with the session, through Stop, until the session is deleted, like a pasted screenshot. The fragment says so, so
  the release curation can reword that note.
- **Order with an ordinary paste.** If you press the new chord and then ordinary paste quickly, the ordinary paste's
  text arrives first: the uploaded file's path is typed when its upload finishes, as attachment paths always are.
- **Files copied in a Linux file manager.** The docs page already says that on Linux such a copy can paste as its path
  on your machine. Where it does, the new chord uploads that path as text rather than the file. The docs say so too.

## Open questions and possible follow-ups

- **Should a covered terminal refuse visibly?** If you want the refusal line behind the "Session unavailable" card, the
  covered terminal would have to keep handling this one command. That is a small change, but it makes this chord behave
  differently from ordinary paste there. I recommend leaving it as it is.
- **Is Ctrl+Alt+V the right Linux chord?** You chose it in planning, and it costs Linux users that chord in terminal
  programs such as Emacs. The alternatives are another chord with Ctrl+Shift (most Ctrl+Shift letters are already
  Farhelm's shortcuts or deliberately left to terminal programs, so any of them would cost programs a chord the same
  way) or no Linux chord, leaving only the Mac menu and chord. I recommend keeping Ctrl+Alt+V unless a Linux user
  reports the loss; changing it later is a one-line keymap change plus its docs.

## PRs

- #5123: feat: paste clipboard text via file upload (also removes the TODO entry).

## Checks

Run now, all on the final code. The last change, from the third review round, was a sentence on a docs page; the Rust
code and the changelog fragment were final before the runs below that cover them, and the docs build ran after it.

- The GUI tests for terminal attachments, the attached terminal, the Keyboard shortcuts dialog, the desktop menu and the
  keymap (`cargo nextest run -p farhelm-gui --lib`, tests under `terminal::attach`, `shortcuts::`, `desktop::` and
  `actions`): 289 of 289 passed (recorded run 23f2af12-5b8f-4747-b111-98e1e7d84dd4).
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings` and `cargo fmt --all -- --check`: clean.
- The test-sleep check (`scripts/check-test-sleeps.py`): no delay without a reason.
- `python3 releasing/check-changelog.py format`, `dprint check` on the changed Markdown it covers (SPEC.md and TODO.md),
  and the docs website build (`bun run build`, which checks every link): passed.

Reused:

- The Keyboard shortcuts dialog's screenshot comparisons in both palettes
  (`scripts/check-gui-goldens.py --scene 'keyboard-shortcuts-*'`): no differences, because the new row is below the part
  of the dialog the picture shows (recorded run d00f859a-c48c-4222-884f-99018b80cd52). It ran on the first complete
  version of the change, which already drew the new row. The review fixes after it changed tests, comments, docs and the
  fragment, nothing that draws, so it still applies.

Skipped:

- The rest of the GUI crate's tests, the other crates' tests, and the end-to-end and desktop batteries. The change is in
  the terminal's paste handling, the keymap, the Mac menu's list of items and the shortcuts dialog's table, and the
  tests above drive those through a real terminal over the test stand-in for the helm. The upload path itself is
  unchanged and is the one pasted screenshots already use.
- A run on a Mac. The Mac menu and chord are checked on Linux from the menu's list of items and the table of terminal
  keys, as the existing menu tests do.

## Review gate

The PR had fresh-context Opus 5.5 reviews at high effort with the plan's charter and the test-authoring checklist. It
passed after three rounds:

- **Round 1** found no bugs. It found four low problems, all fixed: a docstring overstated how pastes are ordered; the
  changelog lacked the Ctrl+Alt+V caveat; my note on the covered-terminal case was wrong, and that case is now reported
  above; and the shortcuts dialog's test did not check that no shortcut it lists wrongly shows Alt. It also found four
  nits, all fixed: stale docstrings, long comment lines, an ambiguous test docstring and inconsistent chord spelling.
- **Round 2** confirmed those fixes. It raised one medium problem, that the dialog's screenshots were out of date. That
  was not so: the comparison had already run on code drawing the new row, with no differences. It also found two low
  problems and four nits, all fixed. The test that keeps terminal keys away from programs now fails if Linux's
  Ctrl+Alt+V ever stops needing its exception there, and the fragment now flags the temporary-file wording. The nits
  were small wording and code tidy-ups.
- **Round 3** confirmed those fixes, including the screenshot answer, and found two low problems, both fixed: I had
  cited the wrong run id for the screenshot comparison (the corrected id is the one above), and a docs sentence about
  files on Linux needed a qualifier. Neither touched code, so the docs build was the check rerun for them, and no fourth
  round was run.
