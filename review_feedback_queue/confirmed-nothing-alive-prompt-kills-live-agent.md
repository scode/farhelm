# A confirmation that promised nothing was alive can still kill a running agent

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

A confirmation can kill a running agent after telling the user nothing was running. A sidebar delete prompt left open
while the session changes under it (for example, the agent exits, so the prompt now just says "delete anyway") sends a
delete that kills whatever is running when the click lands, including an agent that another client, or an agent,
restarted in the meantime. The agent and its tabs are killed without the warning SPEC.md's Delete rule requires. A
narrower path of the same kind exists in the session header's Restart confirmation, described below.

## Details

Source: whole-codebase review, 2026-09-30, slice ui.

Rebase note: when this was rebased onto main at 1ec60cc, the Replace YOLO follow-up path this item originally described
was already covered by `header-replace-recomputes-alive.md` and `sidebar-replace-recomputes-alive.md` from a separate
review, so it was removed from here and its browser verification recipe merged into those items. The header Restart path
was narrowed by 4a683aa (restart asks only while the agent is working) and is restated below for the current code. The
remaining paths share the same capture-from-the-rendered-prompt fix with those two items.

Reviewer's confidence: confirmed (traced end to end in the UI; the supervisor's "flag unset deletes a live agent"
behavior is taken from the TRIAGE_OUTCOMES.md completion record for delete-lacks-liveness-precondition.md, not re-read).

Reviewer's bucket suggestion: highest.

Possible cover for triage to check: TRIAGE_OUTCOMES.md `## delete-lacks-liveness-precondition.md` (outcome `fix code`,
execution complete). Its decision scoped the precondition to "the UI's unconfirmed delete path and Replace's unconfirmed
source delete". The paths below go through a confirmation, so they are outside that decision's letter, but they are the
same race it was meant to close. Not the same as queue item `header-actions-skip-listing-read.md` (header
restart/replace skipping a listing read).

The rule the UI is meant to follow is written down in `crates/farhelm-ui/src/status.rs:369-380` (`shows_nothing_alive`):
"whenever a delete or Replace goes ahead on the strength of it (no prompt at all, or a prompt that warned of nothing
alive), the request carries `only_if_nothing_alive` so the supervisor refuses if the row was stale and something is
running after all." The session header's delete follows it (`crates/farhelm-ui/src/session_view.rs:1675-1684`,
`header_delete_warned_nothing_alive`, sent at 2060-2063; its comment names the exact race: "another client restarted the
session, say"). These paths do not:

1. Sidebar row delete confirmation. `on_delete` (`crates/farhelm-ui/src/list/view.rs:1808-1877`) opens
   `RowPhase::ConfirmingDelete` for a live, Unknown, or tab-holding row. The prompt's wording is recomputed on every
   render from the row's current status (`crates/farhelm-ui/src/list/row.rs:1850-1858`, `confirm_consequence`), and
   nothing closes the prompt when the status changes (the only `leave_phase` calls for this phase are confirm, cancel
   and the outside-click relay, `view.rs:1907`, `1917`, `2754`). So a prompt opened on a running agent reads "delete
   anyway:" (`status.rs:357`) once the listing shows the agent exited with no tabs, or "interrupted by a host reboot,
   which ended the agent; deleting discards the session:" (`status.rs:358-360`). `confirm_delete` (`view.rs:1897-1911`)
   then always calls `do_delete(id, false)`, i.e. no `only_if_nothing_alive`, so the supervisor deletes unconditionally.
   If the session was restarted (by another client, by an agent through fleet operations, or is running again after a
   relaunch the helm has not reported yet) between the listing that produced that wording and the click, the running
   agent and any tabs are killed.

2. The session header's Restart confirmation (narrowed at main 1ec60cc by 4a683aa). The prompt now opens only while the
   session reads working (`restart_needs_confirmation`, `crates/farhelm-ui/src/session_view.rs:113-115` at 1ec60cc), and
   the supervisor demands consent only for a working agent (`crates/farhelm-supervisor/src/service/core.rs:10131`);
   SPEC.md now accepts stopping an idle, waiting or unknown agent unasked. What remains: the confirm still sends
   `stop_if_running: true` (`session_view.rs:1971` at 1ec60cc) even after the prompt's consequence text has drifted to
   "nothing left to stop" (`status.rs` ~514-517). So a new run that reads working when the supervisor handles the
   request is stopped unconfirmed, where SPEC.md says such a run should be "refused rather than stopped unconfirmed, and
   the next attempt asks".

Consequence: a running agent (and its tabs, and whatever it was doing) is killed with no warning that anything was
alive, which is the exact outcome the precondition from delete-lacks-liveness-precondition.md exists to prevent.

How to verify: in a browser test, open a sidebar delete prompt on a running session, make the listing report it exited
with no tabs (the prompt now reads "delete anyway:"), restart it through the API without letting the listing refresh
reach the page, then click "confirm delete"; the DELETE URL carries no `only_if_nothing_alive`.

Fix sketch: capture `shows_nothing_alive` from the same render that drew the prompt (as the header's Delete does) and
pass it from `confirm_delete`. For the header Restart, capture whether the prompt the user answered offered to stop a
working agent, and send `stop_if_running` only when it did.
