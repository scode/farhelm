# Intermediate loss of late-xterm-flush recovery coverage

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Intermediate loss of late-xterm-flush recovery coverage

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g176/c2`.

### g176/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR considered: deleting the held replay-write callback
regression might let a dead attachment stop recovery or replace its outage card. Source refutation of a new functional
regression: the assigned patch only removes the old-terminal-tagged xterm-specific test. terminal.js:5202-5218 retains
the alive/ended checks before reveal and recovery cancellation, and writeBytes at 5335-5344 retains its dead-mount
callback guard. Its grid branch at 5220-5233 flushes and reveals synchronously rather than waiting for term.write
callbacks. The adjacent late-evidence-cannot-confirm-a-closing-socket test remains at terminal-reconnect.spec.ts:2037
onward for the shared transport-ending boundary. Suggested bucket: other if a test-coverage concern; confidence:
confirmed source-only refutation of behavior regression. proposed_drop: no changed functional path and the removed
callback mechanism is xterm-specific, per sources above. possible_cover: none. Could-be-highest: no; stranded recovery
is a UX issue, not established security or loss of work. Caveat: this does remove distinct old-renderer regression
coverage, and the late-evidence test is not claimed to exercise a held xterm callback. Unlike l1, no explicit spec
requirement to retain this exact dependency test was found; removal alone does not substantiate a current recovery
failure.

Synthesis turn 2 — correction of original disposition: absence of a changed production path does not refute test-only
coverage loss. Preserve the original suspected consequence (a dead replay flush retires its successor retry or replaces
the outage card), but supersede the original proposed_drop: proposed_drop is now false, no drop accepted. At
647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513 old terminals still use term.write callbacks. The removed test specifically
receives a replay marker, holds that old terminal write completion, closes its socket, starts recovery, then releases
the callback. Removing the ended guard in terminal.js:5215 could reintroduce that race without the deleted witness.
Retained terminal-reconnect.spec.ts:2037 onward delivers evidence after socket ending, before attach proof; it does not
hold a successful marker flush across a later socket death. Retained terminal-replay-rename.spec.ts:1091-1171 holds the
marker itself while navigating away/remounting and checks the alive guard, not a write completion on a socket-ended but
still-owned mount. A search across retained intermediate e2e found no heldReplay/late-flush/Terminal.prototype.write
witness for this boundary. Thus this is a confirmed distinct intermediate regression-coverage loss, suggested bucket
other, confidence source-only; it is not a confirmed production failure.

Later retirement is a separate fact: f560058da19e02716e6f943883a9508b447aff4e deletes the writeBytes/term.write and
byte-renderer path. At tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db terminal.js:3756-3785 reveals via synchronous
grid.flush, endCatchUp:3878-3915 calls reveal directly, and no deferred write completion exists. The shared dead-socket
contract remains explicit in tip SPEC_impl.md:778 and the retained late-evidence-cannot-confirm-a-closing-socket at
terminal-reconnect.spec.ts:2047-2155 establishes actual successor socket creation and successful input after releasing
held real frames. This is meaningful coverage of the surviving queued-frame race, not equivalent coverage of the retired
callback mechanism. No recommendation to restore an xterm callback test on tip. Original lower confidence framing and
proposed-drop rationale remain above as history, explicitly superseded for the intermediate coverage loss.
possible_cover none; could-be-highest no current security/loss-of-work route; no execution/reproduction.

Returned proposal (claims-110, event 12): **needs-parent**, suggested bucket **other**.

The latest correction is valid: unchanged production code does not refute independent loss of the held-flush regression
witness. That witness has a real intermediate subject, then loses it when all deferred xterm writes are removed. Parent
must decide how to record an intermediate gap with no remaining callback mechanism.

Original stronger consequence: A dead replay flush could retire its successor retry or replace the outage card. The
first event proposed dropping this because no production path changed; latest line 12 explicitly supersedes that
proposed drop and retains confirmed intermediate regression-test loss. The later callback retirement is separate from
functional correctness at the deletion revision.

Unresolved premise or limitation: No remaining deferred xterm flush exists at tip; no actual recovery failure was
observed. A present-code repair would need a surviving equivalent asynchronous completion boundary.

Proposed basis: Preserve confirmed intermediate lost protection and the withdrawn original dismissal rationale. Do not
restore an obsolete callback test or assert a current shipped recovery defect.

Existing parent disposition (event 12): rejected. Full110:647 deletes
heldreplacementxtermwrite-callback-aftermarker/socketdeath/release/outage/laterliveecho witness
whiledeferredonWrittenstilllives. Unchangedaliveguardsdonotrefutelosttestprotection;
adjacentlateevidenceduringsocketcloseisnotheldcompletedmarkerflush. f560removesallterm.write/writeBytescallbacksbytip,
synchronousgridflushdifferentboundary. Preserveintermediategap andinitialdismissalwithdrawal, no
currentobsoletecallbacktestfix/runtimefailure.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g176 record](records/review-through-00de0b6c/groups/g176.json). Record ID: `c2`.
