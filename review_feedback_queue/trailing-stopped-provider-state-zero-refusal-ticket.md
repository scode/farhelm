# Stopped-environment attach diagnosis is excluded by its own freshness gate

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: if a container stops while setup or Update is waiting to reconnect, the run cannot use its new
stopped-environment/Resume explanation; it waits for the ordinary attachment timeout and reports only a timeout instead.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g044/l6`.

### g044/l6

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. TLDR: if a container stops while setup or Update is waiting
to reconnect, the run cannot use its new stopped-environment/Resume explanation; it waits for the ordinary attachment
timeout and reports only a timeout instead. Details: this assigned commit adds HostState::Stopped to attach_refusal at
service.rs:297-299, but the sole caller at 2916-2919 requires status.refusal_attempt > ticket before considering any
mapped state. manager.rs:3841-3889 publishes both initial Stopped and failed-autostart Stopped through set_state. That
follows set_state -> publish -> publish_with_live (5056-5087), which passes refusal_attempt=0; publish_refresh assigns
that zero at 5224. retry_now_with_fresh_window reads the unsigned attempt counter and returns it as the ticket
(2891-2926), so zero can never exceed a returned ticket. Thus no actor-published Stopped state can reach the new
explanation. A stopped observation after a run's preflight is not covered by plan_update_owned's earlier
suspended_refusal check at service.rs:1362-1365. The source contract for attach_refusal at 236-253 and its Stopped
branch provides the intended specific reason, whereas attach_supervisor's timeout at 2923-2925 substitutes waiting for
the provisioned supervisor: timed out. SPEC.md Container sessions:4782-4790 explicitly includes external podman stop or
OOM and requires distinct Stopped reporting with a Resume remedy; the host state can remain correctly Stopped while this
run's result loses the explanation. Finishing checks establish a shipped source route: plan.rs:910-915 makes
AttachSupervisor the action after RestartSupervisor for Update, so an external stop after successful restart can occur
before the new connection attempt. service.rs:2091-2110 keeps the host provisioning lock across revalidation and all
actions. The actor's Stopped branch attempts automatic start through lifecycle::change_with_manager, but Podman::change
at environment/podman.rs:643-651 first tries that same provisioning lock and refuses busy, so an ongoing Update need not
be rescued before its timeout. This is source-permitted external-stop ordering, not an execution observation or measured
incidence. Endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the exact gate at service.rs:2913-2915 and Stopped
branch at 305-307; manager.rs:4067,4098 still uses set_state, whose publication at 5327 supplies zero and assignment at
5464 retains it. Full pinned service history attributes the new Stopped branch and production Podman path to 566, not a
pre-range defect; later service changes do not repair it. Suggested fix: carry a fresh provider-observation attempt
identity to these publications, or otherwise distinguish fresh environment-state refusals from stale supervisor-refusal
tickets; do not simply accept an old pre-update skew. Add an integration-level attach wait check, since directly testing
attach_refusal's match cannot exercise the ticket predicate. Suggested bucket: other, diagnostic correctness plus an
unnecessary bounded wait. Could-be-highest: no established security or work-loss consequence. Confidence: confirmed by
source for the unreachable diagnosis and caller route, not observed execution. proposed_drop: false. possible_cover:
none. Caveats: the host's separate Stopped state/Resume control can still be correct; a container that successfully
reconnects completes normally; this does not allege that the ticket bug causes the container to stop or loses its files.
Existing Suspended/Changing branches may share the gate but are not adopted or attributed as a new issue here. This is
distinct from g056/f1's dial-process diagnosis. No unresolved source premise remains for this bounded finding. No
runtime tests, reproduction or live-install action performed.

Returned proposal (claims-028, event 15): **filtered**, suggested bucket **other**.

The unreachable stopped-environment explanation is established. The exact claim is a lost diagnostic and ordinary
bounded attachment wait after an external stop within setup/Update's reconnect window, which fits the rare-diagnostic
filter.

Original stronger consequence: Setup/Update waits for ordinary attachment timeout and reports a generic timeout instead
of the new Stopped/Resume remedy. The host's separate Stopped state and controls may remain correct. No earlier security
or work-loss consequence was alleged.

Unresolved premise or limitation: No premise remains for the bounded diagnostic defect. Frequency was not measured; the
filter basis is the specifically narrow stop-during-attachment ordering, not an assertion that all container stops are
rare.

Proposed basis: Propose the complete rare-diagnostic filter match, while preserving that the source bug is real. If the
parent broadens the consequence beyond this diagnostic/wait scope, retain and reassess the filter.

Existing parent disposition (event 15): rejected. Parent explicitly accepted named filtered proposal from claims-028
after inspecting it. The unreachable stopped-environment explanation is established. The exact claim is a lost
diagnostic and ordinary bounded attachment wait after an external stop within setup/Update's reconnect window, which
fits the rare-diagnostic filter. Propose the complete rare-diagnostic filter match, while preserving that the source bug
is real. If the parent broadens the consequence beyond this diagnostic/wait scope, retain and reassess the filter.
Checked source/results:
["56659839785691f76da3af5d8f51b511cff4ba8b:crates/farhelm-helm/src/provisioning/service.rs:297-299 maps Stopped to a
Resume explanation, but :2916-2919 requires refusal_attempt > ticket. manager.rs:3841-3889 publishes Stopped via
set_state; :5056-5087 sends refusal_attempt=0; :5224 stores it. nudge_now at :2891-2926 returns an unsigned
attempt-counter ticket. Zero cannot pass the gate.","Same revision, service.rs:2091-2110 holds the provisioning lock
across execution; plan.rs:910-915 puts AttachSupervisor after RestartSupervisor; environment/podman.rs:643-651 makes
automatic start refuse the busy provisioning lock. An external stop after restart and before attachment is
source-permitted.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:service.rs:2913-2915 retains the gate. No runtime incidence
or reproduction is claimed. The defect does not cause the external stop or erase files."] Current coverage/results:
["SPEC.md:4857-4865 and SPEC_impl.md:5973-5981 require distinct Stopped state and automatic-start failure/Resume
reporting. Those describe the host state, which can remain correct; neither expressly accepts the attach diagnostic
bug.","TODO.md Planned and plans/queue/INDEX.md are empty. BUGS.md's transfer hang has a different trigger, duration and
mechanism.","INDEX.md read in full; searches for refusal_attempt, attach_refusal and stopped-environment diagnosis in
queue records and active TRIAGE_OUTCOMES.md found no exact cover. Existing stopped-host-update-floor-unresolved concerns
version eligibility, not attachment diagnosis.","FILTER.md, Rare, self-correcting glitches and imprecise diagnostics:
external stop must fall inside the attachment window, and the entire established consequence is lost detail plus the
existing 30-second attach wait. No security, wrong-target action, durable wrong record or user-work loss is attributed
to this bug."] Exact cited cover: ["FILTER.md: The trigger is rare: a narrow timing window, a crash or interruption at a
specific point, or an input the shipped clients never send.","FILTER.md: an error message, log line or other diagnostic
that is imprecise, misleading, missing detail, or lost."] Original stronger consequence and missing premise remain
preserved: "Setup/Update waits for ordinary attachment timeout and reports a generic timeout instead of the new
Stopped/Resume remedy. The host's separate Stopped state and controls may remain correct. No earlier security or
work-loss consequence was alleged."; "No premise remains for the bounded diagnostic defect. Frequency was not measured;
the filter basis is the specifically narrow stop-during-attachment ordering, not an assertion that all container stops
are rare.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g044 record](records/review-through-00de0b6c/groups/g044.json). Record ID: `l6`.
