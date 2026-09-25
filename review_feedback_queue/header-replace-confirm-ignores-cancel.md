# Header Replace confirm ignores a preceding cancel

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A fast cancel-then-replace double click on the session header's Replace prompt can still replace, and so delete, the
session the user just cancelled on.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F10 / COR-HEADER-REPLACE-CONFIRM-UNGUARDED`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/src/session_view.rs:1687`, `crates/farhelm-ui/src/session_view.rs:1689` — The header Replace confirm
skips the prompt-still-open check, so cancel-then-confirm still replaces

The session header's Replace button (added in #928) opens a small popover with "replace" and "cancel". The "replace"
handler (line 1687) does `confirming_header_replace.set(false); header_confirm_replace();` without first checking that
the prompt is still open. Every other confirmation in the UI makes that check: header restart
(`if !confirming()
{ return; }`), the interrupted card's replace, the sidebar's `confirm_delete` (via
`confirming.remove`), row replace, and tab close. Their comments give the reason: a confirm click can be queued just
behind a cancel click in the same event burst, and without the check the queued confirm acts after the user said no.

Here, in that race, Cancel runs first: it clears the flag and releases the shared lock. The queued "replace" click then
runs `replace()` anyway. The replacement is created and the source session is deleted, even though the user cancelled.
The request also runs without holding the lock, since Cancel released it. And at the end of the task,
`lifecycle.release()` frees whatever other operation has claimed the lock in the meantime, which breaks mutual exclusion
for that operation too.

The fix is one line: `if !confirming_header_replace() { return; }` before clearing the flag.
