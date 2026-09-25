# A deleted row can reappear from an in-flight read

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A session you just deleted can briefly reappear in the sidebar (and even reopen in the right pane), and deleting it
again shows a confusing "not found" error.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F17 / COR-DELETE-NO-READ-FENCE`, tagged **definite**. Anchors and title: `crates/farhelm-ui/src/list/view.rs:1536`,
`crates/farhelm-ui/src/list/view.rs:1537`, `crates/farhelm-ui/src/list/view.rs:1555`, `crates/farhelm-ui/src/ops.rs:248`
— A listing read in flight when a delete succeeds puts the deleted row back

Listing reads are ordered by a generation counter (`ReadGate`, in `ops.rs`). Every read takes a generation when it
starts, and a reply is committed only if it is newer than the last one applied. `ReadGate::fence()` exists for exactly
one situation: its doc says it "supersede[s] every read that started before a locally confirmed mutation", so that an
older reply cannot overwrite what this client just did. The profiles editor and the provisioning surfaces call it.
`do_delete` does not.

On a successful delete, `do_delete` removes the row locally (line 1536) and then asks for a refresh (line 1555). The
reader runs one read at a time (`reader::request_read`): if a read is already in flight, the refresh is only queued
behind it. That in-flight read started before the helm finished the delete, and it is still the newest generation
issued, so `accepts_listing` commits it. The deleted row reappears with its Delete button enabled until the queued
refresh lands. Clicking Delete again in that window gets a confusing "not found" error. The early removal's own comment
says its purpose is precisely that a deleted row never comes back and that a second click never 404s.

The suggested fix is to call `listing_reads.write().fence()` next to the local `retain`, before the refresh.

Restater note: the second half of the claim, that auto-select can reopen the deleted session in the right pane, is
narrower than stated. When the deleted session was selected, `on_removed` clears the selection, and the auto-select
effect normally runs against the already-pruned listing and picks a fallback before the stale reply arrives. The reopen
only happens if auto-select is held off (for example, another row operation is still pending or the page lock is held)
until the stale listing commits. The reappearing row, and the 404 on a second click, are unaffected by this caveat.
