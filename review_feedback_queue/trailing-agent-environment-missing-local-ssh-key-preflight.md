# Sandbox agent launches ask for approval before checking the local SSH key

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Sandbox agent launches ask for approval before checking the local SSH key

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g020/l3`.

### g020/l3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 12 (finding, open):

# Sandbox agent launches ask for approval before checking the local SSH key

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435

## TLDR

When Farhelm's local Tensorlake SSH key is missing or unreadable, an agent can still raise an Allow/Deny card for a new
sandbox. Allow can only fail with a prerequisite that was already knowable before asking, although the GUI disables the
same launch. The user must answer an unnecessary approval before receiving the actionable setup error.

## Details

In crates/farhelm-helm/src/agent_requests.rs:4390-4507, launch_environment_for_agent performs its pre-card checks
through provider.places(), RecipeState::require_launch_source and CreateReq::validate_sandbox_launch. None checks the
local SSH key: Tensorlake's places implementation at crates/farhelm-helm/src/tensorlake/hosts.rs:187-192 checks only the
API key; RecipeState::require_launch_source at environment/recipes.rs:259-268 checks only snapshot presence;
sessions.rs:1258-1333 validates destination, launch and prompt. Given an API key and built recipe, the request therefore
reaches approved at agent_requests.rs:4480 even with SshKeyStatus::NotCreated or Unavailable. The first local-key
prerequisite is hosts.rs:205-219 in ensure_prepared, after the approved request starts the owned launch task and
registers the environment row (provisioning/tensorlake_add.rs:346-400, 525-554). That check deterministically refuses a
non-Ready key before any billed preparation/create. Tensorlake::settings reads the absent/unreadable states separately
at tensorlake/mod.rs:298-315, so snapshot presence does not certify key readiness.

The explicit contract is SPEC_impl.md:6683-6685: ‘Before asking, the helm checks the place's cached availability and the
launcher's own environment-launch validation, so the user is never asked to approve a launch that is then refused before
anything starts.’ SPEC.md:1451-1453 also disables sandbox selection for an absent or unreadable local Farhelm SSH key.
This is a promised specific prerequisite guard, rather than a demand to discover whether Tensorlake has registered the
public key (the latter is explicitly not checkable). Apply the provider's existing local-key admission before raising
the agent card, retaining its post-Allow recheck for changes during the wait.

Suggested bucket: other — an unnecessary approval and delayed prerequisite refusal, with no established billing,
data-loss or security consequence. Confidence: confirmed by source and contract analysis; no runtime behavior observed
and no reproduction written. Caveat: requires a built recipe and a missing/unreadable local key (for example after state
recovery loses the separate key files); it does not affect normally configured keys, nor does it start paid work because
the provider's later guard prevents that. Could-be-highest: no. proposed_drop: false. possible_cover: none.

Synthesis turn 2, complete provider-admission contract checked: SPEC_impl.md Disposable environments (6420-6657)
separates cached place metadata from provider preparation/create, requires lazy Podman preparation, and records resource
ownership before work. EnvironmentProvider::places (environment/mod.rs:343-350) reads cached availability and local
account state only; ensure_prepared performs launch preparation later. Provisioning/tensorlake_add.rs:376-381 explicitly
reserves the fresh provider launch check for ensure_prepared after progress ownership. Accordingly the pre-card
validation sentence at SPEC_impl.md:6683-6685 is NOT a guarantee that arbitrary provider I/O or a future state change
cannot refuse an approved launch. This revision limits l3 to an already-absent/unreadable LOCAL key when the card is
raised, with API key and snapshot already present. It does not report an account-side registration failure, a network
failure, a key removed during the approval wait, or deferred preparation itself as bugs. The full Tensorlake
sandbox-session contract at SPEC_impl.md:7076-7134 specifically lists readable local SSH pair as a prerequisite
(7087-7088), and the product rule at SPEC.md:1451-1453 disables this selection when the key is absent/unreadable.
Tensorlake::settings at tensorlake/mod.rs:295-318 is a read-only disk observation, so the missing check can use existing
local state rather than an expensive provider probe. The shipped key guard remains ensure_prepared at
tensorlake/hosts.rs:205-219; places at 187-192 only checks the account API key, and the REST environment admission at
sessions/sandbox.rs:56-91 adds creation consent but no earlier key check. Thus the source-confirmed observable remains
an unnecessary card followed by a known prerequisite refusal, with no paid create. Suggested bucket: other, confidence
confirmed source/contract analysis. proposed_drop: false; no severity lowering needed because original was other. No
runtime behavior observed. Corrected exact source anchor: the card is asked at agent_requests.rs:4493-4515; 4480 in the
earlier detail is an argument to CreateReq, not the approved call.

Returned proposal (claims-010, event 12): **retain-verified**, suggested bucket **other**.

With a built recipe and API key, an already absent local SSH key does not prevent an agent approval card. Allow reaches
the provider's later local-key prerequisite and refuses. This is a source-supported unnecessary approval, with no billed
create or security consequence.

Original stronger consequence: Unnecessary approval followed by a knowable setup refusal. The history never established
billing, lost work or credential disclosure; the latest record expressly excludes network/account-registration failures
and keys removed during the wait.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record as other, source-confirmed and not reproduced, with the already-missing local-state restriction
and later no-billing guard intact.

Existing parent disposition (event 12): verified. Parent explicitly accepted named retain-verified proposal from
claims-010 after inspecting it. With a built recipe and API key, an already absent local SSH key does not prevent an
agent approval card. Allow reaches the provider's later local-key prerequisite and refuses. This is a source-supported
unnecessary approval, with no billed create or security consequence. Record as other, source-confirmed and not
reproduced, with the already-missing local-state restriction and later no-billing guard intact. Checked source/results:
["190d844e58c96cf197aa838d0b0eea8062f48435: agent_requests.rs:4390-4515 checks cached places, source, trust and request
validity before the card, without local-key admission.","Same revision: tensorlake/hosts.rs:187-192 reports only API-key
availability from places; ensure_prepared:205-219 separately requires SshKeyStatus::Ready before billed work.","Same
revision: provisioning/tensorlake_add.rs:346-400 registers the owned task/row after approval and deliberately defers the
provider's fresh launch check. This does not establish that all later preparation refusals are bugs; the retained case
is a known local prerequisite already missing when the card opens.","No runtime behavior was observed. Missing state
after recovery is one stated possible producer; frequency is unmeasured. Unreadable private files that incorrectly still
report Ready belong to a different existing status-detection defect."] Current coverage/results: {"SPEC.md":"Sandbox
sessions:1455-1460 requires an absent/unreadable local key to disable sandbox recipe selection. Account registration
remains explicitly uncheckable.","SPEC_impl.md":"Agent requests and environments:6192-6194 promises pre-card
cached-place/launcher validation. Tensorlake sandbox sessions:6593-6598 lists API key, readable local SSH pair and built
snapshot as prerequisites. No acceptance of this already-known missing local prerequisite found.","TODO.md
Planned":"Empty.","plans/queue":"INDEX.md empty. Retained key-order plans concern SSH identity offering, not this
pre-card check.","BUGS.md":"No matching acceptance.","queue":"unreadable-sandbox-ssh-key-reports-ready.md read in full:
it concerns metadata falsely declaring an unreadable private pair Ready. This record also holds for
NotCreated/Unavailable accurately reported locally and arises because the agent path never checks that state before
asking. Different root; the existing item is not exact coverage.","TRIAGE_OUTCOMES.md":"Targeted local-key/card/approval
searches found no exact decision.","FILTER.md":"Rare-glitch coverage is incomplete: retries still fail until the local
prerequisite is repaired. The unnecessary approval is not merely an imprecise diagnostic. Retain on ambiguity."} Exact
cited cover: [] Original stronger consequence and missing premise remain preserved: "Unnecessary approval followed by a
knowable setup refusal. The history never established billing, lost work or credential disclosure; the latest record
expressly excludes network/account-registration failures and keys removed during the wait."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g020 record](records/review-through-00de0b6c/groups/g020.json). Record ID: `l3`.
