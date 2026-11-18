# Settings repeats preference and editor wiring

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Several controls build the same preference command, and two Settings editors independently restore focus to the control
that opened them.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Sparse preference commands repeat the same route

All four construct a sparse `PUT /api/preferences` save command with the same diagnostic label. Share the command
constructor, allowing Order to pass its existing object, and keep each caller's write and completion policy.

Reviewed locations: `crates/gui/farhelm-gui/src/settings/choices.rs:377-387`,
`crates/gui/farhelm-gui/src/session_list/order.rs:212-220`,
`crates/gui/farhelm-gui/src/session_list/host_rail/expansion.rs:71-79`,
`crates/gui/farhelm-gui/src/terminal/tunnels.rs:605-617`. Haiku out-P6.md §1, out-P9.md §1. Suggested bucket **other**.

### Settings editors repeat focus restoration through a stored window

Both editors restore focus to their opener with the same focus mark and use the stored window when the callback has no
window. Share that focus operation and keep each section's editor and refresh bookkeeping local.

Reviewed locations: `crates/gui/farhelm-gui/src/settings/credentials.rs:362-371`,
`crates/gui/farhelm-gui/src/settings/recipes.rs:717-727`. Haiku out-P7.md §1. Suggested bucket **other**.
