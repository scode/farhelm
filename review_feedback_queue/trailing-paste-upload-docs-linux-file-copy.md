# Paste via file upload docs describe Linux file-manager copies as uploading the path's text, which the Wayland clipboard no longer does

Reviewed commit: 7a91219346199d9c6d570bb0b2ebdf72f125527f

## TLDR

The Work in a session docs page tells Linux users that a file copied in a file manager and pasted with Ctrl+Alt+V is
uploaded as a text file holding that file's path. Since the Wayland file-paste fix five days earlier, a file manager's
copy that offers a file list uploads the actual file, the same as an ordinary paste. A user who reads the docs expects a
small text file with a path and instead uploads the whole file to the session's host.

## Details

work-in-a-session.mdx:80-81 (added by this commit) reads: "Screenshots and files upload just as they do with an ordinary
paste, and a file that an ordinary paste on Linux turns into its path is uploaded as that path's text." It builds on the
paragraph just above it (lines 74-75, which predate this commit): "On Linux, a file you copy in a file manager pastes as
text, usually its path on your machine, instead of being uploaded". Commit 2a81e97aa (fix: attach files pasted from a
wayland file manager, #4682, 2026-11-22) changed the vendored Wayland clipboard so the terminal's payload read asks for
local files (text/uri-list) first, then an image, then text (vendor/gpui-pre-linux/src/linux/wayland/clipboard.rs:29-37,
ReadOrder::Payload: "local files, then an image, then text, so a file manager's copy attaches its files rather than
typing their paths"). attached.rs's paste docstring at the tip says the same ("On Wayland a file manager's URI list
supplies local files"). Farhelm supports Linux only on Wayland (Cargo.toml:263). So for a file manager that offers
text/uri-list, both an ordinary paste and Paste via file upload attach the file, and the new sentence describes behavior
that no longer happens. The sentence is only true for a file manager that offers the path as plain text with no file
list. Verify by reading the two docs paragraphs against ReadOrder::Payload and the classify ranking (files before text,
attachments.rs classify). Fix: drop the Linux clause at lines 80-81, and correct lines 74-75 so they say a file
manager's copy uploads the file, perhaps keeping a caveat for file managers that offer only text. Lines 74-75 were
already stale after #4682; this commit restated them as a new claim about the new feature.

### Triage notes

- Recorded bucket: `other`. The docs website is excluded from trailing-review targets; the reviewer found this while
  reading the page as context for 7a91219346, which edited it.
- Suggested bucket (reviewer): other. This is misleading user-facing documentation with no data loss, though it can lead
  a user to upload a file they meant to reference only by path.
- Confidence: likely. The source confirms the Wayland payload read ranks text/uri-list files over text. The unverified
  premise is that common Linux file managers (for example GNOME Files and Dolphin) offer text/uri-list on copy; commit
  2a81e97aa's message ("A file manager can supply both files and their paths as text") assumes they do.
- possible_cover: none known. The stale pre-existing paragraph at lines 74-75 may have its own queue entry from review
  of #4682.
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g13/f1 (finding), group g13 "Paste clipboard text via file upload", findings log line 9.
