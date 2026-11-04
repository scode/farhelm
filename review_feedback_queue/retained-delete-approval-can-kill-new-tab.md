# A retained Delete approval can end a newly opened tab

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A Delete approved when nothing was alive can later end a newly opened tab without asking about that work if its host
becomes unreadable between two status reads. The conditional approval and status sequence is established.

## Details

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; concurrent tab opening and status transition required

Source locations: crates/farhelm-supervisor/src/service/session_hosts.rs:639-649 at
ca9b0a19856dc136f63ad2d14f1076027edce1eb.

Preserve g094/f2 and independent a043/a072 provenance. The first guard read excludes the running tab; the later
tab-count read excludes the now-unavailable one. Held matching launch records are skipped, so record settlement does not
restore the warning. A destructive confirmation's scope bounds consent even when its text is old.

In 7987d070, core.rs:14306 calls hosts.untold(), which sees a running tab as readable (session_hosts.rs:580-600). It
then awaits tmux.pane_states_with_markers() at core.rs:14327-14331 and calls hosts.live_tabs() at 14339. If that tab
changes Running -> Unavailable meanwhile, live_tabs counts zero because it matches only Owned { dead:false } at
session_hosts.rs:632-642; Unavailable maps to Unattributed at hosts/terminal.rs:820-832. These are fresh independent
watch reads, not frozen state (terminal.rs:1993-2005). A reconnect failure publishes Unavailable without ending the
child (terminal.rs:3738-3742,3789-3803; tip 3871-3876,3922-3936). The lifecycle claim excludes another tab operation,
not the runtime task. With an ended agent and no other tabs, NothingAlive passes; teardown roots the still-live pinned
tab child at session_hosts.rs:503-514 and ends the tab at 726-733. The affected confirmation can be stale because
another client opened the tab, the ordinary race SPEC.md:1495-1510 explicitly guards; an agent/CLI guarded Delete has
the same defect.

An agent Delete approval saying it stops nothing can terminate work in a terminal tab opened while that approval waited.

Possible coverage and caveats: broad tab-output-hangup.md Delete audit remains ambiguous, not exact stated split-read
repair. TODO.md unavailable-until-restart entry is Near term, not Planned, and does not authorize a consent bypass.
SPEC.md:1495-1510 requires refusal for this precise expanded-liveness case; 1544-1556 crash/coincident-fault exception
does not cover a single tab-host unavailability plus normal request scheduling. FILTER.md exclusions for process loss
and unconfirmed destructive actions apply.

Additional reviewed locations: g094/f2:
7987d070e91684f4c52df7695136cb87e66bd94f:crates/farhelm-supervisor/src/service/core.rs:14306-14339.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g150/f1`, `g094/f2`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
7987d070e91684f4c52df7695136cb87e66bd94f; persists at pinned tip ca9b0a19856dc136f63ad2d14f1076027edce1eb
(session_hosts.rs:587-607,639-649; core.rs:14702-14735).. This was a source-only review; runtime occurrence and
frequency were not measured.
