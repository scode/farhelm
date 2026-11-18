# Session list actions repeat admission and progress logic

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Session actions repeat their request setup, current-work lookup, and progress rendering. Compact and full rows also
repeat part of the status display.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Session actions repeat the same operation admission

Each action finds the current operation store, handles a bad command path, and starts work under the session key. Add a
shared session start helper that takes the command result and completion and returns `NotStarted`, leaving
action-specific guards, logging, and progress clocks at the call sites.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/menu.rs:601-615`,
`crates/gui/farhelm-gui/src/session_list/environment.rs:625-642`,
`crates/gui/farhelm-gui/src/session_list/restart.rs:278-306`, `crates/gui/farhelm-gui/src/session_list/stop.rs:226-247`,
`crates/gui/farhelm-gui/src/session_list/delete.rs:366-392`,
`crates/gui/farhelm-gui/src/session_list/environment_delete.rs:242-267`,
`crates/gui/farhelm-gui/src/session_list/replace.rs:319-346`. Haiku out-P6.md §1. Suggested bucket **other**.

### Session actions repeat the current work lookup

Expose the existing lookup of in-flight work by session key to the other session actions. Each caller can still decide
which kinds of work it cares about.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/outcome.rs:432-437`,
`crates/gui/farhelm-gui/src/session_list/stop.rs:285-290`, `crates/gui/farhelm-gui/src/session_list/restart.rs:314-319`,
`crates/gui/farhelm-gui/src/session_list/replace.rs:414-419`,
`crates/gui/farhelm-gui/src/session_list/delete.rs:435-442`,
`crates/gui/farhelm-gui/src/session_list/environment.rs:323-344`. Haiku out-P6.md §1. Suggested bucket **other**.

### Stop and Delete draw the same progress title

Both draw the same status label, colors, and spinner in place of the title. Share a progress title builder with a
caller-supplied selector, label, and flex sizing so each row keeps its current layout.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/delete.rs:484-516`,
`crates/gui/farhelm-gui/src/session_list/stop.rs:296-327`. Haiku out-P6.md §1. Suggested bucket **other**.

### Compact and full rows repeat the stale-cause mark

Both versions build the same status ID, name, role, and tooltip. Share that base mark and let each row add its own clock
or text styling.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/row.rs:639-660`,
`crates/gui/farhelm-gui/src/session_list/row.rs:758-769`. Haiku out-P6.md §1. Suggested bucket **other**.
