# Late snapshot visibility escapes builder cleanup

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

A snapshot becoming visible only after cleanup's last listing could remain after builder removal. The provider's
relevant visibility guarantees remain unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/build.rs:1166-1207 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g068/l2, event 6):

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c. TLDR: retrying or deleting a recipe after its builder starts
suspending can leave a billed snapshot behind while Farhelm reports cleanup complete and loses the builder record needed
to find it. Mechanism: sweep_builder accepts DELETE success (1166-1169), immediately lists snapshots (1179-1182), and
clears builder_sandbox_id after deleting only those already returned (1195-1207); it never waits for termination. A
builder abandoned by a stopped helm automatically suspends after 900 seconds (build.rs:84-89, create request:920);
recovery reconciles/sweeps it (987-1003). The later shipped host teardown explicitly documents that an in-flight suspend
can still write its snapshot after termination is requested (hosts.rs:35-40 at 8a8c9579e04299410652f0e4681975a7198dfc7b)
and waits_until_terminated before listing (192-197), but the builder path does not. SPEC.md Topology requires
failed/interrupted build resources to stay recorded until cleanup and SPEC_impl.md Tensorlake snapshot builds relies on
builder IDs to attribute orphan snapshots. Suggested bucket: high (ongoing billed cloud storage and broken cleanup, no
established loss of user work or security route). Confidence: likely, unverified after investigation. Checks: inspected
immutable builder implementation, production HTTP DELETE wrapper (http.rs:118-122,240-243: any 2xx is success without
polling), API trait, fake model, later host teardown and full assigned-file diff through pinned tip. The fake terminates
synchronously (e2e.rs:242-245), so its passing source assertions cannot settle this boundary. Missing premise: actual
vendor timing when a builder's automatic suspend overlaps termination; no network/runtime experiment is allowed. A
delayed-termination fake that materializes the suspend snapshot only at Terminated would demonstrate this, or vendor
evidence could refute it by guaranteeing DELETE waits. Fix direction: share bounded wait_until_terminated before
snapshot listing, preserving builder ID on timeout. proposed_drop: false. possible_cover: unknown; parent owns coverage.
Temporal: later commits through 8a8c9579e04299410652f0e4681975a7198dfc7b leave sweep_builder unchanged and add the
analogous host wait rather than repairing this path. Caveat: the cancelled create_snapshot variant of the lead has not
been independently established; finding rests on the documented automatic-suspend route.

Later qualification (review record g068/l2, event 12):

Reviewed hash 744ea4122d033ef616454fe592ecb2173593736c. This appends to the complete initial l2 finding, not a new
discovery. Read named g070 l1: identical DELETE/list/clear ordering, synchronous fake limitation and missing production
late-snapshot premise; possible_cover is now g070 l1 (known duplicate, consolidate at parent). Its other bucket versus
this log's high needs consequence reconciliation: only possible billed storage and cleanup bookkeeping loss have been
identified, not user-data/process/security harm. Source corrections/checks to follow in this synthesis: verify
production HTTP DELETE only checks response success, automatic-suspend route versus completed normal builds, fake
delayed-termination coverage and whether ongoing billing has an established material impact. Confidence remains
likely/unverified after investigation, proposed_drop false; no decision to drop or classify highest. Prior detailed
mechanism and caveats retained.

Later qualification (review record g068/l2, event 16):

Delta to complete l2 record, known duplicate g070 l1: original production http.rs:93-99 returns on any successful HTTP
status; empty:120-122 does not parse or wait for lifecycle completion; terminate_sandbox:240-242 calls that wrapper.
This proves absence of a client-side wait, not that the provider actually returns before snapshot visibility. Current
public https://docs.tensorlake.ai/sandboxes/lifecycle.md says named builders suspend after idle timeout, but its Suspend
vs snapshot wording says suspend pauses the sandbox rather than creating a reusable artifact. That distinction may
concern internal checkpoints versus user-created reusable snapshots; it neither proves historical November 2026 snapshot
listing behavior nor definitively refutes local SPEC_impl/host teardown's explicit late-suspend-snapshot rationale. Do
not cite current docs as proof of a leaked billable artifact. Need inspect the precise host delayed-termination fixture
and vendor API response documentation. No new discovery or runtime/API experiment; unverified after investigation,
proposed_drop false. Severity reconciliation remains pending; no highest consequence established.

Later qualification (review record g068/l2, event 17):

Delta, duplicate g070 l1, proposed_drop false. Checked ca28b604b876770b1ab757783416f83ad1709313 hosts_tests.rs:568-617:
the test pre-seeds snap-a at 583, forces terminate_leaves=Suspending at 584, then asserts timeout and no snapshot list;
it does NOT materialize a new snapshot after DELETE/list. Thus it proves host wait policy against a fake nonterminal
status, not the remote late-visibility premise. Original build_tests.rs:136-156 delegates termination/listing to the
synchronous model; recovery test:612-619 pre-seeds all artifacts on an already terminated builder. Neither tests
production ordering. Current primary https://docs.tensorlake.ai/api-reference/v2/sandboxes/delete.md documents 200 as
sandbox terminated successfully and idempotence, not an explicit accepted/pending status. That wording weakens any
categorical claim that production DELETE is asynchronous; it does not expressly guarantee all in-flight snapshot
artifacts have become list-visible. The client implementation proves no additional wait only. Keep exact missing
premise: a builder's in-flight idle-suspend snapshot can first become list-visible and billable after successful DELETE
and the subsequent list. Local host rationale supports suspicion; no primary vendor ordering guarantee or observation
establishes it. Reclassification recommendation other, aligning g070 l1: bounded source-level lifecycle/test gap and
possible residual storage charges, with no shown material billing magnitude, common recurrence, lost user work or
security harm. High would require a demonstrated severe billing/cleanup consequence or moderate common failure, not
merely indefinite storage duration. Normal successful build already waits for its filesystem snapshot to complete; this
lead targets interrupted/recovery or in-flight snapshot boundaries, not every build.

Later qualification (review record g068/l2, event 24):

Selected g070:l1 line18 supplied in full this turn; duplicate, not NEW. Adopt owner's exact qualification: an
already-listed LocalReady/in-progress snapshot is not the missing route, because delete refusal would retain builder ID.
Missing sequence is first list visibility after successful DELETE and the subsequent successful list, then billable
survival after ID/name attribution is lost. A delayed-appearance fake can exercise our code but cannot establish vendor
permission of that sequence; host comments/tests establish intended waiting, not provider semantics. Keep unverified
after investigation, other, proposed_drop false, possible_cover g070 l1. No repeat of unchanged external-premise
investigation.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g068/l2`; supporting records
`g070/l1`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
