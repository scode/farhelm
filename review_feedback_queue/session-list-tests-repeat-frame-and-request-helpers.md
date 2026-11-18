# Session-list tests repeat frame and request helpers

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Session-list tests maintain separate copies of their frame queries, mouse gestures, and fake-helm acknowledgements.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### List and lifecycle tests duplicate wheel scrolling

Both helpers move the pointer to the session rows, send the same vertical pixel scroll and settle the window. Reuse one
session-list scroll helper.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/list_tests.rs:57-67`,
`crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs:1707-1717`. Haiku out-T5.md §2. Suggested bucket **other**.

### Host rail and session menu tests duplicate the drawn check

Both helpers test for recorded debug bounds under a selector. Put the check in test support beside the existing bounds
helper.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs:1543-1546`,
`crates/gui/farhelm-gui/src/session_list/menu_tests.rs:128-131`. Haiku out-T5.md §2. Suggested bucket **other**.

### Frame queries duplicate the walk up parent links

The row and rail text collectors visit the same subtree in frame order, and the ancestry checks walk the same parent
links. Reuse one frame helper for the parent walk, preserving root inclusion for text collection and checking each
matching answer when IDs are reused.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs:234-264,1759-1774`,
`crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs:142-158,1142-1162`,
`crates/gui/farhelm-gui/src/session_list/row_tests.rs:53-75`. Haiku out-T5.md §2. Suggested bucket **other**.

### Three session-list suites collect all painted text the same way

All three wrappers take a fresh frame and return its painted text. Give session-list tests one shared helper for that
operation.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/environment_delete_tests.rs:94-97`,
`crates/gui/farhelm-gui/src/session_list/environment_tests.rs:231-234`,
`crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs:176-179`. Haiku out-T5.md §2. Suggested bucket **other**.

### Row-menu tests repeat the same click-and-settle sequence

Both generic helpers click the row-menu toggle and the requested item, then settle. Share that helper and have the
Delete-specific wrapper supply its item ID.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/environment_tests.rs:224-229`,
`crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs:160-165`,
`crates/gui/farhelm-gui/src/session_list/menu_tests.rs:146-151`. Haiku out-T5.md §2. Suggested bucket **other**.

### Fake-helm drains repeat the same successful acknowledgements

These helpers drain every command and give each the same empty 200 in order. Share a drain that returns the acknowledged
commands, then let each caller keep its own path filter and result shape.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/seen_tests.rs:37-52`,
`crates/gui/farhelm-gui/src/session_list/pin_drag_tests.rs:34-48`,
`crates/gui/farhelm-gui/src/session_list/marking_tests.rs:396-409`,
`crates/gui/farhelm-gui/src/session_list/pins_tests.rs:33-49`,
`crates/gui/farhelm-gui/src/session_list/retired_tests.rs:119-129`,
`crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs:182-188`. Haiku out-T5.md §2. Suggested bucket **other**.
