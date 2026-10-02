# A disconnected read/unread update can commit without notifying other clients

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Closing the browser during a read/unread update can save the change without notifying other windows, leaving their dots
stale until another event or refresh.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F10 / COR-SEEN-NOTIFY`, reviewer `correctness_data_flow`, pass 3. Confidence: **definite**. Review disposition: **would
fix**. Queue priority at recording: **other**.

Anchor against the reviewed commit: `crates/farhelm-helm/src/sessions.rs:2955`. Recorded from the completed review
without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: The
accepted Replace work in `TRIAGE_OUTCOMES.md` asks its executor to inspect other session routes and fix the same small
ownership mechanism where applicable. That may cover this write-plus-notification path, but it does not name it. The
completed `seen-toggle-report-panics-after-unmount.md` decision concerns a UI signal panic, not loss of the fleet
notification.

Marking a session read or unread performs a database write, then separately advances the fleet revision that tells other
clients to refresh. Both steps belong to the HTTP handler, but the database work runs on a blocking worker and continues
once started even if the handler is cancelled. Closing or reloading the requesting browser can therefore allow the write
to commit while skipping the notification. Repeating the same update does not repair the missed event: the database
reports no change, so the endpoint deliberately leaves the revision alone.

Healthy clients rely on the revision feed rather than periodic list polling. On a quiet fleet, their read/unread dots
can remain stale until an unrelated change, explicit refresh, or reconnection. Keep the write and its notification
together in a helm-owned task using `run_owned`. A controlled cancellation test should let the database worker commit
after the HTTP waiter disappears and verify that the fleet revision still advances once.
