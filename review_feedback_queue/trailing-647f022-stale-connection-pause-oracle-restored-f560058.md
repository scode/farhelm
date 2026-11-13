# Stale-owner pause coverage lost at 647f022 is restored at f560058

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The initial deletion removes a useful regression check, but an equivalent host-terminal test restores it before the pinned tip. Original consequence preserved: losing the test would allow a regression in which a displaced connection pauses a replacement with the same numeric channel to go undetected; no such production defect was established. Exact sensitivity: deletion-revision service/connection.rs:2225-2230 and tip :1552-1557 select an attachment with `.find(|a| a.channel == channel && a.notify.same_channel(tx))`. Omitting `a.notify.same_channel(tx)` is a meaningful competing implementation: after takeover, the only current attachment under the session/terminal key belongs to the winner and still has channel 1, so the loser’s PauseOutput would set the winner’s pause watch. The channel-only comparison passes; the connection-identity comparison rejects it. This is separate from sibling stall isolation. Test evidence: `git log -S` identifies f560058 as the addition of core/tests/host_terminals.rs::a_pause_from_a_client_that_lost_a_takeover_cannot_silence_the_winner (:3654-3707 there; :3649-3702 at tip). GridPeer::connect creates separate duplex connections through actual handle_connection (:3248-3273 at tip); the test attaches loser and winner on channel 1, requires the loser’s TakenOver notice, and waits for winner catch-up. It then executes `loser.send(&ControlMsg::PauseOutput { channel: 1 }).await; loser.round_trip(2).await; test_host.output(b"after the losing pause\r\n");` (tip :3689-3691). Its wait requires that new text or a detach, followed by asserting no detach, so

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g153/l1`.

### g153/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (finding, open):

Reviewed deletion commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513; later repair commit: f560058da19e02716e6f943883a9508b447aff4e; verified surviving at pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: The initial deletion removes a useful regression check, but an equivalent host-terminal test restores it before the pinned tip. Original consequence preserved: losing the test would allow a regression in which a displaced connection pauses a replacement with the same numeric channel to go undetected; no such production defect was established. Exact sensitivity: deletion-revision service/connection.rs:2225-2230 and tip :1552-1557 select an attachment with `.find(|a| a.channel == channel && a.notify.same_channel(tx))`. Omitting `a.notify.same_channel(tx)` is a meaningful competing implementation: after takeover, the only current attachment under the session/terminal key belongs to the winner and still has channel 1, so the loser’s PauseOutput would set the winner’s pause watch. The channel-only comparison passes; the connection-identity comparison rejects it. This is separate from sibling stall isolation. Test evidence: `git log -S` identifies f560058 as the addition of core/tests/host_terminals.rs::a_pause_from_a_client_that_lost_a_takeover_cannot_silence_the_winner (:3654-3707 there; :3649-3702 at tip). GridPeer::connect creates separate duplex connections through actual handle_connection (:3248-3273 at tip); the test attaches loser and winner on channel 1, requires the loser’s TakenOver notice, and waits for winner catch-up. It then executes `loser.send(&ControlMsg::PauseOutput { channel: 1 }).await; loser.round_trip(2).await; test_host.output(b"after the losing pause\r\n");` (tip :3689-3691). Its wait requires that new text or a detach, followed by asserting no detach, so a paused winner cannot pass on pre-pause output. Processing witness verified through source, not just helper documentation: round_trip sends ListSessions and waits for its reply (tip :3365-3378); handle_connection reads successive frames in its loop and awaits handle_control (:347-359,582-583,709-718), whose PauseOutput arm awaits set_attachment_paused inline (handlers.rs:3016-3018) before the next ListSessions frame can dispatch. Thus a spawned ListSessions handler cannot produce its reply before that preceding pause has been handled. The old correct predicate alone did not refute a missing test; this later equivalent test does repair ongoing coverage. Suggested bucket: other; confidence: confirmed by source. could-be-highest: no established security or loss-of-work defect; regression-test coverage only. proposed_drop: propose dropping only the claim that stale-owner pause coverage is still missing at the pinned tip, based on the equivalent f560058 test and verified processing witness; do not erase the intermediate deletion or treat it as covered at 647f022. Parent has accepted no drop. possible_cover: later source test at the named revision, no excluded coverage search. Previous proposed narrowing preserved: retained sibling tests refute the broad claim that stall isolation is entirely untested, but never substitute for stale-owner PauseOutput authorization. No runtime execution or reproduction; all conclusions are source analysis. No unresolved premise remains for l1.

Returned proposal (claims-097, event 14): **refuted**, suggested bucket **other**.

An equivalent test added before the review tip restores the equal-channel/different-connection stale-pause oracle. This refutes ongoing missing coverage, while preserving the intermediate deletion and earlier narrower stall-isolation disposition.

Original stronger consequence: Initially both displaced-viewer pause ownership and cross-attachment stall isolation appeared untested. The latter was narrowed after retained sibling tests were read. The stale-owner oracle really disappeared at 647f022, but was restored at f560058; neither narrowing establishes or refutes a production authorization defect.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Drop only ongoing stale-owner missing-coverage applicability at the tip, subject to the parent's independent check because the oracle concerns ownership enforcement. Preserve the historical gap, all prior caveats and the narrower stall-isolation conclusion.

Existing parent disposition (event 14): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g153 record](records/review-through-00de0b6c/groups/g153.json). Record ID: `l1`.

