# A copy's and a save's status lines replace each other whichever finishes last, so a copy failure can vanish before the next click and a running save can lose its "Downloading…"

Reviewed commit: 90861ba4ad72f3399302d829a087562dfd6135f7

## TLDR

Click a large file to download it, then Shift-Cmd-click a binary file to copy it. The copy fails fast with "Copy failed:
the file is not text or a supported image", but when the download finishes a moment later its "Saved to …" replaces that
failure, which then fades, so the user never learns why the copy did nothing. The other order: start a copy, then a
download; the copy's "Copied to clipboard" replaces "Downloading…" and fades after 5 s while the save is still running,
leaving no status until it ends.

## Details

The copy reuses the download's single status line (download.rs `SENTENCE_ID`, now pub(super), and `show`, which replaces
whatever the line said and cancels its fade). FileLinks keeps several downloads and one copy in flight independently
(files.rs `downloads` and `copying`), and each completion calls `show` with no regard to what the line currently
reports. SPEC.md, "Terminal experience", file-path bullet: a copy's "Copy failed:" with the reason "stays until the next
click", and a save's failure "stays until the next click or download there"; an earlier save completing is neither, yet
it removes the copy's failure. Concurrent saves already shared the line before this commit, but the copy adds a second,
independently timed operation whose failure is the only explanation the user gets. Verify by reading copy.rs
`read`/`copy_failed` and download.rs `saved`/`show`. A fix could keep a failure line until a press (not letting a later
success replace an AtPress line), or keep one line per operation kind.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other. Misleading status, no data loss.
- Confidence: confirmed from source.
- possible_cover: none known; the concurrent-downloads half of this predates the range.
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g11/f2 (finding), group g11 "Reading a small file into memory and copying a terminal file", findings log line 2.
