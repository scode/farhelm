# Launcher views repeat labels and presentation rules

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Launcher controls, search, and recent setups have separate implementations of labels and display rules that are supposed
to match.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Share effort search and change words

Search and the change notice lowercase the same ASCII effort labels and use `default` for no choice. Put their shared
spelling beside the effort labels.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/search.rs:413-419`,
`crates/gui/farhelm-gui/src/launcher/view/changed.rs:104-110`, `crates/gui/farhelm-gui/src/permission/words.rs:16-27`.
Haiku out-P4.md §1. Suggested bucket **other**.

### Share the default-effort help

The select, radio group, and search row all promise the agent’s default reasoning effort in the same words. Give them
one string beside the effort labels.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/view/choices.rs:231-235` and :260-265,
`crates/gui/farhelm-gui/src/launcher/search.rs:361-362`. Haiku out-P4.md §1. Suggested bucket **other**.

### Reuse the managed checkout label

Recent setups and the checkout summary both spell a repository as `gh:owner/name`. Share that label on top of
`repo_identifier` and append the preview status only where it applies.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/recents.rs:244-250`,
`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1184-1191`,
`crates/gui/farhelm-gui/src/launcher/draft.rs:194-197`. Haiku out-P4.md §1. Suggested bucket **other**.

### Share the repeated restart refusal words

Agent and Command launches separately construct the same refusals about unsupported resume and uncaptured conversations.
Share the harness-specific formatter and the fixed no-conversation string, keeping the conditions for each launch kind.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/entry.rs:348-361` and :371-379. Haiku out-P4.md §1. Suggested
bucket **other**.

### Share the escape spelling used to cut folder labels

Shortening a folder label repeats the protocol text module’s rule for which characters become escape notation. Share an
iterator over whole display units so the folder label can cut between units and the escaping rule has one source.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/view/folder_list.rs:89-106`,
`crates/farhelm-proto/src/text.rs:113-132`. Haiku out-P5.md §1. Suggested bucket **other**.

### Share recent-setup permission rendering

Both recent-setup surfaces resolve the effective mode and render its label with YOLO in danger ink. Share the permission
fragment and leave the strip’s default omission and each surface’s surrounding summary where they are.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/view/recents.rs:354-383`,
`crates/gui/farhelm-gui/src/launcher/view/search.rs:654-655` and :683-694. Haiku out-P5.md §1. Suggested bucket
**other**.
