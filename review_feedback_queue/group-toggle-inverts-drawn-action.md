# A group toggle can reverse the action shown

Reviewed commit: `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`

## TLDR

Clicking Collapse group can expand the group if its shared state changed after the frame was drawn. The next frame then
shows the opposite of the action you chose.

## Details

The label and expanded state come from the frame's group header. The callback carries the group ID, then reads the
current collapse preference and sends its inverse. A queued or shared update can change that preference while the
earlier frame is still displayed. The action-binding rule in `SPEC.md:5383` requires the direction from the displayed
frame. Record the shown direction with the callback or ignore the action when it is stale. Source:
`crates/gui/farhelm-gui/src/session_list/groups.rs:162` and `:223`.

The group UI introduced this path in `6c00dbfb9`. Source-confirmed. Reported independently from the session-group
rendering, ordering, persistence, and nesting assignments.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g025/g025-01.
