# Page keys skip rows in a lifted terminal

Reviewed commit: 0592c5c6d667abe2f08706185b7566febb3434f7

## TLDR

Keyboard paging loses its shared edge row when the terminal is lifted by one row and skips retained output between pages
when two or more rows are lifted.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/terminal/attached.rs:1598`.

attached.rs 1591–1601 promises one line of overlap, but uses the full committed height minus one. Painting physically
removes lifted rows at the top (1812–1826 and 2116–2138). At a stable lift L the visible page spans only rows minus L.
On an unclamped page step adjacent pages therefore overlap by zero when L is one, and leave L minus one rows unseen when
L exceeds one. The shortcut remains available under the displayed exit/status lines when focused (attached.rs
2341–2360). Existing attached/tests.rs 1335–1385 asserts a two-row lift with both lines, and 3932–3942 establishes that
a stalled snapshot can keep terminal focus. SPEC.md at this revision 3754 provides history paging from the keyboard and
SPEC.md tip 3302–3304 promises access to retained lines. The one-line overlap is explicit in the handler's contract and
the unlifted assertion at attached/tests.rs 3166–3212. The exited-alternate assertion at 3042–3045 checks only leaving
and returning to follow. The page calculation is present before both patches at parent
3c3d8a78fea8a8e29ed502ddd5cf7384c1d802f3 attached.rs 1593–1604; 0592c5c6 newly makes paging available into retained
primary history on exited alternate screens. Derive the page step from the visible row count. Bucket other for a
keyboard-browsing gap with output still retained and reachable by finer scroll controls. Confidence confirmed by
geometry, handler and existing lift assertions. No paging experiment was run. Possible coverage none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `scrollback/l15`.
