# Terminal help and input repeat modifier mappings

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The terminal and its help keep separate copies of platform modifier rules. The terminal also copies modifier conversion
for mouse and keyboard input.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Drag help repeats the terminal's selection modifier rule

The help and the input check independently choose Option on macOS and Shift on Linux. Share a platform-to-modifier
mapping with both a display name and a modifier test so the instructions follow the behavior.

Reviewed locations: `crates/gui/farhelm-gui/src/terminal/drag_notice.rs:108-113`,
`crates/gui/farhelm-gui/src/terminal/attached.rs:3717-3722`. Haiku out-P8.md §1. Suggested bucket **other**.

### Mouse and keyboard input copy the same modifiers

Both paths copy the same five GPUI modifier fields into the terminal input type. Share that conversion and let the mouse
path call `.bits()` on the result.

Reviewed locations: `crates/gui/farhelm-gui/src/terminal/attached.rs:1942-1948`,
`crates/gui/farhelm-gui/src/terminal/attached.rs:3522-3531`. Haiku out-P8.md §1. Suggested bucket **other**.
