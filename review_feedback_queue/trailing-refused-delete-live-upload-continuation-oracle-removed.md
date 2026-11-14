# Refused Delete loses its in-flight upload preservation oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: considered whether deleting pane-marker, bystander-pane, renamed-owner, brace-directory and
same-pane history tests removes essential protection for the shipped terminal-host implementation.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g139/g139-c2`.

### g139/g139-c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: considered whether deleting pane-marker,
bystander-pane, renamed-owner, brace-directory and same-pane history tests removes essential protection for the shipped
terminal-host implementation. Details: assigned deletions in marker_model.rs (-171...), restart_under_concurrency.rs
(-320...), restart_with_resume.rs (-699... and -996...), session_lifecycle.rs (spaced owner and renamed-session Delete),
and the refused-Delete upload test all construct tmux through the removed test-only creation path. SPEC_impl.md:2015ff
and :2288ff require every new terminal to be a host, a host restart to allocate a fresh host, and old tmux sessions to
migrate rather than respawn. The same-pane marker/reuse invariants and nested tmux brace parser therefore have no host
equivalent. Existing host_relaunches.rs:491-575 directly checks retained history, absence of old visible screen, default
modes, fresh socket, generation and old-host cleanup; restart_with_resume.rs:763-829 retains restart reaping of a
reparented daemon. The old tmux adoption path still exists at this intermediate revision, so its test loss is real as
temporary coverage loss; this record does not assert adoption itself has stopped. Direct mutations used by renamed-pane
tests are outside the supported contract: SPEC_impl.md:3009ff says 'Direct interaction with it, by the user or by a
program running in a session, is unsupported' and 'does not track, reap, or restore windows, panes, or configuration
created or changed that way.' Suggested bucket: other if reported as coverage, not a runtime defect. Confidence:
confirmed source analysis. proposed_drop: no distinct host defect or retained same-pane contract shown; tmux-specific
checks retire with the test-only creation surface, and history/process cleanup have host coverage. The separate retained
upload-priority contract is recorded as g139-l1, rather than dropped with these tests. possible_cover: none searched.
Could-be-highest: no established product security or work-loss route; this consideration concerns removal of tests, not
evidence of unsafe production ownership. Caveat: a fresh source finding in retained old-session adoption would require
separate evaluation; no claim that unsupported tampering permits unsafe kills is made here. Discovery-pass check of an
additional deletion consequence: investigated whether removing
restarting_a_live_session_stops_its_tree_and_reuses_the_terminal also loses the independent live-descendant reap or
successful annotation-clear oracle. At the SAME reviewed revision, host_terminals.rs:333-422 already contains
restart_of_a_host_agent_stops_its_tree_before_a_new_host_starts: it installs a real host child plus unmarked
hangup-immune grandchild, observes both alive, and a wrapper at the actual replacement host start records both gone; it
then asserts a distinct host, resumed-ready marker, old host death and Running outcome. This discriminates the
live-rooted-tree ordering beyond the retained self-exited-daemon test cited earlier.
restart_under_concurrency.rs:482-545 separately stops a ready generation, requires its annotation, restarts and asserts
the new generation has none, then releases its next natural exit and asserts no annotation. This source evidence refutes
a separate loss of these oracles; the same-pane identity remains intentionally obsolete. Through-tip search located the
same live-tree test, not a late replacement. Original candidate consequence, bucket other, no product/highest route,
proposed-drop bases and no-execution caveat remain unchanged. No new issue or open lead from this check. Discovery
reopens ONLY the refused-Delete upload part of this grouped candidate: the removed
a_refused_delete_preserves_an_in_flight_upload used a renamed tmux pane to trigger a read-only preflight, but its
upload-survival assertion may remain applicable to a terminal-host Delete preflight refusal. Original disposition
grouped that assertion with obsolete pane mechanics; production correctness or unsupported direct tmux mutation alone
cannot refute a retained, fixture-independent oracle. Need inspect host preflight ordering, current upload cancellation
sites and all remaining refused-Delete/upload tests at the reviewed revision and through tip. Source basis already
delivered: assigned 00781 patch lines 9-115 deletes a test that begins/acks a partial upload, provokes preflight
refusal, then finishes and publishes the same transfer. Missing premise: whether a supported host refusal still promises
no transfer cancellation and has an equivalent distinguishing test. This is an investigated-source lead to pursue now,
not an allegation of current cancellation or data loss. All other candidate subcases and their original proposed-drop
bases remain preserved above. Provisional bucket other for coverage only; could-be-highest no established shipped route;
no execution or reproduction. Discovery source result: the upload-survival concern is valid independently of tmux and is
retained as a distinct test-only finding under this original ID. Reviewed teardown.rs:655-668 explicitly promises that a
refused read-only preflight leaves the transfer and staged bytes available, cancelling only in finish_teardown after
prepare_teardown succeeds. At pinned tip this remains teardown.rs:277-285 and :505-518, with host roots/record discovery
and scope enumeration named as preflights. The remaining a_successful_delete_cancels_an_in_flight_upload_before_removal
at attachment_uploads.rs:845-897 starts and acknowledges a transfer, then only exercises successful Delete and requires
UploadAborted; it cannot catch cancellation moved ahead of a refusal. The host refusal fixture
a_stopped_recorded_host_is_guarded_and_then_signalled (tip host_terminals.rs:788-824) checks the host remains stopped,
then performs unconditional Delete, but contains no in-flight transfer or subsequent successful publication. The removed
test uniquely combined an actual refusal with continuation/commit of the SAME acknowledged partial upload. This confirms
a retained contract and a removed discriminating witness, not a current cancellation bug. Earlier c2 proposed drop for
this upload-survival subcase is superseded; other pane-specific subcase drops and all their original bases remain above.
Suggested bucket other; confidence confirmed by source for deletion and retained host contract, with final through-tip
equivalent-test checks continuing next. proposed_drop:false for upload-survival finding. possible_cover:none known in
supplied neighbours (their replay, raw-input, migration and cleanup findings are distinct). Could-be-highest:no; only
regression protection loss is established, with no current work-loss or security route alleged. No runtime tests,
reproductions or live actions. Final replacement and attribution checks: git log over the WHOLE
exclusive-base-to-inclusive-tip range for attachment_uploads.rs reports only the reviewed deletion and
f560058da19e02716e6f943883a9508b447aff4e, whose file diff removes only Attach.grid. No later replacement was added in
this file. Pinned searches for BeginUpload/run_upload/UploadHandle/UploadSignal/abort_session_uploads across core tests,
host refusal tests, teardown and browser tests found no Delete-preflight-plus-live-transfer continuation assertion.
handlers.rs:10999-11041 and :11125-11178 were checked because they do exercise real BeginUpload: their conformance cases
deliberately hold lifecycle gates, test unrelated request responsiveness, and abort transfers as fixture cleanup;
DeleteSession is a separate table case, not a refusal while the same acknowledged upload is completed.
attachment_uploads.rs:2130-2170 covers a failed attachment-directory detach only after peer.upload has already committed
the attachment; it cannot detect cancellation of an unfinished transfer by an earlier preflight. Other upload
cancellation unit tests inject UploadSignal directly and do not dispatch a failing Delete preflight. Thus no equivalent
discriminating oracle was found through pinned tip. Suggested fix: retain the partial-upload/ack, refused read-only
preflight, then same-transfer completion/publication test sequence using a host-supported preflight refusal (for example
a conflicting/malformed recorded-host fixture), rather than retaining tmux rename mechanics. Final confidence confirmed
source-only coverage loss; proposed_drop:false for this upload-survival defect, bucket other. Current prepare/finish
ordering is correct and is not a test substitute. All readily checkable premises settled; no remaining open lead for
g139-c2. This discovery changes an earlier candidate to one retained finding under the same ID; it does not revive the
refuted same-pane/tree/annotation subcases, whose original consequences and proposed-drop bases remain in this
detail/history.

Returned proposal (claims-088, event 12): **retain-verified**, suggested bucket **other**.

Retain the upload-survival test deletion separately from the retired pane fixtures. A refused read-only preflight still
promises to leave an acknowledged partial upload usable. The successful-Delete test cannot detect cancellation moved
before a refusal. Preserve the history of the earlier grouped drop and its supersession for this subcase.

Original stronger consequence: The grouped candidate considered loss of pane ownership, bystander protection, same-pane
history/reuse, tree reaping, annotations and refused-Delete upload protection. It originally proposed dropping all as
retired tmux mechanics or replacement coverage. Reopening established the independent upload-survival oracle loss and
superseded that subcase's drop; tree/annotation refutations and obsolete same-pane limitations remain separate, with no
observed unsafe production kill or upload loss.

Unresolved premise or limitation: A current shipped cancellation or data-loss bug would need cancellation before a
refused preflight. Current source ordering is correct; that stronger product route is not alleged. An equivalent
unlocated test would narrow the coverage conclusion.

Proposed basis: Retain the upload-survival test-only finding as other. The flag conservatively preserves parent
assessment of the grouped ownership/process-protection subcase drops; this consultation does not authorize them. Use a
supported host preflight refusal and prove the same acknowledged transfer can still publish.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g139 record](records/review-through-00de0b6c/groups/g139.json). Record ID: `g139-c2`.
