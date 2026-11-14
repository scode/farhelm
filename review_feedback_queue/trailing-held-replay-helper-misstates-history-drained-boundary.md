# held-replay helper claims history is applied before its oracle proves it

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: held-replay helper claims history is applied before its oracle proves it

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g179/l1`, `g181/c2`.

### g179/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. TLDR: the rewritten takeover-test explanation says all
received grid frames have been applied and are ready to reveal, although history can still be queued when this test’s
reveal occurs. Future readers get an incorrect synchronization guarantee. Details: grid-attach.spec.ts:433–435 newly
adds the inference "The grid applies frames as they arrive, so they are all there to reveal." openCatchUp at :422–426
only sends attach.jsonl and checks hidden/unrevealed. The fixture contains two history messages with two rows each.
grid-terminal.js:1973–1994 queues model history, gives a no-backlog arrival one bounded slice, and schedules remaining
work; :1886–1918 applies later slices by animation frame/task. grid-model.js:971–1005 checks its yield predicate after
every row, so even this small fixture has no source guarantee that both rows finish before a yield.
terminal.js:3774–3778 expressly says history may still be applying and reveals the current screen via
scrollToBottom/flush; grid-terminal.js:1997–2000 flush only paints and does not drain history. The actual test at
grid-attach.spec.ts:442–450 waits for revealed, checks the detach banner and visible first screen row, and never asserts
drained history. Its executable screen/reveal checks remain useful and are not claimed broken. Governing contract:
SPEC_impl.md, The grid client, says history is applied "in slices between frames so that the screen stays usable"; the
reveal implementation’s explicit caveat agrees. Fix the rationale to say the current screen is applied and can be
revealed while received history may still be pending; do not add a history wait unless a caller truly needs that
boundary. Range attribution: compared f560058 parent lines738–745, which says the grid applies frames as they arrive but
does not add the new all-there-to-reveal inference; the complete base-to-tip diff retains that newly added sentence.
Therefore this is a changed documentation claim, not a new history-loss or test-oracle behavior regression. Suggested
bucket: other, test synchronization documentation. Confidence: confirmed by source/contract analysis only; no runtime
behavior observed. proposed_drop:false. Could-be-highest: no established security or loss-of-work route. possible_cover:
caller-supplied g181/c2 is the same underlying applied-history-versus-marker distinction in a different test helper, so
consolidation may be appropriate; no prohibited coverage sources searched. Caveats: ordinary tiny fixture drains likely
finish immediately; timing likelihood does not establish the unconditional explanation. No reproduction, test, build or
live-install action performed. Original lead consequence narrowed to the false documentation inference;
fixture/caller/flush/yield premises are now checked, with no unresolved premise.

Returned proposal (claims-111, event 12): **retain-verified**, suggested bucket **other**.

The new all-there-to-reveal inference promises history application that reveal does not guarantee. The executable test
checks the current screen and banner, which remain useful. The finding is the false explanation, not demonstrated
history loss or a broken assertion.

Original stronger consequence: The initial lead asked whether this fixture/callers needed history completeness; source
establishes only a misleading all-frames-applied inference. Preserve that the test's actual screen/reveal checks remain
useful, no history-loss or test-oracle behavior regression is asserted, and ordinary small-fixture timing is likely
prompt.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record the false test synchronization explanation as other. Qualify it to current-screen reveal with
history potentially pending; do not demand an unnecessary history wait.

Existing parent disposition (event 12): duplicate. Parent read full111,actual179event12 and direct f560
grid-attach410-452. Newly added 'applies frames as they arrive, so they are all there to reveal' does infer whole
applied readiness beyond merely transport arrival; history slices may still await tasks while flush paints onlyscreen.
Same false applied-history synchronization contract as retained181c2's explicit arrived-and-been-applied helper,
different location requiring same qualified current-screen-versus-history explanation. Combine both exact claims in one
item, preserve tinyfixture likely prompt, actual current-screen/banner assertions useful and no false pass/history-loss
allegation. Do not add universal history wait.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g179 record](records/review-through-00de0b6c/groups/g179.json). Record ID: `l1`.

### g181/c2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e

## TLDR

The revised test helper describes a fully applied replay, but it only waits for the replay marker. Older terminal output
can still be queued at that point. This is a test synchronization/documentation defect; no shipped output loss has been
demonstrated.

## Details

The changed docstring says 'the replay has arrived and been applied to the still-hidden terminal'. The helper at
:136-154 polls only __farhelmIslands[el].test.replay.heldReason == marker. terminal.js:4073-4082 handles replay_complete
by immediately calling endCatchUp, whose :3886-3888 held-marker path sets heldReason without waiting for the grid's
history queue. grid-terminal.js:1876-1930 drains queued history in bounded slices scheduled on later animation
frames/message-channel tasks. term.ts:162-215 explicitly states and implements that unapplied history is absent from
text reads, and terminal-readiness.ts:40-55 waits only for revealed. Therefore neither this helper nor the subsequent
reveal wait establishes the newly documented applied-history boundary. SPEC_impl.md, The grid client, explicitly says
the client applies a large history 'in slices between frames so that the screen stays usable'. terminal.js:3774-3776
also explicitly says history may still be applying when the screen reveals.

The backend-independent completeness concern remains narrower than a product bug: the agent test at :371-375 reads only
spam-line-200, which is in the current screen after spam 200; the tab test waits only for TABSPAM-200. Those observables
do not independently prove historical rows arrived. That weak last-line assertion was already present before this
commit, so it is not claimed as newly introduced missing-history behavior. The newly changed helper contract should say
the replay marker arrived and the current screen was applied, with history potentially pending. If callers need fully
applied history, add a separately named history-drained oracle and assert an older line as well; do not make all
held-state tests wait for a stronger boundary indiscriminately.

Suggested bucket: other, confined to test synchronization documentation and its overstated observable. Confidence:
confirmed by source/contract analysis; no execution. proposed_drop: false for the newly false helper contract. Preserve
prior proposed drop for bufferBytes/bufferChunks/bufferedBytes themselves: those removed controls describe the retired
byte-stream buffer and are not current grid guarantees. The previous broad c2 dismissal is narrowed by this revision,
not silently removed. possible_cover: none; no queue or coverage sources searched. Could-be-highest: no established
security or loss-of-work route; missing test evidence alone does not prove such harm. Caveat: the current callers'
hidden/visible, marker-reason, first-frame-tail and live-input assertions remain useful and this finding does not
require reveal to block on all history.

Returned proposal (claims-112, event 7): **retain-verified**, suggested bucket **other**.

The newly changed helper docstring claims replay has been applied, but the helper observes only marker arrival. Queued
older history can still be pending. This establishes an inaccurate test synchronization contract, not shipped output
loss or a reproduced false pass.

Original stronger consequence: Initially considered lost replay completeness coverage after removal of byte buffers.
That broad dismissal was reopened: a test can observe marker/current screen while older output remains queued. Latest
scope retains only the newly false helper contract; weak last-line completeness assertions already existed. No actual
product loss was established or lowered from highest.

Unresolved premise or limitation: No runtime ordering, false pass or shipped history-loss consequence was observed.
Those stronger claims need separate evidence; the false helper contract is established by source.

Proposed basis: Record as other: clarify marker/current-screen readiness versus fully applied history. If a caller needs
all history, give it an explicit drained-history witness; this finding does not require delaying every reveal. Preserve
the prior dismissal only for retired byte-buffer controls.

Existing parent disposition (event 7): verified. Parent explicitly accepted named retain-verified proposal from
claims-112 after inspecting it. The newly changed helper docstring claims replay has been applied, but the helper
observes only marker arrival. Queued older history can still be pending. This establishes an inaccurate test
synchronization contract, not shipped output loss or a reproduced false pass. Record as other: clarify
marker/current-screen readiness versus fully applied history. If a caller needs all history, give it an explicit
drained-history witness; this finding does not require delaying every reveal. Preserve the prior dismissal only for
retired byte-buffer controls. Checked source/results:
["f560058da19e02716e6f943883a9508b447aff4e:e2e/tests/terminal-replay-rename.spec.ts:127-154 newly says replay has
arrived and been applied, then polls only test.replay.heldReason == marker. The commit patch confirms this wording is
newly authored.","Same revision terminal.js:4073-4082 calls endCatchUp(marker) directly on replay_complete; :3886-3888
records heldReason without a history-drain condition. :3774-3776 explicitly permits history still applying at
reveal.","Same revision grid-terminal.js:1876-1930 drains history in slices scheduled on future animation
frames/message-channel tasks. helpers/term.ts:162-215 explicitly excludes unapplied history from text reads.
helpers/terminal-readiness.ts:40-55 waits for revealed, not historyPending == 0.","The patch retains hidden/placeholder,
marker-reason, first-visible-frame-tail and current-screen/live-input checks. Removed
bufferBytes/bufferChunks/bufferedBytes are old byte-buffer observations. The existing spam-line-200/TABSPAM-200 checks
do not independently prove all historical rows; that weakness predates this patch."] Current coverage/results: ["SPEC.md
Terminal experience requires replayed screen and scrollback; SPEC_impl.md The grid client explicitly permits large
history application in slices. That acceptance does not accept a falsely documented test boundary.","TODO Planned is
empty. Searches of plans/queue found no unlanded fix for this helper contract; empty-scrollback-messages and tmux
removal plans are landed (plans/REPORTS.md:9,29,32). BUGS has no matching item.","INDEX/matching queue search found
replacement-history-absence-asserted-before-delivery.md. Its full record concerns a negative old-history assertion in
replace.spec.ts without a delivery witness, not this positive held-marker helper docstring. Shared sliced-history
mechanism is context, not exact coverage or an alias.","Active TRIAGE_OUTCOMES searches found no matching decision.
FILTER.md does not match this ordinary, persistent documentation overstatement."] Exact cited cover: [] Original
stronger consequence and missing premise remain preserved: "Initially considered lost replay completeness coverage after
removal of byte buffers. That broad dismissal was reopened: a test can observe marker/current screen while older output
remains queued. Latest scope retains only the newly false helper contract; weak last-line completeness assertions
already existed. No actual product loss was established or lowered from highest."; "No runtime ordering, false pass or
shipped history-loss consequence was observed. Those stronger claims need separate evidence; the false helper contract
is established by source.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g181 record](records/review-through-00de0b6c/groups/g181.json). Record ID: `c2`.
