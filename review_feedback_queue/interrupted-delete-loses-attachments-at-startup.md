# A session that survives an interrupted Delete loses its attachments at the next supervisor start

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

If the supervisor dies (crash, out-of-memory kill, host reboot, a kill during an upgrade) while a Delete is part-way
through, or if a failed Delete cannot put the session's attachments back, the session is still there afterwards. The
next time the supervisor starts, every file pasted or dropped into that session is silently deleted. The user sees the
session as still existing, with no sign its attachments were removed.

## Details

Found by the 2026-10-22 prioritized persistent review, group `068-uploads-2of2` (finding f1). The reviewer proposed a
drop; an independent reviewer disagreed and asked for it to be recorded.

Suggested bucket: highest (loss of user-owned files of a session that still exists), subject to the cover question
below.

Confidence: confirmed from code reading (no run). Recording rechecked the startup sweep and the module docs at the
reviewed commit, and that the current main's `reconcile_at_startup` still discards the whole quarantine.

Mechanism:

- Delete first detaches the session's attachments directory by renaming it into the reserved `attachments/.quarantine/`
  directory (`crates/farhelm-supervisor/src/service/teardown.rs:656-657`, `quarantine_session_dir` at
  `crates/farhelm-supervisor/src/attachments.rs:320`). Further steps run before the session row is removed: the per-row
  checkout archive decisions (teardown.rs:658-785) and the final transaction `delete_session_archiving_memberships`
  (teardown.rs:786).
- If the supervisor dies anywhere in that window, the row is never deleted, so after restart the session is reloaded as
  live. Startup reconciliation (`reconcile_at_startup`, attachments.rs:440) then calls `discard_quarantine_root` on the
  quarantine directory (attachments.rs:461-462), removing everything parked there, the live session's attachments
  included. It never checks whether a parked directory's session id prefix (`<session-id>-<uuid>`, attachments.rs:330)
  names a session in `known_sessions`.
- The same end state arises without a crash when a Delete fails after quarantine and the best-effort move back
  (`restore_quarantined`, attachments.rs:387, called at teardown.rs:823) also fails: an error is logged, the row stays,
  and the next startup discards the parked files.
- The module docs claim the crash window leaves "debris in a reserved directory that the startup reconciliation removes,
  not a user's file and not a surviving session missing its attachments" (attachments.rs:301-313). That is false for a
  crash before the row commit.

Trigger: a user-requested Delete of that session, interrupted by a supervisor crash or kill between the quarantine
rename and the row commit, or followed by a failed restore rename. The window spans checkout archive work and an SQLite
transaction, so it is not instantaneous.

How to verify: in a test, quarantine a session's attachments as teardown does, leave the row in place, run
`reconcile_at_startup` with that session id in `known_sessions`, and observe the parked directory removed while the
session row remains.

Fix sketch: at startup, for each parked `<session-id>-<uuid>` directory whose session id is in `known_sessions` and
whose session directory is absent, rename it back instead of discarding it; discard only parked directories whose
session no longer exists. Correct the attachments.rs:301-313 docs.

Possible cover, for triage to judge. These pull in opposite directions, and triage should settle which governs:

- SPEC.md "Attachments" (around SPEC.md:2007-2009): "An explicitly requested Delete may remove them before a later step
  fails and leaves the session row for retry; this partial deletion is acceptable, with a visible failure and no
  rollback guarantee." SPEC.md "Partial deletion" (around SPEC.md:3147-3152) says the same and adds that reviewers must
  not flag partial deletion alone as a defect. The trigger is always the user's own Delete of that session. Against the
  cover: in the crash case the failure is not necessarily visible (the client may only see a lost connection, and after
  restart the session looks intact), and the removal happens later, at startup, not before a later Delete step fails.
- TRIAGE_OUTCOMES.md "failed-delete-strands-attachments-in-quarantine.md" (outcome `fix code`, executed in PR #914)
  found that "a later startup quarantine sweep can then delete those attachments even though the session survives ...
  conflicts with the delete contract and the quarantine invariant that a retained session keeps its files", chose to
  preserve the row-and-files-together invariant, and set the completion criterion "verify that a retained session's
  attachments remain reachable and are not removed by startup reconciliation". The executed fix restores the directory
  on a returned error, which cannot reach the crash case or a failed restore.
- FILTER.md "Session status and history after a crash or a partly failed operation" does not match: its exclusions
  include loss of user data.
