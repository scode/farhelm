# Resume test retires create stub before client settles

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a

## TLDR

Failure cleanup may remove the create stub before a pending launcher settles, allowing a real test session to start
afterward. The complete browser ordering remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `e2e/tests/tensorlake-lifecycle.spec.ts:642-650 at 1a57aa8e265cbb59cedc228433b189f59f7ad59a`.

Source assessment (read with the later corrections below) (review record g036/l6, event 16):

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a. TLDR: a failure while this browser test holds Resume may
allow its sleep-300 command to start as a real session after cleanup stops the create stub, leaving test state behind.
Synthesis rechecked ordering and cancellation. Test :592-597 unconditionally publishes Connected and answers success
after gate release; finally :643 releases the gate, :645 drains/removes only the Resume POST rewrite, :646
drains/removes the create rewrite, then :647-650 awaits preference/template restoration while the page remains open.
drainedRoute.ts:21-24 falls back when stopped; :39-42 marks stopped before draining only current callback work and
removes that handler. No barrier awaits subsequent client requests. The earlier fleet create routes do not exist: fleet
intercepts hosts and lifecycle only; afterEach at :136-142 drains fleet routes and unrouteAll without cancelling a
launcher. api.rs:5495-5505 issues private fetch_hosts AFTER Resume; create_form.rs:5737-5766 continues when that GET
reports Connected, :5789 awaits key minting, and :6078-6089 dispatches the ordinary create. Route callback completion is
therefore earlier than possible client settlement. Parent lifetime checked: list/view.rs:4453-4467 mounts the form while
show_create, and :4506-4517 unmounts it only on explicit cancellation; restoring quick_launch_template is not a
show_create condition. The mocked feed also does not forward real preference updates. Accessible cancellation checked:
create_form.rs:2951 documents unmount cancellation, but hidden progress retains the mounted form (:6368-6402); backdrop
:6364, Escape :6446, Close :6468-6472, and Cancel :7918-7925 all refuse while submission owns ops.busy_now. Thus
suggesting normal Cancel during held preparation would be incorrect. Local Playwright worker source was unavailable at
e2e/node_modules/playwright/lib/worker/workerMain.js and lib/index.js, so no claim is made about exact framework task
scheduling. Confidence: likely; unverified after investigation. Exact missing scheduling premise: private host GET/key
mint completes AFTER stopCreate marks interception stopped/removes it and BEFORE document/context teardown, with the
form still mounted. Static source establishes no ordering/cancellation barrier and an open page across subsequent
cleanup requests, but cannot establish this browser interleaving or its frequency. A deterministic failure-path
experiment holding the helper GET across stopCreate, then releasing it before context teardown and checking server
session state, would settle the premise. Runtime experiments remain forbidden by brief. Suggested bucket other, confined
to test resource leakage/cross-test pollution; no highest route. proposed_drop false; possible_cover none known.
Concrete remedy: a retiring flag makes the released Resume return a refusal instead of success, or explicitly tear down
the document/context while keeping the create refusal installed until task cancellation; do not remove interception
solely because the Resume route drained. Temporal: introduced in assigned new quick-launch test and unchanged through
pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. Existing l1/l2/l3/c4 refutations and exact drop bases remain
intact.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g036/l6`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.

## Additional ordinary-launch test retires the trap too early

Additional reviewed commit: `6e423e30068b62c6cfa276c6d720ee35bb70387a`, introducing change
`112fd9d812e95df7f69f66c85820d76faaf89582`.

The same client-settlement lifetime gap affects the new existing-host launch test in
`e2e/tests/tensorlake-sandbox-sessions.spec.ts:201-262`. It submits the ordinary command `true` at `232-234` and tracks
only its two setup sessions (`211-214`). If an assertion fails while Resume is held, finally releases that gate, then
disposes the fixture routes before deleting those two IDs (`255-259`). It neither retires the submitted launcher nor
waits for its independent continuation before removing the create refusal (`221-225`).

`api.rs:5666-5676` performs a fresh host GET after successful Resume. `create_form.rs:6658-6682` accepts that connected
real SSH host, then awaits key minting (`6704`) and ultimately sends the ordinary create (`7004-7016`) after its
retained-draft checks. Draining the Resume callback cannot join those later browser phases. A delayed host GET/key mint
can therefore outlive the trap and continue while teardown deletes the setup sessions; any extra created ID is outside
the test's owned list. Same-commit lifecycle tests explicitly retire the owner before removing downstream traps
(`tensorlake-lifecycle.spec.ts:483-485,695-697`), providing a local pattern for the missing boundary.

Keep the create trap until the accepted launcher has settled, or retire the document while that trap remains installed,
then drain routes and clean captured ownership. Success already observes the controlled create refusal and is outside
the failure path. The earlier vendor/cloud escape allegation was refuted: a real SSH Resume falling through before a
controlled success is refused. It is not revived by this ordinary-create route.

Suggested bucket remains **other**. Confidence for the added scope is **verified missing lifetime fence and
source-compatible continuation**, with actual extra allocation and frequency unmeasured. The existing item's older
uncertainty remains as recorded; this extension does not authenticate a historical browser outcome. No exact accepted
limitation, Planned item, pending repair, known bug, ledger or filter cover was established beyond this same queue item.
No runtime or network experiment ran.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g026/l1`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. The log retains the original refuted claim and the later distinct cleanup
finding.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): tensorlake-sandbox-sessions.spec.ts:223-280
retains the ordinary command at :250, held Resume route at :233-238 and create refusal at :239-243. Finally at :274-279
still releases Resume and disposes fixture routes before deleting only the two known setup IDs. api.rs:5730 onward still
reads host state around Resume, and create_form.rs:6817,6863 continues through Resume and key mint before creation.
Later host-wake hardening does not retire this mounted launcher before interception disappears. The existing
resume-test-retires-create-stub-before-client-settles.md remains the same root with additional affected scope. The
missing fence is source-confirmed; actual allocation after trap removal remains unmeasured.
