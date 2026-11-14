# Fence the initial feed greeting before measuring silent probe invalidation

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Fence the initial feed greeting before measuring silent probe invalidation

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g069/l3`.

### g069/l3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 16 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462

## TLDR

The delayed-probe browser tests can accept a recipe-status refresh caused by the initial feed greeting as evidence that
probe completion invalidated the status without a feed notice. They do not establish the silent, healthy-feed premise
before switching the fixture to ready, so the regression they intend to catch can be masked. This is a test-oracle
defect, not an observed product failure.

## Details

The new tests arm feed.notifyOnConnect(1) at line 793 and wait only for feed.waitForConnection(1) at 884.
helpers/fleet.ts:663-700 counts a connection and marks its test-side socket open before sending the greeting;
waitForConnection at 723-732 waits only for that count. Neither confirms browser delivery, Rust consumption, or
completion of the mounted recipe-status consumer's greeting work. The test then waits for oldReason and opening probe
counts at 903-906, captures recipeReadsBefore and sets settled=true at 908-912. Its later oracle accepts any recipeReads
increment and the ready display, with one socket and no extra probe, at 913-926.

There is an independent recipe-status refresh route: create_form.rs:3226-3249 restarts build_status on every feed
notice; tensorlake/recipes.rs:378-385 likewise restarts place_status. feed.rs:289-315 observes notices in a queued
effect and spends them by calling the consumer; its docs explicitly say an effect run is queued work. FleetFeed receives
the JS report asynchronously through channel.recv at feed.rs:419-424. assets/events.js:311-332 emits that report only on
socket.onmessage, after the fixture's socket-arrival counter has already advanced. No tested dependency orders this
report/consumer completion before the line-908 baseline. A source-permitted order is: mount the consumer and display a
stale initial recipe response; admit its opening probes; capture the baseline and set settled=true; then consume the
already-sent initial greeting and restart the recipe-status resource. That later request reads the fixture's now-ready
values at lines 829-845, satisfying both the read-count and display oracles. It forces no additional probe because
use_environment_places handles a feed notice with refresh=false (environment_ui.rs:713-714), and needs no second
connection. The separate readyPlaceReads condition can be met by the intended refreshing-place poll at
environment_ui.rs:715-725. Thus even if the recipe resource lost its observed_places dependency, this alternative
refresh could pass the test.

The exact distinction matters: response-counting before fulfill alone is not the defect, and a genuinely late
probe-triggered response would exercise the intended path correctly. The missing attribution is whether the greeting's
recipe refresh is already retired before the no-notice interval starts. Socket openness/arrival does not establish
FLEET_FEED.healthy either; feed.rs:121-139 expressly says health begins only on a delivered frame. Establish greeting
consumption and the relevant consumer's idle/settled state before setting settled=true, or mount the tested surface only
after an acknowledged greeting and fence its opening reads. The existing waitForFeedReadersSettled helper documents this
separation but only measures list/session/sound readers (helpers/fleet.ts:90-153), so using it alone would still need to
account for the recipe resource.

Contract: the changed test's own comment at lines 784-787 says 'A healthy, silent fleet feed cannot repair the stale
recipe-place response; the mounted consumer must invalidate it when the probe settles.' The supplied test-authoring
checklist requires establishing fixture premises and an observable that distinguishes competing mechanisms. Suggested
bucket: other (maintainer test evidence). Confidence: confirmed source-level unfenced alternative, with
occurrence/frequency unmeasured; no execution or reproduction. proposed_drop: false. Could-be-highest: false; no product
security or loss-of-work route. possible_cover: g026/f4 concerns another test's consumer attribution, not this
delayed-probe oracle; no forbidden coverage sources searched. Caveats: ordinary fast greeting consumption will usually
make these tests exercise the intended mechanism; no current product invalidation bug is claimed. Attribution checked
across the entire pinned range: git log for this assigned file shows only its introduction at this commit and the later
connected-container Delete insertion at 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa. This oracle gap is introduced with the
assigned test, not merely inherited from the immediate parent. It is distinct from l1/c3/l2 cleanup lifetime findings.
The lead's original suspected false-pass consequence is retained.

Discovery-pass completion checks: the full assigned-file diff from this commit to pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db contains only the later openRowMenu import and connected-container Delete test
insertion; the delayed-probe body and its feed fence are unchanged. Exact helper line check confirms connection
readiness at fleet.ts:723-732 is just a count loop. Reviewed the remaining placement, restored-select request binding,
focused-history consumption, ordinary-folder boundary, refusal, leftover confirmation and quick-launch assertions
without identifying another substantive issue in this pass. This does not revise, drop or lower earlier IDs.

Returned proposal (claims-044, event 16): **retain-verified**, suggested bucket **other**.

The delayed-probe tests count a later recipe read without retiring the initial greeting's recipe consumer. A late
greeting can provide the tested refresh independently of changed-place invalidation. This is a conditional test-oracle
defect; no product invalidation failure or actual false pass was observed.

Original stronger consequence: The original lead's suspected false pass on an initial greeting refresh is retained. The
record claims test evidence can be masked, not that product probe invalidation is broken.

Unresolved premise or limitation: No missing source premise for the unfenced alternative. Actual greeting scheduling,
occurrence and false-pass frequency were not measured. Fast greeting consumption will ordinarily exercise the intended
invalidation path.

Proposed basis: Retain the missing attribution witness, with its conditional scheduling and no-runtime caveats.

Existing parent disposition (event 16): verified. Parent explicitly accepted named retain-verified proposal from
claims-044 after inspecting it. The delayed-probe tests count a later recipe read without retiring the initial
greeting's recipe consumer. A late greeting can provide the tested refresh independently of changed-place invalidation.
This is a conditional test-oracle defect; no product invalidation failure or actual false pass was observed. Retain the
missing attribution witness, with its conditional scheduling and no-runtime caveats. Checked source/results:
["350a4b33b6d4476f4a78e43cea124bc2b828f462:e2e/tests/podman-environments.spec.ts:784-926 arms the greeting, waits for
socket connection, displays stale status and admits probes, then snapshots recipeReads and switches settled. The final
oracle accepts any increment with ready status, unchanged probe count and one connection.","Same
revision:e2e/tests/helpers/fleet.ts:663-700 increments connections and open-socket state before sending the greeting;
:723-732 waits only for the count. It does not acknowledge delivery or recipe-reader settlement.","Same
revision:crates/farhelm-ui/src/feed.rs:289-315 handles notices in an effect, and :419-424 receives asynchronous JS
reports. assets/events.js:311-332 reports on message delivery. Neither route is fenced by the test's connection
count.","Same revision:crates/farhelm-ui/src/list/create_form.rs:3216-3249 and tensorlake/recipes.rs:364-385 restart
recipe resources on feed notices independently of observed_places. environment_ui.rs:713-725 handles notices without a
new probe and polls while refreshing, so the alternative need not violate either final counter assertion.","The assigned
test file is newly added at 350a4b33; git log over the full pinned range shows the later Delete insertion as its only
other change. Pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the same delayed-probe baseline and oracle at
approximately 973-1002."] Current coverage/results: ["Both specifications searched for delayed probe, feed greeting,
silent feed, recipe refresh and place invalidation; SPEC.md Container sessions requires background probes and
SPEC_impl.md Disposable environments describes cached readiness. Neither accepts an unfenced regression
oracle.","TODO.md Planned empty. plans/queue searched for greeting/drain/probe/recipe-refresh premises; related Podman
and agent-operation plans are landed (plans/REPORTS.md:21,28). No unlanded work removing this oracle's cause
found.","BUGS.md's transfer/SSH-helper issue has a different trigger and scope. Active TRIAGE_OUTCOMES.md searched for
named test, initial greeting and probe/recipe terms; no exact decision found. Archive not read.","INDEX.md and matching
queue files checked. outage-recovery-test-can-credit-ordinary-probe.md is another attribution defect, but its
independent producer is a terminal reconnect timer, not this greeting-driven recipe read.
staged-start-test-lacks-reader-drain-witness.md concerns terminal ownership and process start. Neither fixes or covers
this test. No exact duplicate found.","FILTER.md does not filter a false regression oracle merely because its
alternative ordering is rare; it is not wholly a self-correcting display or imprecise diagnostic."] Exact cited cover:
[] Original stronger consequence and missing premise remain preserved: "The original lead's suspected false pass on an
initial greeting refresh is retained. The record claims test evidence can be masked, not that product probe invalidation
is broken."; "No missing source premise for the unfenced alternative. Actual greeting scheduling, occurrence and
false-pass frequency were not measured. Fast greeting consumption will ordinarily exercise the intended invalidation
path.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g069 record](records/review-through-00de0b6c/groups/g069.json). Record ID: `l3`.
