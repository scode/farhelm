# Screen readers can lose a session’s group context

Reviewed commit: 6c00dbfb951ce35de11c669f1c3e2bb206e02146

## TLDR

The sidebar uses indentation to show which group contains a session. A screen-reader user can reach the session without
any announcement of its group, especially after the header scrolls away.

## Details

The virtual list renders headers and session rows as siblings. The grouped flag adds visible indentation and a guide,
while each row's accessible name and description describe its session without the group. At
`c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see `crates/gui/farhelm-gui/src/session_list.rs:1130` and
`crates/gui/farhelm-gui/src/session_list/row.rs:325`. An accessible group name on each member could preserve this
context when the virtual list no longer draws the header.

Suggested bucket **other**. Source-confirmed absence from the tree from trailing reviewer g029/g029-d6. Equivalent
access to this particular cue is inferred from the Session groups contract. Screen-reader output was unmeasured.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g029/g029-d6.
