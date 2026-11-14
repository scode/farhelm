# Migration can remove moved checkout work through an unprotected live spare claim

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Considered whether the new per-launch uncertainty wording masks cleanup deleting a claim still
being used to prepare a checkout after supervisor restart. The original suspected consequence was recursive removal of a
launch's claimed tree while it was in use (could-be-highest: true for loss of user-owned work if such a route existed).
Source refutation: the assigned patch changes only comments. LiveClaims::protects at spare_checkouts.rs:45-50 retains
the global uncertain flag OR the exact (session,generation) pair, and sweep_namespace at :542-549 skips protected
claimed names before discard_child. The startup caller service/core.rs:7822-7896 derives exact-generation protection
from terminal-host evidence, treating unattributed observations, missing/unreadable provenance, and missing/unreadable
preparation state conservatively; NotStarted and CloneStarted are both protected. repo_cache.rs:120-123 skips a held
cache flock before any trash or spare cleanup, matching its changed terminal-host comment at :84-91. Contract:
SPEC_impl.md, Owned checkout admission and lifetime, :3966-3974 states 'failed or uncertain liveness observations retain
possible claims rather than inferring abandonment.' Suggested bucket if the suspected destruction route existed:
highest; current confidence confirmed source-refuted for the changed comments and their direct consumers. proposed_drop:
code preserves both global and exact-generation claim protection and no behavior is changed by either assigned comment
patch. possible_cover: none. Caveats: source-only; no execution observed, and this does not in

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g096/c2`.

### g096/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Considered whether the new per-launch uncertainty
wording masks cleanup deleting a claim still being used to prepare a checkout after supervisor restart. The original
suspected consequence was recursive removal of a launch's claimed tree while it was in use (could-be-highest: true for
loss of user-owned work if such a route existed). Source refutation: the assigned patch changes only comments.
LiveClaims::protects at spare_checkouts.rs:45-50 retains the global uncertain flag OR the exact (session,generation)
pair, and sweep_namespace at :542-549 skips protected claimed names before discard_child. The startup caller
service/core.rs:7822-7896 derives exact-generation protection from terminal-host evidence, treating unattributed
observations, missing/unreadable provenance, and missing/unreadable preparation state conservatively; NotStarted and
CloneStarted are both protected. repo_cache.rs:120-123 skips a held cache flock before any trash or spare cleanup,
matching its changed terminal-host comment at :84-91. Contract: SPEC_impl.md, Owned checkout admission and lifetime,
:3966-3974 states 'failed or uncertain liveness observations retain possible claims rather than inferring abandonment.'
Suggested bucket if the suspected destruction route existed: highest; current confidence confirmed source-refuted for
the changed comments and their direct consumers. proposed_drop: code preserves both global and exact-generation claim
protection and no behavior is changed by either assigned comment patch. possible_cover: none. Caveats: source-only; no
execution observed, and this does not independently certify all terminal-host liveness implementation owned by adjacent
groups. Discovery turn 3 cross-group synthesis: parent-supplied latest g111/l1 establishes a new check against the
original c2 safety hypothesis: core.rs:7843 treats an old terminal-kind row as not live after migration, including
migration cleanup failure; claim taking does not hold the repository-cache flock. g111 reports that the confined
recursive remover can retain a child handle while the take moves that child into the allocation, where an
already-running host-backed borrower can write user work. The old c2 conclusion remains valid ONLY for the comment-only
nature of the assigned patches and for LiveClaims::protects on pairs actually inserted; it is not a refutation of
omitted old-kind pairs or of moved-child traversal. Could-be-highest remains true, with original deletion/work-loss
consequence preserved. Current state: substantive cross-interface lead pending direct source check; no final
source-refutation or accepted drop for that migration route. Need check exact old-kind exclusion, remover ordering, take
ordering, borrower lifetime, and full-range attribution. proposed_drop for the original comment-regression theory is
retained as an attribution-only basis, not a drop of the migration interaction. possible_cover: g111/l1 is the
parent-supplied owner of this migration/removal route; no independent coverage sources searched. No execution evidence.
Direct-source discovery result: the previously source-refuted blanket safety conclusion is revised. At
1ed482dde220452e424973faf38a5e1318986333 core.rs:7837-7866 skips protection for a row with no in-memory terminal and
stored TerminalKind::Tmux, without observing migration success. old_terminals.rs:306-318 logs failed reap, and :351-356
proceeds after its deadline. spare_take.rs:38-40 takes no cache lock; :156-181 moves children by the retained
claim-directory handle into the allocated directory, then unlinks the old claim name, so parking the claim cannot
exclude those child moves. spare_checkouts.rs:542-556 selects an unprotected claim and :611-618 parks then opens/removes
it. checkout_trash.rs:320-349 retains opened child directory handles and traverses them; :351-353 deletes files through
those handles. Its source-name identity check occurs only after traversal (:374-380), so it can detect a moved name too
late to restore already-deleted files. An already-running host-backed borrower is a supported independent writer:
store.rs:3389-3393 attaches existing managed ancestors, core.rs:12202-12217 restricts Ready to the original preparation
and explicitly permits borrowers to use incomplete contents, and SPEC.md:3013 preserves sessions already in terminal
hosts through migration. Therefore a child opened before the old take moves it can be recursively deleted after it
reaches the allocation, including new work that borrower writes there. The origin itself fails on unlink of the parked
claim (:176-181), so this is not a claim that its own later merge/hook/agent writes lost work. Current TLDR: a failed
attempt to end an old checkout preparation can let startup delete work in that checkout even while a separate retained
session uses it. Current suggested bucket: highest, for the established conditional source route to loss of
borrower-owned work. Current confidence: confirmed conditional concurrency/source route, not runtime observed.
Could-be-highest remains yes. Current proposed_drop: false for this route; retain the original
comment-only/unchanged-predicate proposed drop solely as attribution of the assigned comment patch, not as refutation of
the interaction. Exact competing contracts: SPEC.md:1749-1752 says Startup removes leftover building and claimed
directories except claims still in a live launch's clone stage; SPEC_impl.md:3966-3968 says failed or uncertain liveness
observations retain possible claims rather than inferring abandonment. SPEC.md:3014-3016 says failure to end the server
or stop what ran there never stops startup, those sessions are treated as ended, accepting a second agent on Restart.
That general migration fallback does not expressly authorize deleting retained host-backed borrower work. The :1753-1755
out-of-band/replaced-spare acceptance is not a direct cover for a legitimate live claim and moved allocation contents.
Independent contract judgment remains the parent's; no parent drop accepted. possible_cover: parent-supplied g111/l1 is
the same route and owns the detailed migration attribution; this revision corroborates rather than creates a duplicate
novel issue. Full-range attribution is being completed next; no runtime action. Attribution check complete: git log over
the entire exclusive 6e423e30068b62c6cfa276c6d720ee35bb70387a through inclusive 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db
range shows only 04d85f0c and 1ed482d modify the four assigned paths. However dbfc69751215dbf15e1d31d5bed73c3754c4ebc4
is within the complete reviewed commit list and introduces old-terminal migration plus unconditional
tmux.no_server_after_startup (its core.rs diff). At dbfc core.rs:8126-8137 pane collection still claims conservative
handling for errors, but tmux.rs:3041-3042 forces presence absent; its presence module explicitly says queries answer
absent without spawning anything. pane_states_checked at :4907-4923 returns an empty map for that synthetic definitive
absence, and core.rs:8173-8188 therefore omits old claim protection already in dbfc, before 1ed482d rewrites it as
TerminalKind::Tmux => false. The dbfc parent core.rs:6428-6433 instead adopts an existing server. A base-revision source
search finds no OldTerminals/no_server_after_startup mechanism. Thus this is an in-range migration interaction carried
through the assigned comment revisions, not proved out of range by predating 1ed482d, and the comment-only attribution
basis cannot dispose of the underlying route. Same-ID source-check lead is closed as this retained finding; no readily
checkable premise remains open. Source-only corroboration of parent-owned g111/l1; exact scheduling remains a
conditional interleaving, not a runtime occurrence. Highest consequence, original rejection history, all specification
text and no-accepted-drop caveat are preserved. Discovery pass completion: rechecked all assigned deltas and their
unchanged identity/predicate boundaries after recording this route. No separate account-selection, managed-root removal,
cache-comment, or LiveClaims-predicate defect was established. No additional local IDs were opened. This finding is a
newly source-corroborated revision of c2, matching g111/l1, rather than a new independent duplicate. No open source lead
remains.

Returned proposal (claims-063, event 9): **filtered**, suggested bucket **highest**.

The latest conditional source route is established, including possible borrower-owned work loss; the earlier blanket
safety refutation is invalid. The entire named trigger also fits the current upgrade-plus-rare-condition filter: an old
tmux preparation survives from an earlier release, migration fails to end it, and its child move races startup removal
while a retained borrower uses the allocation. The filter expressly includes affected-session work loss. This is a
proposed filter disposition requiring an independent drop check, not an accepted drop. The reported g111/l1 alias was
not supplied here and is left for parent reconciliation.

Original stronger consequence: Initially suspected recursive removal of a launch's in-use claimed tree and proposed a
comment-only/direct-predicate refutation. Later events withdrew that blanket safety conclusion: omitted old-kind
protection permits deletion through a child handle after the child reaches an allocation, potentially losing work
written by a retained host-backed borrower. The origin's own later agent/hook work is not the demonstrated writer
because its take fails on the parked claim's old-name unlink. Highest remains appropriate before any approved filter
drop; rarity does not lower it.

Unresolved premise or limitation: No missing source premise remains for the conditional interleaving described, and no
runtime occurrence or frequency is established. Filter agreement must be obtained independently because it would dismiss
an established possible loss of user-owned work. The reported same-root g111/l1 is not in this supplied batch.

Proposed basis: Propose the current FILTER.md upgrade-plus-another-rare-condition rule for the complete named migration
route. Keep highest and the full conditional loss mechanism in the parent's drop register. The assigned comment-only
attribution is true but is not a valid out-of-range or source-refutation basis; dbfc introduces the interaction within
the full review range.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: upgrade-plus-rare-filter-for-established-moved-borrower-tree-loss. Full063 conditional
highest loss route established: failed old-terminal migration omits live claim protection, cache cleanup parks/opens
then recursively deletes via child handle while un-locked take moves child into allocation, and retained host-backed
borrower writes there. Origin itself fails old-name unlink and is NOT the later writer. Comment-only1ed attribution is
not full-range exclusion: in-range dbfc introduced old-pane liveness omission. Ordinary migration fallback acceptance
does not accept borrower deletion; no other exact cover. Proposed filter needs older-release preparation plus failed
ending/move-removal interleaving and affected-session work only; gate must explicitly judge borrower/affected-session
scope, no security/DB/bricked-install consequence. Keep highest and all corrected source premises until clear agreement,
reconcile111l1 separately. No observed timing/loss/frequency or generalized live-claim safety.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g096 record](records/review-through-00de0b6c/groups/g096.json). Record ID: `c2`.
