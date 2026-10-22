# An uninstall retry without a connection ends the host's terminal server unchecked

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

After a failed uninstall, retry without a connection can stop live sessions without checking them. One conditional
source route uses a custom Farhelm command that becomes unavailable while the supervisor stays active. The original
route, where the supervisor exits but its private terminal server survives, retains an unverified survival premise.

## Details

Found by the 2026-10-22 prioritized persistent review, group `023-provisioning-verify-4of4` (finding f1). This file
holds only the part of that finding the independent drop check did not accept as covered.

Covered, and not part of this item: a session or tab started on the host while a confirmed removal runs may be ended
with it. SPEC.md "Topology" (SPEC.md:156-159) accepts that: "A session or tab started on the host while removal runs may
be ended with it: the confirmation-time check is sufficient, though an implementation may check more strictly." The
original finding's main window (the confirmation re-plan at `crates/farhelm-helm/src/provisioning/service.rs:1847-1866`
followed by disable, remove-unit and reload before the stop in `execute`, `service.rs:1987-2005`) falls under that.

Uncovered remainder, the retry without a connection:

- `uninstall_plan` (`service.rs:1245-1262`) runs `require_no_running_sessions` only when the host has a connected
  client. That check reads a fresh session list and counts a session of unknown status as running
  (`service.rs:2386-2438`).
- With no connected client (`service.rs:1302-1361`), the plan proceeds once the probe reports `Absent`
  (`service.rs:1346-1349`) and the unit file is gone (`service.rs:1424-1426`). It does no session check of any kind, and
  it does not consult the helm's last-known sessions for that host, which SPEC.md:165-166 shows the helm keeps ("cached
  sessions included").
- The plan still includes `StopSupervisor` with the host's private tmux socket
  (`crates/farhelm-helm/src/provisioning/plan.rs:834-841`). Per its documentation (`plan.rs:757-767`) the stop ends the
  unit's control group and then ends any tmux server still reachable at that socket, panes included.

Trigger: a run fails after the unit file is removed but before the stop completes, so the supervisor keeps running and
the host stays connected (`plan.rs:757-760` describes exactly that ordering). Sessions are started on it during that
time, which is after the failed run ended, so they were never checked and were not "started while removal runs". The
supervisor then exits without the tmux server going with it, and the user chooses uninstall again. Consequence: the
retry ends that tmux server and the sessions' processes in it.

SPEC.md:169-171 allows this retry only on the grounds that the supervisor cannot start again: "Such a retry may go ahead
without a connection only once the unit file is gone, since nothing can start the supervisor again after that." It does
not waive the rule at SPEC.md:155-156 that uninstall refuses "while the host has any session that has not ended (an
unknown status counts) or any open terminal tab". With no connection, every last-known session on that host has unknown
status.

Suggested bucket: highest (ends running user processes without the required check). Confidence, as the reviewer reported
it: confirmed from code reading that the window exists; not exercised. The independent check confirmed the retry path's
missing session check and the stop's reach from code. Unverified premise: how a supervisor whose unit file is gone comes
to exit while its tmux server, with live panes, survives. A crash is the obvious route, and `plan.rs:765-767` notes that
an inactive unit can leave tmux behind. Nobody has reproduced it.

How to verify: in a test harness, fail an uninstall after `RemoveUnit` (for example at `DaemonReload`), create a session
on the still-connected host, kill the supervisor process but leave its tmux server running, then retry uninstall. The
retry should refuse; at the reviewed commit it proceeds and the stop ends the session's tmux server.

Fix sketch: on the no-connection retry, refuse while the helm's last-known session list for the host holds any session
that has not ended or has tabs (treating unknown as running), and name them as the connected check does. Or, before
`StopSupervisor`, list sessions directly through the private tmux socket and refuse if any remain. The plan's
documentation and SPEC.md:169-171 should then state which of these the retry relies on.

Reviewer's original `proposed_drop` (rejected for this remainder by the independent check): SPEC.md:157-159 and 169-171
cover both the confirmation window and the no-connection retry. The independent reviewer agreed for the first and
disagreed for the second, as above.

### Still-active supervisor through an unavailable custom command

Additional reviewed commit: `0f6ad6e0d680a518b46bf43b50ff704877931216`. A normal host registration can retain a bare
custom executable such as `fh` (`crates/farhelm-helm/src/hosts.rs:753–787`; `store.rs:1725–1741,1779–1793,5757–5807`).
Successful connection records identity without replacing that name (`store.rs:6628–6679`; `transport.rs:134–163`).
Initially let `fh` resolve to a symlink of the ordinary panel-installed binary so connected uninstall's binary check
passes.

Removal deletes the unit file, then fails before stopping the supervisor (`provisioning/service.rs:1984–2005`). Sessions
are started after that failed run ends. If `fh` subsequently becomes unavailable and the helm loses its connection, the
supervisor can remain active while reconnect attempts fail. Probe fallback to the private binary applies only to literal
`farhelm`, not `fh` (`provisioning/backend.rs:1252–1258,2100–2104`). Offline planning accepts absence, omits the
nonabsolute binary check and requires the unit file gone, without rejecting this active unit
(`service.rs:1347–1360,1422–1429`). Confirmation repeats those facts (`1847–1889`); execution stops the active service
and its private tmux (`backend.rs:2625–2642`; `plan.rs:841–848`).

Confidence for this extension: **confirmed conditional source mechanism**, without the original supervisor-exit premise.
The custom-command disappearance and failed-uninstall history are conditions, not observed occurrence. It establishes
unchecked process termination, not deletion of unrelated filesystem work; that separate allegation remains
source-incomplete. Suggested bucket: highest. Possible cover remains ambiguous under deliberate-configuration and paired
fault allowances (`SPEC.md:3181–3194,1336–1348`). The accepted window for sessions started while removal runs does not
exactly match sessions started after its failed run ended. Fresh independent review agrees only that literal `farhelm`
with an answering standard fallback is source-refuted; that does not exclude custom names. Proposed drop: none.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, run
`4fae501c-3686-4822-aa6c-037266637632`, `a001/f7`, scoped fallback audit `a005/x11`. No runtime reproduction.
