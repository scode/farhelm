# Row directory and host tooltips are unescaped

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Hovering a row to check a session's full working directory can show a visually reordered path when the directory
contains direction-control characters.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as `F31 / SEC-ROW-TOOLTIPS-RAW`,
tagged **possible**. Anchors and title: `crates/farhelm-ui/src/list/row.rs:1412`,
`crates/farhelm-ui/src/list/row.rs:1608`, `crates/farhelm-ui/src/list/row.rs:1623` — The sidebar row's directory and
host tooltips are raw while the title tooltip is escaped

Within one sidebar row, the title's tooltip goes through `display_peer`, but three other tooltips do not. They are the
compact-mode open button's tooltip, which is the full cwd (line 1412), the host name's tooltip (line 1608), and the
`.session-cwd` tooltip (line 1623). The visible host name and cwd text are direction-isolated (via `peer-value` or
`dir="ltr"`), but as the code's own comment at `row.rs:1519` notes, native tooltips do not inherit DOM direction
isolation. The cwd tooltip matters most, because the visible path is shortened and SPEC relies on this tooltip as the
way to see the full, untouched directory. A directory name containing direction-control characters can display reordered
there. This is an inconsistent application of the peer-text rule rather than a new class of problem. The fix is to wrap
these three `title` values in `display_peer`.
