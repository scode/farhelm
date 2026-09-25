# DELETE has no liveness precondition

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Clicking Delete on a row that still shows exited or interrupted moments after the session was restarted (here, in
another window or by an agent) kills the running agent with no confirmation.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F2 / COR-DELETE-NO-LIVENESS-PRECONDITION`, tagged **definite**. Anchors and title: `crates/farhelm-ui/src/api.rs:2340`,
`crates/farhelm-ui/src/list/view.rs:1587`, `crates/farhelm-ui/src/list/view.rs:1605`,
`crates/farhelm-helm/src/sessions.rs:2723` — DELETE carries no liveness precondition, so a stale "ended" row can kill a
freshly restarted agent without confirmation

Whether a delete gets confirmed is decided only in the browser, from the status the row last rendered (see F1). The
request itself has no condition attached. `api::delete_session` sends a bare `DELETE /api/sessions/{id}`, the helm's
handler (`sessions.rs:2723`) forwards it to the supervisor as `DeleteSession { req_id, session_id }`, and that protocol
message has no field for a precondition. The supervisor deletes whatever is there, killing a live agent if there is one.

Restart already handles the same race differently. Its request carries `stop_if_running`, which is set to true only
after the user confirms, and the supervisor re-checks liveness itself before acting. Delete has neither safeguard.

A row can show Exited or Interrupted while the agent is in fact running again, for several reasons:

- Another client, or an agent, restarted the session. SPEC says "Agents may intentionally stop, rename, and restart
  sessions".
- This client's own header restart succeeded, but that path only refreshes the session view's detail read, not the
  sidebar listing (see F13).
- Just after a relaunch the helm keeps reporting the cached Interrupted status for a few seconds, until its next probe.
- Under a latched build mismatch, the feed and the fallback poll are both off, so the listing can stay stale
  indefinitely.

In any of these windows, clicking Delete on the stale row goes straight to `do_delete` and kills the running agent with
no prompt. The source delete at the end of Replace and Replace-with has the same gap.

This matters because Delete is the one irreversible, process-killing action, and its only safeguard depends on a
client-side snapshot that can be arbitrarily old. Nothing re-checks at the moment of the kill. The suggested fix: have
the unconfirmed path send an explicit precondition (for example `only_if_ended`), and have the supervisor refuse with a
distinguishable 409 when the agent or any tab is alive. The UI would then turn that 409 into the normal confirmation
prompt. The same should apply to Replace's source delete, and a header restart should also request a listing read.
