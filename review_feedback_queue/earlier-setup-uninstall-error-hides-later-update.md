# An earlier setup/uninstall error hides a later update failure

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An earlier planning error hides the reason a later Update failed.

## Details

F217 — **definite** — `crates/farhelm-ui/src/provisioning.rs:2615` — An earlier setup/uninstall error hides a later
update failure

A setup or uninstall planning refusal leaves an error field populated while a subsequent Update is admitted. Update does
not clear it, and rendering gives it priority over the current update error. The user sees stale advice instead of the
latest failure explanation. Associate diagnostics with their operation and attempt, retiring superseded nonsticky errors
or showing current and unresolved prior outcomes separately.

## Evidence and triage context

- crates/farhelm-ui/src/provisioning.rs:1987–2010 stores setup/uninstall planning failures in action_error and then ends
  planning.
- crates/farhelm-ui/src/provisioning.rs:239–241 admits Update once no update owns the row and planning has ended; lines
  2139–2145 dispatch begin_update.
- crates/farhelm-ui/src/provisioning.rs:2023–2045 clears pending plans and nonsticky update diagnostics, but does not
  clear action_error or action_warning.
- crates/farhelm-ui/src/provisioning.rs:2099–2114 records new update planning failures in update_error. Lines 1135–1159
  likewise record update submission refusals or ambiguous outcomes there.
- crates/farhelm-ui/src/provisioning.rs:2615–2622 selects action_error before update_error and action_warning before
  update_warning. Lines 2855–2859 render the selected error.
- crates/farhelm-ui/src/provisioning.rs:1674–1695 clears tracked state and qualifying update diagnostics on success, but
  not action_error.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"basis": "review_feedback_queue/FILTER.md:24–34", "comparison": "Although the consequence is diagnostic, a normal
  planning refusal followed by Update does not satisfy the required rare trigger."}
- {"basis": "crates/farhelm-ui/src/provisioning.rs:1699–1719", "comparison": "The intentional priority for unresolved
  update submission uncertainty is a different mechanism. This finding concerns an earlier setup/uninstall planning
  refusal suppressing a later update's diagnostics."}

Caveats:

- No incorrect server mutation is established.
- No runtime reproduction was performed.
- A failed execution's retained step messages may remain separately visible. The definite hidden case includes update
  planning and submission errors selected through update_error.
- Starting another confirmed-plan flow or cancelling its confirmation can clear action_error; merely starting Update
  does not.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_13_cor:p1:F3`.

- `ui_desktop_13_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
