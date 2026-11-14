# Container dial staging disables its process-exit diagnosis

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: when SSH refuses a container connection because its container host has reached its
session limit, Farhelm reports a generic wait-for-hello failure instead of explaining the container host's SSH session
limit. Other pre-helper container process exits similarly miss their intended diagnosis. Source route:
SystemTransport::connect introduced in 1038bb1f4549f6896fa6e064fecf540f4548b4da creates Some(SshDiagnostics) for every
RemoteRoute, including PodmanExec, and records ProcessRoute::LocalPodman or NestedSsh (transport.rs:228-235, 387-404,
650-692). At 04ff, HostKind::ssh_access explicitly returns None for Podman (store.rs:886-890). manager::HostActor::dial
still interprets None as proof that no process marker exists and immediately advances to WaitingForHello
(manager.rs:4143-4147), before observing a marker. ssh_failure_detail permits process-exit classification only at
ConnectingToHost (manager.rs:5256); both the nested session-limit branch (5262-5270) and local exec exit branch
(5258-5260) are consequently unreachable for real container dials. The raw bounded stderr may still include the mux
refusal, but the explanatory container-host/session-limit diagnosis is suppressed. Contract: SPEC_impl.md, One route for
transport and provisioning, says 'When the fallback login fails, launch refusal or connection unavailability names the
container host's SSH session limit.' The source also documents that marker or successful-hello evidence, not transport
kind alone, outranks exit evidence (manager.rs:5237-5242). Suggested fix: stage by the actual TransportConnection
process diagnostic

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g056/f1`.

### g056/f1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. Original record used the earlier staging/storage revision
04ffdbb35773cef36e73eb798c005ddbeb9a9de6; production reachability is established at 566, as corrected below. Also
confirmed present at 56659839785691f76da3af5d8f51b511cff4ba8b (manager.rs:4268 and 5381). TLDR: when SSH refuses a
container connection because its container host has reached its session limit, Farhelm reports a generic wait-for-hello
failure instead of explaining the container host's SSH session limit. Other pre-helper container process exits similarly
miss their intended diagnosis. Source route: SystemTransport::connect introduced in
1038bb1f4549f6896fa6e064fecf540f4548b4da creates Some(SshDiagnostics) for every RemoteRoute, including PodmanExec, and
records ProcessRoute::LocalPodman or NestedSsh (transport.rs:228-235, 387-404, 650-692). At 04ff, HostKind::ssh_access
explicitly returns None for Podman (store.rs:886-890). manager::HostActor::dial still interprets None as proof that no
process marker exists and immediately advances to WaitingForHello (manager.rs:4143-4147), before observing a marker.
ssh_failure_detail permits process-exit classification only at ConnectingToHost (manager.rs:5256); both the nested
session-limit branch (5262-5270) and local exec exit branch (5258-5260) are consequently unreachable for real container
dials. The raw bounded stderr may still include the mux refusal, but the explanatory container-host/session-limit
diagnosis is suppressed. Contract: SPEC_impl.md, One route for transport and provisioning, says 'When the fallback login
fails, launch refusal or connection unavailability names the container host's SSH session limit.' The source also
documents that marker or successful-hello evidence, not transport kind alone, outranks exit evidence
(manager.rs:5237-5242). Suggested fix: stage by the actual TransportConnection process diagnostics (or an exhaustive
process-transport predicate), advancing immediately only for a socket transport; retain ConnectingToHost until the
process marker for Podman exec. Suggested bucket: other (diagnostic correctness; source establishes no lost work or
credential exposure). Confidence: confirmed by source and contracts, not execution. proposed_drop: false.
possible_cover: none, coverage sources not consulted. Caveats: provisioning launch setup's separate
nested_session_failure classification at 566 is unaffected, and unit-level classifier tests can bypass the erroneous
actor staging by supplying ConnectingToHost directly. No runtime reproduction or test was run. Additional source checks:
manager::attempt_failure passes progress.stage unchanged into ssh_failure_detail at 04ff (manager.rs:5378-5395). At
review endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, the premature stage advance remains at manager.rs:4477 and the
ConnectingToHost classification gate remains at 5621. The full One route for transport and provisioning section was also
checked at 04ff and retains the cited session-limit contract. Synthesis revision, turn 2: the source mechanism exists at
04ff, but that revision registers only Tensorlake (lib.rs:2440-2453); its missing Podman provider returns
StatusUnanswered (environment/mod.rs:207-209), and RetryLater refuses before dial (manager.rs:3337-3342). Thus the
earlier implication of an ordinary production Podman classifier route at 04ff was too broad. At 566, lib.rs:2465-2481
installs the real Podman provider and the store/places-backed SystemTransport; an admitted container observed Running
reaches dial (manager.rs:4175-4223; environment/podman.rs:1237-1257). The reported consequence and other bucket are
preserved. This is a diagnostic defect during an independently failing connection, not a cause of the connection failure
itself. Current-turn source checks also confirmed that advance_attempt really assigns progress.stage before publication,
including inactive reprobes (04ff manager.rs:3965-3974), hello success advances to SettingUp, and only a pending hello
with an observed marker should advance to WaitingForHello. No runtime observation. Synthesis evidence completion: at 566
both local Podman and remote-base Podman take HostAccess::Remote, spawn a piped child and return process_connection, so
Some(SshDiagnostics) reaches the actor even though HostKind::Podman.ssh_access is None (transport.rs:180-241;
store.rs:896-901). A clean pre-hello stdout close maps to ClosedBeforeHello (farhelm-proto/src/io.rs:585-602); a write
failure can be BrokenPipe. manager::dial calls handshake_termination, settles the owned process, and passes
progress.stage unchanged to attempt_failure (4268-4360). Hence the failed child does reach the classifier; the failure
is its false WaitingForHello boundary, not absence of diagnostics. If a current-attempt connected marker was actually
observed, ssh_failure_detail itself upgrades ConnectingToHost to WaitingForHello and correctly suppresses numeric
diagnosis; a completed hello advances to SettingUp, and protocol/version-skew evidence remains authoritative. These
legitimate precedence cases are outside f1. Prior-attempt marker knowledge only changes generic ConnectingToHost wording
and does not restore or remove classification. Without current marker/hello evidence, local Podman should retain its
container-execution exit diagnosis (including code 255 without calling it SSH); nested SSH should retain the
place-host/session-limit diagnosis. In the buggy path the output still appends bounded escaped stderr and the actual
numeric status (manager.rs:5440-5486); no claim is made that all diagnostic evidence is absent. Source checks this turn
reconfirm endpoint 00de manager.rs:4477 and 5621-5640 retain both the early advance and gated branches. No runtime
reproduction was performed. Caller/capacity premise checked in synthesis: a master already full during the preceding
provider status command may cause an earlier status refusal, so f1 specifically requires the independently failing
process dial after a successful Running observation and successful route resolution. Shipped actors run in separate
Tokio tasks (566 manager.rs:2023-2031); route resolution returns a BaseRoute and ReadyPlace value rather than an
SSH-slot lease (podman_probe.rs:338-362), and Podman exec command lowering chooses the place Supervisor master without
reserving capacity across the earlier status observation (access.rs:301-336). Sibling actors can consume the remaining
channel while this actor awaits its store/route lookup, after status succeeded. The claimed route therefore does not
require a failed Podman status to bypass RetryLater. Local Podman has no SSH capacity premise; any ordinary pre-helper
exec exit reaches the same false stage. Parent-supplied g055/c3 quoting refutation does not change this actor-stage
contract. All readily checkable source premises now resolved; f1 remains source-confirmed at 566 and endpoint, other,
proposed_drop=false, not a general connectivity or security finding.

Returned proposal (claims-035, event 14): **retain-verified**, suggested bucket **other**.

Container dials have process diagnostics but are advanced to WaitingForHello from the host-kind predicate, disabling
their pre-helper exit classifier. This removes intended explanation from an independently failing dial; it does not
cause connectivity failure or erase all evidence.

Original stronger consequence: Missing session-limit explanation and other pre-helper exit diagnoses, other. Earlier
attribution to production Podman at 04ff was narrowed to 566, where the provider ships. Raw escaped stderr/numeric
status survive, provisioning's separate classifier is unaffected, and this is not a general connectivity or security
finding.

Unresolved premise or limitation: Actual incidence is unmeasured. The nested capacity scenario specifically needs
successful Running status and route resolution followed by an independently failed dial; a master full already at status
may refuse earlier. Concurrent slot use can produce the later failure, and local Podman has no SSH-capacity premise. No
stronger security or lost-work consequence established.

Proposed basis: Record verified diagnostic-stage defect in other; preserve reachability correction, capacity timing
caveat, marker/hello precedence and retained raw evidence. Do not filter the whole record using only one rare subset.

Existing parent disposition (event 14): verified. Parent explicitly accepted named retain-verified proposal from
claims-035 after inspecting it. Container dials have process diagnostics but are advanced to WaitingForHello from the
host-kind predicate, disabling their pre-helper exit classifier. This removes intended explanation from an independently
failing dial; it does not cause connectivity failure or erase all evidence. Record verified diagnostic-stage defect in
other; preserve reachability correction, capacity timing caveat, marker/hello precedence and retained raw evidence. Do
not filter the whole record using only one rare subset. Checked source/results:
["56659839785691f76da3af5d8f51b511cff4ba8b lib.rs:2465-2488 installs the real provider/store-backed transport;
manager.rs:4175-4223 admits provisioned environments observed Running and reaches dial. Earlier 04ff production
reachability was correctly withdrawn.","5665983 store.rs:896-901 returns None for Podman ssh_access.
manager.rs:4251-4289 retains process diagnostics but advances at :4268-4269 before the marker. :4335-4345 settles the
process and forwards the progress to attempt_failure.","5665983 manager.rs:5372-5402 gates LocalPodman and NestedSsh
diagnoses on ConnectingToHost; :5443-5482 still preserves bounded stderr/refusal evidence and numeric status.
attempt_failure:5501-5521 passes progress.stage unchanged. Current marker, successful hello, protocol and version-skew
precedence remain outside the finding.","1038bb1f4549f6896fa6e064fecf540f4548b4da transport.rs:190-233 creates process
connections for every RemoteRoute. 5665983 access.rs:302-327 selects the place supervisor master and
podman_probe.rs:338-362 returns route/probe values without reserving an SSH session across status and dial.","Endpoint
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db manager.rs:4477-4478 and :5621-5642 retains the defect. No reproduction or
runtime observation."] Current coverage/results: {"SPEC.md":"Container sessions uses the ordinary supervisor connection
and errors contract; no acceptance of this false stage found.","SPEC_impl.md":"One route for transport and
provisioning:5995-6008 requires process diagnostics for local/remote Podman and explicit container-host session-limit
explanation. Helm transport:2825-2936 distinguishes marker/hello evidence from exit status; it does not authorize
kind-only advancement.","TODO.md Planned":"Empty.","plans/queue":"No unlanded index entries. Residual Podman plan's
session-limit requirement is already landed, not covering planned repair.","BUGS.md":"Escaped SSH-helper transfer hang
is unrelated.","queue INDEX and files":"Targeted WaitingForHello/process-exit/session-limit/master/route searches found
no exact existing item.","TRIAGE_OUTCOMES.md":"No exact active decision found.","FILTER.md":"Rare imprecise-diagnostic
filter could cover a narrowly timed capacity-race subset alone. The finding also covers ordinary local/remote pre-helper
container process failures with no established rarity premise, so its entire boundary does not match. Retain."} Exact
cited cover: [] Original stronger consequence and missing premise remain preserved: "Missing session-limit explanation
and other pre-helper exit diagnoses, other. Earlier attribution to production Podman at 04ff was narrowed to 566, where
the provider ships. Raw escaped stderr/numeric status survive, provisioning's separate classifier is unaffected, and
this is not a general connectivity or security finding."; "Actual incidence is unmeasured. The nested capacity scenario
specifically needs successful Running status and route resolution followed by an independently failed dial; a master
full already at status may refuse earlier. Concurrent slot use can produce the later failure, and local Podman has no
SSH-capacity premise. No stronger security or lost-work consequence established.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g056 record](records/review-through-00de0b6c/groups/g056.json). Record ID: `f1`.
