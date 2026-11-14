# Deleting replay degradation tests also removes surviving grid idle-bound assertions

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the browser tests stop checking that an actively progressing replay stays hidden until it
finishes, and that an already-connected replay reveals after genuine silence even while empty frames keep arriving.
Those protections still matter for the current terminal; an incorrect timer can reveal a deep replay early or leave the
pane hidden indefinitely without these tests noticing. This is a test-sensitivity defect, not an observed production
failure. Original wording/consequence preserved: 'Removed xterm replay tests leave an acknowledged coverage gap, not a
demonstrated regression'; the removal also leaves old supported terminals without size/chunk/idle/ending protection at
the intermediate revision. Correction: a production defect need not be introduced for a lost useful regression assertion
to be reportable. Later f560058da19e02716e6f943883a9508b447aff4e deletes xterm and the byte-size/chunk buffer bounds, so
those particular subjects do disappear before pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. It does not retire
the idle policy. Source basis: the deleted parent test at 715-781 injects four real payloads over a total longer than
the idle window, asserts no reveal while progress continues, then asserts an idle reveal and retained content. The
deleted parent test at 1392-1457 requires reveal while bounded empty-frame traffic is still active and observes another
empty frame after reveal. At pinned tip terminal.js:3633-3729 clears and rearms the catch-up timer; 4152-4166 ignores
zero-length frames; 4465-4476 rearms on accepted grid frames. Its module contract at 93-

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g177/l1`.

### g177/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: the browser tests stop checking that an actively
progressing replay stays hidden until it finishes, and that an already-connected replay reveals after genuine silence
even while empty frames keep arriving. Those protections still matter for the current terminal; an incorrect timer can
reveal a deep replay early or leave the pane hidden indefinitely without these tests noticing. This is a
test-sensitivity defect, not an observed production failure. Original wording/consequence preserved: 'Removed xterm
replay tests leave an acknowledged coverage gap, not a demonstrated regression'; the removal also leaves old supported
terminals without size/chunk/idle/ending protection at the intermediate revision. Correction: a production defect need
not be introduced for a lost useful regression assertion to be reportable. Later
f560058da19e02716e6f943883a9508b447aff4e deletes xterm and the byte-size/chunk buffer bounds, so those particular
subjects do disappear before pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. It does not retire the idle policy.
Source basis: the deleted parent test at 715-781 injects four real payloads over a total longer than the idle window,
asserts no reveal while progress continues, then asserts an idle reveal and retained content. The deleted parent test at
1392-1457 requires reveal while bounded empty-frame traffic is still active and observes another empty frame after
reveal. At pinned tip terminal.js:3633-3729 clears and rearms the catch-up timer; 4152-4166 ignores zero-length frames;
4465-4476 rearms on accepted grid frames. Its module contract at 93-124 explicitly requires reveal on idle and calls the
idle bound a trust boundary against a peer holding the terminal hidden. The same progress/empty-frame decisions existed
in terminal.js at the deletion revision:5725-5764 and its receiveGrid path. Actual replacement observables checked:
grid-attach.spec.ts pins marker, malformed stream, detach and close endings, including DOM visibility and retained first
row, not progressing-versus-silent idle behavior. Pinned terminal-replay-rename.spec.ts:927-1080 checks CONNECTING
expiry and delayed-main-thread expiry ending on a marker; it supplies neither a continuously progressing valid stream
longer than the window nor repeated empty frames. terminal-reconnect.spec.ts:1359-1431 tests completely muted
replacement sockets staying on the retry ladder, while 2075-2155 tests late evidence on CLOSING sockets; neither
distinguishes total elapsed time from progress silence or empty traffic. Targeted git grep across pinned e2e and UI
js-tests found no equivalent positive idle-reveal/progress/empty-frame assertion. Suggested fix: migrate the two policy
assertions to the static grid attachment harness, feeding valid grid frames and empty binary frames, retaining explicit
DOM visibility, reveal reason, ongoing-traffic and content witnesses. Do not restore byte/chunk buffering or require
xterm's one-write invariant for a grid. Suggested bucket: other, regression protection in tests; could-be-highest: no
established shipped security/loss-of-work route, and the source still implements the correct guard. Confidence:
confirmed by source and assertion comparison, no execution or reproduction. Proposed_drop: false. Possible_cover: none;
queue/coverage sources not searched. The commit description's acknowledgment is context only and is not treated as
specification acceptance.

Returned proposal (claims-110, event 7): **retain-verified**, suggested bucket **other**.

Two removed assertions protect idle semantics that survive in the grid-only terminal: valid progress must keep catch-up
hidden beyond the total idle duration, and empty traffic must not postpone reveal. Retained tests cover other endings
and do not distinguish those competing timer regressions.

Original stronger consequence: Old terminals lost size/chunk/idle/ending replay protection at the intermediate deletion
revision. An early event proposed dismissal because no production regression was shown and the author acknowledged the
gap; later events expressly correct that rationale. The surviving concern is early reveal during continued progress or
indefinite hiding under empty traffic if those correct guards later regress. Retired byte/chunk/xterm-write subjects
remain historical, not current repair requirements.

Unresolved premise or limitation: No current timer implementation failure or shipped security/work-loss route is
established. Highest would require such a concrete consequence; the confirmed impact is maintainer regression-test
sensitivity.

Proposed basis: Retain the two surviving grid idle-policy witnesses as other. The historical proposed dismissal
concerned a guard source calls a trust boundary, so parent should include it in independent-drop reconciliation even
though this recommendation retains the finding and asserts no current security failure.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g177 record](records/review-through-00de0b6c/groups/g177.json). Record ID: `l1`.
