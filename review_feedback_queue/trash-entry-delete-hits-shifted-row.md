# A one-click trash delete can hit a checkout that moved under the pointer

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

In the trash dialog, each archived checkout has a delete button that removes it permanently with one click and no
confirmation. The dialog re-reads and re-lays out its list while open, for example when another client or an approved
agent deletes a session and a new archive appears, or when a host disconnects or reconnects. If that happens just as you
click, the rows move under the pointer and the click lands on, and permanently deletes, a different archived checkout
than the one you aimed at, including any uncommitted or unpushed work in it.

## Details

Found by the 2026-10-22 prioritized persistent review, group `042-session-delete-3of3` (finding f1).

Suggested bucket: highest (permanent loss of user-owned work, applied to the wrong target), with the caveat that the
trigger is rare.

Confidence: likely. The mechanism is confirmed by code reading: row-bound click handlers, newest-first insertion,
buttons re-enabled in the same render that inserts the row, and host sections that appear and disappear. Recording
checked the handler and the re-read effect. Unverified premise: that the re-read reply on a fast host arrives within a
typical aim-to-click interval; nothing was rendered or timed.

Mechanism:

- The per-entry delete (`crates/farhelm-ui/src/list/trash.rs:727-740`, the `trash-entry-delete` button) captures its own
  row's `entry_id` at render and dispatches at once, with no confirmation. SPEC.md "Managed checkouts", the trash
  paragraph (around line 1111), deliberately accepts that for the clicked checkout.
- Rows are newest first per host (`crates/farhelm-supervisor/src/checkout_trash.rs:356-360`), inside host sections shown
  or hidden by whether the host has entries or is reachable.
- The effect at `trash.rs:372-419` re-reads on `revision` (bumped in `crates/farhelm-ui/src/list/view.rs` around 1580
  when a managed session disappears between complete fleet listings), on any host signature change (connect, disconnect,
  incarnation change), and on refresh.

Two layout shifts follow from that:

1. A new archive lands on host B (another client deletes B's last session using a managed checkout). B's reply inserts
   the new row at the top of B's section and, in the same render, clears `loading`, which re-enables B's delete buttons
   (`target()` requires `!loading`, `trash.rs:182-184`). Every B row moves down one row at the moment the buttons become
   clickable again, so a click aimed at row Y lands on the newer row above it.
2. Host A, rendered above B, disconnects (`state.remove`, `trash.rs:391`) or its section appears or disappears, shifting
   all of B's rows by a whole section. B is re-read too, and on a fast host (the helm machine's own supervisor) its
   reply re-enables B's buttons within milliseconds.

The browser resolves the click from the element under the pointer, and the keyed row's closure carries that row's ID, so
the shifted-in checkout is deleted. Nothing downstream catches it: the helm only checks that the host connection is the
same one (`crates/farhelm-helm/src/sessions.rs:525-540`, `crates/farhelm-ui/src/api.rs:3602-3617`), and the supervisor
only rechecks that the ID is still a deletable archive (`crates/farhelm-supervisor/src/service/core.rs:5078-5096`,
`checkout_trash.rs:377-473`), which the wrong row is. There is no restore.

A further trigger, in the user's own hands and not verified: an open in-place confirmation band (`ConfirmBand`, rendered
above a host's rows at `trash.rs:712`, or first in the scroll region at 666 for the global action) does not disable the
per-entry delete buttons. The band's actions and the entry delete buttons are both right-aligned
(`crates/farhelm-ui/assets/app.css:8203-8206`, `8083-8088`, `8170`). Cancel removes the band and pulls the rows up, so a
double-click on Cancel could land the second click on an entry's delete button. Whether the x-ranges overlap depends on
button widths and was not rendered.

How to verify: open the trash with two or more archives on a host, hover a lower row's delete button, and from another
client delete a session whose managed checkout is the last reference on that host. After the re-read, the button under
the pointer belongs to a different checkout. A browser test can assert that a click dispatched at row Y's pre-refresh
coordinates deletes row X.

Fix sketch: do not let a listing change re-target a pending click. Ignore entry-delete clicks for a short interval
(around 500 ms) after the host's rendered listing or the dialog layout changes; or bind the click to the entry seen at
pointerdown or pointerenter and refuse when the listing generation changed since; or keep rows from being inserted or
removed above existing ones while the dialog is open. Disabling entry deletes while a confirmation band is open covers
the double-click case. SPEC.md "Session list" already holds back session-list reordering while the pointer is over the
list "so a row does not move out from under the pointer on its way to a click" (around line 1492), which is a precedent.

Possible cover, for triage to judge: the acceptance that the per-entry delete has no confirmation (SPEC.md "Managed
checkouts", trash paragraph; SPEC_impl.md archived-checkout trash) concerns the clicked checkout, not one that moved
under the pointer. SPEC.md's "Confirmed 2026-10-10" paragraph (around line 3327) accepts that "an action a user takes
inside a sub-second window before the screen updates ... may be carried out as delivered"; here the screen has updated
and the click acts on the new layout, so whether it applies is ambiguous. `FILTER.md`, "Races a person would have to win
inside a sub-second window", resembles the trigger but excludes consequences that lose user-owned work or act on the
wrong target. TODO.md's Near term item "Bind GUI answers to what the user saw" is related but not in the Planned bucket,
and this handler already binds to its own row.
