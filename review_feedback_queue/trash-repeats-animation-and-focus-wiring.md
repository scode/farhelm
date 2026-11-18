# Trash repeats animation and focus wiring

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Trash repeats the spinner's timer and independently looks up the same focus destination on confirmation and
cancellation.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Trash and spinner animations each own the same tick counter

Both hold an animation handle, count ticker notifications on an entity, and redraw the owning view. Share the tick
counter and have the spinner project its count to a step modulo eight.

Reviewed locations: `crates/gui/farhelm-gui/src/trash.rs:777-810`,
`crates/gui/farhelm-gui/src/components/spinner.rs:42-74`. Haiku out-P9.md §1. Suggested bucket **other**.

### Trash confirmation repeats the opener lookup

Confirm and cancel both look up the host's Delete button or the footer's Delete all button and restore focus with a
mark. Share that focus helper and let the caller keep the mark it already took.

Reviewed locations: `crates/gui/farhelm-gui/src/trash/dialog.rs:185-191`,
`crates/gui/farhelm-gui/src/trash/dialog.rs:239-245`. Haiku out-P9.md §1. Suggested bucket **other**.
