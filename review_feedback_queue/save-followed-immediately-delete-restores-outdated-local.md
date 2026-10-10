# Save followed immediately by Delete restores an outdated local snapshot

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A queued Delete after Save could give Undo an outdated template.

## Details

F304 — **possible** — `crates/farhelm-ui/src/list/templates.rs:995` — Save followed immediately by Delete restores an
outdated local snapshot

Save clears busy and restarts the list while its resource still retains the old value. The rendered Delete button is
disabled during reload, but an already queued callback checks only busy and can capture that old entry for Undo before
deleting the saved version. Callback delivery was not reproduced. Add a handler-time pending guard or use the
acknowledged saved snapshot so Undo preserves the latest content.

## Evidence and triage context

- crates/farhelm-ui/src/list/templates.rs:965-979 clears busy, updates the editor baseline, and restarts the list.
- crates/farhelm-ui/src/list/templates.rs:995-1007 checks busy and then clones the retained list entry, without checking
  templates.pending().
- crates/farhelm-ui/src/list/templates.rs:1018-1025 deletes the stored template and retains that cloned entry for Undo.
- crates/farhelm-ui/src/list/templates.rs:1196 disables the rendered button during pending reads, but supplies no
  handler-time pending guard.
- Cargo.lock:1359-1362 pins dioxus-hooks 0.7.10. Its use_resource.rs:39-64 marks a restart pending without clearing the
  previous value; lines 191-195 invoke that callback.
- SPEC_impl.md:3917-3921 accepts cross-client reservation races, not this single-client outdated snapshot.
- review_feedback_queue/FILTER.md:130-135 excludes loss of user-owned work from the sub-second interaction filter.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The missing handler-time pending check is confirmed.
- Delivery of the stale callback after Save completes and before the disabled state prevents activation was not
  reproduced.
- Ordinary clicks after the disabled state is applied are protected.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_11_cor:p1:C3`.

- `ui_desktop_11_cor:p1:C3`: confidence as filed: possible; suggested bucket as filed: not separately tagged in
  candidate list.
