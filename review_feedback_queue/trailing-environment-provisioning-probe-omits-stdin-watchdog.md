# Environment provisioning probes omit the required stdin deadline

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Environment provisioning probes omit the required stdin deadline

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g055/l5`.

### g055/l5

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 14 (finding, open):

# Environment provisioning probes omit the required stdin deadline

Reviewed assigned extraction commit: 1038bb1f4549f6896fa6e064fecf540f4548b4da. Integrated production Podman revision:
56659839785691f76da3af5d8f51b511cff4ba8b. Full-range attribution checked from 6e423e30068b62c6cfa276c6d720ee35bb70387a
through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.

## TLDR

Farhelm's setup/update probe starts an environment's stdio helper without the promised two-minute input-silence limit.
Its normal fifteen-second probe timeout belongs to the helm; it supplies no remote deadline if that machine stops
executing while the gateway/client still holds the helper's stdin open. The confirmed defect is the missing environment
invocation contract, not an observed leaked process or measured billing incident.

## Details

At 1038bb1, backend.rs:1558-1572 probe_script emits exec resolved internal stdio with only an optional --state-dir.
spawn_probe at 1577-1615 passes that script to remote_command, which only prepends REMOTE_RUNTIME_PREFIX and wraps the
selected route. It does not use HostAccess::stdio_argv, the new common constructor that adds --stdin-idle-limit-secs 120
for environments. At integrated 5665983 the same hand-built command remains in probe_script (1606-1622) and spawn_probe
(1626-1664). Podman exec therefore receives the same unlimited helper as SSH. Production reachability:
service.rs:2992-3009 converts resolve_host_access into a remote ProbeTarget for admitted container rows; its update path
uses probe_target_for_row and backend.probe (plan_update_owned at 1343-1373; 1715-1721 belongs instead to uninstall_plan
and is not claimed as a Podman route). This is not an unreachable direct test construction.

The helper has no default watchdog: main.rs:2761 accepts an optional budget and :2822-2824 makes None wait forever. Its
documented source premise is that a gateway may retain its TCP peer after a sleeping helm stops sending (2746-2751). A
successful short probe normally drops its FrameWriter after handshake and calls stop_probe; backend.rs:2410-2423
explicitly closes stdin and :2287-2289 terminates/reaps the local process group. Those paths explain why ordinary
success need not leak, but are local-helm work, not a remote expiry guarantee while the helm is asleep/unavailable.
Clean EOF also disables the watchdog by design, so no claim is made about a probe that already delivered clean EOF. The
missing-flag defect does not require asserting that killing every supported Podman client necessarily leaves a live
helper.

Exact contract at 1038bb1 and 5665983, SPEC_impl.md One route for transport and provisioning (6478-6481): 'All
environment stdio routes pass `--stdin-idle-limit-secs 120` unconditionally and emit the existing 30-second heartbeat.
The supported environment helper has that option. Killing a Podman exec client does not reliably end its in-container
process, so the helper's own stdin watchdog supplies the bound.' The method's route is explicitly shared by
provisioning; this probe is another invocation of internal stdio but bypasses that policy. A fifteen-second probe needs
no steady heartbeat to stay below a 120-second deadline, so adding the environment-only option does not require turning
discovery into a steady-state connection. Ordinary SSH probes must retain compatibility without this flag.

Attribution caveat: the unlimited Tensorlake provisioning probe already exists at the exclusive range base
(backend.rs:1566-1582). Its then-contract SPEC_impl.md:6942-6946 explicitly limited opt-in to 'Only Tensorlake's
steady-state SSH command'. Thus 1038 is not alleged to newly break existing Tensorlake probe behavior. In-range
c8bc155cc36723c8c53870944d07ce8ec61ef9ed replaces that narrow contract with all environment routes, 1038 adds the common
policy but leaves this bypass, and 566 ships the newly supported Podman probe through it. This record reports the new
route/contract mismatch and preserves that historical boundary rather than misattributing the old Tensorlake omission.

Suggested bucket: other for the confirmed environment helper invocation/expiry guarantee mismatch. Could-be-highest: no
established security or loss-of-work consequence. Confidence: confirmed by pinned source and contract; conditional
remote orphan duration/billing was not reproduced or established beyond the documented retained-stdin premise.
proposed_drop: false. possible_cover: none in the supplied neighbour records; no prohibited coverage search. No runtime
tests, builds or reproductions performed. Fix direction: carry the route's environment idle-limit policy into
probe_script/spawn_probe while retaining positive-absence markers, resolved-binary reporting and the ordinary SSH legacy
invocation.

Final source checks: integrated backend.rs:1618 is the exact missing-flag command and access.rs:35-37 is the
environment-only option constructor it bypasses. At endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db the same unlimited
probe remains at backend.rs:1602-1618 (command line 1614). The exact c8bc155cc36723c8c53870944d07ce8ec61ef9ed
specification diff both adds the all-environment-route rule and replaces the earlier steady-state-only exception; this
attribution is from the whole range, not merely the parent of 1038. plan_update_owned is a run_owned task, which handles
dropped request cancellation while the helm is executing but cannot keep its local timers/cleanup executing when its
machine sleeps. Admitted environments pass its initial-coordinate guard; Podman uninstall is separately refused and is
not needed for reachability. The no-watchdog invocation and new Podman Update route are source-confirmed; actual
orphaning/billing incidence and external client behavior remain explicitly unobserved. No uninvestigated lead remains
for the reported contract mismatch.

Returned proposal (claims-035, event 14): **retain-verified**, suggested bucket **other**.

The admitted environment Update probe bypasses the environment stdin policy and invokes an unlimited helper. The
contract mismatch is established; actual orphan duration or billing remains conditional and unobserved.

Original stronger consequence: Initial lead suggested a helper outliving two minutes and possible extra process
lifetime/billing. The latest record confines its verified consequence to the missing invocation/expiry contract,
retaining the conditional remote-orphan premise and normal-cleanup caveat. No highest consequence was asserted.

Unresolved premise or limitation: Actual orphaning/billing incidence and external client's retained-stdin behavior were
not reproduced. Local cleanup cannot run while the helm machine sleeps; clean EOF already delivered is outside the
claim. No security or user-owned-work loss is established.

Proposed basis: Record a verified invocation-contract defect in other, with conditional lifetime/billing consequences
and full-range attribution caveat intact.

Existing parent disposition (event 14): verified. Parent explicitly accepted named retain-verified proposal from
claims-035 after inspecting it. The admitted environment Update probe bypasses the environment stdin policy and invokes
an unlimited helper. The contract mismatch is established; actual orphan duration or billing remains conditional and
unobserved. Record a verified invocation-contract defect in other, with conditional lifetime/billing consequences and
full-range attribution caveat intact. Checked source/results: ["56659839785691f76da3af5d8f51b511cff4ba8b
backend.rs:1606-1622 emits internal stdio with optional state only; :1626-1664 wraps it through route.command without
HostAccess::stdio_argv. access.rs:30-47 is the environment-only flag constructor it bypasses.","5665983
service.rs:1343-1373 admits installed environments and calls probe_target_for_row/backend.probe; :2992-3009 converts
their resolved access to Remote ProbeTarget. This proves production Update reachability, not the separately refused
Podman uninstall path.","5665983 farhelm/src/main.rs:2746-2751 documents a sleeping helm with retained gateway stdin;
:2761,2821-2827 supplies no watchdog without a passed limit and disables it after clean EOF.","Exclusive base
SPEC_impl.md:6942-6946 limited the policy to Tensorlake steady-state transport. The
c8bc155cc36723c8c53870944d07ce8ec61ef9ed SPEC_impl diff adds all environment routes and removes that exception. 566
ships the Podman Update path. Thus the new contract/route mismatch is in range even though old Tensorlake probes were
unlimited.","Endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db backend.rs:1602-1618 still omits the option. Ordinary
successful cleanup/clean EOF is not alleged to leak; no runtime test performed."] Current coverage/results:
{"SPEC.md":"Container sessions promises ordinary updates in containers; no exemption for provisioning helper expiry
found.","SPEC_impl.md":"Current One route for transport and provisioning:5999-6003 requires the flag for all environment
stdio routes; Tensorlake hosts preserves the no-default-watchdog/clean-EOF distinction. No short-probe exemption
found.","TODO.md Planned":"Empty.","plans/queue":"No unlanded index entries.","BUGS.md":"Accepted local escaped-helper
transfer hang is different from a missing remote input-silence watchdog.","queue INDEX and
files":"probe-timeout-discards-parsed-remote-path.md concerns lost parser metadata at cleanup, and
tunnel-helper-exit-may-block-flushing-stdout.md concerns blocking exit after the watchdog fires. Neither removes this
missing flag or covers it exactly.","TRIAGE_OUTCOMES.md":"No exact environment-probe expiry decision
found.","FILTER.md":"No filter fully matches: potential retained process lifetime is not merely a rare
display/diagnostic or safe retry."} Exact cited cover: [] Original stronger consequence and missing premise remain
preserved: "Initial lead suggested a helper outliving two minutes and possible extra process lifetime/billing. The
latest record confines its verified consequence to the missing invocation/expiry contract, retaining the conditional
remote-orphan premise and normal-cleanup caveat. No highest consequence was asserted."; "Actual orphaning/billing
incidence and external client's retained-stdin behavior were not reproduced. Local cleanup cannot run while the helm
machine sleeps; clean EOF already delivered is outside the claim. No security or user-owned-work loss is established.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g055 record](records/review-through-00de0b6c/groups/g055.json). Record ID: `l5`.
