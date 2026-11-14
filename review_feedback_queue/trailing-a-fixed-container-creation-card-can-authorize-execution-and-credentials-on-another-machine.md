# A fixed container-creation card can authorize execution and credentials on another machine

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: A fixed container-creation card can authorize execution and credentials on another machine

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g041/l2`.

### g041/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

# A container-creation card can follow a retargeted SSH place

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435

## TLDR

Allow on an agent's pending container card can create the container, run its command and install the recipe's
credentials on a different physical machine from the one named when the card appeared. Changing that container host's
SSH destination and successfully refreshing its Podman check does not invalidate the card.

## Details

Interface g020/l1 event line 9 identified this route; this group independently checked the approval contract, retarget
admission, current-route dispatch, fresh probe and credential installation sources. The assigned UI patch at
approvals.rs:1093-1104 renders the captured place_name as the container host. agent_requests.rs:4485-4515 captures only
numeric place and display name; approved():2865-2886 checks only the requesting origin after approval. Its clone-source
check at :4519-4528 does not guard a create's destination, and :4541-4548 passes numeric place to
start_session_environment.

hosts.rs:1132-1165 permits SSH retarget under the destination row's locks. store.rs:7566-7602 permits it after
store/podman.rs:215-250 checks existing environments and retained recipe resources; pending approvals are not counted. A
first-use place with no environment rows or retained images/helpers has no such reference.
environment/recipes.rs:259-268 explicitly permits Podman launch without a built artifact. Before Allow the operator can
retarget the place and call the ordinary refresh route (hosts.rs:1665-1671); environment/podman_probe.rs:435-475 reads
the CURRENT row and stores a success bound to its new BaseRoute. A successful probe on replacement machine B is
therefore legitimate fresh availability, not a stale A success.

provisioning/tensorlake_add.rs:357-397 rechecks current provider availability and registers the new environment only
after approval. environment/podman.rs:148-184 resolves CURRENT place route for commands; :975-1033 freshly
prepares/builds at that numeric place, comparing to its current probe rather than the route the card named. The retained
recipe and session command do not pin the outer SSH machine.

Credential consequence is independently established in shipped source: provisioning/tensorlake_add.rs:597-606 calls
delivery.setup for the new row and recipe. provisioning/credential_delivery.rs:347-369 applies recipe bindings; :434-444
resolves the environment's live access route; :575-608 installs the configured Codex auth.json, and :616-652 resolves
and installs the selected GitHub token. These secrets go inside the container on B. Runtime success is not claimed;
source supplies the complete successful path.

Contract: SPEC_impl.md:9548 states exactly 'What a card shows is what an approval does.' :9555-9556 states exactly 'A
launch is also refused if the target host's connection changed while the card waited, since the approval was for the
machine the card named.' SPEC.md:4330-4332 requires the environment card to name where the environment will be created.
SPEC_impl.md:1071-1074 binds answered controls to what was drawn. Mutable settings of the same named recipe, considered
separately under l1, do not authorize replacement of the physical container place.

Suggested bucket: highest, because both execution and configured recipe credentials cross the machine boundary the
pending approval named. Confidence: confirmed by source and contract analysis; no runtime tests or reproduction.
proposed_drop: false. possible_cover: same physical-host route as g020/l1; parent owns deduplication. Could-be-highest:
established shipped credential-installation and execution route, not hypothetical functionality. Caveats: requires a
concurrent operator retarget and successful fresh Podman probe before Allow, plus no environment/image/helper references
blocking retarget; does not require an agent to gain host-edit permission. Original consequence preserved and
strengthened by checked credential path. Capture the physical place route/entry with the planned launch and reject
mismatch under admission coordinated with destination writes, or invalidate the old card and ask again.

Discovery pass 1 interface synthesis: independently checked agent_requests.rs:907-924 and :4182-4213. An ordinary
requester can name a different ordinary container host; resolve_host supplies that separate place and the plan imposes
no equality to origin.host. The concrete retarget sequence requires the requesting session to remain on a DIFFERENT
ordinary host from the retargeted SSH container place, for example a local requesting session. Retargeting only that
place leaves the requester origin fences live; retargeting the origin itself is not the established route. This narrows
the stated premise without changing the original physical-target execution/credential consequence, highest bucket,
confidence or proposed_drop false. This is an update to a known finding, not a new discovery.

Reopened synthesis, normative counterevidence: independently read the complete reviewed SPEC.md:399-409 and pinned-tip
:405-415, together with reviewed SPEC_impl.md:9548-9563 and tip :9090-9105. SPEC.md expressly states: "Approvals,
settings changes and confirmed operations therefore follow the entry." It lists an approved "stop, restart, rename,
create or clone" and says it "may be carried out against the entry's new machine", with no installation-identity check.
Only agent host edit/removal gets its explicit destination exception in that passage. This is physical-host acceptance
counterevidence, not merely l1 recipe mutability, so prior assertions that no physical acceptance exists are superseded.
The competing SPEC_impl sentence still says a launch refuses target-connection change because the approval named the
machine. Source-confirmed retarget/execution/credential route and separate-origin/cold-place/fresh-probe constraints
remain unchanged; normative defect status now requires scope/precedence reconciliation, not a claim that source refutes
this acceptance. possible_cover additionally includes this exact product logical-entry rule; parent has accepted no
drop. Highest consequence retained; proposed_drop false pending remaining contract checks. Next check is documentation
scope and any explicit reconciliation of pending environment creation with logical entry versus physical place. No
runtime observation.

Reopened synthesis conclusion: the documents include an explicit precedence rule, not only competing prose. Reviewed
SPEC_impl.md:3-6 states exactly: "SPEC.md defines what the product does; when this document and SPEC.md disagree about
observable behavior, SPEC.md wins." Reviewed SPEC.md:399-409 explicitly makes approvals follow the logical host entry,
including creates/clones against its new machine; its stated destination exception covers agent host edit/removal, not
environment creation. SPEC.md Concepts defines a Podman place as a registered container host (:48-51), and the
environment approval subsection (:4322-4342) requires naming that host but adds no physical-endpoint or install-identity
exception. The supported sequence preserves that SAME host row while the operator corrects its destination. On that
reading the product clause applies to the container place, and the implementation document sentence promising
target-connection refusal is subordinate conflicting documentation. This is not source refutation of the effect:
execution and recipe credentials on the replacement physical machine remain the confirmed source consequence, with
separate-origin/cold-place/fresh-probe restrictions unchanged.

Latest disposition supersedes earlier blanket confirmed-defect/no-acceptance statements, without erasing them: retain
finding l2 and its highest consequence for independent review; proposed_drop is now the exact explicit SPEC.md
logical-entry acceptance plus SPEC_impl.md precedence rule quoted above, NOT rarity, caller permission speculation or
recipe mutability. possible_cover: reviewed SPEC.md:399-409 and tip :405-415 (the logical-entry retarget acceptance),
together with reviewed SPEC_impl.md:3-6; same route as g020/l1. Confidence: confirmed by source for the physical
behavior; proposed normative acceptance is supported by explicit document precedence and the ordinary registered-host
meaning of place, and remains unaccepted by parent. Suggested bucket if retained as a defect: highest, unchanged.
Could-be-highest remains yes because the shipped credential/execution route is established; acceptance, not harm
reduction, is the proposed-drop basis. Independent reviewer can reject applicability of the logical-entry rule to
environment places, but no accessible source premise is missing and source execution cannot resolve that normative
disagreement. The residual decision is whether this product rule is intended to cover the ordinary host entry when used
as a container place; explicit scope excludes no such use. No lead remains uninvestigated. No
runtime/build/install/reproduction or unrelated discovery performed.

Returned proposal (claims-025, event 9): **retain-verified**, suggested bucket **highest**.

The card retains the numeric container place and its displayed name without binding its SSH route. A separate requesting
host can stay live while an operator retargets an unused destination and successfully refreshes Podman there. Allow then
registers and prepares at that current route, runs the retained command there and can deliver configured credentials.
The source route crosses the physical-machine boundary the card named.

Original stronger consequence: Lines 5-6 raised then established execution on a changed physical SSH machine; lines 8-9
independently added configured Codex/GitHub credential delivery. Line 9 narrows the route to a requester on another
ordinary host so origin fences survive. This is not an agent gaining host-edit authority. Preserve all these caveats and
the highest consequence.

Unresolved premise or limitation: No missing source premise for the stated conditional route. It requires a different
still-live ordinary requesting host, an operator retarget, no environment or retained image/helper references blocking
it, successful fresh Podman probe on B before Allow, and successful downstream setup. Credentials must be configured for
the credential consequence. None was reproduced.

Proposed basis: Retain highest from established conditional wrong-machine execution and credential delivery. Do not use
same-named-recipe mutability as coverage. Any parent deduplication with g020/l1 must preserve the independent source
checks and origin-separation caveat and would need an independent drop check.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g041 record](records/review-through-00de0b6c/groups/g041.json). Record ID: `l2`.
