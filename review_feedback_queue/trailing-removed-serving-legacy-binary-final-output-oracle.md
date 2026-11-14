# Deleting the immediate-exit test loses the real-ticker legacy final-output oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Deleting the immediate-exit test loses the real-ticker legacy final-output oracle

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g141/l7`.

### g141/l7

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 17 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513

## TLDR

At this intermediate revision old tmux terminals still run, but their test no longer checks that an already-attached
viewer receives a program's final binary write or its correctly reset rendered recovery when the program immediately
exits. Host-terminal tests cover a different output path, and the retained legacy recovery test bypasses the real
lifecycle observer. This is lost test protection, not an observed output-loss bug.

## Details

The assigned patch removes non_utf8_terminal_output_survives_live_stream from session_lifecycle.rs (old-side :637-746
around function :651 at the exclusive base; the immediate parent carries the same test before deletion). It starts
Supervisor::serve(), handshakes after the accept loop/ticker is live, creates the gated binary fixture, attaches through
the real client, consumes initial replay, waits for fixture readiness and sends the stimulus. The existing attachment
must then receive raw 0xff before any reset, or ESC c followed by UTF-8 replacement-character BINARY-MARKER, within
bounded time/output; the serving supervisor must remain alive. The raw-versus-rendered distinction prevents a lossy live
decoder from satisfying the recovery branch merely by emitting U+FFFD. No reattach is allowed to retrieve the final
output.

The applicable legacy contract is explicit at this reviewed revision: SPEC_impl.md:2956-2963 says old terminals remain
served while fresh terminals use hosts; :3105-3107 says "The supervisor streams raw pane output to the client";
:3128-3137 describes the raw control stream versus rendered capture-pane recovery. The changed creation policy does not
make existing legacy attachment output obsolete. The retained actual test at service/connection.rs:2294-2481,
pane_death_recovers_missing_final_output_after_client_resume, creates a real tmux producer but manually builds
Forwarder/ActiveAttach, suppresses live output with a seam, probes pane death itself, and calls
ticker::publish_pane_deaths directly twice (:2427-2433). It requires ASCII FINAL-OUTPUT after a reset and exactly one
markerless recovery. That is meaningful deterministic coverage of Forwarder recovery and pause handling; it neither
sends non-UTF-8 payload nor runs serve()/the actual ticker-to-publication path. Its own comment (:2288-2292) says the
e2e repetition exercises the scheduling race without the seam. The removed test is that complement. A break in actual
lifecycle observation/publication could therefore pass the retained direct-publication test; a lossy legacy live decode
could pass the ASCII payload checks.

Pinned source searches for BINARY-MARKER, binary fixture use, non_utf8_terminal_output, non-UTF-8/byte-clean output and
death recovery across Rust e2e, service and tmux source found no equivalent retained real-serving legacy test at
647f0227. tmux/stream.rs:1320-1335 still names the removed test as the diagnostic trace used for missing final
notifications. codec-unit tests exercise decoding, not this lifecycle/client boundary. c6 retained real-host,
host-runtime and grid final-output tests remain valid for their host pipeline and do not refute this historical legacy
gap.

Whole-range attribution: exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a contains the complete real-serve binary
test at session_lifecycle.rs:637-746. The relevant path history to reviewed 647f0227 contains 47f33bbd,70bc66a9
and647f0227; the assigned deletion at647f0227 removes the still-applicable oracle. The next in-range
dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 ends old terminals at startup, followed by driver removal
in1ed482dde220452e424973faf38a5e1318986333. No change to connection.rs or tmux/stream.rs occurs between647f0227
anddbfc6975. The finding is bounded to supported legacy service before retirement, not a demand to retain a legacy test
at final tip. A focused test-owned preexisting tmux fixture plus a real serving supervisor can preserve the oracle
without restoring production tmux creation.

Suggested bucket: other (historical test-only coverage). Confidence: confirmed source/contract and actual retained-test
comparison; no execution. proposed_drop:false for this legacy scope. c6 original host/grid source refutation and its
proposed-drop basis remain valid separately. Could-be-highest:no established security or user-work-loss route from test
deletion; no current output loss asserted. possible_cover: supplied g140-f4 tests deep initial replay-tail/exactly-once
ordering and g140-f5 tests signal-driven no-output teardown of two legacy clients; neither has this final binary
write/live-versus-rendered/no-reattach lifecycle oracle. They share the retirement sweep but protect distinct
boundaries, so no same-cause cover is claimed. No excluded coverage metadata or prohibited queue material searched. No
runtime tests, reproductions, live-install actions or production modifications.

Returned proposal (claims-090, event 17): **retain-verified**, suggested bucket **other**.

At the deletion revision, adopted legacy terminals still had a shipped byte-output path. Removing the real-serving
binary immediate-exit test removed protection for that path before retirement. The retained manual recovery test checks
a different boundary.

Original stronger consequence: Loss of the already-attached viewer's final binary-write/reset-recovery oracle through
the real serve/ticker/client lifecycle. This was a lead until the direct-publication ASCII test was read; that check
establishes the bounded test gap, not a shipped output-loss bug. Host final-output tests and the separate
deep-replay/two-client teardown concerns remain distinct.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain as historical test-only feedback bounded to 647f0227 before legacy retirement. No current legacy
test restoration demand follows automatically. g141/c6 is the separate host/grid refutation, not an alias or coverage.

Existing parent disposition (event 17): rejected. Parent read full claims090.647f0227 removes realserve/ticker/client
alreadyattached immediate-exit binary0xff/resetrecovery test while existinglegacybyteoutput stillsupported.
DirectASCII/manualpublish test cannot detect omittedreal-ticker publication orbinary/live-rendered distinction;
historical testgap confirmed. dbfc6975 retiresoldterminals and1ed482 removesdriver beforetip, so no currentlegacyoracle
restoration demanded. Hostgrid finaloutput/exit tests and c6's obsolete bytecontract refutation remainvalidseparate; no
currentproductoutputloss/security/workloss. Preserve originalleadresolution, exacttestloss and laterretirement instead
ofclaiming gapneverexisted.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g141 record](records/review-through-00de0b6c/groups/g141.json). Record ID: `l7`.
