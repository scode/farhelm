# A detached terminal's frozen screen is never cleaned up

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

After another window takes over a session, the frozen terminal screen kept in this window is never cleaned up when that
terminal goes away. When the host comes back the pane can be blank, with no terminal and no "take control" button, until
the user leaves and reopens the session, and each stranded screen keeps memory alive.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F32 / COR-TOMBSTONE-LEAK`, tagged **definite**. Anchor and title: `crates/farhelm-ui/assets/terminal.js:2520`
— a frozen terminal screen ("tombstone") is never cleared when its terminal leaves the view, so the pane can come back
blank.

In the session view, the browser-side terminal code (`crates/farhelm-ui/assets/terminal.js`) keeps a "tombstone" when
automatic reconnect gives up because of a takeover by another client or a stall. A tombstone is the terminal's last
screen, held on display so the user sees what they lost alongside the "Detached … take control" banner. It also stops
the view from silently reattaching until the user asks. `cancelReconnect(el, "restore")` (`:1586-1610`) creates it,
unmounts the live terminal element and removes its reconnect controller. After that, the element appears only in the
`tombstones` map.

Every time the set of terminals the view wants changes, the view's reconciler `sync()` tears down terminals that left.
It walks the union of three maps, mounted terminals, pending mounts and reconnect controllers (`:2520`), but not
`tombstones`. So when a tombstoned terminal leaves the wanted set, its tombstone is never buried. That happens when the
tab is closed or reaped, or when the session view syncs to an empty set because the host went stale. The comment at
`:1142-1144` explicitly promises that "a departed terminal takes its tombstone with it". Only the view's full teardown
(`unmountAll`, `:5178`) includes tombstones.

Two problems follow. First, tombstones are keyed by element id. When the host comes back, the same terminal returns with
the same id, path and generation, and `sync()` asks `tombstoned(spec)` first (`:2581`). That returns true and skips the
element before the takeover branch that paints the "Detached … take control" banner. If the page has since re-rendered
that element, the user gets a blank pane with no terminal, no banner and no take-control button until they leave and
reopen the session. Second, each stranded tombstone keeps a full xterm instance alive (up to 12,000 lines of scrollback
plus listeners) for the life of the view, which is a memory leak. Suggested change: add `...tombstones.keys()` to the
teardown union at `:2520`, and let the `held` lookup fall back to `tombstones.get(el)`, which already carries path and
generation.
