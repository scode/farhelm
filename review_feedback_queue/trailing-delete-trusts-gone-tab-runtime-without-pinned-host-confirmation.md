# Gone-runtime Delete bypass: unverified after investigation

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Gone-runtime Delete bypass: unverified after investigation

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g166/f1`.

### g166/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 22 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Whole-range comparison: exclusive base
6e423e30068b62c6cfa276c6d720ee35bb70387a through inclusive 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. Linked original ID:
c5, which retains the original highest wrong-target termination concern and all history; this ID tracks only the
distinct orphan-host cause.

## TLDR

Original distinct consequence preserved: 'If a tab host loses its socket while its supervisor connection ends, Farhelm
can report the session deleted while the terminal-host process still runs, with no session entry available for retry.'
The earlier confirmed production label was too strong. The inspected Delete bypass exists, but no supported production
route to Gone while an adopted real host stays alive was established.

## Details

Conditional bypass remains as recorded in prior events: session_hosts.rs:245-260 retains non-placeholder tab runtimes
and skips settling their records; :733-740 calls end_launch_host directly; host_launch.rs:694-696 treats shutdown
Err(Gone) as success, unlike tab Close's end_tab_host process confirmation at host_tabs.rs:1052-1062. The fake close
fixture at core/tests/host_terminals.rs:3688-3780 deliberately removes a private socket and uses the test runner as host
peer. It is not a shipped producer of the required real-host state.

Actual production checks: hosts/client.rs:639-646 routes every failed connect through confirm_with_process. At
:1220-1238 a named live host turns Gone into Unavailable; failed/timed-out process walks do too.
hosts/process.rs:190-247 identifies a real host from this user's absolute --socket argument and the random file name
independent of directory spelling. A stuck/full-backlog host and a host whose socket vanished are therefore normally
Unavailable, not Gone. Runtime reconnect at hosts/terminal.rs:4128-4142 uses that guarded client outcome. Runtime
shutdown at :4280-4306 sets Gone after wait_for_end; its fresh-connect path at :4319-4332 uses the same guard.
registry.rs:336-366 rechecks a live process and refuses socket deletion, under the no-concurrent-spawn precondition.
Normal stop's remove_left_socket at hosts/stop.rs:606-628 follows confirmed stop. Terminal host owns its listener from
listen (host.rs:1323-1383) through Host::run; run removes the socket only after shutdown/SIGTERM finishes serving
(:247-265), then spawn.rs:90-108 exits immediately. Failed start cleanup at spawn.rs:125-182 occurs before successful
adoption. No normal listener-close/rebind or directory cleanup producer leaving the same serving real host alive was
found. Process-walk unreadable rows/argv are skipped (process.rs:215-232), so the guard is not mathematical proof of
absence; pairing that with a missing/unreachable private socket still needs an actual supported producer and cannot be
assumed from the stand-in test.

Whole-range provenance: the exclusive base already contains the same Err(Liveness::Gone) success branch at
host_launch.rs:684 and the direct self.tabs end_launch_host call at session_hosts.rs:746-751, with the same placeholder
partition at :248. Bounded git log over these paths lists seven in-range commits. A second bounded log -G for
Err(Liveness::Gone), for tab in self.tabs, end_tab_host and confirm_with_process over session_hosts.rs, host_launch.rs,
host_tabs.rs, hosts/client.rs and hosts/terminal.rs returns no matching change in the entire range. This establishes no
in-range change to the relevant conditional bypass/guard structure, rather than merely immediate-parent preexistence. No
introduction of a supported live-host producer was found in the inspected listener and socket lifecycle.

Contract: SPEC_impl.md, Terminal host, Delete ends every terminal host of the session, requires retaining
session/records on unconfirmed host end. The bypass would conflict if the missing real-host producer were established.
Correct production paths do not refute a test-oracle defect, but this record alleges a runtime route, not a specific
weak surviving assertion.

Suggested bucket: other for conditional orphan-host cleanup/resource leak only. Could-be-highest: no established
distinct security or user-work-loss route; original highest wrong-target concern remains separately preserved under c5
and has no accepted lowering/drop. Confidence: unverified after investigation for a supported production defect;
confirmed only as conditional control flow. Exact missing premise: a shipped, supported event that leaves an adopted
real host process alive while its runtime becomes Gone despite the live-process connection guard, without deliberate
private-path manipulation or substituting a non-host stand-in. Available source checks found guarding behavior rather
than that event; source cannot establish an unspecified external or unreadable-process event. Evidence that would settle
it: a concrete supported caller/source sequence defeating that guard, or an existing retained real-host trace
identifying both the Gone transition and the still-live pinned peer. No reproduction was requested or performed.

proposed_drop: attribution and established-route limitation only, proposed for caller judgment; the conditional
consequence and prior stronger wording are preserved, not silently erased. Do not retain the earlier
confirmed-production claim on the fake fixture alone. possible_cover: none searched. No prohibited coverage sources,
runtime tests, builds, reproductions or live-install actions. No open uninvestigated source lead remains; the remaining
unsupported producer premise is explicit.

Returned proposal (claims-105, event 22): **retain-unverified**, suggested bucket **other**.

The caller distinction is established conditional control flow. The live real-host producer remains unestablished after
investigation, and the relevant bypass already exists at the exclusive base. Retain the uncertainty and
range-attribution caveat rather than treating the stand-in fixture as a shipped reproduction or declaring unspecified
external faults impossible.

Original stronger consequence: Earlier c5/f1 history called this a confirmed production route: Delete could report a
session removed while its terminal host remains alive and no session entry permits retry. That label is withdrawn to
conditional source behavior. The earlier unrelated highest wrong-target/process-loss consequence remains under c5; no
distinct user-process/data-loss route is established for f1.

Unresolved premise or limitation: A supported shipped event leaving the adopted real host alive while its runtime
becomes Gone despite the live-process connection guard; also an in-range introduction or producer change. The stand-in
test and unreadable-process possibility do not supply that sequence.

Proposed basis: Retain as unverified after investigation, bucket other for the isolated orphan-host/resource
consequence. Preserve the earlier confirmed label's correction, original wording, split provenance and preexisting-range
evidence for the parent's independent decision.

Existing parent disposition (event 22): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g166 record](records/review-through-00de0b6c/groups/g166.json). Record ID: `f1`.
