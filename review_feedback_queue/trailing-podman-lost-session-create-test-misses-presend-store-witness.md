# Lost-create test does not observe its promised pre-send durability boundary

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the new lost-create test says login/install facts become durable before container creation is sent, but reads them only after the failed send returns. Persisting those facts after sending could pass this test, leaving its stated crash-boundary guarantee untested. This is test assurance, not an established product ordering defect.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g067/l5`.

### g067/l5

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 23 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b; unchanged through pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: the new lost-create test says login/install facts become durable before container creation is sent, but reads them only after the failed send returns. Persisting those facts after sending could pass this test, leaving its stated crash-boundary guarantee untested. This is test assurance, not an established product ordering defect. Details: tests.rs:248-250 states 'The selected login and install facts become durable before run.' The initial absent-login assertion at :270-273, create call at :299-308 and saved login/pending/facts assertions at :309-327 observe only before the whole create and after its return, never during send. The retry/error and external-run count at :329-344 protect post-return retry behavior. In fake run_bounded at :57-86, the sole send-time store read is conditional helper_oracle at :66-82, checks recipe-builder ownership and is off in this session test, which sets only lost_run at :254. The fake supplies a lost error and subsequent absent observation independently of the session store fields. Concrete competing ordering: await run into a local result, then write selected facts/login/pending marker, then process that result. This would preserve the same post-return assertions and second-run refusal while leaving the required facts absent during the external send. Own source checks rule out an accidental lower-layer oracle: production run at podman.rs:421-470 uses the selected image/name/labels, and scoped_output at :295-321 resolves the ordinary place and invokes the command. Neither reads this session host's stored login/install admission before sending. Current create correctly awaits set_podman_prepared_create at :1103-1115 before run at :1116-1125; correctness today does not establish the test's sensitivity. Exact SPEC_impl.md:6430-6431 contract: 'Preparation observes and atomically records the full login identity, prepared facts and pending-send marker before container creation is sent.' Supplied test-authoring rules require an observable that distinguishes competing mechanisms. Add a session-send store oracle for this host's selected login, facts and pending marker, analogous to the helper oracle, or narrow the comment to post-return persistence/retry. This does not require every failure case in one test. Whole-range attribution: git log of this file over exclusive 6e423e30068b62c6cfa276c6d720ee35bb70387a through inclusive tip lists only its introduction at reviewed commit56659839785691f76da3af5d8f51b511cff4ba8b, and the full reviewed-to-tip file diff is empty. Thus this assigned temporal claim and oracle omission are in-range and unrepaired. Suggested bucket: other (maintainer regression assurance). Confidence: confirmed by source/fixture/counterfactual assertion analysis; no execution, mutation, reproduction or runtime observation. proposed_drop:false. possible_cover:none established in supplied refreshed neighbours; g066's shared-serializer oracle and g064's scheduling oracle are different tests/mechanisms. Could-be-highest:false; no shipped security or user-work-loss route alleged. This revision resolves the introduction/persistence and lower-layer-call premises, retaining the original narrow false-assurance consequence. No open lead remains under l5. The discovery pass continues after recording.

Returned proposal (claims-130, event 23): **retain-verified**, suggested bucket **other**.

The test promises that login/install facts are durable before sending creation, but observes those facts only after create returns. Its fake send has no enabled session-state oracle. The established consequence is missing regression assurance for that temporal contract; current production ordering is correct.

Original stronger consequence: None alleged beyond test assurance. Events 21-23 explain that a hypothetical run-then-persist implementation could pass while a crash during run leaves facts absent; they explicitly do not claim that current production has that order or that any crash/work loss occurred. No withdrawal or consequence lowering appears in this named history.

Unresolved premise or limitation: No missing source premise for the narrow test-assurance claim. No runtime or mutation experiment was run. A shipped ordering bug, real crash, actual orphan, process loss or user-work loss has not been established and must not be inferred from the test counterfactual.

Proposed basis: Retain as other with source-confirmed confidence and the execution caveat. A session-send oracle can inspect this host's saved login, prepared facts and pending marker before returning the fake outcome; alternatively narrow the test comment to the post-return persistence/retry guarantee it currently observes. This consultation makes no parent disposition and is not an independent drop check.

Existing parent disposition (event 23): verified. Parentreadallprimary21-23/directtest240-346/fake45-90, thenfullfreshfocused130result. New566testcommentpromisesselectedlogin/installfacts/pendingmarkeratomicdurabilitybeforesend, butobservesabsentbeforewholecreate andsavedafterreturn; sendhelperoracleoffandonlybuilderstate, actualrunpathdoesnotreadsessionadmission. Concreteawaitrun→persist→processresultcounterpartpassesretry/count/postreturnassertionsyetviolatespresendcontract. Productioncurrentlycorrectordernotrefutation. Fileintroduced566/unchangedtip. BothSPECcontractsrequireboundary; Planned/unlandedindexempty, BUGS/queue/activeledgernoexactcover, existingrebuildorderpersistenceitemdifferentresource/fake. NoentireFILTERmatch. Retainothernarrowtestassurance, no actualcrash/orphan/userworkfailureorunrunmutationclaim. Allfocusedcoverage/caveats retainedinclaims130result.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g067 record](records/review-through-00de0b6c/groups/g067.json). Record ID: `l5`.

