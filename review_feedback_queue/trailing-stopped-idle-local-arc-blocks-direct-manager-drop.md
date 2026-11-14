# Stopped actors retain strong manager references across idle waits

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Stopped actors retain strong manager references across idle waits

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g022/f3`, `g024/l5`, `g026/f8`, `g027/l8`, `g067/l4`.

### g022/f3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

# Stopped actors delay the manager's direct-drop cleanup

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b

## TLDR

Releasing a connection manager without explicitly shutting it down can leave its tasks and unrelated supervisor
connections alive while a stopped environment actor waits. Overlapping idle actors may retain each other indefinitely;
normal graceful helm shutdown explicitly cancels them and is outside this finding.

## Details

Revises local synthesis lead f3; independently inspected the exact source named by g024/l5 event35. manager.rs:3845
initializes a named Option<Arc<ConnectionManager>> by upgrading the actor's Weak reference. :3846-3855 borrows it to
check the place. Its only move at :3864 belongs to a left-to-right short-circuited condition requiring
!stopped.attempted and gate. When either is false, the local remains owned throughout the same arm's idle select loop at
:3873-3892. A named owned local is dropped at the end of that arm, not at its last borrow; the wait can retain the last
strong manager reference. The idle reference is unnecessary after gate calculation.

This prevents the documented direct-drop cleanup from being entered immediately: the manager owns its actor map and
ConnectionManager::drop at :3137-3145 calls shutdown. shutdown/withdraw_actors at :3093-3133 withdraws published clients
and aborts all actors. The module explicitly says actors hold only Weak so the manager/actor map can be dropped (:1161).
Its direct-drop test at :11929-11975 actually drops the manager without explicit shutdown, expects connection closure
and no further dials, but its fixture creates an ordinary SSH host rather than a Stopped environment. Fixture at
:7425-7430 has no cleanup override. Thus the contract is source-visible even though normal serving uses a stronger
explicit shutdown.

Independent graceful-caller checks: lib.rs:2574-2585 always reaches state.manager.shutdown_and_wait after its serving
select, and rest_harness.rs:1007-1010 explicitly calls manager.shutdown before drop. This record does not claim an
ordinary desktop/standalone graceful shutdown hang, process/work loss, or an unbounded leak from one stopped actor
alone. The original conditional indefinite-retention consequence from g024/l5 is retained for source confirmation of the
supported repeated-Stopped route and endpoint/attribution next; the already confirmed narrower consequence is delayed
fallback manager/resource cleanup.

Suggested bucket: other, resource lifetime/API/fixture correctness. Confidence: confirmed source/language analysis for
strong retention across the idle wait; not runtime observed. Could-be-highest:false, no security or user-work-loss route
established. proposed_drop:false. possible_cover:g024/l5 event35 supplied by caller identifies the same mechanism;
preserve both owners for parent consolidation. No tests, builds, runtime reproduction or live action. Fix direction:
confine the place-gate upgrade to a short scope; separately upgrade/move only when an actual owned lifecycle start task
needs it. Preserve that task's legitimate ownership and explicit shutdown.

Reopened synthesis verification completed: the ordinary repeated-Stopped route is supported independently of an alive
place supervisor. At this reviewed revision, environment/podman.rs:1237-1276 inspects the recorded canonical container
and maps exited/stopped/created to Stopped. Its command path at :150-184 resolves a current base route, cached probe and
direct place commands. environment/podman_probe.rs:338-378 returns an older successful probe for the current endpoint
(or refreshes directly), and :409-415 executes bounded commands; neither depends on the connection manager reporting the
place supervisor connected. Two environments at such a place can therefore repeatedly report Stopped while the gate is
false, never spawning a start and leaving the named manager local owned during every idle wait. A second supported
branch is attempted=true after a failed automatic start; it short-circuits before moving the local even when gate is
true.

The actor map really retains all shutdown handles and published clients: manager.rs:1075-1135 owns status, supervising
JoinHandle and AbortOnDrop for each host; HostActor.manager at :3153 is intended to be Weak. If two stopped actors have
ordinary staggered wait boundaries and each finishes its bounded status observation before the other releases its
reference, one strong local remains while the other drops then upgrades again. They can perpetuate manager retention
through repeated 45-second reprobes without a new external owner. This conditional schedule needs no arbitrary suspended
thread or surviving provider create; it is the idle actor/control-flow route itself. With only one idle retained local,
destruction normally becomes possible when that arm ends; no unconditional indefinite single-actor leak is alleged.
Explicit shutdown cancels all these actors even with strong locals, so the confirmed lib.rs and harness shutdown
exclusions remain.

Attribution: the exact parent-to-56659839785691f76da3af5d8f51b511cff4ba8b patch adds self_ref, actor.manager,
StoppedEpisode and this entire upgraded-local/conditional-move/wait arm. This is not attributed to the earlier assigned
ee39 provider-interface refactor itself. Inclusive endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the same
arm at manager.rs:4050-4101, including the local upgrade at4054. No later applied in-range repair. The user explicitly
reopened this group for that late interface evidence; this is an independent agreement record, not a new broad review of
g024 code.

Final agreement: f3 agrees with supplied g024/l5 event35 on the conditional direct-drop fallback/resource consequence,
and excludes the claimed normal graceful shutdown hang as that caller already does. Suggested other bucket, confirmed
conditional source/language/contract confidence, could-be-highest=false, proposed_drop=false and possible_cover=g024/l5
remain unchanged. No independent test-oracle/documentation defect is dismissed by production shutdown correctness, and
no such additional defect is claimed here. No missing readily checkable premise or open local lead remains. Runtime
occurrence/frequency was not measured. No tests, builds, reproduction, external actions or live-install changes.

Returned proposal (claims-129, event 10): **duplicate**, suggested bucket **other**.

This record independently agrees with the canonical idle-local ownership defect. Preserve its source checks as
provenance for g024/l5 rather than create a second queue entry.

Original stronger consequence: The original lead preserved conditional prevention of fallback cleanup; later records
confirm it for overlapping actors while keeping one-actor cleanup delay separate. No highest consequence or withdrawal
appears.

Unresolved premise or limitation: None for the conditional source claim. Runtime incidence/frequency unmeasured;
indefinite retention needs overlapping persistent stopped actors. Normal explicit shutdown, single-actor indefinite
retention, security and user-work loss remain excluded.

Proposed basis: Exact named same-root alias; retain all substance under g024/l5. This proposes deduplication without
rejecting or lowering the canonical defect.

Existing parent disposition (event 10): duplicate. Parent read full primary corroboration and claims129. Exact same
named idle Arc/conditional move/Drop ownership as024l5, same one-actor delay and conditional overlapping-actor
retention, direct local provider status and explicit-shutdown exclusions. Preserve independent original-to-tip and
admitted-row checks under canonical024l5; no residual or stronger consequence lost.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g022 record](records/review-through-00de0b6c/groups/g022.json). Record ID: `f3`.

### g024/l5

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 37 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. TLDR suspected: stopped environment actors can keep the
manager and its other connections alive after the caller releases the manager. Source already inspected: new manager
self_ref and HostActor.manager are Weak, but AttemptOutcome::Stopped upgrades self.manager into a local
Option<Arc<ConnectionManager>>, uses it to compute the place gate, and conditionally moves it only if an automatic start
is admitted. When gate is false or stopped.attempted is already true, that Option can remain owned across the inner
select loop waiting for nudge/reprobe. Two stopped actors with staggered waits could each retain a strong manager
reference while the other reacquires one, preventing the Drop implementation that owns actor shutdown. A single actor
would release at its next wait boundary, so do not claim an unconditional permanent leak from one actor. Readily
checkable missing premises: exact lexical local drop scope under this if-let chain, actual Drop/actor map ownership
contract, a supported source route to two simultaneously gated/already-attempted stopped actors, independent runtime
ownership/shutdown exceptions, whole-range attribution and endpoint persistence. No execution or incidence claimed.
Suggested bucket if established: other for manager/resource lifetime; no security/user-owned-work-loss route
established, could-be-highest:false. proposed_drop:false pending source checks. possible_cover:none in complete
refreshed interface context; prior c6 concerned Arc::get_mut constructor compatibility, not reverse strong retention
during actor waits. Original c1 concerned hold/cancellation deadlock, not manager destruction. No tests, builds,
reproduction, live action or checkout edits. Investigation continues immediately. Immediate source update:
manager.rs:3137-3145 explicitly relies on Drop to stop every spawned actor when a manager is released, and self_ref docs
:1161 state Weak prevents retaining the actor map. ActorHandle owns abort handles and supervisors (:1075-1134), and
shutdown withdraws all clients and aborts every actor (:3069-3134). In the Stopped arm the upgrade is a NAMED local let
binding (:3845), and the only move at :3862-3871 is short-circuited when gate is false or the episode was already
attempted; the enclosing arm includes the wait loop :3873-3892. Official Rust Reference destructors/drop-scopes
(https://doc.rust-lang.org/reference/destructors.html#scopes-of-local-variables) says let locals belong to their
containing block; non-moved manager therefore survives the await, independent of last borrow use. Distinct protective
boundary found and retained: normal serving shutdown explicitly invokes manager.shutdown_and_wait at lib.rs:2584, so do
not claim this prevents ordinary graceful desktop/standalone shutdown. rest_harness restart likewise explicitly shuts
down at :1009. The remaining concern is the documented direct manager-drop fallback and any path relying on it,
including manager fixtures/public embedding; two idle actors can overlap strong holds across reprobes if their status
operations continue. Exact endpoint retains the whole local-upgrade/conditional-move/wait shape at manager.rs:4054-4101.
Pending checks now only supported independent status route, explicit direct-drop callers/fixtures, exact language
short-circuit basis and initial-range attribution. No runtime occurrence/frequency or stronger harm established.
Verified conclusion: the direct manager-drop fallback is broken by idle Stopped actors retaining this local Arc. With
one such actor, releasing the callers last Arc does not immediately withdraw unrelated clients; destruction waits until
the actor leaves that arm. With two persistently Stopped actors whose reprobing intervals overlap, one can retain the
manager while the other drops and reacquires its reference, keeping the manager/actor map and all its other connections
alive indefinitely until explicit shutdown or runtime cancellation. This is a conditional source/control-flow route, not
an observed leak/frequency claim. The unsupported stronger assertion that normal desktop or standalone graceful shutdown
hangs is explicitly NOT made: lib.rs:2555-2585 calls shutdown_and_wait regardless of the select outcome; this cancels
the actors and breaks their holds. rest_harness::restart_with likewise calls shutdown.

The drop semantics are settled by official Rust primary references:
https://doc.rust-lang.org/reference/destructors.html#scopes-of-local-variables associates named let locals with the
containing block; https://doc.rust-lang.org/reference/expressions/if-expr.html#chains-of-conditions specifies
left-to-right short-circuit evaluation. When !stopped.attempted is false or gate is false, the let Some(manager) move is
NOT evaluated. The original initialized local Option<Arc<_>> survives to the end of the Stopped arm, which is after the
select loop. Nonlexical borrow lifetimes do not end this owned value early. The wait defaults to the 45-second reprobe
cadence (SPEC_impl.md:5843-5854), and both start=None and attempted=true are ordinary idle paths. The outer loop reloads
registry/status and can immediately upgrade again while a second actor holds a reference; no invariant synchronizes all
actors to release together. Staggered normal waits and status calls that complete before the other wait ends suffice; no
arbitrary scheduler suspension is needed.

A supported gate-false source route does not require a manager-connected place: two registered Podman containers can be
stopped/exited while their ordinary place has no usable running Farhelm supervisor but still accepts local/SSH Podman
commands. Provider status at environment/podman.rs:1237-1276 inspects canonical identity through place commands and maps
stopped/exited/created to Stopped. place/command at :148-188 uses cached current-endpoint route and direct commands.
environment/podman_probe.rs:338-378 and :409-415 resolve registry/base route and run bounded commands; they never ask
manager.status(place). Therefore status can continue to return Stopped while the actor gate observes its place as not
connected. Multiple environment rows per ordinary place are the supported ownership model, not an invented control
shape. Alternatively already-failed automatic-start episodes retain the Arc even with a connected place. The named local
is neither needed nor moved on these idle paths.

Exact teardown contract and caller evidence: manager.rs:3137-3145 states Drop is what stops spawned actors when the
manager is dropped; :1161 says actors hold only Weak so manager/actor-map shutdown can happen. Existing direct-drop test
at :11929-11978 destructures Fixture and drops manager WITHOUT calling shutdown, waits for a peer closure and asserts no
further dials. It creates an ordinary host only, so does not exercise the new self-retaining Stopped path. Fixture at
:7425-7430 has no Drop override. The actor map owns all cancellation handles/status clients (:1075-1134); only
shutdown/withdraw_actors at :3069-3134 performs the global retirement. The idle actor strong holds stop this destructor
from being reached. This is production manager lifetime/API correctness and a missed fixture domain, not a claim that
runtime destruction cannot ultimately abort tasks.

Attribution/persistence: exact parent-to-566 patch adds Weak self_ref, actor.manager, and the entire Stopped arm/local
upgrade. Immediate parent source search finds no StoppedEpisode/self_ref/actor Weak manager/upgrade; exclusive base also
has no Podman Stopped actor path, so this is not a pre-range lifecycle bug. Assigned
56659839785691f76da3af5d8f51b511cff4ba8b introduces the faulty ownership route. Inclusive endpoint
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db manager.rs:4054-4101 retains it. Fix direction: limit upgrade to a short
gate-calculation scope, and separately upgrade/move only when spawning a real owned lifecycle operation; ensure no
unused strong manager reference survives an idle await. Keep legitimate owned lifecycle tasks alive through settlement
and preserve normal explicit shutdown.

Suggested bucket other (manager/resource lifetime; strongest established effect is retained tasks/store/connections on a
documented direct-drop path). Confidence confirmed conditional source/language/contract analysis; no execution.
Could-be-highest:false; no user process loss, secret exposure or work loss established. proposed_drop:false.
possible_cover:none in refreshed interface context; original c1/c6 concerns remain separate and unchanged. All readily
checkable drop-scope, short-circuit, actor-map, provider-route, direct-drop caller, graceful-shutdown exclusion and
whole-range attribution premises resolved. Two initial podman_probe context guesses were nonexistent paths and yielded
no evidence; git ls-tree found the actual environment/podman_probe.rs path, which was then read. No runtime
test/build/reproduction/live action or checkout modification. Pass-end verification: complete later manager diff hunk
locations were inspected; changes concern Deleting/seal/drain behavior and an unrelated sealed-result weak upgrade, not
this Stopped local or its drop/wait scope. The exact endpoint retains the faulty arm. Exclusive-base targeted source
search, like the immediate-parent search, finds none of this Weak self-reference/StoppedEpisode/manager upgrade
machinery. Continued assigned pass after finding: checked shared lifecycle record/release/retry tail at ba7cc026:90-154,
source-bound replacement validation at 350a4b33 sessions.rs:3960-4065, test provider adapter forwarding/guard lifetime
at 12c79f61 environment/mod.rs:545-680, and initial host projection/request admission. No second independent defect
arose from these checks. Normal graceful teardown exclusion remains confirmed. New verified finding l5 only; no
unresolved source premise/open lead or unverified-after-investigation finding for this pass. No runtime tests, builds or
reproductions. Turn 9 named synthesis, immediately after exact supplied inputs: read g022/f3 event10 and g023/f2 event13
in full. Both independently corroborate this SAME l5 source/language/direct-drop/provider-status/overlap route, and
preserve the explicit graceful-shutdown exclusion. g022 additionally checks both the unsupported one-actor-indefinite
interpretation and the supported multiple-staggered-actor condition. g023 explicitly narrows its older
stranded-lifecycle-hold refutation to perform/release behavior: it does not refute idle manager-Arc retention.
Legitimate owned lifecycle tasks remain distinct from the unused idle local. No new root cause, severity change,
proposed drop or accepted lowering; retain original l5 owner and all original consequences/caveats. Supplied remaining
g026/g027/g067 records will be read before the separate discovery search. Named turn 9 synthesis complete: exact g026/f8
event24, g027/l8 event23 and g067/l4 event20 read in full. g026 independently finds its seal/listing and fixture-field
patches leave the idle scope intact; sealed finisher AppState ownership is not an implicit stopped-actor release. g027
checks the actual Deleting holds, per-host stop_actor and global withdraw_actors: explicit selected-host Delete/hold can
cancel that actor, but ordinary unsealed gate-false Stopped requires none of those operations and losing the external
manager Arc invokes none. No later deletion/seal repair or contradictory premise is established. g067 strengthens the
concrete status premise with two important explicit qualifications: use NORMALLY ADMITTED environment rows retaining
saved remote_farhelm and remote_state_dir, because manager.rs:4185-4197 refuses unadmitted initial setups before
provider status; and direct local-place commands suffice without SSH assumptions. podman_probe current_row reads only
the registry, SystemPlaceCommands::process lowers BaseRoute directly, and access.rs:64-94/434-447 requires an ordinary
Local/SSH route, not a live manager supervisor. SharedConnection::Supervisor chooses an SSH sharing policy and is not
connection admission. This corrects any overly broad reading that a merely registered but unadmitted environment
suffices. Normal provisioned stopped containers retain both coordinates, so the original ordinary route/consequence
stands unchanged. All five requested interfaces independently agree with canonical l5; supporting IDs and
introduction/persistence provenance retained here, no duplicate discovery or parent-approved drop inferred. Normal
explicit serving/REST teardown remains protected, one-actor indefinite leak remains excluded, owned lifecycle work
remains intentionally retained. Other bucket/source-confirmed conditional/no-execution/could-be-highest
false/proposed_drop false unchanged. No readily checkable named premise remains unresolved. Proceeding to separate
discovery beyond this and known l4/matrix.

Returned proposal (claims-129, event 37): **retain-verified**, suggested bucket **other**.

The named strong manager local survives the idle await when the automatic-start condition short-circuits. That prevents
the destructor implementing direct-drop cleanup from running immediately. Two persistent Stopped actors with staggered
waits can keep a reference available for successive upgrades. Normal serving and REST restart explicitly shut the
manager down and remain protected.

Original stronger consequence: From the first event: two staggered stopped actors could keep the manager and its other
connections alive after external release; one actor was explicitly insufficient for an unconditional permanent leak.
Later events establish that same conditional indefinite retention. The normal desktop/standalone graceful-shutdown hang
is expressly identified as unsupported and not claimed; there is no earlier highest-bucket claim or withdrawal in this
named history.

Unresolved premise or limitation: No missing source premise for the conditional resource-lifetime claim. Runtime
incidence, duration and frequency were not measured. Indefinite retention requires at least two persistently Stopped
actors whose ordinary waits/status completions overlap; one idle actor alone only establishes delay until its arm ends.
No established route to secret exposure, user-process loss or other user-owned-work loss.

Proposed basis: Retain one source-confirmed other-bucket finding under canonical g024/l5, carrying all limits and
supporting provenance. No exact cover/filter found. Consolidate only the four same-root supporting IDs, not g023/f2's
original hold-release concern. This is a recording proposal, not a parent decision or independent drop-gate verdict.

Existing parent disposition (event 37): verified. Parent read all late primary histories, direct Stopped/Drop source and
language rules, then full fresh claims129 result. New566 named Option<Arc> survives the conditional skipped move across
idle select; one actor delays direct Drop cleanup, two ordinarily admitted persistent Stopped actors may overlap
status/reprobe waits and retain map/tasks/connections indefinitely. Saved install coordinates and confirmed owned IDs
are required; direct local commands can inspect Stopped independently of manager place connection. Already-attempted
branch separately skips move. Explicit serving shutdown_and_wait and REST restart remain protected. No runtime
incidence, unconditional single-actor leak, normal graceful-shutdown failure, security or user-work loss asserted.
Endpoint/current-main persists; exact coverage/filter searches found no complete match. Retain other resource-lifetime
defect; four same-root aliases and023f2 supporting discussion retained as provenance, not separate roots. Full
coverage/source/caveats claims129.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g024 record](records/review-through-00de0b6c/groups/g024.json). Record ID: `l5`.

### g026/f8

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 24 (candidate, open):

Reviewed interface introduction: 56659839785691f76da3af5d8f51b511cff4ba8b. Assigned manager patches compared:
47f33bbdbd5ff4a6e437e97a800f9083be43096e and f560058da19e02716e6f943883a9508b447aff4e. Endpoint checked:
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. Source finding owner: g024/l5, exact supplied events 31-33 and 35.

## TLDR

The later session-list seal plumbing and fixture field removal leave the stopped actor's strong-manager hold unchanged.
The supplied narrow direct-drop resource-lifetime consequence remains supported; explicit graceful shutdown still
cancels actors. This local record is the requested interface synthesis, not a separately counted production defect.

## Details

Independent pinned-source comparison: introduction manager.rs:3845 upgrades Weak into a named
Option<Arc<ConnectionManager>>; :3862-3864 moves it only after !stopped.attempted and gate both succeed; :3873-3892
waits in the same containing block. At endpoint :4054-4101 the entire ownership/condition/wait shape remains. Official
Rust Reference local-variable drop scopes
(https://doc.rust-lang.org/reference/destructors.html#scopes-of-local-variables) associates a let local with its
containing block; condition chains (https://doc.rust-lang.org/reference/expressions/if-expr.html#chains-of-conditions)
skip later operands after false. Thus a short-circuited condition does not consume this local, and its owned Arc
survives the idle await. The direct-drop contract is explicit at introduction manager.rs:3137-3145; shutdown withdraws
clients, drains the actor map and aborts actors (:3067-3134). Actor handles own cancellation/status/connection resources
(:1075-1140), while the manager self_ref documentation :1161 says actors hold Weak to permit this teardown.

The supported gate-false route was checked independently: environment/podman.rs:1237-1276 inspects recorded owned
containers and maps stopped/exited/created to Stopped. Its command/place helpers (:148-184) use a current route and
cached usable probe; environment/podman_probe.rs:338-378 and :409-415 resolve/run direct commands without requiring
manager.status(place). Therefore a stopped container can be re-observed while its ordinary place's Farhelm connection is
unavailable. One idle actor delays Drop until it releases the local; overlapping stopped actors can keep references
available for successive upgrades, preserving the conditional retained-manager/map/other-connections consequence g024
described. No observed lifetime, frequency or unconditional permanent single-actor leak is claimed.

Caller exclusion independently confirmed: endpoint lib.rs:2581-2591 always reaches manager.shutdown_and_wait after the
serving select; rest_harness.rs:1006-1016 explicitly calls manager.shutdown before dropping it. The direct-drop test at
introduction manager.rs:11929-11975 actually drops manager at :11962 but creates only an ordinary connected host, so it
does not cover this new Stopped branch. Explicit shutdown aborts actors regardless of strong references; ordinary
graceful desktop/standalone shutdown hanging is not established. The later seal finisher (sessions/sandbox.rs:339-360;
started at lib.rs:2521) owns AppState and processes sealed environments, not stopped-actor locals, and supplies no
release or ownership repair for this branch. This comparison makes no separate claim about the finisher's total lifetime
beyond the normal serving/runtime boundary.

Attribution and later integration: immediate parent and exclusive-base searches have no
StoppedEpisode/AttemptOutcome::Stopped/self_ref/self.manager.upgrade machinery. Complete later manager diff hunk
locations show seal state, publication, finisher bookkeeping and tests; the only added manager upgrade is the short
sealed-listing if-let at endpoint :5121. It does not change the older Stopped block. Exact g026 patch 47f33bb only
forwards/destructures sealed and updates the scripted reply; f560058 only removes two SessionInfo fields. Therefore
neither g026 change introduces, repairs or disproves the g024 issue.

Suggested bucket: other, matching the supplied narrow manager/resource-lifetime effect. Could-be-highest: false; no user
work/process loss or secret exposure established. Confidence: confirmed conditional source/language/contract analysis,
not execution. proposed_drop: no new independent g026 report because g024/l5 owns the identical production cause; this
is a local attribution/dedup proposal only, not a rejection or accepted drop of g024/l5. possible_cover: g024/l5
event35. Original consequence/exclusions and the parent's lack of accepted drops are preserved. All readily checkable
premises for this requested interface comparison are settled. No runtime tests, builds, reproductions, installs, nested
agents, checkout modifications or live actions. Revision closes local synthesis lead f8 as a candidate.

Returned proposal (claims-129, event 24): **duplicate**, suggested bucket **other**.

The two assigned later manager patches do not repair the idle local's ownership or wait scope. Its source-confirmed
direct-drop consequence belongs in the canonical finding.

Original stronger consequence: From the first f8 event: one idle stopped actor delays documented direct-drop cleanup;
multiple actors may retain the manager indefinitely through staggered reprobes. Explicit graceful shutdown protected,
resource-only impact. Final candidate preserves this scope and proposes duplicate attribution only.

Unresolved premise or limitation: None for this source persistence/interface conclusion. Runtime incidence remains
unmeasured, indefinite retention requires overlapping persistent stopped actors, and no normal
graceful-shutdown/security/user-work-loss route is established.

Proposed basis: Consolidate this exact same-root interface record into canonical g024/l5, preserving its independent
persistence checks. No correctness rejection, consequence lowering or accepted parent drop.

Existing parent disposition (event 24): duplicate. Full primary and129: original idle-Arc root unchanged by47f33 seal
listing orf560 fixture-field cleanup. Finisher owns state but does not narrow ordinary unsealed Stopped local scope;
explicit shutdown remains protective. Same024l5 defect, retain later-patch/finisher checks and original resource-only
conditional limits as provenance.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g026 record](records/review-through-00de0b6c/groups/g026.json). Record ID: `f8`.

### g027/l8

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 23 (candidate, open):

# Later deletion and withdrawal changes leave canonical g024/l5 intact

Reviewed commit: 222bf6253bcd168db04bcbb1df20180c88306998. Original introducing commit:
56659839785691f76da3af5d8f51b511cff4ba8b. Endpoint checked: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.

## TLDR

Stopped environment actors can keep their manager, tasks and other host connections alive after the caller releases its
last manager reference. One idle actor delays fallback cleanup until it leaves its wait; two persistently stopped actors
with staggered waits can keep each other able to reacquire the manager indefinitely. This is the exact canonical g024/l5
event35 consequence, locally confirmed across the assigned later deletion changes, not an independent new discovery.
Normal serving shutdown and the test restart path explicitly shut the manager down and remain protected.

## Details

Read complete own 19-event history and exact g024 events31-33/35 before revising. At original 566 manager.rs:3845-3893,
let manager = self.manager.upgrade() creates a named Option<Arc<ConnectionManager>>. The gate borrows it; the only move
into an owned start task is conditional on !stopped.attempted and gate. If either is false, that move is not evaluated
and the initialized local survives the idle select loop inside the same arm. Independently consulted official Rust
Reference local-variable drop scopes (https://doc.rust-lang.org/reference/destructors.html#scopes-of-local-variables)
and condition chains (https://doc.rust-lang.org/reference/expressions/if-expr.html#chains-of-conditions): the named
local belongs to its enclosing block; condition operands short-circuit left-to-right. Borrow last-use does not destroy
this owned value early. The original manager self_ref docs :1161 expressly require Weak actor ownership so the
manager/actor map can drop; ActorHandle :1075-1135 owns actor cancellation handles and connections.

Local interaction result: assigned 222 manager.rs:3298-3339 makes shutdown/shutdown_and_wait call withdraw_actors, which
marks terminal shutdown, drains the actor map, withdraws clients and aborts actors. This cancels the idle future and
drops the local Arc WHEN explicitly called. Drop :3342-3350 calls shutdown and cannot be reached while those strong
references survive. Per-host hold_connection :1655-1695 retires that host's client and nudges it to park; release_hold
:1702-1714 clears the hold and nudges again. stop_actor :3243-3259 removes and cancels only the selected actor. These
operations can release that particular wait, but are not automatically invoked by losing the external manager owner. The
new Deleting/seal maps and status/snapshot overlay do not end an idle Stopped arm or call global withdrawal. Their
records contain phase/failure data, not a global lifetime cancellation signal.

The assigned host removal at hosts.rs:1506-1517 explicitly takes a Deleting hold for the selected host, later stopping
its actor only on successful forgetting. Finisher :1450-1471 runs removal for already-sealed machine-deleting
environments. An ordinary unsealed, gate-false Stopped episode requires no Delete, no seal and no connected listing.
Therefore the fact that an actual Delete wakes a selected actor, or a sealed host projects Deleting, does not repair
direct-drop fallback for the reachable ordinary stopped fleet. This interaction is separate from local l4 missing
deletion-outcome writes and from g028/l3 stale captured finisher eligibility; no same-symptom merge or change to those
findings is proposed.

Supported source premise independently checked: original environment/podman.rs:1237-1276 maps an inspected canonical
stopped/exited/created container to Stopped. inspect :226-264 uses provider commands; place/command :148-184 obtains the
current registry/base route through podman_probe.runtime :338-378 and runs direct bounded commands via :409-415,
independently of the place manager connection. Two stopped containers can therefore keep answering Stopped while the
place has no connected Farhelm supervisor, leaving the gate false. Alternatively a completed failed automatic attempt
leaves attempted true. Outer actor run :3643-3693 reloads and probes independently; it has no fleet barrier forcing all
idle manager references to be released together. SPEC.md Container sessions' reboot/start paragraph :4782-4789 and
complete SPEC_impl Environment rows/provider state :6434-6461 and Environment ownership/refusal :6572-6592 specify the
ordinary Stopped/one-start/failure behavior. SPEC_impl connection actor/cadence paragraphs :5809-5854 describe
independent actors and 45-second reprobing. None accepts self-retention across direct manager release.

Protected boundary rechecked in local later source: 222 lib.rs:2568-2592 runs manager.shutdown_and_wait after the
serving select, whether HTTP, token control or shutdown ends serving. Original rest_harness.rs:1007-1010 explicitly
calls manager.shutdown before drop on restart. SPEC_impl serving teardown :7170-7174 requires cancellation/join while
the runtime exists. These paths break the retained references; no normal desktop/standalone graceful-shutdown hang is
established. The direct-drop contract is independently explicit in original manager.rs:11929-11976: its test
destructures Fixture and drops manager without shutdown, then waits for connections to close and checks no more dials.
Fixture :7425-7430 adds no destructor. That ordinary-host fixture misses the new Stopped state. Correct explicit
shutdown does not refute this distinct direct-drop contract or the fixture's limited coverage.

Attribution/persistence: exclusive-base targeted search contains none of the self_ref/StoppedEpisode/Stopped arm
machinery. Original parent-to-566 patch adds the Weak manager, actor field, episode and this arm, confirming an in-range
introduction. Later manager diff hunk locations through the endpoint change Deleting/seal/listing behavior and fixture
metadata, not this arm or Drop/withdrawal scope. Exact endpoint :4050-4102 retains the named upgrade, short-circuit move
and wait. Complete 222-to-endpoint manager diff removes only two obsolete test fixture fields; no later production
repair. Fix direction remains canonical: scope the gate's temporary upgrade to gate calculation, then separately
upgrade/move a manager only for the real owned lifecycle task. Preserve legitimate task settlement and explicit
shutdown.

Suggested bucket: other, manager/resource lifetime. Could-be-highest: false; no secret exposure, loss of user processes
or work-loss route established. Confidence: confirmed conditional source/language/contract analysis, no execution or
incidence measurement. proposed_drop: duplicate-only consolidation with canonical g024/l5 event35, preserving its
original ID, introducing commit, consequences and false proposed_drop; not a correctness rejection, specification
acceptance, parent-approved drop or lowering. possible_cover: exactly the parent-supplied g024/l5 events31-33/35, same
ownership cause. One actor's delay is not described as an unconditional permanent leak; indefinite retention requires
overlapping persistent Stopped actors and status/probe completion before the other releases its reference. Runtime
cancellation or explicit shutdown ultimately releases these tasks. All named readily checkable local source premises
resolved; l8 closes to this duplicate candidate. Existing c1/c2/l3/l4/l5/l6/l7 identities, original consequences and
proposed-drop bases unchanged. No unrelated discovery, forbidden coverage search, nested agents,
tests/builds/reproductions, checkout writes, installs or live actions.

Returned proposal (claims-129, event 23): **duplicate**, suggested bucket **other**.

Global withdrawal cancels idle actors when explicitly called; direct manager release cannot reach that fallback while
the actors retain strong references. Selected-host deletion/holds do not automatically run merely because the external
manager owner disappears.

Original stronger consequence: Original l8 already limits the supplied claim to direct-drop resource retention,
one-actor delay and overlapping-actor indefinite continuation, with explicit shutdown protected. Final duplicate
candidate retains that scope; it is not a withdrawal of the canonical defect.

Unresolved premise or limitation: None for the conditional ownership/withdrawal claim. Runtime occurrence/frequency not
measured. Indefinite retention needs two persistent overlapping stopped actors; explicit shutdown/runtime cancellation
releases them. No security/user-process/user-work loss established.

Proposed basis: Merge exact same-root corroboration into g024/l5 with the later deletion/withdrawal checks preserved. No
rejection or lowering of the retained production finding.

Existing parent disposition (event 23): duplicate. Full primary and129: selected stop_actor/Delete/hold or
sealed-resource finisher do not run merely when external owner disappears; global withdrawal does explicitly cancel but
depends on Drop unless called directly. Same024l5 local-Arc defect, with later withdrawal interaction checks preserved
and explicit serving/REST teardown exclusions.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g027 record](records/review-through-00de0b6c/groups/g027.json). Record ID: `l8`.

### g067/l4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 20 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b; relevant provider/probe route persists through pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: an ordinary container place can keep answering Podman status commands
while its Farhelm supervisor is disconnected. This supports g024/l5's conditional route in which stopped actors keep the
manager and its other connections alive after a caller drops it without explicit shutdown. This is same-root interface
evidence, not a second independent lifetime defect. Details checked directly: podman.rs:1237-1276 obtains a confirmed
ID, inspects the container, validates its session ownership and maps exited/stopped/created to Stopped. inspect at
226-236 invokes exists and container inspect through command at 169-184. place at 148-164 checks only registry
existence, cached/current base route and route equality. Assigned podman_probe.rs runtime at 338-378 reads
current_row/base_route and accepts an older successful current-route probe; command at 409-415 delegates directly to
run_bounded. current_row at 499-505 only reads the store. SystemPlaceCommands::process at 143-165 lowers
BaseRoute::command without any manager status input. access.rs:434-447 admits Local or ordinary SSH rows by
kind/destination; :64-94 executes local argv directly or selects the ordinary SSH sharing policy. The
SharedConnection::Supervisor name chooses an SSH master; it is not admission based on a connected Farhelm supervisor.
Therefore a local place with working Podman and a systemd user manager, but no answering Farhelm supervisor, directly
establishes the required route without any SSH assumption. Existing stopped environments may share that place.

Caller prerequisite checked rather than omitted: manager.rs:4185-4197 refuses an initial unadmitted environment whose
saved remote_farhelm or remote_state_dir is missing. The reported route uses normally provisioned environments retaining
both coordinates. For those, :4198-4219 asks provider.status and maps Stopped before testing whether the place is
manager-connected. The later arm at :3845-3892 upgrades the weak manager into a named local, computes the
place-connected gate, and only moves that local when both !attempted and gate hold. When gate is false it retains the
local across the idle select. Direct inspection of the tip arm at :4050-4101 confirms the same structure. A diff of
podman_probe.rs/access.rs from 566 to tip contains comments only; podman.rs changes only lifecycle lock error handling
outside the status/place/command route. No manager-connection prerequisite was found that refutes g024's provider-route
premise.

Preserved lifetime consequence and limits: g024/l5 events 31-33 and 35 establish staggered stopped actors repeatedly
retaining the manager, its store, tasks and unrelated connections on the documented direct-drop path. One actor can
delay destruction until it leaves the arm; two persistently stopped actors with overlapping waits can retain it
indefinitely while one reacquires a reference held by the other. This is conditional source/control-flow evidence, not
an observed leak or frequency claim. manager.rs:3137-3145 explicitly relies on Drop to call shutdown and stop spawned
actors. The already-attempted idle path is distinct and remains in g024's original record, irrespective of the place
gate. Normal explicit graceful shutdown is protected: lib.rs:2582-2584 stops background services and calls
shutdown_and_wait, and no desktop/standalone graceful-shutdown hang is asserted. Fixing the unused strong reference must
preserve real owned lifecycle tasks through settlement.

Suggested bucket: other (manager/resource lifetime on a direct-drop path). Confidence: source-confirmed provider/caller
premise and independently inspected manager boundary; original lifetime/drop-language analysis supplied by g024. No
runtime observation, test, build, reproduction, installation, live action or checkout change. proposed_drop:false.
possible_cover:g024/l5 event35 is the original same-root owner; consolidate this record as supporting alias, not
duplicate novelty. Could-be-highest:false; no user-work loss, secret exposure or user-process loss established. This
revision resolves all readily checkable premises assigned to g067, adds admitted-installation and tip-persistence
checks, and preserves the original consequences/exclusions. No open lead remains. Earlier g067 findings and
candidate/drop bases are unchanged.

Returned proposal (claims-129, event 20): **duplicate**, suggested bucket **other**.

The provider can report Stopped using direct local commands independently of the place's Farhelm connection. That
establishes a concrete gate-false route for the canonical manager-lifetime defect; it is not a second defect to queue.

Original stronger consequence: Original l4 carries g024's one-actor delay and conditional overlapping-actor retention of
tasks/connections, resource-only other impact. Later events strengthen that same premise rather than lower it. Normal
explicit graceful teardown remains protected throughout.

Unresolved premise or limitation: No missing source premise for the direct-local provider route or conditional lifetime
result. Requires admitted install coordinates, owned existing stopped containers and usable Podman/current route.
Runtime incidence and overlap frequency remain unmeasured; no security or user-owned-work consequence established.

Proposed basis: Consolidate supporting provider-route evidence into canonical g024/l5. Keep the normally admitted-row
and direct-local-place qualifications. This is alias attribution, not correctness rejection, consequence lowering or an
independent drop decision.

Existing parent disposition (event 20): duplicate. Full primary and129 independently establish admitted
remote_farhelm/state-dir coordinates, canonical owned container and usable route/probe prerequisites. Direct local place
commands can report Stopped while place manager supervisor disconnected; SSH sharing policy is not admission. This
corroborates024l5 gate-false premise; same root, no runtime frequency/security/user-work claim.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g067 record](records/review-through-00de0b6c/groups/g067.json). Record ID: `l4`.
