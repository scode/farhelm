# Shared GUI controls repeat rendering and validation

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Controls draw their own focus rings and repeat small formatting rules that should stay consistent across forms.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Share the focus-ring element

Each control builds an absolute two-pixel border for its focus ring. Share an element builder that takes the insets and
color and leaves corner styling and the decision to show the ring with each control.

Reviewed locations: `crates/gui/farhelm-gui/src/components/button.rs:1163-1200`,
`crates/gui/farhelm-gui/src/components/select.rs:414-427`,
`crates/gui/farhelm-gui/src/components/radio_group.rs:775-787`, :840-852, :900-912 and :953-965,
`crates/gui/farhelm-gui/src/components/menu_item.rs:312-326`,
`crates/gui/farhelm-gui/src/components/list_row.rs:125-136`,
`crates/gui/farhelm-gui/src/components/tab_strip.rs:240-256`,
`crates/gui/farhelm-gui/src/components/copy_field.rs:288-299`,
`crates/gui/farhelm-gui/src/components/checkbox.rs:240-251`, `crates/gui/farhelm-gui/src/components/mod.rs:545-558`.
Haiku out-P2.md §1. Suggested bucket **other**.

### Share ASCII capitalization for validation errors

All three capitalize the first byte of a validation reason when it is an ASCII letter. Share that formatter so the three
forms keep the same behavior for punctuation and non-ASCII input.

Reviewed locations: `crates/gui/farhelm-gui/src/components/custom_flags.rs:156-162`,
`crates/gui/farhelm-gui/src/settings/recipes/editor.rs:125-131`,
`crates/gui/farhelm-gui/src/templates/fields.rs:540-546` (also
`crates/gui/farhelm-gui/src/components/custom_flags.rs:157-162`,
`crates/gui/farhelm-gui/src/settings/recipes/editor.rs:126-131`,
`crates/gui/farhelm-gui/src/templates/fields.rs:541-546`). Haiku out-P2.md §1, out-P4.md §1 and out-P7.md §1, out-P8.md
§1. Suggested bucket **other**.
