# Arrow navigation retains an obsolete position after an item disappears

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An outdated menu position could move keyboard focus onto Delete.

## Details

F88 — **possible** — `crates/farhelm-ui/src/list/row.rs:1301` — Arrow navigation retains an obsolete position after an
item disappears

When the agent ends, MarkSeen disappears and later menu actions shift position. The retained navigation index does not
follow the action identity: ArrowDown from the stale Replace position can select Delete rather than Stop. Subsequent
Enter could delete an ended session, including uploaded files, without confirmation. Browser/user activation was not
reproduced. Reconcile the requested navigation target by action identity whenever the item set changes, together with
actual focus.

## Evidence and triage context

- crates/farhelm-ui/src/list/row.rs:48–53 keeps Stop and Delete offered for ended sessions; 114–121 establishes Rename,
  MarkSeen, Clone, ReplaceWith, Replace, Stop, Delete order; 991 removes MarkSeen when the session is no longer live.
- crates/farhelm-ui/src/list/row.rs:1111–1125 retains requested separately; 1293–1315 reconciles menu_focus but not
  menu_requested after the item-set change.
- crates/farhelm-ui/src/menu_panel.rs:548–553 prefers requested; 604–613 computes and applies the next position;
  1155–1160 makes Next from stale index 4 select index 5 in the six-item menu.
- crates/farhelm-ui/src/menu_panel.rs:1079–1087 leaves Enter to native activation.
  crates/farhelm-ui/src/list/row.rs:2373–2377 dispatches the focused Delete action.
- crates/farhelm-ui/src/list/view.rs:2011–2042 deletes an ended session without tabs immediately with NothingAlive.
- crates/farhelm-supervisor/src/service/teardown.rs:877–878 calls discard_quarantined;
  crates/farhelm-supervisor/src/attachments.rs:365–366 removes uploaded files recursively. SPEC.md:1297–1301 permits
  that removal for Delete, but explicitly not for Stop.
- review_feedback_queue/FILTER.md:38–42 excludes user-data loss from the proposed rare-glitch filter. SPEC.md:821–842
  permits deliberately requested deletion of ended sessions; it does not accept navigation unexpectedly substituting
  Delete for Stop.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No browser reproduction was run.
- A user who notices the changed focus can avoid activation; that does not establish that the whole consequence is
  harmless.
- Retain the possible destructive consequence for triage despite the source report's highest_possible=no label.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_12_cor:p1:C1`.

- `ui_desktop_12_cor:p1:C1`: confidence as filed: possible; suggested bucket as filed: highest.
