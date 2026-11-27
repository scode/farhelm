# The Skip session confirmations switch's in-app help was not updated for the open-tabs exception

Reviewed commit: 52fec2470f4f06ab08f336c3cb6cdf62100a6587

## TLDR

With the switch on, deleting a single session that has shell tabs open now always asks first. The website docs were
updated to say so, but the switch's own hover help and description in Settings still say it makes delete act "on the
first click", naming only YOLO, host and trash as exceptions.

## Details

crates/gui/farhelm-gui/src/settings/choices.rs:305-306 (tooltip) "Stop, delete, replace, restart and close terminal tabs
on the first click"; choices.rs:357-359 (description) adds only "YOLO, host and trash confirmations still follow their
own rules." SPEC.md's Skip session confirmations paragraph now says the switch does not skip a single session's Delete
while it has an open terminal tab. The header's Delete help was fixed for exactly this reason (delete.rs:270-271: so it
"does not promise a Delete without a question"); the switch's help makes the same promise. Pre-existing part: the
environment Delete exception was already missing. Fix: add both exceptions to the description at choices.rs:357.

Reviewer revision notes: Revision: adds that the gap was known at authoring. The changelog fragment
releasing/changelog.d/terminal-tab-shortcuts.md, edited by 52fec2470f, says in its curation context: "The Settings
switch's own help still says it deletes on the first click; it does not mention the open-tabs exception (nor the sandbox
and container Delete it never skipped)." So the omission was noticed and left, not fixed; nothing else in the tree fixes
it.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (misleading user-facing text; behavior is safer than the text says).
- Confidence: confirmed (source).
- possible_cover: the commit's own changelog fragment notes the gap as known (terminal-tab-shortcuts.md, curation
  context); there may be a TODO or queue entry for it that I have not searched, per the brief.
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g10/f1 (finding), group g10 "Session list menus: delete with open tabs, marked menu focus", findings log line 3.
