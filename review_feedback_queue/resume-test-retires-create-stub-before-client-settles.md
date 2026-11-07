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
