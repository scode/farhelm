# Update planning can follow an unseen registry retarget

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A delayed Update plan can freeze a host entry's newer destination while the requesting GUI still shows its previous
destination; whether that consent is sufficient is unresolved.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The request identifies the logical host entry; server planning reads its current binding. A compatible other-client
retarget/adopt can therefore make the server freeze B while the stale GUI still associates acceptance with A. Post-plan
frozen-target and observed-GUI-change guards do not refute this earlier boundary. The conditional route is
source-complete, with unmeasured occurrence. SPEC.md's logical-entry retarget acceptance conflicts with rendered-binding
wording in SPEC_impl.md:5444–5457; individual Update has additional logical-entry context. No security or user-work-loss
consequence is established.

Source locations: `crates/farhelm-ui/src/provisioning.rs:1571-1583; crates/farhelm-ui/src/api.rs:4614-4623`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. TLDR of the concern: an Update click from an older GUI
snapshot can plan against a host entry's newly corrected destination because its planning request carries only the entry
ID. Confirmed source property, not an established specification violation. prepare derives a local HostBinding from its
host prop, calls plan_host_update(base, host.id), then associates the returned token with that local binding. The
bodyless HTTP route (farhelm-helm/src/provisioning/http.rs:224-240) plans from the current row (service.rs:1112-1125),
retaining that row's registration with its target at 1220-1249. Confirmation checks the retained registration and
planned target (service.rs:1998-2093,2123-2143), not the GUI's earlier displayed destination. The shipped
set_destination producer serializes its database change and reconcile at farhelm-helm/src/hosts.rs:897-945. The GUI
rejects changes it has observed at provisioning.rs:3280-3288,3421-3425,3536-3565; these guards do not transmit an
expected destination to the server. No special backend value or arbitrary browser handler invocation is required for the
underlying stale-snapshot property. No actual event-delivery schedule, unauthorized agent retarget, credential exposure
or user-work-loss consequence was established, and no such consequence is claimed. Identity mismatch remains guarded;
absent/skewed targets are separately supported. Proposed exclusion: SPEC.md:252-262 expressly defines the row as one
logical host and permits approvals/settings/confirmed operations already in flight to follow its new destination or
adopted identity. SPEC.md:118-120 makes the Update click the authorization without a second confirmation.
TRIAGE_OUTCOMES.md:7740-7756 and 7892-7908 record the same logical-host decision; this is distinct from f1's Update-all
at-click membership acceptance. Because a contrary consent interpretation could be security-sensitive, retain this open
for the parent's independent exclusion check rather than dropping it here. Suggested bucket other as a covered
interface/specification question; source property and proposed specific coverage confirmed, defect confidence not
established. proposed_drop: exact logical-host acceptance in SPEC.md:252-262, subject to independent parent review.
possible_cover: same rule and the cited ledger outcomes. Searched SPEC.md, SPEC_impl.md, TODO.md Planned, unlanded
plans/queue, BUGS.md, queue INDEX and TRIAGE_OUTCOMES; no narrower Update exception to logical-host acceptance was
found. Earlier broad search output clipped unrelated search text; focused searches and exact cited sections were
subsequently read in full before relying on coverage. Provenance: bodyless planning and local-binding association
already exist at exclusive base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c (provisioning.rs:1559-1584; api.rs:4594-4611);
not introduced by sentence-case edits. No tests or experiments were run.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g050/f3`, `g107/f3`, `g079/f5`, `a053/d3`,
`a058/g107-f3`, `a064/g050/f3`, `a071/g050-f3`. These are original reviewer identities for this concern, not additional
bugs. Full revisions, withdrawals and independent verdicts remain in the private review evidence. No fix or triage
decision was executed.
