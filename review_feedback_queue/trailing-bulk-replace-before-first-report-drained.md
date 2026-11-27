# Bulk Restart/Replace right after an agent exits can start over a session that could resume

Reviewed commit: 98b1ba580ae85e21b2eac1a0e18fe4312f6c3a91

## TLDR

If an agent saves its first conversation record and exits almost at once, there is a moment (usually well under a
second, up to about two seconds when the report watcher is off) when Farhelm lists the session as ended with nothing to
resume. If the marked bar's Restart/Replace is pressed in that moment, the session is started over with no question
instead of resumed: the source session and its terminal history are deleted, and Farhelm loses its Resume link to the
conversation. The agent's own conversation files are not touched, so the conversation can still be resumed by naming it.
A queued row that drops out of the list view (a filter, the listing cap, a failed listing read) while it waits is also
sent unchecked, so it can be started over the same way.

## Details

Provenance: trailing review through `dbb153ec`. The independent drop check (consultation on group g09's set-aside
candidate c3) disagreed with "no shipped route" and named the undrained-report window; the owning reviewers were then
continued once each. Group g09 (bulk spec, marking, progress) turned c3 into this finding (route 1); group g08 (bulk
core) revised its finding f1 (route 2, the hidden-row check skip). Both reviewers suggested `other`; this item records
`highest` because the bucket is uncertain (see "Bucket" below). Confidence: likely; every step is from source, and the
unverified premise is timing: that the GUI presents a frame with the stale offer and a press lands before the drained
offer arrives.

Supervisor side (both routes):

- `reap_pass` (`crates/farhelm-supervisor/src/service/ticker.rs` ~1001-1037 at the tip) runs when a terminal's program
  exits, hints a session change at once and observes the exit, but runs no report drain. Reports are applied only by
  `capture_pass` (`service/capture.rs` ~112), whose callers are the ticker's `tick` (every 2 s), startup and reload, the
  report watcher (`report_watch.rs`, 100 ms floor, disabled for good if its backend fails), and Restart, which waits for
  a drain (`core.rs:12426-12436`: "a report waiting on disk may name the conversation this restart must resume"). So an
  exit can be listed with `restart_offer` still NotCaptured while the session's first report waits on disk.
- Startup and reload drain before serving (`core.rs:6366-6368`), so a host reboot does not leave such rows.

Route 1, the first row on each host (g09, c3):

- A bar drawn from that listing sees an ended, tab-less row with a non-Resume offer, so `How::of`
  (`crates/gui/farhelm-gui/src/session_list/bulk.rs` ~441-451) gives `Replace(NothingAlive)`. `bulk_start` asks nothing,
  and `run_batch` sends each host's first row at the press with no re-check.
- Neither the helm nor the supervisor re-checks the offer before deleting: the helm's replace creates and then deletes
  with no liveness or offer check (`crates/farhelm-helm/src/sessions.rs` `ReplaceReq` docs ~4610), and the supervisor's
  delete does not call `capture_pass`.

Route 2, a queued row hidden at its turn (g08, f1):

- `Queued::changed_since_press` (`bulk.rs:671-685`) re-runs `How::of` on the current row, so a listed row whose offer
  turned Resume while it waited is caught and gets the "changed while it waited" line. It returns "send" when the store
  or the row is absent, so a hidden row is sent with the decision frozen at the press. Its docstring ("a row the store
  no longer holds is sent all the same, for the helm to answer") is contradicted by the helm's own docs above: the helm
  does not re-check.
- Route 2 still needs the press inside the same drain window (the row must be listed NotCaptured at the press), plus the
  row hidden when the drain lands and at its turn. The drop check found no other shipped way for a hidden ended row to
  change state (status is recomputed from the terminal and the recorded outcome on every reply, `core.rs:7295-7306`).

SPEC.md, "Session list" (bulk Restart/Replace paragraph): "each session whose conversation can be resumed gets its
ordinary Restart, and every other one its ordinary Replace, so a session that can resume is never started over", and a
queued row "whose state changed while it waited so that it would now get something else" is not sent and says so.

Fix sketch: have the helm or the supervisor drain reports (`capture_pass(true)`) and refuse a Replace whose source now
offers Resume when it came from the bulk path, for example with an "only if not resumable" precondition checked before
the create. For route 2, also treat an absent row as unknown at its turn (leave a "not started" line) or compare against
the row as last seen, and fix the docstring.

Bucket: recorded as `highest` because losing a valid Resume offer counts as losing user-owned work
(`review_feedback_queue/FILTER.md` treats "losing or replacing a valid Resume offer" that way), and the route is in
shipped code. What would move it down:

- The window exists only before a session's first report is applied, so only a conversation that just started, or that
  just gave its first reply (Pi and OMP report after the first reply), is at stake. The vendor's conversation record
  survives and can be resumed by naming it (Restart with or Replace with, user-named conversation).
- With the report watcher working, the window is usually sub-second. SPEC.md, input timing ("Confirmed 2026-10-10: an
  action a user takes inside a sub-second window before the screen updates ... may be carried out as delivered ...
  Guards this spec names explicitly ... still hold"), may cover that part; whether "a session that can resume is never
  started over" is such a named guard is for triage. The up-to-two-second case needs the watcher to have failed.
- g08 found no shipped harness that reports its conversation only at exit, and no drain delay long enough for a human
  reaction in normal operation.

Possible cover: the SPEC.md input-timing rule quoted above, for the sub-second part only. Nothing covers the window when
the watcher is off, or route 2's skipped turn-time check.
