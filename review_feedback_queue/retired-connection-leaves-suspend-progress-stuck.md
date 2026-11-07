# Retired connection leaves suspend progress stuck

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a

## TLDR

A failed Suspend on a retired connection can leave the host showing Suspending with lifecycle controls disabled until a
separate recovery action.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/tensorlake/lifecycle.rs:80-91,145-160 at 1a57aa8e265cbb59cedc228433b189f59f7ad59a`.

Source assessment (read with the later corrections below) (review record g035/l4, event 13):

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a; unchanged lifecycle, manager and helm host-view code through
pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b (git diff --exit-code on those paths produced no differences). TLDR:
after a host connection task has already retired, a failed Suspend can leave the host labelled Suspending after the
request ends, with its row Suspend/Resume controls disabled. Closing the error and choosing the ordinary Retry menu item
recovers it; passive page refresh does not. Mechanism: lifecycle.rs:80-91 calls hold_connection for Suspend.
manager.rs:1572-1596 overwrites the completed handle's Retired phase with TensorlakeChanging even when the task is
finished or its nudge receiver is closed. The supervisor's one-time Retired publication (:2004-2048) has already
happened. On lifecycle failure :150-160 releases the hold but does not revive; release_hold (:1619-1630) only clears
hold and nudges the dead receiver, leaving the phase unchanged. The explicit revival branch is resume && result.is_ok().
hosts.rs:480-532,573-577 builds passive GET /api/hosts replies from registry rows and snapshots without reconciliation.
UI hosts.rs:4417-4424,4720,4940-4947 disables row lifecycle actions for TensorlakeChanging;
hosts/attention_list.rs:157-161 suppresses that phase as routine work instead of retaining the retirement problem. The
refusal modal (UI hosts.rs:228-310) provides Close, not a lifecycle retry; the request wrapper (:2154-2188) records the
refusal, releases mutation busy/page claim and performs no automatic recovery on error. Ordinary recovery was rechecked:
menu Retry (:4902-4935) guards busy only, row busy (:3170-3173) does not derive from phase, dispatch (:2194-2225)
reaches helm hosts.rs:1469-1480 and manager.retry_now. manager.rs:2845-2880 checks finished task/closed receiver
independently of displayed phase and revive (:2901-2955) restarts it. sync_registry (:1727-1774) independently respawns
such handles; add_host (helm hosts.rs:929), alias save (:1113-1118), and provisioning registry/attach paths
(provisioning/service.rs:935-938,998,2703-2709) invoke reconciliation or revival. Thus a relevant registry mutation, a
successful direct Resume, or helm startup also recovers; retargeting this Tensorlake host is not offered and is not a
suggested remedy. An already-live actor ordinarily resumes its own loop after release; the finding specifically requires
the actor to have completed retirement before the lifecycle hold, with no concurrent Retry/reconcile reviving it. The
retired-actor setup is already exercised by hosts_tests.rs:1476-1501 for successful Resume. SPEC.md, Topology at this
revision says failed/timed-out lifecycle requests reconnect and progress lasts until vendor confirmation; SPEC_impl.md,
Tensorlake hosts says every failure releases the hold and the transient phase ends at hold release. Suggested fix:
preserve/restore retired or failure display, or revive on failed lifecycle completion; regression test can reuse the
existing retired-actor fixture, inject a suspend failure, then assert settled phase and recovery. Suggested bucket
other: recoverable progress/diagnostic and lifecycle-control defect, with no established security or user-work loss.
Confidence confirmed for the conditional structural transition. Runtime incidence is unverified after investigation:
source establishes supported retirement handling and the faulty transition but does not establish how often a production
actor retires; no runtime experiment or frequency claim was made, and no 'rare' assertion should be inferred from
earlier wording. proposed_drop: false. possible_cover: no coverage sources searched; actor-lifecycle interface groups
may overlap. No unresolved source premise remains for the conditional bug; dynamic incidence measurement is not asserted
as evidence.

Later qualification (review record g035/l4, event 15):

Supplement to the complete local l4 synthesis record, not a new discovery. Reviewed full hash
1a57aa8e265cbb59cedc228433b189f59f7ad59a, persists through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR:
after completed connection-actor retirement, failed Suspend overwrites the dead handle with Suspending and release_hold
cannot republish or revive; the UI retains progress and disables row lifecycle controls until a separate recovery
action. The complete source mechanism, regression recipe and available recoveries remain in prior l4 events.
Parent-supplied g044:l4 (log line 13) independently confirms host_views:480-533 and state_view:538-554 are passive
reads, no periodic retirement-repair timer was found among sync_registry callers, and lifecycle failure performs no
reconciliation. It confirms generic Retry via manager.rs:2864-2880,2916-2955 and registry/provisioning reconciliation
via :1731-1774 recover even when the displayed phase masks Retired; UI hosts.rs:4417-4418,4720 corroborates disabled row
Resume. Preserve its limiting premise: retirement publication must already have completed before the hold; if that
publication arrives later, it can settle the displayed phase. SPEC.md Topology:286-291 and SPEC_impl.md Tensorlake
hosts:6018-6034 require failure recovery and transient intent ending with hold release. Suggested bucket other;
confidence confirmed conditional source route, runtime incidence unverified and no frequency claim. proposed_drop false.
possible_cover: g044:l4 explicitly supports local g035:l4 and is a duplicate/supporting record, not a second finding.
g034:l2 remains distinct (live Connected actor, ambiguous cloud Resume, unverified gateway consequence); do not merge or
upgrade either. No security/data-loss consequence or new unresolved lead established. No coverage search or runtime
actions.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g035/l4`; supporting records
`g044/l4`, `g040/g035:l4`, `g049/iface_g035_l4`, `g045/interface-g035-l4`. The retained private log preserves earlier
wording and its corrections. This record does not claim runtime reproduction.
