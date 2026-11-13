# Intermediate loss of old-renderer per-tab flow isolation coverage

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Intermediate loss of old-renderer per-tab flow isolation coverage

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g176/c3`.

### g176/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR considered: deleting the per-tab stalled-write test might remove the mechanism that keeps sibling terminals responsive. Source refutation of behavior regression: patch 00838-000.patch removes only the OLD_TERMINAL_TAG test and its unused fixture import. terminal.js:50-59 and SPEC_impl.md:1992-1998 establish that grids apply messages directly and do not send write-callback pause/resume. Actual new-terminal pacing is tested in farhelm-helm/src/terminal.rs:2511-2571 (slow browser pauses/resumes, browser pause/resume never forwarded) and farhelm-supervisor/src/service/grid_stream/tests.rs:472-511 (paused flood resumes with every retained history line). Terminal tab teardown/restart identity and real sibling interactivity tests remain in terminal-tabs.spec.ts:658-826. Suggested bucket: other if a coverage concern; confidence: source-only refutation of a changed flow-control contract. proposed_drop: the removed test's deliberate term.write stall cannot operate on grid terminals and no flow-control implementation changed in this assigned patch. possible_cover: none. Could-be-highest: no; the considered consequence is responsiveness/test coverage, not established loss of user-owned work. Caveat: backend single-stream tests do not independently prove simultaneous sibling interactivity; distinct old-renderer coverage is removed, but this is not evidence that the retained grid mechanism shares a blocking write callback.

Synthesis turn 2 — correction of original disposition: no changed production path and a grid having no term.write callback do not refute loss of a regression test for old terminals that still existed at 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. Preserve the original consequence (one stalled tab could block its agent/sibling or fail to resume), and supersede the original proposed_drop: proposed_drop now false, no drop accepted. The deleted witness paused a real producer by holding exactly one xterm instance write callback, proved its agent and sibling interactive with input round trips, then released callbacks and proved that tab resumed. At that intermediate revision terminal.js:5335-5370 still used per-mount pendingWrite/paused state and term.write completion to send pause/resume, and SPEC.md Terminal experience retained old terminal and per-viewer isolation contracts. A competing regression such as making browser pendingWrite/paused bookkeeping page-global would be caught by the removed three-terminal witness; the grid-only single-stream pacing tests and healthy-tab mount/restart tests cited in the first record would not exercise it. Confirmed intermediate test-only coverage gap; suggested bucket other, source-only confidence, no claim that the competing regression exists.

Distinguish the later retirement: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 ends old terminals; f560058da19e02716e6f943883a9508b447aff4e removes old-renderer browser pause/resume and term.write bookkeeping. At pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db terminal.js:13-35 applies grid messages directly and the helm owns each stream flow gauge, so the deleted xterm callback stall is no longer an applicable witness. The surviving shared no-head-of-line-blocking contract has concrete lower-layer coverage: tip farhelm-helm/src/client.rs:6172-6275, a_full_terminal_queue_detaches_that_terminal_without_blocking_the_others, drives the real demux loop, leaves channel 1 undrained, sends a healthy channel 2 frame after overflow, checks it arrives and channel 1 detaches visibly. This directly distinguishes awaiting capacity on one terminal queue from continuing to deliver another. The retained slow-browser/grid-stream pause/resume tests cited originally still cover the separate pause/drain/resume path; they do not establish real simultaneous sibling input under a deliberately paused grid stream, and the demux overflow witness is not misrepresented as that browser test. No current isolation bug is established and no obsolete xterm test should be restored at tip. Original proposed drop/rationale remains above as history, now superseded for the intermediate gap. possible_cover none; could-be-highest no security/loss-of-work route established. No reproduction or runtime execution.

Returned proposal (claims-110, event 13): **needs-parent**, suggested bucket **other**.

The removed browser witness genuinely tested one old renderer stalling while the agent and sibling stayed interactive and the stalled tab resumed. Neither unchanged production behavior nor separate grid pacing tests refutes that intermediate loss. The xterm write-flow mechanism itself is gone by tip.

Original stronger consequence: One stalled tab could block its agent/sibling or fail to resume if old-renderer flow isolation regressed. Original source-based proposed drop is explicitly superseded at line 13: lack of a new functional bug does not disprove lost test protection. Later retirement removes the test's exact subject but does not retrospectively replace its intermediate protection.

Unresolved premise or limitation: No current browser write-callback flow-control mechanism remains. No actual stalled sibling or lost output was reproduced; present grid behavior needs its own relevant witness, not an obsolete xterm test.

Proposed basis: Keep the intermediate regression-protection loss distinct from both a current product isolation bug and the surviving lower-layer demux test. Parent must decide the recording treatment for a subject retired within the review range.

Existing parent disposition (event 13): rejected. Full110:647 deletes one-xtermcallbackhold/observedpause/agent+siblingliveinput/resume-afterrelease browserwitness whileper-mountpendingWriteflowremains. Productionunchangedandrealhelmchanneloverflowunitnot equivalent browserpausedinputoracle; historicalgapisreal. f560retirescallbackflowandtiphasnopendingWrite, no currentobsoletefixturefix. Keep separatefromdeferredreplayflush, latergridisolationquestions, no runtimeblocking/worklossinferred.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g176 record](records/review-through-00de0b6c/groups/g176.json). Record ID: `c3`.

