# Archive epoch can change without waking other clients

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Archive epoch can change without waking other clients

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g022/f2`, `g035/c2`.

### g022/f2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 06d4b68f3a9e913f47cd426fedee5946a4eed77b

## TLDR

After deleting a session that archives its checkout, other open windows can keep an empty or outdated Trash count until
another fleet change or an explicit Trash open. A filtered window can miss the newly archived work entirely in its idle
Trash display.

## Details

The new archive branch in crates/farhelm-helm/src/sessions.rs:3376-3382 increments trash_requests but relies on
forget_deleted_session to publish the fleet wake; finish_replacement:4195-4202 has the same reliance. That tail calls
manager.forget_session. At manager.rs:2405-2434, forgetting only bumps FleetEvents when it actually removed a
cached/live session. The supervisor can legitimately make that removal observable before the delete reply:
service/teardown.rs:1062-1065 removes the session and hints SessionsChanged, then :1081 onward performs unbounded
on-disk cleanup, including discard_quarantined at :1149-1150. The request handler only sends SessionDeleted after that
teardown returns (service/handlers.rs:1369-1382). The manager can therefore consume the hint, commit a list without the
session, and bump the fleet (manager.rs:4603-4615,4659-4662) while the Trash epoch is still old. Another client's
completed hosts read can consume that wake. Later the reply increments the epoch, but forgetting the already-removed row
returns false (store.rs:9227-9237), so no new event is emitted. This is a source-established ordering, especially
visible when post-removal attachment cleanup is slow; it does not require a forged request or unshipped control.

Other idle windows have no guaranteed repair timer: feed.rs:233-234 disables fallback polling with a healthy feed,
list/view.rs:2314-2320 reads hosts on fleet notifications, and list/trash.rs:737-768 invalidates its reader only on the
delivered epoch or other explicit causes. Stable periodic manager refreshes have no cache change or unanswered hint and
need not emit another event (manager.rs:4659-4662). A window whose complete listing shows the removed managed row may
independently refresh Trash; filtered/truncated windows are the explicit motivation of the new code and lack that
fallback.

Contract: SPEC_impl.md:4538-4542 says
`A per-host in-memory Trash epoch advances on accepted nonempty Delete or Retry, on a session Delete or Replace that archived a checkout, and on a recorded notice change, before the existing fleet wake. Hosts listings carry it; only its change invalidates an idle host’s Trash reader`.
The archive fact is a separate observable change even when the session cache is already current. Publish an explicit
fleet bump after advancing the epoch in both archive branches, rather than depending on a changed-only session-cache
deletion.

Suggested bucket: high/material UX (other windows can retain an incorrect Trash count indefinitely during an otherwise
idle healthy connection). Confidence: confirmed by source and contract analysis; no execution, build or reproduction.
Could-be-highest: no; files remain archived and retrievable, and no security or data-loss route is claimed.
proposed_drop: false. possible_cover: none; coverage/queue sources were not read. Caveats: an unrelated later event,
reconnect, app load or explicit dialog opening repairs the display; the issuing window may already perform its own
refresh. This does not claim that archived files are lost.

Final initial-pass checks: teardown.rs:1155-1187 reaches SessionDeleteOutcome after file cleanup without another
sessions-change hint. manager.rs:5055-5071 and 5168-5197 confirms stable Connected refreshes compare equal and emit no
wake, so the background refresh is not a guaranteed repair. Exact new archive branches are sessions.rs:3376-3379 and
4195-4200. Original consequence and high bucket retained. No missing source premise remains for this interleaving;
runtime incidence was not measured.

Synthesis turn 3, first checks: the end-of-range source at 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the same
missing explicit fleet bump in do_delete_session (:3731-3734) and finish_replacement (:4576-4581), while
delete_checkout_trash (:633-635) explicitly bumps after changing the epoch. No later applied fix at the reviewed range
endpoint. Supervisor hints.rs:148-161 sends on an owed Notify and then waits for a new mark; its throttle does not
manufacture a second late notification. The post-row-removal teardown interval is sufficient for the sole delete hint
and all previous pending marks to drain before the late archive reply. This strengthens the original ordering without
changing the stale-display consequence or bucket. Parent crossinput g017/c1 is about a zero-valued wire golden fixture
and is separate from this product wake defect; no queue or coverage artifacts were read.

Synthesis turn 3 completed (source-only, no reproductions). Additional readily checkable premises resolved:

- List/delete concurrency is real, rather than an assumed schedule through a serialized RPC connection.
  client.rs:3057-3123 registers a per-request oneshot, releases the pending-map mutex, sends the frame and awaits only
  that reply. list_sessions (:3336-3351) and delete_session_fenced (:3923-3951) use that same multiplexed primitive.
  Supervisor handlers.rs:550-619 deliberately spawns ListSessions with its own list-admission permit rather than a
  management/delete permit; :2895-2897 dispatches to it. Thus a list can finish while the delete teardown performs
  post-removal disk cleanup. The manager seed epoch is incremented only in the later forget tail, so it cannot reject
  the already-completed early refresh in this ordering.
- No generic mutation middleware supplies a missing late wake: middleware.rs:66-76 and lib.rs:2657-2664 only
  retain/await the request task. forget_deleted_session at sessions.rs:3388-3411 performs tunnel/bookkeeping cleanup
  then manager.forget_session; tunnels.rs:165-181 has no event when there are no tunnels (and no mandatory wake at all).
  An ordinary unpinned session without active tunnels suffices.
- Replace-with can change the folder or obtain a fresh checkout (sessions.rs:4000-4086 and :4160), allowing its source
  deletion to archive the old last-reference checkout. Both accepted-create publication and optional pin-inheritance
  publication precede that source deletion; neither is a guaranteed wake after the late archive fact. finish_replacement
  (:4248-4254) and the REST success wrapper (:4273-4300) return without an extra wake. Plain same-folder Replace
  normally retains the checkout and does not take the archived branch.
- Transport keepalive is not an accidental repair: events.rs:206-276 sends one handshake revision and thereafter
  revision text only on revisions.changed(); idle sends WebSocket Ping and a healthy Pong resets the liveness timer.
  assets/events.js:293-337 spends its handshake deadline after the first text frame and does not periodically
  resubscribe a healthy socket. feed.rs:361-369 updates feed state only on a revision/down report. No periodic revision
  or healthy-client reconnect is guaranteed.
- The end-of-range manager.rs:2483-2537 still has changed-only forget. Endpoint ordinary Delete and Replace still lack
  an archive-specific bump. The later sessions change matching event publication
  (56659839785691f76da3af5d8f51b511cff4ba8b) moves successful-create history publication; it supplies no archive-tail
  wake. Feed/event/notices files have no endpoint difference from the reviewed commit. No later applied in-range fix was
  found.

Disposition remains reported/confirmed, high/material UX, proposed_drop false, possible_cover none. Original wording and
citations above are retained; no severity lowering or parent drop is accepted. The narrower consequence remains stale or
missing Trash display until a repair trigger, not loss of archived files, and actual runtime incidence remains
unmeasured. g017/c1 test-fixture sensitivity is separate and was not re-reviewed or counted here. Open lead IDs after
synthesis: none.

Returned proposal (claims-012, event 7): **retain-verified**, suggested bucket **high**.

A supervisor can publish session removal and let the helm consume that hint before its later archive reply. The reply
changes the Trash epoch, but changed-only cache forgetting need not publish a second fleet event. An idle filtered
window can retain its earlier count. This is a source-supported interleaving, not a runtime observation.

Original stronger consequence: Other open windows can keep an empty/outdated Trash count until another fleet change or
explicit Trash open; filtered windows can miss newly archived work in the idle display. The history consistently
disclaims file loss and retains high/material-UX priority.

Unresolved premise or limitation: Runtime incidence and duration are unmeasured. Whether the maintainer regards an
indefinitely idle stale count as 'briefly wrong' for FILTER.md remains a recording-policy ambiguity; it is not missing
source support for the interleaving.

Proposed basis: Retain the conditional source finding and its high recommendation, with repair triggers and unmeasured
incidence explicit. g017/c1's zero-valued wire fixture is a distinct test sensitivity concern, not an alias.

Existing parent disposition (event 7): verified. Parent explicitly accepted named retain-verified proposal from
claims-012 after inspecting it. A supervisor can publish session removal and let the helm consume that hint before its
later archive reply. The reply changes the Trash epoch, but changed-only cache forgetting need not publish a second
fleet event. An idle filtered window can retain its earlier count. This is a source-supported interleaving, not a
runtime observation. Retain the conditional source finding and its high recommendation, with repair triggers and
unmeasured incidence explicit. g017/c1's zero-valued wire fixture is a distinct test sensitivity concern, not an alias.
Checked source/results:
["06d4b68f3a9e913f47cd426fedee5946a4eed77b:crates/farhelm-supervisor/src/service/teardown.rs:1062-1065 removes the
session and hints before unbounded cleanup at 1081-1187; service/handlers.rs:1371-1382 sends SessionDeleted only after
mutation completion.","At that revision client.rs:3057-3106 releases request-map ownership before awaiting its own
reply, and supervisor service/handlers.rs:599-619 spawns list work with separate admission. A list may therefore
complete during post-removal
cleanup.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:crates/farhelm-helm/src/sessions.rs:3725-3735 and 4575-4581 advance
the archive epoch without an explicit fleet bump. manager.rs:2507-2535 bumps only when cache forgetting changed rows.
The ordinary Delete tail at sessions.rs:3743-3766 reaches that manager
method.","06d4b68f3a9e913f47cd426fedee5946a4eed77b:manager.rs:4602-4615,4662-4664 emits for changed cache or an
unanswered hint, not every stable refresh. UI feed.rs:233-234 disables fallback while healthy; list/trash.rs:738-764
reacts to delivered epoch or explicit causes.","Files remain archived and retrievable. An unrelated fleet event,
reconnect, load or dialog open repairs the count; complete unfiltered listings and the issuing window may have
independent refresh triggers. No loss of archived files is established."] Current coverage/results: ["Current
SPEC_impl.md:4039-4057 requires epoch advancement before the fleet wake and says ordinary events/fallback ticks do not
themselves read Trash. Its other-helm reconnect exception is a different scope. SPEC.md Trash section supplies no
acceptance of missed same-helm archive publication.","TODO.md Planned and plans/queue/INDEX.md are empty. The retained
bell-trash-reads.md plan text describes this feature, but the plan is already landed and cannot cover a remaining defect
as unlanded work.","BUGS.md has no archive/count item. INDEX and active TRIAGE_OUTCOMES searches found layout,
migration, listing-test and read-admission issues, not the missing post-archive wake. Read
trash-read-eligibility-can-remain-cached.md: it concerns an extra read during pending cleanup, not a missed epoch
delivery.","FILTER.md rare/self-correcting display filter is a possible ambiguity. No guaranteed repair event bounds how
long this healthy idle window stays stale, so the whole 'briefly wrong' boundary is not established. Retain under the
rule that unclear coverage/filtering does not authorize omission."] Exact cited cover: [] Original stronger consequence
and missing premise remain preserved: "Other open windows can keep an empty/outdated Trash count until another fleet
change or explicit Trash open; filtered windows can miss newly archived work in the idle display. The history
consistently disclaims file loss and retains high/material-UX priority."; "Runtime incidence and duration are
unmeasured. Whether the maintainer regards an indefinitely idle stale count as 'briefly wrong' for FILTER.md remains a
recording-policy ambiguity; it is not missing source support for the interleaving.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g022 record](records/review-through-00de0b6c/groups/g022.json). Record ID: `f2`.

### g035/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (candidate, open):

Reviewed commit: 06d4b68f3a9e913f47cd426fedee5946a4eed77b. Revision reason: discovery-neighbours supplies g022/f2, which
invalidates the earlier unqualified assertion that every archived Delete/Replace publishes its epoch before a guaranteed
fleet wake. Original possible consequence retained: other clients keep a stale or missing Trash display after
archive/cleanup. The local UI check remains sound conditional on epoch delivery: HostTrashReader reacts to changed host
trash_epoch and retains paused explicit demand. It does not prove the producer wakes the host listing. g022/f2
establishes that sessions.rs:3377 and 4198 advance the epoch but changed-only manager.forget_session may emit no late
wake after an early supervisor SessionsChanged/list already removed the session cache. That separate producer defect is
already recorded by g022/f2 and is not a new local finding. The previous c2 proposed-drop basis is narrowed to the UI
consumer and explicit cross-helm acceptance; it must not dismiss the same-helm archive wake route. Suggested bucket for
the known producer consequence: high as g022 records, no work loss alleged. Confidence: local consumer source mechanism
confirmed; producer premise corrected from supplied source-backed interface record. proposed_drop: only local consumer
failure refutation and exact cross-helm acceptance ('Changes through another helm are noticed on reconnect, app load or
dialog open.'), not a drop of g022/f2. possible_cover: g022/f2, already supplied by caller; no forbidden coverage
sources searched. Could-be-highest: no established security or loss-of-work route; files remain archived. Source-only,
no runtime observation. No new local lead is needed for an already owned, synthesized defect.

Returned proposal (claims-021, event 10): **needs-parent**, suggested bucket **high**.

The UI consumer refutation is valid only if a changed host epoch is delivered. The original same-helm archive
consequence is not refuted by that check. Preserve the supplied separately owned producer claim for parent
reconciliation rather than writing or dismissing a second finding here.

Original stronger consequence: Another client's accepted cleanup/archive remains stale or missing in Trash. The original
unqualified same-helm invalidation refutation was withdrawn; cross-helm delayed discovery remains explicitly accepted.
Files remain archived; no work-loss route was alleged.

Unresolved premise or limitation: Parent must reconcile g022/f2's complete supported early-list-before-epoch sequence
and its final priority. Conditional consumer correctness alone cannot dismiss that producer route.

Proposed basis: Do not create a second local consumer finding, and do not count the broad candidate as source-refuted.
Keep the producer consequence with its owning claim; high is the supplied producer suggestion pending parent
reconciliation.

Existing parent disposition (event 10): duplicate. Parent read claims-012 g022/f2's complete early-list-before-epoch
route and claims-021 g035/c2. Conditional UI consumer correctness in trash.rs:690-698,738-764 cannot ensure epoch
delivery. g022/f2 already retains the exact same-helm producer failure: supervisor removal hint can be consumed before
archive completion, epoch later advances, and changed-only cache forgetting supplies no second fleet wake. This alias
adds consumer corroboration and withdraws its earlier guaranteed-wake refutation. Both originalIDs/wording remain;
existing cross-helm reconnect-only acceptance covers only that separate branch. Files remain retrievable and high
reflects the retained idle cross-window count consequence, not data loss.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g035 record](records/review-through-00de0b6c/groups/g035.json). Record ID: `c2`.
