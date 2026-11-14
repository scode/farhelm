# Known Delete confirmation bypass is separate from guarded tab-close identity

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Reconciled the parent-supplied g166/c5: Delete can accept a held tab runtime's Gone state without
the adopted-process check used by explicit/automatic tab close. The original conditional orphan-host consequence is
preserved; it is a known interface issue, not a new discovery from this assigned deletion. Suggested bucket: other for
unconfirmed host cleanup; could-be-highest only if surviving work/credential harm is additionally established. No such
higher consequence is established here. Confidence: source-confirmed conditional dispatch/contract discrepancy; a
real-host producer is unverified after investigation, not runtime-observed. proposed_drop: no drop of g166/c5, c1 or c2
is proposed or accepted. This local reconciliation lead is closed to a candidate because the parent already owns the
known issue through g166/c5; its useful limiting evidence is retained below. possible_cover: exact supplied g166/c5
record. Source checks: session_hosts.rs:245–260 keeps nonplaceholder Gone runtimes and omits their records from
independent settlement; :510–521 pins the child only, which may already be reaped; :733–740 calls end_launch_host, and
:771–791 treats Ok as confirmed. host_launch.rs:694–696 returns Ok on shutdown Err(Gone). The distinct close wrapper at
host_tabs.rs:1052–1062 checks the adopted host process in that case. SPEC_impl.md:2458–2461 requires that 'a host not
confirmed ended, within its budget or at all, keeps the session and its records for a retry, so Delete never reports a
session gone while one of its hosts may still run.' Thus c2's source-refuted wrong-target tab

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g163/l1`.

### g163/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (candidate, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Reconciled the parent-supplied g166/c5: Delete can
accept a held tab runtime's Gone state without the adopted-process check used by explicit/automatic tab close. The
original conditional orphan-host consequence is preserved; it is a known interface issue, not a new discovery from this
assigned deletion. Suggested bucket: other for unconfirmed host cleanup; could-be-highest only if surviving
work/credential harm is additionally established. No such higher consequence is established here. Confidence:
source-confirmed conditional dispatch/contract discrepancy; a real-host producer is unverified after investigation, not
runtime-observed. proposed_drop: no drop of g166/c5, c1 or c2 is proposed or accepted. This local reconciliation lead is
closed to a candidate because the parent already owns the known issue through g166/c5; its useful limiting evidence is
retained below. possible_cover: exact supplied g166/c5 record.

Source checks: session_hosts.rs:245–260 keeps nonplaceholder Gone runtimes and omits their records from independent
settlement; :510–521 pins the child only, which may already be reaped; :733–740 calls end_launch_host, and :771–791
treats Ok as confirmed. host_launch.rs:694–696 returns Ok on shutdown Err(Gone). The distinct close wrapper at
host_tabs.rs:1052–1062 checks the adopted host process in that case. SPEC_impl.md:2458–2461 requires that 'a host not
confirmed ended, within its budget or at all, keeps the session and its records for a retry, so Delete never reports a
session gone while one of its hosts may still run.' Thus c2's source-refuted wrong-target tab-close routes do not prove
all-host Delete confirmation. No accepted legacy retirement clause waives this separate obligation.

Producer limitation investigated: actual runtime reconnect at hosts/terminal.rs:4128–4142 gets Gone through
HostConnection::connect_with. hosts/client.rs:639–646 always applies confirm_with_process; :1220–1239 converts Gone into
Unavailable if a matching live host process is found or the overall process search errors. Consequently merely unlinking
the socket and ending a connection does not establish Gone for an ordinary readable, correctly named live terminal host.
The retained close regression at host_terminals.rs:3688–3780 uses this test runner as a non-host-command-line stand-in;
it proves the synthetic state and close safeguard, not that ordinary-host premise. However hosts/process.rs:220–247
ignores unreadable process rows and missing argv, and procs.rs:348–356,1126–1134,1723 onward shows per-process read
failures/truncation return None. Therefore a real live host omitted by one such per-row/argv observation can still yield
Gone; a total walk failure/timeout cannot. The exact missing premise for a real orphan occurrence is a missing/unusable
socket and ended owning connection together with a live adopted host omitted from that process search (or no matching
command line), followed by Delete after its child ended. Source establishes the conditional branches but does not
establish that an installed host actually met the observation-failure premise. A retained failure trace or platform
process-read evidence covering a real host would settle occurrence; none was requested or collected. No readily
accessible source check supplied a guaranteed ordinary producer.

Full-range attribution: exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a already dispatches held tabs through
end_launch_host at session_hosts.rs:746–753 and accepts Gone there at host_launch.rs:684–686. The complete base-to-tip
relevant diff changes terminal wrappers/comments, not those bypass branches; no in-range introduction of this runtime
chain was identified. This is stronger than merely predating the assigned commit, but it does not excuse an in-range
newly false documentation claim; that attribution belongs to g166/parent. Prior wrong-target process-loss consequence
and exact source refutations in c2 remain intact. Received g097/l4's separate startup PID-to-owner misattribution and
g007/c4/g128/l1's later hidden-work consent routes are known interface issues and are not refuted by accurate pane/host
tags on the ordinary close path. No tests, reproductions or live-state actions.

Returned proposal (claims-103, event 12): **needs-parent**, suggested bucket **other**.

The conditional Delete bypass is source-supported and distinct from guarded tab-close identity. Its runtime branches
predate the entire range. The input attributes a potentially in-range false documentation promise to parent-owned
g166/c5, which is not supplied here; that record must carry the evidence rather than this consultation inventing its
attribution or dropping it.

Original stronger consequence: Successful Delete may forget a live host process. Possible subsequent security or
user-work harm was considered but not established; no reduction of such a proven consequence occurred.

Unresolved premise or limitation: A real live host must have a missing/unusable socket and ended owning connection, be
omitted from process search, have an ended child, and then reach Delete. Occurrence is unverified. In-range attribution
of the documentation defect must be checked in the parent-owned record.

Proposed basis: Preserve this as evidence for the supplied known g166/c5 concern. The runtime implementation is
pre-range, but that alone cannot dismiss an in-range newly false specification/comment claim.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g163 record](records/review-through-00de0b6c/groups/g163.json). Record ID: `l1`.
