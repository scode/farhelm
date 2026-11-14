# Recipe-template refusals still claim agent use is GUI-only

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: a refused recipe-template edit or deletion still tells the agent that recipe templates
can only be used in the GUI, although the assigned feature enables ordinary agent create to use a named recipe template
with approval. This may be stale user-facing restriction guidance, independent of correct refusal to edit or delete
those templates. Source checked so far: 350a4b3 agent_requests.rs:3420-3425 documents GUI-only use pending billing
design and returns Recipe templates can only be used and edited in Farhelms GUI. The 190d844e assigned test retains that
exact diagnostic for TemplateDelete and TemplateEdit at 14901-14919, while the new
environment_launches_always_ask_and_deny_creates_nothing fixture explicitly exercises the plain-create recipe-template
case and gets its direct-Allow card. Need check the current helper and every caller, complete governing product section,
exclusive base and introducing feature diff, and endpoint persistence before concluding an in-range documentation
defect. Suggested bucket if confirmed: other (usage/diagnostic accuracy); Could-be-highest: false, no unintended
mutation or work loss is alleged. Confidence: source-supported lead, exact scope and attribution still being checked.
proposed_drop: false. possible_cover: none in supplied neighbour records; local f2 concerns false Placement catalog
documentation and obsolete helper links, rather than this user-visible recipe-template restriction. No runtime
observation, test, reproduction or build. Verified source conclusion, discovery pass 4: at 190d844e
agent_requests.rs:3734-3739, the helper still

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g018/f9`.

### g018/f9

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 29 (finding, open):

Reviewed feature commit: 190d844e58c96cf197aa838d0b0eea8062f48435; assigned earlier filter commit:
350a4b33b6d4476f4a78e43cea124bc2b828f462. TLDR: a refused recipe-template edit or deletion still tells the agent that
recipe templates can only be used in the GUI, although the assigned feature enables ordinary agent create to use a named
recipe template with approval. This may be stale user-facing restriction guidance, independent of correct refusal to
edit or delete those templates. Source checked so far: 350a4b3 agent_requests.rs:3420-3425 documents GUI-only use
pending billing design and returns Recipe templates can only be used and edited in Farhelms GUI. The 190d844e assigned
test retains that exact diagnostic for TemplateDelete and TemplateEdit at 14901-14919, while the new
environment_launches_always_ask_and_deny_creates_nothing fixture explicitly exercises the plain-create recipe-template
case and gets its direct-Allow card. Need check the current helper and every caller, complete governing product section,
exclusive base and introducing feature diff, and endpoint persistence before concluding an in-range documentation
defect. Suggested bucket if confirmed: other (usage/diagnostic accuracy); Could-be-highest: false, no unintended
mutation or work loss is alleged. Confidence: source-supported lead, exact scope and attribution still being checked.
proposed_drop: false. possible_cover: none in supplied neighbour records; local f2 concerns false Placement catalog
documentation and obsolete helper links, rather than this user-visible recipe-template restriction. No runtime
observation, test, reproduction or build.

Verified source conclusion, discovery pass 4: at 190d844e agent_requests.rs:3734-3739, the helper still documents Recipe
templates stay GUI-only until agent-requested billing is designed and returns the user-facing claim Recipe templates can
only be used and edited in Farhelm’s GUI. This is materially false for use: complete SPEC.md Agents and environments,
4289-4342, explicitly permits a recipe-target template through --template at 4324-4325, always behind its own Allow/Deny
creation card at 4330-4335. Complete SPEC_impl.md Agent requests and environments, 6658-6702, says a plain create with
no --host whose template targets a recipe takes the same launch path at 6680. The assigned new Deny test exercises that
form and obtains LaunchEnvironment before Deny; correct execution establishes the stale diagnostic contradiction rather
than refuting it. The helper is used by write_template_for_agent on supplied recipe/target fields (3171), and
find_template refuses existing recipe templates (3725-3729), reached by template write/edit at 3207 and delete at 3325.
Thus an ordinary agent editing/deleting an existing recipe template receives an overbroad GUI-only-use restriction, even
though only the template-management action should be refused. Existing test at 14915-14919 asserts this stale text, so
update that expectation with the helper wording. The current edit/delete prohibition remains valid; no additional
authorization or runtime dispatch defect is alleged. Suggested fix: distinguish template management from launching by a
known recipe-target template, and update the obsolete pending-billing rationale. Suggested bucket: other (user-facing
diagnostic and internal contract accuracy). Confidence: confirmed source and full-section contract contradiction; no
runtime observation. Could-be-highest: false; no security or lost-work route. proposed_drop: false. possible_cover: none
among supplied latest interface records; local f2 is separate Placement/helper-link documentation. Lead is resolved as
this finding. Remaining checks for this record: exclusive-base/parent semantics and exact feature-diff attribution,
caller endpoint persistence; these can refine provenance without changing the confirmed reviewed-revision contradiction.
All prior consequences/citations retained; no parent drop/lowering accepted.

Full-range attribution and persistence resolved: the exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a already
contains this message/comment at agent_requests.rs:3417-3422. At that revision the only create resolver supplies
known_recipes:&[] (agent_launch.rs:167-172), so recipe-template use is not yet the new supported feature. The 190d844e
immediate parent likewise uses the restricted resolver at agent_requests.rs:2372-2379 and retains the message at
3633-3638. The exact parent-to-child feature diff adds the hostless non-spawn recipe-template branch at child
agent_requests.rs:1991-2008, calling plan_environment_create and launch_environment_for_agent, and replaces ordinary
resolver inputs with recipe-aware resolve_placed at 2437 onward. Its plan explicitly loads the same saved templates at
4215-4228 and recipes at 4229-4235; the assigned new test exercises this same supported form. Thus semantic attribution
is 190d844e making a previously true use restriction false, not a claim that it newly introduced the literal. Scoped
whole-range -G history identifies only 12c79f6 confinement changes and this 190d844e feature among changes touching the
helper/callpath tokens. Pinned endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the helper comment/message at
3735-3740, both production refusal callers at 3172/3730, and the stale assertion at 14924; no later in-range repair.
This is a current user-visible refusal-message defect plus its obsolete internal rationale, not the existing f2
catalog/helper-link issue, and not a claim that the valid management prohibition should be lifted. All readily checkable
source premises for f9 are settled. New verified finding f9, other, proposed_drop false; no unverified premise or open
lead remains. No runtime tests/builds/reproductions or coverage-source search.

Final contract/caller check: read complete reviewed SPEC.md Sandbox sessions (1441-1568), plus agent
template-management/discovery paragraphs (4197-4227). SPEC.md:1563-1564 explicitly limits GUI-only to listing and
writing and states that farhelm agent create --template may launch one from an ordinary host. This directly settles the
use-versus-management premise; it is not inferred only from source behavior. Current production write preflight at
agent_requests.rs:3170-3171 and existing-definition lookup at 3207, and delete lookup at 3325, confirm the diagnostic
reaches real management requests before approval. Correct classification and refusal remain intended. The stale test
assertion is corroboration/required wording maintenance, not a separate ineffective-test finding. No open source premise
remains.

Returned proposal (claims-127, event 29): **retain-verified**, suggested bucket **other**.

An ordinary agent editing or deleting a saved recipe template receives a refusal saying recipe templates can only be
used and edited in the GUI. Management refusal is intended, but the use restriction contradicts the supported create
--template launch path. The obsolete pending-billing comment and matching test assertion are the same wording defect,
not separate findings. This is diagnostic and contract accuracy; no authorization failure or loss of user work is
alleged.

Original stronger consequence: None. From event 26 onward the proposed consequence is inaccurate user-facing restriction
guidance, independent of the valid edit/delete refusal. All events expressly deny an authorization or user-work-loss
allegation; event 29 treats the stale assertion as corroboration rather than a separate test defect.

Unresolved premise or limitation: None for the narrow source-confirmed misleading-refusal claim. No runtime observation,
frequency measurement, agent reliance, security impact, or lost-work consequence was established or claimed. No
reproductions, tests, builds, process actions, live-install operations, checkout writes, or VCS mutations were
performed.

Proposed basis: Retain one verified other-priority finding for the overbroad recipe-template-management refusal and
obsolete rationale. Preserve intentional management restrictions and approval requirements. Correct wording and its
existing assertion, rather than lifting the restriction. No coverage, filtering, refutation, withdrawal, consequence
lowering, or final parent decision is proposed.

Existing parent disposition (event 29): verified. Parentreadallprimaryevents26-29/direct190dhelper3734-39/SPEC1554-68,
thenfullfreshfocused127result2092tokens. Confirms190d addsordinaryhostcreate--template
recipeenvironmentlaunch/directAllowwhileexistingedit/deletepreapprovalrefusalstillclaimsGUI-onlyUSEandpendingbillingrationale;managementrestrictioncorrect.
Base/reviewedparentrestrictedrecipe resolvermakessemanticcontradictionnewinrange, tip/mainstillwrong.
CurrentfullSPECsectionsconfirmuse-vsmanagement, Plannedempty, exactplansalreadylanded, BUGSunrelated,
INDEX+matchingRestarttemplateplanitemdifferent, activeledgernoexactcover. FILTERrare-diagnosticrequiresraretrigger;
ordinaryshippededit/delete deterministicdoesnotqualify. Oneotherdiagnostic/sourcewordingitem includingtestexpectation,
notauthorizationfailure/testoracle/newsourcebehaviorchange. No premise/runtimeobservation/security/worklossclaim.
Preservefullsource/coverage/caveats in claims127result forqueuewriting.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g018 record](records/review-through-00de0b6c/groups/g018.json). Record ID: `f9`.
