# Stopped container offers Suspend when its container host is unavailable

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: a stopped container waiting for its container host to reconnect offers Suspend instead
of an unavailable or Resume action; the wrong action can ask for a pointless suspension confirmation and then tell the
user to choose Resume, which that menu does not offer. Source: HostRow:4394-4412 only includes Stopped in its resumable
predicate for waiting_for_place: false. With true, offers_resume is false and lifecycle_action defaults to Suspend. Its
actual menu:4930-4945 is disabled by busy/lifecycle_changing only; neither follows a disconnected place.
on_lifecycle:2344-2385 dispatches the selected false resume flag and, because this host is not Connected, opens the
existing Suspend confirmation even for an empty retained session list. Podman change:636-673 inspects the resource
before mutation; lifecycle_verb:1421-1434 refuses a stopped/exited/created container's suspension with 'the container is
not running; use Resume to start it'. This backend safety does not correct the wrong UI affordance. Shipped state
premise: Podman status:1237-1261 maps observed stopped/exited/created to EnvironmentStatus::Stopped;
manager.rs:3840-3861 publishes Stopped with waiting_for_place true whenever the recorded place host's supervisor is not
Connected. That does not imply Podman observation is impossible: Podman.place:150-167 and command:170-189 use the
place's local/SSH base route and cached successful probe, and podman_probe::runtime:338-375 requires the recorded
endpoint/probe but no supervisor connection. Thus a stopped container on the helm machine while that machine's ordinary
supervisor is

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g031/f7`.

### g031/f7

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 21 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. TLDR: a stopped container waiting for its container host to
reconnect offers Suspend instead of an unavailable or Resume action; the wrong action can ask for a pointless suspension
confirmation and then tell the user to choose Resume, which that menu does not offer. Source: HostRow:4394-4412 only
includes Stopped in its resumable predicate for waiting_for_place: false. With true, offers_resume is false and
lifecycle_action defaults to Suspend. Its actual menu:4930-4945 is disabled by busy/lifecycle_changing only; neither
follows a disconnected place. on_lifecycle:2344-2385 dispatches the selected false resume flag and, because this host is
not Connected, opens the existing Suspend confirmation even for an empty retained session list. Podman change:636-673
inspects the resource before mutation; lifecycle_verb:1421-1434 refuses a stopped/exited/created container's suspension
with 'the container is not running; use Resume to start it'. This backend safety does not correct the wrong UI
affordance. Shipped state premise: Podman status:1237-1261 maps observed stopped/exited/created to
EnvironmentStatus::Stopped; manager.rs:3840-3861 publishes Stopped with waiting_for_place true whenever the recorded
place host's supervisor is not Connected. That does not imply Podman observation is impossible: Podman.place:150-167 and
command:170-189 use the place's local/SSH base route and cached successful probe, and podman_probe::runtime:338-375
requires the recorded endpoint/probe but no supervisor connection. Thus a stopped container on the helm machine while
that machine's ordinary supervisor is unavailable has a concrete source route to this state and to the not-running
refusal. No claim is made that manual Resume is guaranteed to succeed while the place is unavailable. Contract: SPEC.md
Container sessions:4782-4789 distinguishes Stopped from suspension, gates automatic start on connected place, and offers
Resume for failed start. The waiting state may withhold Resume, but should not substitute an affirmative Suspend for an
already stopped resource. Omit the action or present unavailable Resume. Suggested bucket: other (wrong lifecycle
affordance and contradictory remedy; no mutation harm). Confidence: confirmed by source/caller analysis, no runtime
execution. proposed_drop false. possible_cover none. Could-be-highest no established security/work-loss route; provider
validates and refuses before mutating. Whole-range attribution: the parent of 350a4b33 has only a Suspended predicate
and no Stopped UI branch; the assigned patch introduces the explicit waiting_for_place:false guard while adding Stopped
support. Later 1d24c6a changes busy gating and 222bf625 suppresses lifecycle controls for Deleting, but neither changes
the Stopped predicate. At pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db it remains at hosts.rs:4481-4504, so no
later in-range fix. Original suspected consequence retained. This new discovery is separate from f1's provider/start
diagnostic formatter and g024/c4's backend paused-container noun. All useful source premises checked; no unresolved lead
remains for this issue.

Returned proposal (claims-018, event 21): **retain-verified**, suggested bucket **other**.

Stopped with waiting_for_place true falls through to Suspend. The backend refuses the stopped-resource suspension, but
that does not correct the offered operation or contradictory Resume remedy.

Original stronger consequence: Wrong Suspend affordance and pointless confirmation/refusal for an already stopped
container; no unsafe mutation was alleged. No claim that manual Resume must work while its container host is
unavailable.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the menu eligibility/verb issue separately from f1's displayed progress wording; those require
different decision fixes.

Existing parent disposition (event 21): verified. Parent explicitly accepted named retain-verified proposal from
claims-018 after inspecting it. Stopped with waiting_for_place true falls through to Suspend. The backend refuses the
stopped-resource suspension, but that does not correct the offered operation or contradictory Resume remedy. Retain the
menu eligibility/verb issue separately from f1's displayed progress wording; those require different decision fixes.
Checked source/results: ["350a4b33b6d4476f4a78e43cea124bc2b828f462:UI hosts.rs:4394-4412 includes Stopped in the
resumable predicate only with waiting_for_place false; :4930-4945 offers the resulting Suspend and gates it only on
busy/changing.","350a4b33:helm manager.rs:3840-3861 publishes Stopped with waiting_for_place = !gate, where gate depends
on the place supervisor being Connected.","350a4b33:environment/podman.rs:150-189 resolves local/SSH place command
routes independently of a supervisor client; :1237-1261 maps exited/stopped/created to Stopped. :636-684 checks actual
resource state; lifecycle_verb:1421-1434 refuses suspension of a stopped container with use Resume to start
it.","Exclusive-base-to-350a4b33 diff introduces the waiting_for_place:false predicate with Stopped. Tip
hosts.rs:4481-4504 retains it, and hosts.rs has no later diff after 222bf625. Source only; no guarantee manual Resume
succeeds while the place is unavailable."] Current coverage/results: ["SPEC.md Container sessions:4857-4865
distinguishes Stopped, connected-place automatic start and failed-start Resume. SPEC_impl.md:5979-5983,6161-6166 agrees.
Neither accepts affirmative Suspend for Stopped.","TODO.md Planned and plans/queue/INDEX.md are empty.
podman-environments is landed per REPORTS.md:21. BUGS.md is unrelated.","INDEX.md, queue content and active
TRIAGE_OUTCOMES.md searches for waiting_for_place, stopped lifecycle, Suspend/Resume remedy found no exact item.
FILTER.md's rare trigger requirement is not established by ordinary place-supervisor disconnection."] Exact cited cover:
[] Original stronger consequence and missing premise remain preserved: "Wrong Suspend affordance and pointless
confirmation/refusal for an already stopped container; no unsafe mutation was alleged. No claim that manual Resume must
work while its container host is unavailable."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g031 record](records/review-through-00de0b6c/groups/g031.json). Record ID: `f7`.
