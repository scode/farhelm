# Removing the replay suite loses the legacy replay-tail oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Farhelm still serves existing tmux sessions, but this deletion removes their regression check
that a terminal is revealed only after its complete retained replay. An early marker or duplicate marker on a deep
legacy reattach is no longer rejected by that oracle. This is a confirmed test gap from source, not an observed product
failure.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g140/f4`.

### g140/f4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: Farhelm still serves existing tmux sessions, but this
deletion removes their regression check that a terminal is revealed only after its complete retained replay. An early
marker or duplicate marker on a deep legacy reattach is no longer rejected by that oracle. This is a confirmed test gap
from source, not an observed product failure. Details: the deleted
a_deep_scrollback_reattach_marks_only_after_the_last_replay_chunk (old replay_marker.rs:575-605, against the reviewed
commit's parent) seeds 4,000 rows, requires the retained head and spam-line-4000 before the marker (:592-598), asserts
more than one 32-KiB chunk (:600-604), and uses assert_marker_splits (:422-454) to count exactly one marker after
draining later frames. The concurrent-output scenario also checked replay/live separation. Those contracts still apply
to legacy terminals: SPEC_impl.md:2956-2958 says this section covers old terminals for as long as both kinds exist and
the supervisor still serves them; connection.rs:1627-1645 describes the unchanged marker as after every replay byte and
before any live byte. Actual remaining coverage was checked by pinned git grep for ReplayComplete/replay_complete across
supervisor source, e2e tests and helm source, then direct source inspection. Grid-stream tests.rs:300-332 checks marker
position after the FIRST history message and later eventual convergence; :335-379 checks eventual multi-read
convergence. These use a host GridStream, not the legacy Forwarder. handlers.rs:11034-11063 and :11475-11501 establish
marker arrival on small live legacy cat attachments (including channel 2), but do not seed or assert a multi-chunk
replay tail. connection.rs:2294-2481 has a small READY initial replay and asserts markerless death recovery, not deep
initial replay completion. Helm client.rs:7161-7208 and terminal.rs:1480-1504 synthesize the marker, so test relay
ordering rather than the supervisor's emission boundary. Core tests/host_terminals.rs:3329-3354,3433-3451 retain a grid
Attached-before-data check and non-default channel 2 arrival. Therefore non-default correlation, grid
Attached-before-data, and markerless death recovery are NOT asserted as wholly lost. Source correctness alone does not
refute this coverage loss. Suggested fix: retain a focused legacy raw replay-tail/exactly-once regression using a
test-owned tmux fixture, even though production no longer creates tmux terminals. Suggested bucket: other, regression
coverage; confidence: confirmed through source/contract comparison. proposed_drop: false, replacing the initial
candidate's insufficient source-correctness rationale. possible_cover: none known for this exact oracle; no excluded
metadata or queued findings searched. Could-be-highest: no established security or loss-of-work route; terminal replay
regressions are the potential consequence, while the established defect is lost test coverage. Material caveats: no
current product regression claimed and no runtime reproduction performed; obsolete same-pane/new-tmux creation mechanics
need not be preserved. This revision corrects the deleted test's exact line range and confirms the scripted nature of
the relay tests without changing the consequence.

Returned proposal (claims-089, event 10): **retain-verified**, suggested bucket **other**.

Deletion removed a discriminating deep legacy replay-tail/exactly-once oracle while that terminal path was still
supported. Later retirement removes the product path; preserve that historical scope rather than demanding legacy
fixtures at current main.

Original stronger consequence: Initial candidate proposed dropping all replay-suite loss because replacement
implementation/tests exist. That rationale was withdrawn. Final claim is specifically deep legacy
replay-tail/exactly-once regression protection, not no marker tests, not total ordering-coverage loss, and not a
demonstrated replay failure.

Unresolved premise or limitation: No missing source premise for the bounded intermediate coverage loss. Current
actionability needs the parent's recording judgment because later in-range retirement removed the tested substrate.

Proposed basis: Preserve the source-established historical finding and the later retirement qualification. Do not
silently turn later product correctness or removal into refutation of the intermediate test defect.

Existing parent disposition (event 10): rejected. Parent read full claims089.647f0227 removes deep4000row/>32KiB legacy
replay-tail/exactly-one-marker test while existing legacyterminal service remains supported. Smaller
marker/channel/host-grid convergence tests do not prove that exact legacy multi-chunk tail boundary; real intermediate
coverage gap, preserve narrowed scope. dbfc6975 then retireslegacyterminals atstartup and1ed482 removesdriver beforetip,
making a currentlegacyfixture repair obsolete. No residual host replay-oracle deficit demonstrated by this record; no
runtimeoutputfailure/security/workloss. Historical finding and later substrate retirement remain explicit rather than
retroactive source-refutation.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g140 record](records/review-through-00de0b6c/groups/g140.json). Record ID: `f4`.
