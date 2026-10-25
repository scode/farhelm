# The closed-model Enter test can misattribute a restart

Reviewed commit: b37d328ce973442d007ce518b69a26d8a14ebec7

## TLDR

An incorrectly admitted early Enter can start an asynchronous restart after the immediate request-count assertion; the
later primary press can then be refused while the earlier request satisfies the test. Production behavior remains
correct.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `e2e/tests/restart-with.spec.ts:1682-1687`. Locations and revisions in the assessment below identify
the reviewed source; later main is outside this review's pinned range.

Reviewed commit b37d328ce973442d007ce518b69a26d8a14ebec7. TLDR: a restart regression test can pass when Enter in the
closed model field starts a restart before the separate action the test requires. Suggested bucket other; this is a test
oracle defect, not an established production restart bug. Confidence confirmed conditional false-pass mechanism by
source inspection; the adverse browser schedule has not been executed. Concrete competing implementation: set
LaunchControls.on_model_primary_enter to (!model_open()).then_some(launch_from_choice) at the shared caller.
LaunchControls already supports that callback (launch_controls.rs:135-136,297-310), but the correct current caller omits
it (create_form.rs:7255-7350). This mutation preserves open-picker selection behavior, including the held-model test at
restart-with.spec.ts:1771-1796. In the closed-field round at 1682, it synchronously claims the operation, freezes the
null model, and spawns a request task (create_form.rs:4833-4847,5064-5068,5105-5127). The test's immediate
bodies.length == 1 check can occur before that early request reaches its intercepted POST. The task may wait on browser
eval for its new intent key (api.rs:2839-2847) or on HTTP dispatch; the key is reused only for an equal binding
(create_form.rs:5114), while this round changed the model from enter-model to null. The helper then records before == 1
and focuses/presses the primary (restart-with.spec.ts:1575-1583). That native submit remains focusable while busy
because it uses aria-disabled, not disabled (create_form.rs:6014-6025). Its handler calls submit_composer (5926-5928),
whose already-held claim refuses the second attempt (4833-4835). Only the early request reaches refuseEnterRestarts,
producing numbered refusal 2 and a null-model body. After it releases the claim, every later helper assertion passes:
expected refusal, no aria-disabled, modal focus, exactly before+1 bodies, and with.model == null
(restart-with.spec.ts:1583-1592,1685-1687). The helper has not proved the later primary action caused the request. The
open-field held test does not exercise closed Enter and cannot close this gap. Correction to the first event:
mint_intent_key is browser eval in the web build, not an HTTP key-mint request. Caveat: source-only review; occurrence
frequency is unmeasured. possible_cover: TRIAGE_OUTCOMES.md model-effort-test-never-proves-model-accepted concerns a
different test and proof obligation; no exact accepted coverage found. proposed_drop: none. A targeted fix would
establish and hold a completed no-submit boundary for the closed Enter, then distinguish a request newly caused by the
explicit primary action; no fix was made.

Provenance: `g255/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
