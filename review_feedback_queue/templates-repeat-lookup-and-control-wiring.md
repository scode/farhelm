# Template surfaces repeat lookup and control wiring

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Template screens repeat lookups, refusal wording, control construction, and shortcut input mapping.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### The launcher and switcher repeat template lookup refusals

Both paths look up a named template and assemble identical unavailable and missing-name messages. Share the lookup and
message parts, with the surface supplying the sentence ID.

Reviewed locations: `crates/gui/farhelm-gui/src/templates/dispatch.rs:221-243`,
`crates/gui/farhelm-gui/src/templates/dispatch.rs:295-313`. Haiku out-P8.md §1. Suggested bucket **other**.

### The template editor and sequence suggestions repeat the available-name read

The editor's name check and the sequence's advisory list both require an available collection and collect names in helm
order. Share that read and retain the sequence's separate Enter lookup, which deliberately uses retained names.

Reviewed locations: `crates/gui/farhelm-gui/src/templates/armed.rs:475-499`,
`crates/gui/farhelm-gui/src/templates/editor.rs:1494-1507`. Haiku out-P8.md §1. Suggested bucket **other**.

### The template editor and save panel repeat the name-kind radio

Both build the Base name and Fixed name options with the same labels and help. Share the radio builder, giving each
surface its own ID, current selection, and change handler.

Reviewed locations: `crates/gui/farhelm-gui/src/templates/editor/rows.rs:652-678`,
`crates/gui/farhelm-gui/src/templates/save_panel.rs:308-340`. Haiku out-P8.md §1. Suggested bucket **other**.

### Templates and the launcher read the same model catalog

Both build and decode the same `GET /api/launch-catalog` request. Share the typed read and leave retries, failure
presentation, and quick-launch continuation to their existing callers.

Reviewed locations: `crates/gui/farhelm-gui/src/templates/dialog.rs:766-789`,
`crates/gui/farhelm-gui/src/launcher/view.rs:477-500`. Haiku out-P8.md §1. Suggested bucket **other**.

### Template shortcuts and terminal input both identify physical keypad digits

Both paths decode the same native Mac keycodes and XKB names. Share a physical keypad digit lookup in the input mapping
crate and retain the terminal's separate logical-character check and operator mapping.

Reviewed locations: `crates/gui/farhelm-gui/src/templates/armed.rs:84`,
`crates/gui/farhelm-gui/src/templates/armed.rs:186-194`, `crates/gui/farhelm-input-mapping/src/keys.rs:218-244`. Haiku
out-P9.md §1. Suggested bucket **other**.
