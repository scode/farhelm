# Saving an untouched host alias can restore an older alias

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Clicking Save without typing can undo a host rename that just happened through an approved agent request.

## Details

Bucket: **other**. GUI migration final whole-tree review, R8-4.

Source references: `crates/gui/farhelm-gui/src/hosts/settings.rs:609–623, 668–700, 921`,
`crates/gui/farhelm-gui/src/drafts.rs:114, 153, 226` at the reviewed commit (main
`crates/gui/farhelm-gui/src/drafts.rs:114, 153, 248`), `crates/farhelm-helm/src/hosts.rs:988, 1009, 1029, 1037, 1086`.

The untouched Alias editor follows pushes, but its Save button can still submit the currently held value. Open it,
approve an agent rename, then save before the new alias is drawn or while the agent's locked edit is ahead of the GUI
save. The helm serializes the writes, but update_alias uses the provided value unconditionally and can commit the old
alias last.

Sol rated it source-confirmed and critical. Opus verification R8-4 confirmed the gap and rated it noncritical: one
ordinary alias is easy to restore and the untouched field normally follows pushes. SPEC.md “One GUI at a time” prohibits
undoing approved agent or CLI writes. Its sub-second delivery allowance is possible cover for before-redraw clicks but
does not clearly permit a redundant Save to undo a write that was already in progress, and the template last-write-wins
exception does not apply.

Treat saving an untouched draft as Cancel, or compare the expected prior alias under the existing locks. Test a GUI save
queued behind an agent write.
