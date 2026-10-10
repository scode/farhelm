# Cancelled uninstall planning leaves SSH helpers

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Cancelled uninstall planning leaves SSH helpers.

## Details

`F12 / COR-UNINSTALL-CANCEL` — **definite** — `crates/farhelm-helm/src/provisioning/service.rs:1162` — Cancelled
uninstall planning leaves SSH helpers

Preparing an uninstall confirmation runs directly on the HTTP request’s task. That preparation calls the host-probe
backend, which starts an isolated SSH process group and a separate task to drain stderr. Explicit group cleanup and
waiting for the stderr reader occur when the probe completes; dropping the probe future runs only the direct child’s
`kill_on_drop` behavior.

Closing or reloading the page during planning can therefore kill SSH while leaving a configured helper, such as a
`ProxyCommand`, alive. If that helper keeps the inherited stderr pipe open, the detached reader can remain alive too.
Repeating this can accumulate helper processes and readers on the helm’s machine.

Run the planner on a helm-owned task so that its bounded probe reaches normal cleanup, or give the backend
cancellation-safe ownership of group termination, child reaping, and reader cleanup.

Suggested bucket: **other**. No possible cover was identified. The consequence requires a concrete helper that survives
SSH and holds inherited stderr; plain SSH and a local probe do not establish it. This concerns helm-side helpers, not
loss of remote user sessions. The discovery-probe fix at `TRIAGE_OUTCOMES.md:5910–5927` addressed another caller; the
analogous update-planning path already existed.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **other**.

Originating reviewers and passes: `helm_systems p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Concrete helper that holds inherited stderr required. Prior discovery ledger5910–5927 different
caller. UPDATE analogous preexists.

## Filed reviewer metadata

- `helm_systems p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable

## Additional finding from whole-repository collection

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

Cancelling update planning could leave local SSH helpers behind.

F126 — **possible** — `crates/farhelm-helm/src/provisioning/service.rs:992`;
`crates/farhelm-helm/src/provisioning/service.rs:994` — Cancelling Update planning can leave SSH helpers running

Update planning can be cancelled before its process-group cleanup runs, detaching the stderr reader as well. A
configured helper that survives SSH and retains stderr could then remain across repeated cancellations, accumulating
processes and tasks. That surviving-helper premise was not reproduced; plain SSH alone does not establish accumulation.
Give planning a helm-owned task or cancellation-safe probe ownership, and explicitly include this caller if extending
the related uninstall item.

## Evidence and triage context

- crates/farhelm-helm/src/provisioning/http.rs:201 directly awaits plan_update.
- crates/farhelm-helm/src/provisioning/service.rs:950–957 directly enters the planner, which awaits backend.probe at
  :992–996.
- crates/farhelm-helm/src/provisioning/backend.rs:1285–1290 enables direct-child kill_on_drop; :1873 spawns a separately
  owned stderr reader.
- crates/farhelm-helm/src/provisioning/backend.rs:1846–1851 performs group termination and reader joining only when
  explicit cleanup runs.
- crates/farhelm-helm/src/provisioning/service.rs:470–475 protects discovery with run_owned, unlike Update planning.
- crates/farhelm-helm/src/provisioning/http.rs:201 awaits the planner on the request task.
- crates/farhelm-helm/src/provisioning/service.rs:957 and :994 contain no owned-task boundary.
- crates/farhelm-helm/src/provisioning/backend.rs:1289 kills only the direct child on drop; :1873 spawns the reader
  independently.
- crates/farhelm-helm/src/provisioning/backend.rs:1831–1851 implements explicit group termination and reader joining
  that dropped futures do not reach.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/uninstall-cancel.md:14–29 shares the mechanism but records uninstall planning; its mention of
  pre-existing Update does not unambiguously cover this additional caller. TRIAGE_OUTCOMES.md:5910–5927 covers
  discovery. BUGS.md:80–101 concerns post-exit output draining, not cancellation.
- review_feedback_queue/uninstall-cancel.md:14–29 describes uninstall and notes analogous Update; it is possible partial
  coverage, not a sufficiently explicit duplicate basis. TRIAGE_OUTCOMES.md:5910–5927 fixed discovery only.

Caveats:

- Requires a configured helper that survives SSH; retaining stderr also keeps the reader alive. No runtime reproduction
  or remote session loss was established.
- Confidence is reduced to possible for the reported leak because a helper surviving SSH is required and was not
  reproduced. Plain SSH does not prove accumulation.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_08_cor:p1:F1`,
`helm_state_provisioning_08_sec:p1:F1`.

- `helm_state_provisioning_08_cor:p1:F1`: confidence as filed: possible, stated in the finding heading; suggested bucket
  as filed: other.
- `helm_state_provisioning_08_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
