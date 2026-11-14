# Environment endpoint authority needs reconciliation with logical-entry consent

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Environment endpoint authority needs reconciliation with logical-entry consent

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g052/c3`.

### g052/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 19 (candidate, open):

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435; contract persistence checked at range endpoint
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. Original TLDR and strongest consequence preserved: an
ordinary-card/standing-permission bypass could allow unconfirmed billed creation; the separately owned g020/l1 route can
execute the captured command and deliver configured recipe credentials on the new physical endpoint of a retargeted
container-place entry after the card captured its old display name. Do not erase that credential/execution effect or
recast source observation as runtime evidence. Its classification as beyond the user's consent is now investigated
uncertainty because an exact product rule explicitly permits some approval retargeting. No parent drop or lowering
accepted.

Assigned representation/consumer results remain confirmed and narrow. LaunchEnvironment at proto approvals.rs:203-220
holds recipe:String, symbolic Place::PodmanHost(i64), captured place_name, resolved launch and prompt; no place
destination/connection evidence is encoded. UI renders the captured name. The handler forwards that numeric place after
Allow, checking the clone source where applicable rather than the physical place endpoint. Direct-Allow at proto
approvals.rs and helm approvals.rs excludes standing permissions and AlwaysAllow, and the distinct enum tag prevents
older UIs interpreting it as ordinary Launch. These facts refute only the original bypass/old-card hypotheses, not
physical retargeting. A separate private snapshot could bind the endpoint without necessarily changing this wire shape
IF that is the intended authority contract.

Late synthesis checks/results: read all 17 prior local log events before revision. Read reviewed SPEC.md:399-409, the
COMPLETE Agents and environments subsection at 4289-4342, the origin-connection paragraph at 4084-4099, SPEC_impl.md's
opening authority NOTE and 9539-9568, then the current versions of the logical-entry and machine-named clauses and the
opening NOTE. Targeted searches of these pinned specs plus immediate protocol/caller files found no explicit
container-place exception to the logical-entry rule. Directly checked proto place-field docs (207-210) and
plan_environment_create (agent_requests.rs:4191-4205): --container-host resolves an ordinary registry host by display
name and stores its id. This is a real logical registry entry, not an unrelated cloud account or a newly created
environment row. SPEC.md:4091-4092 and SPEC_impl.md:9311-9315 bind the requesting connection; they do not independently
establish a place-target veto, and g020's supplied route keeps its different origin connected.

Competing exact authority text retained: reviewed SPEC.md:399-405 says 'a host entry is one logical host from the user's
point of view', 'Approvals, settings changes and confirmed operations therefore follow the entry', and includes
in-flight 'create or clone' operations that 'may be carried out against the entry's new machine, and Farhelm adds no
check of the installation identity to prevent that.' Lines 405-406 name agent host edit/removal cards as narrower
because they explicitly show destination; the environment-card requirement at 4330-4334 names the container host and
creation consequence but does not itself demand displaying or freezing its destination. Conversely SPEC_impl.md:9548
says 'What a card shows is what an approval does', and 9555-9556 says 'A launch is also refused if the target host's
connection changed while the card waited, since the approval was for the machine the card named.' Both statements
persist at endpoint SPEC.md:405-415 and SPEC_impl.md:9090-9098. They must not be reconciled by silently omitting one.
Precedence itself is explicit: the SPEC_impl.md opening NOTE says 'SPEC.md defines what the product does; when this
document and SPEC.md disagree about observable behavior, SPEC.md wins.' This favors the product's logical-entry
allowance when the clauses cover the same operation. Recipe mutability does not resolve this conflict and is not the
acceptance basis here.

Remaining exact missing premise: whether that general logical-entry create/clone allowance was intended to include a
pending environment-creation card's CONTAINER PLACE, or whether these cards have an unstated physical-machine exception
matching the implementation document. Source establishes entry identity, symbolic request representation, physical
retarget route and textual precedence, but cannot establish an unstated scope exception or maintainer intent. A contract
clarification explicitly applying or excluding logical-entry retarget consent for container-place approvals would settle
it; more execution cannot resolve normative intent. Best supported acceptance argument is the broad product allowance
plus precedence, but it remains a proposed specification-acceptance interpretation for the independent caller check.

Suggested bucket: preserve the original highest/security concern proposal for the separately owned changed-endpoint
credential/execution effect, conditional on its being outside accepted logical-entry consent; no independent
lowering/drop by g052. Could-be-highest: yes, original consequence retained. Confidence: confirmed source mechanics;
unverified after investigation as a reportable authorization violation because of the exact competing authority scope
above. proposed_drop: retain every prior proposal narrowly: (1) direct-Allow/unknown-tag source refutation of
standing-permission or ordinary-card bypass; (2) explicit accepted older-UI unknown-kind list-decoding failure in
SPEC_impl.md Launch templates; neither drops endpoint retargeting. New possible acceptance basis (3): SPEC.md's quoted
logical-entry allowance, with explicit SPEC-over-implementation precedence, could accept this separately established
route if it applies to the container place. No one has accepted any drop or lowering. The machine-named clause remains
contrary evidence, not erased. possible_cover: g020/l1 remains the separate finding owner; this record only reconciles
the assigned added representation and caller assumptions, not a duplicate retarget finding. Caveats: no claim that the
wire alone must carry private endpoint evidence, no claim of observed credential exposure, and no broader approval or
recipe-definition conclusion. All readily checkable source premises resolved; the remaining issue is investigated
contract-scope uncertainty, not an uninvestigated source lead. No runtime tests, builds, reproductions, live-install
actions, checkout edits, coverage searches or nested agents.

Returned proposal (claims-033, event 7): **needs-parent**, suggested bucket **highest**.

The original old-card/standing-permission bypass is refuted, but the record expressly preserves a separate
physical-endpoint authority defect owned by g020/l1. The fields and immediate caller do not bind the physical container
host. Parent must consolidate with that owner rather than discard this record as a blanket authorization refutation.

Original stronger consequence: Unconfirmed billed creation or execution beyond the creation confirmation could be
highest. Latest history adds the independently owned changed-endpoint command/recipe-credential consequence and
explicitly does not withdraw it.

Unresolved premise or limitation: The full cold-place retarget and recipe-credential route is owned by g020/l1 and
supplied by the caller; this consultation confirms the wire/caller boundary only. A private provisioning snapshot could
repair it without changing the wire fields.

Proposed basis: Independent check may drop only the ordinary-card/standing-permission bypass and accept the explicitly
specified unknown-card list refusal. Retain/consolidate the endpoint consequence with g020/l1; it is not accepted by
those clauses.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: split-direct-Allow-gating-refutation-from-pending-place-authority-root. Full033 distinct
LaunchEnvironment tag requires direct Allow, standing permissions cannot bypass and AlwaysAllow refuses; older UI
whole-list decoding refusal is explicitly accepted. These narrow facts do not freeze physical Podman place: the i64/name
is captured before approval, only clone source rechecked later. Preserve supporting evidence under020l1/041l2 and
pending parent010 logical-host-vs-physical-place contract question; no blanket refutation of credentials/commands
reaching a changed endpoint. A private later provisioning snapshot would require its own proof; direct-Allow fields
alone do not provide it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g052 record](records/review-through-00de0b6c/groups/g052.json). Record ID: `c3`.
