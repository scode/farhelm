# Removing stale-window kill tests does not introduce sibling-target cleanup

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Removing stale-window kill tests does not introduce sibling-target cleanup

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g166/c5`.

### g166/c5

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (candidate, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

## TLDR

Considered whether removing stale-pane kill and cross-session resize guards could make cleanup terminate another tab or
session. Terminal-host cleanup uses the selected host handle and the process pinned when that handle was adopted, rather
than resolving a vanished tmux pane to a current window.

## Details

Assigned patch lines 108-201 and 305-374 remove tmux target-specific tests. The replacement cleanup at
service/core/host_launch.rs:682-719 first requests shutdown through the passed HostTerminal. If that fails, it aborts
the runtime and uses host.adopted_process(), which is fixed for the handle's lifetime (hosts/terminal.rs:2453-2466); it
does not connect again to a replacement socket. end_adopted_process at host_launch.rs:727-744 uses terminate_verified.
hosts/process.rs:476-506 verifies identity and socket command-line ownership before SIGTERM and again before SIGKILL.
With no adopted peer, end_launch_host delegates to the session-tag-checked fallback described at host_launch.rs:665-674;
this candidate does not claim a full audit of that fallback or the process sweep. SPEC_impl.md, Terminal host,
Connecting and Ending a host the protocol cannot end, establishes these replacement identity contracts. A vanished tmux
target's empty-window fallback is no longer involved.

Suggested bucket: highest if cleanup targeted unrelated user processes. Could-be-highest: yes. Confidence: confirmed
source refutation of the removed tmux fallback mechanism; no execution. proposed_drop: source refutation, selected host
handle and pinned process replace tmux window resolution. possible_cover: none. Caveat: independent groups own host
process verification, fallback shutdown and cleanup sweeps; no claim that every ownership mechanism has been
exhaustively audited here.

## Discovery-turn split

The original suspected wrong-target termination of unrelated user processes and its original highest consequence,
wording and source-refuted proposed-drop basis are retained in this ID and its append-only history. No drop is accepted.
The later distinct Gone-runtime orphan-host cause previously appended under c5 is now tracked separately as linked f1.
Its production-trigger and whole-range attribution require investigation; the previous confirmed label does not
establish a shipped producer. c5 remains the original ownership candidate; f1 owns the distinct cleanup cause and
uncertainty.

Returned proposal (claims-105, event 14): **refuted**, suggested bucket **highest**.

Deleting the retired stale-pane/window tests does not introduce their old target-resolution mechanism into host cleanup.
The separately discovered Gone-runtime omission belongs to g166/f1 and is not a reason to erase this original
highest-consequence candidate.

Original stronger consequence: Cleanup could terminate another tab/session and lose unrelated user processes or work.
This potential-highest route remains preserved separately from f1's orphan-host concern.

Unresolved premise or limitation: No surviving stale tmux target-resolution caller was identified. Broader ownership
defects remain outside this narrow refutation.

Proposed basis: Source refutation limited to the deleted stale-pane/window fallback; host cleanup selects an adopted
handle/process and checks ownership.

Existing parent disposition (event 14): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g166 record](records/review-through-00de0b6c/groups/g166.json). Record ID: `c5`.
