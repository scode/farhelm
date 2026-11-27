# A copy-contents click's late clipboard write overwrites whatever the user copied while it was reading

Reviewed commit: 90861ba4ad72f3399302d829a087562dfd6135f7

## TLDR

Shift-Cmd-click on a file starts a read that can take seconds (up to 10 MB from a remote host). If, meanwhile, the user
copies something else (selects text in a terminal, which copies on release, presses Cmd-C, uses a click-to-copy button)
or a program writes the clipboard through OSC 52, the file's contents replace it the moment the read finishes. The user
then pastes the file instead of what they copied last, and the only hint is a fading "Copied to clipboard".

## Details

FileLinks::copy (copy.rs, around lines 170-205 at the tip) spawns a task that awaits Transfers::download_bytes and then,
in FileLinks::read, calls cx.write_to_clipboard unconditionally. The only supersession rule is between file copies: a
new copy replaces `copying`, which drops the earlier read (module and method docs: "Only the latest copy may reach the
clipboard"). Nothing records that some other clipboard write happened after the click. SPEC.md "Terminal experience"
states that every completed terminal selection re-copies, so an ordinary drag-select during a slow read is itself a
later clipboard write that the file then clobbers. SPEC.md's file-path bullet says only that a copy says "Copying…" then
"Copied to clipboard"; it neither accepts nor rules out the stale overwrite. The best-effort clipboard rule covers the
OS losing writes, not Farhelm reordering its own. Verify by reading copy.rs `copy`/`read` and the selection copy path:
no clipboard-generation or 'newer write' check exists. A fix could abandon an in-flight copy when the app itself writes
the clipboard (a shared generation counter that every clipboard write bumps, checked before the late write), or at least
on the next terminal selection copy or press; the status line would then say the copy was superseded.

### Triage notes

- Recorded bucket: `high`. The reviewer suggested other but said it could be high if pasting the wrong content is likely
  enough to matter; recorded as high because the bucket is uncertain.
- Suggested bucket (reviewer): other. The user loses a clipboard entry, not a file, credential or session, and they
  started the copy themselves. It could be high if the triager thinks pasting the wrong content (say, into a terminal or
  a chat) is likely enough to matter.
- Confidence: confirmed from source that no ordering check exists. That users hit it depends on reads slow enough to
  overlap another copy, which is likely for multi-MB files on remote hosts.
- possible_cover: none known
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g11/f1 (finding), group g11 "Reading a small file into memory and copying a terminal file", findings log line 1.
