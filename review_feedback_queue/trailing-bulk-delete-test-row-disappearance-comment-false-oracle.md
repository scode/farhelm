# Bulk Delete test comment promises a reply-completion oracle it does not have

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the new bulk Delete test tells future maintainers that a row can disappear only after
that row's mocked reply is handled. A listing refresh can remove the second row sooner, so that explanation overstates
what its row-disappearance waits prove. This is a narrow test-documentation defect; the separate notice assertion
remains meaningful.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g029/f2`.

### g029/f2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 5 (finding, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa. TLDR: the new bulk Delete test tells future maintainers that
a row can disappear only after that row's mocked reply is handled. A listing refresh can remove the second row sooner,
so that explanation overstates what its row-disappearance waits prove. This is a narrow test-documentation defect; the
separate notice assertion remains meaningful. Details: newly added lines 959–962 state 'Each row leaves only when its
own reply has been handled' and infer both reply effects from both rows being gone. The same patch's environmentDeletes
at 719–725 instead records the request and calls cleanupSession(request,id) before fulfilling its intercepted browser
reply. fleet.ts:576–590 stops/deletes through the real API; the helm's ordinary delete contract at sessions.rs:3607–3611
forgets the row before answering. After handling the FIRST mock reply, view.rs:3133 requests an explicit refresh, then
lifecycle.rs:122–153 can poll the second queued Delete while that independent read is outstanding. A refresh that
snapshots after the second real cleanup but commits before its mock reply is handled no longer lists the second row.
commit_listing at view.rs:1935–1953 accepts a current same-query reply and at 2310 replaces the listing directly; its
absence reconciliation at 2236–2249 preserves pending OPERATION state but does not reinsert the absent row. No
pending-Delete row-retention gate exists in that commit path. Thus the comment's 'only' guarantee is false even with the
event feed stubbed. Narrow the comment to the independent persistent-notice witness, or explicitly gate response
completion if row disappearance itself is intended as that witness. Contract: root AGENTS.md says comments teach future
readers the constraints/invariants that survive refactors; the supplied test checklist requires observables to
distinguish the competing mechanisms. Attribution: git show of 2ef10815 adds these exact comment lines; git log of the
full pinned range identifies this as the final change to this file, and git diff from this commit through
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db is empty. Suggested bucket: other (misleading maintainer test documentation).
Could-be-highest: no established security or user-work-loss route. Confidence: confirmed source/ordering contradiction,
not an observed run; actual incidence was not measured. proposed_drop: false. possible_cover: none; no prohibited
coverage sources searched. Caveats: preserve f1's original refutation—the first count is published before the second
request, the host remains listed and non-deleting, and the final .delete-notice absence assertion waits for that count's
retirement. This finding does not claim the existing test false-passes or that product Delete is incorrect. It
supersedes only any prior implication that a sound final assertion excuses a separately false explanatory comment. No
reproduction or runtime tests.

Returned proposal (claims-016, event 5): **retain-verified**, suggested bucket **other**.

The new comment promises that each row leaves only after its own reply is handled. A refresh caused by the first success
can observe the second real cleanup before the second mock browser reply is handled. The final notice assertion remains
meaningful; this is a false explanation of its row waits.

Original stronger consequence: The named f2 history consistently confines the claim to misleading maintainer
documentation. f1's earlier suspected false pass is explicitly preserved as refuted and is not silently adopted here.

Unresolved premise or limitation: None for the source/ordering contradiction. Actual incidence was not measured. No
assertion ineffectiveness or product failure is inferred.

Proposed basis: Record the narrow comment correction. Explain the persistent notice as the completion witness, or add
explicit response-completion gates if the row waits themselves are meant to establish that fact.

Existing parent disposition (event 5): verified. Parent explicitly accepted named retain-verified proposal from
claims-016 after inspecting it. The new comment promises that each row leaves only after its own reply is handled. A
refresh caused by the first success can observe the second real cleanup before the second mock browser reply is handled.
The final notice assertion remains meaningful; this is a false explanation of its row waits. Record the narrow comment
correction. Explain the persistent notice as the completion witness, or add explicit response-completion gates if the
row waits themselves are meant to establish that fact. Checked source/results:
["2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa:e2e/tests/tensorlake-sandbox-sessions.spec.ts:959-962 adds the 'only when its
own reply has been handled' explanation. environmentDeletes:719-725 awaits real cleanupSession before
route.fulfill.","Same revision:e2e/tests/helpers/fleet.ts:576-589 stops/deletes via the real API.
crates/farhelm-helm/src/sessions.rs:3603-3607 states successful deletion forgets the helm row before answering.
list/view.rs:3068-3133 removes the first row and requests an explicit refresh; list/lifecycle.rs:123-150 then allows the
second Delete while that independent refresh is outstanding.","Same revision:list/view.rs:2236-2249 preserves held
operation state during absence reconciliation but does not restore missing listing rows; :2310 replaces the listing with
the fetched result. A current accepted read after second real deletion can therefore remove its row before mock-reply
handling. No pending-Delete row-retention gate appears in this commit path.","git diff 2ef10815..00de0b6 for this spec
is empty. The comment defect is in range and persists. g029/f1's first-count publication and final persistent-notice
witness remain valid; no existing-test false pass or product Delete error is asserted."] Current coverage/results:
["SPEC.md:1524-1529,1543-1548 and SPEC_impl.md's Environment Delete and Testing sections do not accept the false
row/reply equivalence. Root AGENTS.md requires comments to teach actual constraints; this comment instead overstates
one.","TODO.md Planned is empty. Plans/queue searches for bulk Delete, reply handling and the exact test found the
landed environment-agent-operations design and other selection/pacing rules, not an unlanded correction of this oracle
comment.","BUGS.md has no test-comment acceptance. INDEX.md/matching queue text and active TRIAGE_OUTCOMES.md searches
found no exact row-disappearance comment item. Existing bulk sort-save or reply-witness issues are different causes.
g029/f1 is a withdrawn false-pass hypothesis, not duplicate coverage. Archive not read.","FILTER.md does not cover a
stable source comment that teaches a false invariant. Actual adverse response ordering being unmeasured does not make
the incorrect categorical 'only' promise a rare self-correcting diagnostic."] Exact cited cover: [] Original stronger
consequence and missing premise remain preserved: "The named f2 history consistently confines the claim to misleading
maintainer documentation. f1's earlier suspected false pass is explicitly preserved as refuted and is not silently
adopted here."; "None for the source/ordering contradiction. Actual incidence was not measured. No assertion
ineffectiveness or product failure is inferred.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g029 record](records/review-through-00de0b6c/groups/g029.json). Record ID: `f2`.
