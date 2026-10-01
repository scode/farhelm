# A displaced window can steal the session back

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

When another window takes over a session just as a terminal tab is opening here, that tab can steal the session back:
this window's agent pane shows "Detached … take control" while the other window gets evicted.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F51 / COR-TAKEOVER-LATCH`, tagged **definite**. Anchor and title: `crates/farhelm-ui/assets/terminal.js:2625`
— the takeover latch leaves terminals that are still attaching running, and they evict the winner.

Farhelm allows one client to be attached to a session's terminals at a time: when a second browser or desktop window
opens the session, it takes the session over and the first one is "displaced". A displaced view is supposed to keep its
last screen and show a "Detached … take control" banner rather than reattaching on its own; otherwise the two clients
would keep evicting each other. In the browser-side terminal code (`crates/farhelm-ui/assets/terminal.js`), the function
that enforces this is `latchTakeover`: when the view learns it lost the session, it sets a latch so nothing more is
attached, and it cancels two kinds of in-progress work: mounts still waiting to start ("pending" mounts) and automatic
reconnect attempts.

It does not touch a third kind: a terminal that the view's normal reconciliation (`sync()`) has already mounted and
whose WebSocket attach is still in flight. That attach goes through the ordinary attach route (`sync()` at line 2606
mounts without the "refuse if someone else owns it" option that automatic reconnects use) and carries this view's lease,
the identifier the supervisor uses to tell clients apart. The supervisor (`displaced_by_attach` in
`crates/farhelm-supervisor/src/service/terminals.rs:580`) treats an attach with a different lease as a new client and
displaces whoever currently holds the session, which is the winner.

The trigger is narrow but ordinary: a tab gets mounted around the moment of takeover (the user opens a tab, or the
3-second poll discovers one), and the winner's attach reaches the supervisor before this view's takeover notice arrives.
The result is the reverse of what was intended: the losing view's new tab now holds the session while its agent pane
still shows "Detached … take control", and the winner is evicted and latches in turn. Separately, the pending mounts
that `latchTakeover` does cancel are just unmounted with no banner, so those panes stay blank until something else
changes the set of terminals the view wants.

This is precisely the silent steal-back the latch exists to prevent, and it breaks the SPEC rule that a displaced client
keeps its snapshot and a take-control action instead of reattaching. The suggested fix: in `latchTakeover`, also unmount
mounted terminals whose attach has not yet been confirmed (not yet proved attached, or socket not yet open), and paint
the "Detached: <reason>" banner with the reclaim button on those and on the cancelled pending ones, the same way
`sync()` already does for tabs it discovers while latched.

Related: `new-tab-mount-displaces-owner-during-recovery.md` (from a second review) has the same root with a different
trigger: a view whose sockets were dead during the takeover never latches, and its reconciler mounts a newly appeared
tab on the displacing route after connectivity returns. The fix here (have the latch also unmount unconfirmed attaches)
does not cover that case, because no latch ever fires; the two are best fixed together.
