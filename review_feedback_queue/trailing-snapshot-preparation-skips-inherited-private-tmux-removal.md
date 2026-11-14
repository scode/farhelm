# Snapshot tmux retention follows an explicit preparation exception

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Preserve the original conditional consequence: a prepared clone can retain and select an obsolete
private tmux inherited from a custom base without a supervisor database. Source supports that route, but the governing
specification explicitly exempts snapshot preparation from the removal step in this same commit. The earlier claim of a
confirmed cleanup-contract violation was too strong.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g072/l1`.

### g072/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (candidate, open):

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d TLDR: Preserve the original conditional consequence: a
prepared clone can retain and select an obsolete private tmux inherited from a custom base without a supervisor
database. Source supports that route, but the governing specification explicitly exempts snapshot preparation from the
removal step in this same commit. The earlier claim of a confirmed cleanup-contract violation was too strong. Details:
Original source route remains: snapshot_preparer.rs:87-99 rejects setup-managed installations and a copied supervisor
database, :109-119 deliberately skips RemoveBundledTmux; tensorlake_backend.rs:188-200 checks only supervisor.db;
prepared-clone admission at tensorlake_add.rs:1415-1501 bypasses ordinary installation. The custom-base input premise is
supported by SPEC.md:480-485: "The editor's Software block offers managed Ubuntu, Tensorlake image and snapshot bases,
and a container image with its login user, each stating its supported providers, plus Ubuntu packages for every base.
The custom-base note explains that the base must run systemd as its init system, allow passwordless sudo, be Ubuntu or
Debian with apt, and include bash, curl and git." Thus an inherited binary without database/service is not rejected by
the listed base prerequisites. However SPEC_impl.md:2869-2871 at the reviewed commit explicitly states: "Snapshot
preparation skips the step, as it skips attachment and lingering: the sandbox image it installs into has no Farhelm
installation (preparation refuses setup-managed installs and copied state), so it is not expected to carry an earlier
tmux." This is the specific exception to SPEC.md:183-185: "Every Linux setup and Update plan therefore removes that file
as a step of its own, after the supervisor is started or restarted, and a `tmux` typed in a session afterwards no longer
runs that copy." Snapshot preparation's separate contract at SPEC_impl.md:6617-6623 says: "Preparation reuses ordinary
payload resolution/staging and host-independent Add actions. Host attachment is expressly excluded." The explicit tmux
exception qualifies that general reuse statement. It documents skipping removal, not an executable absence guard. The
expectation in the rationale can be false on a custom base, but that alone does not overturn the stated exception or
establish a separate user-facing guarantee that prepared snapshots remove inherited tmux. History checks: git log -S
'Snapshot preparation skips the step' over the bounded range identifies the reviewed commit itself as introducing that
exception. My previous synthesis event incorrectly described it as a later clarification: it was relocated later, not
first introduced later. At endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db the same text is SPEC_impl.md:8018-8020. No
relevant executor/preparer/copied-state/clone-admission source change occurs after the reviewed commit through that
endpoint; this is no later repair. The retention route and explicit exception coexist from introduction through
endpoint. Suggested bucket: other if retained; no established security/loss-of-work consequence. This is a candidate for
independent drop review, not another novel finding. Confidence: confirmed conditional source route and exact
specification exception; no execution observations. Initial 'confirmed contract violation' conclusion is retracted.
proposed_drop: exact specification acceptance of skipping this step at SPEC_impl.md:2869-2871, quoted above, already
present in the reviewed commit. The available contracts do not promise ordinary setup's precise removal result for
snapshot preparation in the face of that explicit exception. Parent has not accepted this proposed drop; retain this
candidate for independent disposition. Could-be-highest: no established security or lost-work route. Keeping an older
executable is not by itself an established vulnerability. possible_cover: g045/l2, independently reported same
skip/retained private tmux per the parent. Likely duplicate, not separate novelty; no queue/coverage-source search.
Checks actually made: bounded full terminal-substrate contract at the reviewed revision; custom-base prerequisites;
complete prepared-snapshot implementation contract; endpoint source and contract; path-limited history/diff and
exact-text introduction search. Source-only analysis; no tests/reproductions. Caveats: Existing expectation does not
enforce a clean binary directory. A request to strengthen clean-base behavior could be a product change, but no such
request or stronger snapshot promise was found. This proposed drop does not claim the inherited-copy behavior cannot
occur. Discovery-turn interface synthesis: named input g045/l2:15 independently confirms the same exact
SPEC_impl.md:2868-2871 exception. This agrees with the prior l1 source/contract reconciliation: retain conditional
custom inheritance, specific skip exception and pending independent proposed-drop status together. It is duplicate
context only, not a new finding, execution observation or accepted parent disposition. The supplied
discovery-neighbours.json was read fully in bounded replacement chunks after the first broad read clipped; it contains
no additional assigned-group question changing this conclusion. New discovery search begins after this synthesis.

Returned proposal (claims-046, event 12): **covered**, suggested bucket **other**.

The conditional retention route is possible, but the exact specification explicitly exempts snapshot preparation from
removal. The initial assertion of a confirmed cleanup-contract violation was withdrawn; this is not a claim that
inherited tmux is impossible.

Original stronger consequence: Prepared clones can retain and select an obsolete private tmux inherited from a custom
base without supervisor state. Earlier events called this a confirmed setup-cleanup violation; that conclusion was
retracted after finding the same-commit explicit exception. No security or work-loss route was claimed.

Unresolved premise or limitation: No stronger promise that preparation removes inherited tmux was found. The clean-base
rationale is an expectation, not an exhaustive guard.

Proposed basis: Exact specification coverage of skipping removal. Preserve the conditional retained-binary consequence
and withdrawal history; do not replace them with a false clean-base guarantee or claim an accepted parent drop.

Existing parent disposition (event 12): excluded. Parent explicitly accepted named covered proposal from claims-046
after inspecting it. The conditional retention route is possible, but the exact specification explicitly exempts
snapshot preparation from removal. The initial assertion of a confirmed cleanup-contract violation was withdrawn; this
is not a claim that inherited tmux is impossible. Exact specification coverage of skipping removal. Preserve the
conditional retained-binary consequence and withdrawal history; do not replace them with a false clean-base guarantee or
claim an accepted parent drop. Checked source/results:
["500548b9253b07a26fd03119679184b69544e31d:crates/farhelm-helm/src/provisioning/snapshot_preparer.rs:87-99 rejects
setup-managed installs and copied state; 109-119 deliberately skips RemoveBundledTmux. installation.rs:53-58 records the
same expectation.","Same revision: tensorlake_backend.rs:188-198 checks only supervisor.db absence;
tensorlake_add.rs:1415-1448,1473-1503 admits prepared facts and attaches without ordinary installation. A base with an
inherited private tmux and no matching database/service is not excluded by these guards.","Same revision
SPEC_impl.md:2854-2871 explicitly describes the skip. Current SPEC_impl.md:8003-8020 retains it. The inspected
preparer/executor/copied-state/clone-admission files have no diff from the reviewed commit to
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. The exception existed at introduction; it was not a later repair.","No
execution. Retention and unexpected program selection are conditional source consequences; no vulnerability or lost work
follows merely from an old executable's presence."] Current coverage/results: {"SPEC.md":"Ordinary Linux setup/Update
cleanup at 181-188 is general; custom bases are supported. This does not negate the specific preparation
exception.","SPEC_impl.md":"Exact current coverage at 8018-8020 and reviewed coverage at 2869-2871.","TODO.md
Planned":"Empty.","plans/queue":"No unlanded plans in INDEX.md; historical prepared-snapshot and tmux plans supply
context, not future fix coverage.","BUGS.md":"No match.","queue and active ledger":"Searched snapshot preparation,
inherited/private tmux and removal. No exact open queue item or active outcome. Parent-supplied possible g045/l2 alias
was not independently inspected because it is outside this batch."} Exact cited cover: ["SPEC_impl.md:8018-8020:
'Snapshot preparation skips the step, as it skips attachment and lingering: the sandbox image it installs into has no
Farhelm installation (preparation refuses setup-managed installs and copied state), so it is not expected to carry an
earlier tmux.'"] Original stronger consequence and missing premise remain preserved: "Prepared clones can retain and
select an obsolete private tmux inherited from a custom base without supervisor state. Earlier events called this a
confirmed setup-cleanup violation; that conclusion was retracted after finding the same-commit explicit exception. No
security or work-loss route was claimed."; "No stronger promise that preparation removes inherited tmux was found. The
clean-base rationale is an expectation, not an exhaustive guard.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g072 record](records/review-through-00de0b6c/groups/g072.json). Record ID: `l1`.
