# The stale-suspension browser test counts unrelated host reads as its readiness oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The stale-suspension browser test counts unrelated host reads as its readiness oracle

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g026/f4`.

### g026/f4

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: ba7cc0264028eebb39647deb1d25b055e6886935

## TLDR

The browser regression test can report a successful wake without establishing that the launch saw the stale suspension
it is supposed to tolerate. A regression in that integration path can therefore escape this test. This is a
test-evidence issue, not an observed product failure.

## Details

Synthesis rechecked the actual response boundary. fleet constructs each response's rewritten phase at :65-78, after
route.fetch and response.json complete (:63-64), then increments the global reads counter at :79 and fulfills that fixed
JSON at :86. Changing publishedState after that point cannot alter an already prepared response. Therefore the earlier
record's observation that counting precedes fulfill is NOT by itself a failure: if either counted response belongs to
the helper, the stale snapshot is already fixed and will exercise it even if delivery/processing happens later. The
remaining issue is consumer attribution, not late processing.

The scenario waits for two reads after answered.wait (:581-585), then publishes Connected (:595). Those reads include
the independent display owner (list/view.rs:2150-2155) as well as resume_host_connected (api.rs:5718,5736). The display
owner starts a mount read and a feed read (list/view.rs:2311-2333). fleet arms a revision-1 greeting (:49); stubFeed
sends that greeting on subscription (helpers/fleet.ts:697-700). The Resume route publishes another notice before
answering (:555-557). reader.rs:444-456 records a notice behind a currently running read, and :517-535 starts that
follow-up when the current read finishes. The test waits for session/terminal readiness (:572-573; attachSession in
helpers/term.ts:406-423), not display-host-reader idleness. Thus a revision-1 display read still awaiting route.fetch
when the answer baseline is captured, followed by the queued Resume-notice display read, can supply both counted
snapshots before the helper's own route.fetch resolves. Connected is then published, and the helper's response is
constructed as Connected. Its api.rs:5740-5741 immediately succeeds without passing a stale post-Resume suspension to
WakeWatch. This is a source-permitted ordering; no run established its frequency or occurrence.

Meaningful competing failure: integration could initialize/reset the helper's WakeWatch from a post-Resume non-suspended
phase instead of the saved pre-Resume suspension. The unchanged classifier would then reject a genuinely delivered stale
suspension as SuspendedAgain; if the test's two reads are both display reads and the helper first receives Connected,
that integration regression passes. The pure unit tests at api.rs:8918-8929 construct WakeWatch directly with a
suspension and therefore do not cover this helper wiring. The parent revision's helper already waits through an ordinary
suspension; this report does not claim the new test necessarily misses a particular pre-existing stale-suspension bug.

The Test authoring review requires 'Assert the fixture premise before the action or measurement that depends on it' and
'Assert an observable that distinguishes the competing mechanisms.' Bind readiness to a wake-owned snapshot/observation,
or explicitly drain and fence the display reader before relying on request counts. Suggested bucket: other (maintainer
test quality). Could-be-highest: no. Confidence: confirmed source-level ambiguity; the false-pass interleaving is source
analysis, not execution. proposed_drop: false. possible_cover: direct WakeWatch unit coverage, limited to classifier
semantics; no coverage-source searches. This revision narrows the initial finding and preserves its original consequence
while correcting the response-processing rationale.

Final readily-checkable setup premise: terminal-readiness.ts:364-380 shows waitForSessionReady checks only terminal
island, matching/open socket, replay reveal and focus. helpers/term.ts:386-392 checks only terminal text. Neither
observes or drains the host display reader. create_form.rs:6211-6212 starts resume_host_connected independently after
accepted form release; no shared reader lock fences its HTTP reads against list/view.rs:2155. These checks leave the
two-display-snapshot schedule source-permitted. No claim is made that this schedule occurred in a run, and
snapshot-before-fulfill alone remains explicitly refuted as an independent bug.

Returned proposal (claims-014, event 13): **retain-verified**, suggested bucket **other**.

The test's global response count cannot identify a wake-owned stale snapshot. Independent display reads can satisfy its
threshold before the helper's response is constructed. This is an established source-level test-oracle gap; its adverse
ordering was not run.

Original stronger consequence: The browser regression can pass without delivering the stale suspension to the wake
helper, allowing an integration regression to escape. Earlier count-before-fulfill processing rationale was explicitly
corrected and must not be resurrected as a separate defect.

Unresolved premise or limitation: No runtime schedule, frequency or mutation result. Those limits do not remove the
source-established absence of consumer attribution. The original parent helper already tolerated ordinary suspension; no
claim that this test misses a specific pre-existing product bug.

Proposed basis: Record the conditional test-oracle gap as other, preserving its source-only confidence and the corrected
response boundary.

Existing parent disposition (event 13): verified. Parent explicitly accepted named retain-verified proposal from
claims-014 after inspecting it. The test's global response count cannot identify a wake-owned stale snapshot.
Independent display reads can satisfy its threshold before the helper's response is constructed. This is an established
source-level test-oracle gap; its adverse ordering was not run. Record the conditional test-oracle gap as other,
preserving its source-only confidence and the corrected response boundary. Checked source/results:
["ba7cc0264028eebb39647deb1d25b055e6886935:e2e/tests/tensorlake-lifecycle.spec.ts:61-86 prepares the response after
route.fetch, counts every hosts response, then fulfills fixed JSON. :555-557 publishes a feed notice before answering
Resume; :581-585 waits for two unowned reads; :595 changes to Connected.","At that hash list/view.rs:2150-2155
independently fetches hosts, :2311-2314 starts a mount read, and :2328-2333 requests reads on feed notices.
reader.rs:444-456 retains a demand during a running read and :517-535 immediately follows it after completion.
helpers/fleet.ts:695-697 sends the armed greeting. These permit a pending display read and its follow-up to supply both
counted responses.","api.rs:5718-5719 captures the initial phase independently, :5736 fetches its post-Resume snapshot
and :5740-5741 immediately accepts Connected. The independently spawned helper at create_form.rs:6211-6212 shares no
display-reader fence. terminal-readiness.ts:364-380 observes terminal readiness only, not display-reader
idleness.","api.rs:8918-8927 directly tests WakeWatch with an initial suspension but does not establish the browser
helper's state wiring. A helper initialized/reset from the wrong phase could pass the browser case when it first
receives Connected; no claim is made that current product code has that regression.","Preserved correction:
count-before-fulfill is not independently defective. A counted wake response already contains the stale snapshot, so
later publication cannot rewrite it. The remaining defect is consumer attribution."] Current coverage/results: {"SPEC.md
and SPEC_impl.md":"SPEC_impl.md:6731-6742 explicitly requires tolerating the starting suspension. This does not accept a
browser test that fails to establish delivery of it. No exact test-oracle acceptance found.","TODO.md Planned":"Empty.
Plan-decisions-to-revisit discusses size of already accepted wake tests, not an accepted oracle
limitation.","plans/queue":"sandbox-suspend-overlap.md requires busy/new/stale suspension tests and already landed their
implementation; wake-launch-stuck.md addresses hidden-launcher and displayed-connection deadlines. Neither unlanded
stated work removes this response-consumer ambiguity.","BUGS.md":"Unrelated SSH transfer-output hang only.","INDEX.md
and queue files":"No exact global-read-count wake oracle item found. Read resume-preview-readiness-witness.md: its
independent-boundary defect concerns fresh checkout preview versus displayed Connecting and a different test. It does
not cover stale-suspension consumption. Other matching lifecycle test cleanup records concern different failure
mechanisms.","TRIAGE_OUTCOMES.md":"No matching active decision found. Archive not read.","FILTER.md":"No entire boundary
matches. This is durable missing test discrimination, not a rare self-correcting product display or safe retry
failure."} Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "The browser
regression can pass without delivering the stale suspension to the wake helper, allowing an integration regression to
escape. Earlier count-before-fulfill processing rationale was explicitly corrected and must not be resurrected as a
separate defect."; "No runtime schedule, frequency or mutation result. Those limits do not remove the source-established
absence of consumer attribution. The original parent helper already tolerated ordinary suspension; no claim that this
test misses a specific pre-existing product bug.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g026 record](records/review-through-00de0b6c/groups/g026.json). Record ID: `f4`.
