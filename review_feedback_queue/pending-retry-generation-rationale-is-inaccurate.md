# Pending retry generation rationale is inaccurate

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The retry rationale and regression model a generation change that real pending-create retries do not make.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The regression manually advances generation to one, while real pending retries retain generation zero.
Current-ticket-under-lock and stored-launch guards refute specific stale-ticket data paths, not every retry history or a
universal fingerprint claim. The original broad source-refutation was corrected. This is a documentation/oracle contract
mismatch; the distinct orphaning/file-loss routes remain recorded separately.

Source locations: `SPEC_impl.md:5782-5786; crates/farhelm-supervisor/src/container.rs:488-489,1204-1215`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d. Final targeted record repair preserves f5 as confirmed OTHER,
independently retained by a0213/a0233 per parent. User-facing TLDR: the documented and tested protection against an
older container helper using a retry ticket is stronger than what the actual retry counter establishes. Source: pending
takeover accepts generation zero and preserves it (store.rs:3860-3892,3959-3995; core.rs:10067-10081). The test fixture
starts at generation zero (container.rs:689-706); the test writes generation one (1207-1211) while its child calls
prepare with zero (1004-1009). It therefore verifies unequal-generation refusal, not isolation between actual
generation-zero retry attempts. SPEC_impl.md:5783 and container.rs:488-489 describe the latter stronger guarantee.
proposed_drop: none for this rationale/test mismatch. possible_cover: none established. Corrected retry scope:
core.rs:8914 gates the launch/folder/title comparison at 8915-8931 on is_agent_request_fingerprint. Ordinary REST/GUI
create has request_fingerprint None (helm sessions.rs:1535-1559) and a separate resolved-launch fingerprint path
(supervisor handlers.rs:426-454; core.rs:1588-1609,9204-9228). Those are distinct contracts, not proof that every
same-key GUI lifecycle preserves one immutable launch/cleanup history. Reading the recorded row in
validate_retry:9067-9092 does not justify such a whole-source claim. Known ordinary-GUI
template-change/reboot/generation-zero findings supplied by the parent, a003d1/a032/g115f4 and highest
initialized-work-loss g113f3, are expressly not excluded or re-adjudicated by f5. This pass does not invent a broader
conditional consequence or claim independent runtime authentication of those findings. Guard scope correction: the
ticket-reading helper at container.rs:567-580 holds the permanent lock and reads the current ticket; the outer
ContainerLaunch::prepare fast path at 530-545 can return on initialized/running state without that ticket-generation
comparison, and launch.rs:1165-1166 separately proceeds to exec afterward. Consequently current-ticket reasoning applies
only to the demonstrated helper path, not every shim or later phase. No blanket stale-shim or broader-case
security/work-loss exclusion remains. Related prior-disposition correction (parent-owned a005 OTHER, no new local
finding ID): SPEC.md:3635-3636 explicitly requires Replace and Replace with to create a fresh session AND container
before deleting the source. Generic successful-Create/later-launch-failure text at 1041-1045 and generic Replace wording
at 1408-1421 do not erase this container-specific promise. Helm finish_replacement:3787,3821 proceeds from created
SessionInfo to source deletion, while supervisor core.rs:15191-15227 publishes container intent and
15389-15394,15481-15487 accepts the tmux launch; actual container preparation is deferred to launch.rs:1163-1166. Thus
my prior claim that no narrower container-readiness promise exists was false and is superseded. Preserve the
independently retained a005 OTHER discrepancy, without inventing a new unauthorized-Delete allegation. f1/f3 remain
retained; f2 exact accepted Delete scope and f4 withdrawal remain unchanged. No checkout writes, tests, builds,
experiments or other runtime actions.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g117/f5`, `g123/f4`, `a021/x-g117-f5`,
`a023/f1`. These are original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and
independent verdicts remain in the private review evidence. No fix or triage decision was executed.
