# Mouse test claims to check SGR viewer restoration that the grid protocol does not carry

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The rewritten mouse-test rationale claims coverage of SGR encoding restored to a browser, although the browser never receives that encoding setting. It still usefully checks SGR mouse input after reattachment.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g178/g178-l2`.

### g178/g178-l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. TLDR: The rewritten mouse-test rationale claims coverage of SGR encoding restored to a browser, although the browser never receives that encoding setting. It still usefully checks SGR mouse input after reattachment. Details: the changed comment at e2e/tests/mouse-modes.spec.ts:390-395 claims to pin a branch that sends SGR encoding to a returning viewer. Complete TerminalState at crates/farhelm-proto/src/grid.rs:1127-1153 has no encoding field; MouseMode at :1098-1108 exposes only off/click/drag/motion. grid-terminal.js:1630-1648 builds coordinate/button messages and explicitly says the client cannot see encoding. service/grid_stream.rs:1064-1077 reads the retained terminal modes, and grid_input.rs:737-745 chooses SGR there. The test's :399-405 counts reports after switching session selection through reattachWithModes (:247-257), which closes/replaces only its browser socket. Additional synthesis caller/lifetime checks settle the remaining premise: service/handlers.rs:1898-1916 states the host runtime is already connected and creates HostAttach from terminal.clone(); grid_stream.rs:1022-1040 subscribes to that host's retained status and creates a fresh per-attachment InputEncoder. hosts/terminal.rs:1205-1227 establishes shared runtime ownership across cloned handles, backed by an Arc, separate from viewer attachment. Thus a new attachment/encoder is exercised, but no browser SGR field or emulator checkpoint restoration is exercised. Preserve original consequence: misleading coverage documentation can cause maintainers to believe this test exercises SGR viewer replay/restoration. Correct its rationale to post-reattach mouse delivery. Do not infer a missing product field, a need for a client encoder, or a product encoding defect. Related DECSET-replay wording at :357-358 is stale too, but the finding targets the newly changed rationale. Suggested bucket: other. Could-be-highest: no established security or loss-of-work route. Confidence: confirmed protocol, implementation and caller/lifetime source analysis; no execution observation. proposed_drop: false; no lowering or accepted drop. possible_cover: none; no queue/excluded metadata search. This revision records the actual runtime/attachment boundary and preserves the narrower coverage-documentation consequence; no premise remains open.

Returned proposal (claims-111, event 10): **retain-verified**, suggested bucket **other**.

The changed comment says the test pins SGR encoding sent to a returning viewer. The viewer receives reporting mode, while the supervisor selects the encoding from retained terminal modes. The test usefully checks post-reattach mouse delivery, not the stated viewer restoration branch.

Original stronger consequence: False confidence that the reconnect test covers SGR viewer replay/restoration. The initial lead was about whether assertions observe that mechanism, not an established product encoding failure. Preserve the related stale DECSET-replay wording caveat; the changed rationale is the finding's target.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record the misleading test-coverage explanation as other; correct its contract to SGR mouse delivery after a fresh browser attachment.

Existing parent disposition (event 10): verified. Parent explicitly accepted named retain-verified proposal from claims-111 after inspecting it. The changed comment says the test pins SGR encoding sent to a returning viewer. The viewer receives reporting mode, while the supervisor selects the encoding from retained terminal modes. The test usefully checks post-reattach mouse delivery, not the stated viewer restoration branch. Record the misleading test-coverage explanation as other; correct its contract to SGR mouse delivery after a fresh browser attachment. Checked source/results: ["f560058d:e2e/tests/mouse-modes.spec.ts:390-405 explicitly claims SGR encoding restoration from what the viewer is sent, then counts SGR reports after reattachment. The text remains at 00de0b6c and recording main 24bc0580.","f560058d:crates/farhelm-proto/src/grid.rs:1098-1108,1127-1153 exposes only mouse reporting mode and no mouse encoding field in complete TerminalState.","f560058d:grid-terminal.js:1630-1648 constructs coordinate/button reports and explicitly says the client cannot see the encoding. service/grid_stream.rs:1064-1079 reads retained modes and encodes input; grid_input.rs:737-745 chooses SGR.","The false test explanation is source-established. This supplies no missing product field, encoder defect or runtime failure."] Current coverage/results: ["SPEC_impl.md The grid client:1836-1868 assigns interpretation and input encoding to the supervisor. SPEC.md Terminal experience's mouse behavior does not accept fictitious viewer encoding restoration coverage.","TODO Planned empty; BUGS, active TRIAGE_OUTCOMES, INDEX and matching queue searches for SGR/viewer restoration and mouse-test rationale found no exact cover.","plans/queue/tmux-drop-from-docs.md's rationale sweep is related context but already landed (REPORTS:32), and the incorrect text survives at recording main. Other plan hits concern unrelated layout or history numbering. No unlanded fix found.","FILTER does not cover a persistent misleading test contract. No exact alias or existing item extension identified."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "False confidence that the reconnect test covers SGR viewer replay/restoration. The initial lead was about whether assertions observe that mechanism, not an established product encoding failure. Preserve the related stale DECSET-replay wording caveat; the changed rationale is the finding's target."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g178 record](records/review-through-00de0b6c/groups/g178.json). Record ID: `g178-l2`.

