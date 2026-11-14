# Legacy-container replacement and owned-template cleanup preserve their contracts

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The removed positive legacy-container UI checks cover intentionally retired behavior; the
replacement checks refusal and explicit removal, with cleanup attempted even when setup fails. Original considered
consequence retained: losing a supported isolated-launch guarantee could allow an ordinary host launch where isolation
was requested, but no such route is established by these test edits or their inspected callers. SPEC.md:4818-4823 says
“The earlier experimental model, whose supervisor stayed outside the container and bind-mounted a host checkout, is
retired” and “Templates, recent setups and history with that choice still load. Using them drops or refuses the retired
choice with an explanation and cannot create another old-style session. The old launcher switch and account-selection
flow are gone.” Source: templates.spec.ts:1039-1052 verifies a historical image_file object remains stored, then checks
refusal and unchanged draft title; lines 1056-1059 wait for real save and verify the stored field is absent.
github-checkout-composer.spec.ts:129-154 checks absent retired controls and an intercepted ordinary checkout payload
without launch.container. launcher.rs:666-670 refuses a nonnull container field before mutating the cloned launcher
state. Synthesis cleanup check: the try begins before putTemplate at line 1038, and finally at 1063-1064 always invokes
cleanupOwnedSteps for this test's unique template; helpers/cleanup.ts attempts every owned step and preserves the
original body failure while reporting cleanup failures. The removed unrouteAll is unnecessary in this chang

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g039/c2`.

### g039/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (candidate, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc. TLDR: The removed positive legacy-container UI checks cover
intentionally retired behavior; the replacement checks refusal and explicit removal, with cleanup attempted even when
setup fails. Original considered consequence retained: losing a supported isolated-launch guarantee could allow an
ordinary host launch where isolation was requested, but no such route is established by these test edits or their
inspected callers. SPEC.md:4818-4823 says “The earlier experimental model, whose supervisor stayed outside the container
and bind-mounted a host checkout, is retired” and “Templates, recent setups and history with that choice still load.
Using them drops or refuses the retired choice with an explanation and cannot create another old-style session. The old
launcher switch and account-selection flow are gone.” Source: templates.spec.ts:1039-1052 verifies a historical
image_file object remains stored, then checks refusal and unchanged draft title; lines 1056-1059 wait for real save and
verify the stored field is absent. github-checkout-composer.spec.ts:129-154 checks absent retired controls and an
intercepted ordinary checkout payload without launch.container. launcher.rs:666-670 refuses a nonnull container field
before mutating the cloned launcher state. Synthesis cleanup check: the try begins before putTemplate at line 1038, and
finally at 1063-1064 always invokes cleanupOwnedSteps for this test's unique template; helpers/cleanup.ts attempts every
owned step and preserves the original body failure while reporting cleanup failures. The removed unrouteAll is
unnecessary in this changed case: the replacement installs no route handlers. No late-writer reproduction or runtime
execution was performed. Suggested bucket: other, test coverage/cleanup only. Confidence: confirmed source and
retirement contract. proposed_drop: removed behavior is explicitly retired and the replacement preserves rather than
silently clears requested legacy intent; cleanup is attempted across the shown setup/body paths; parent has not accepted
proposed drops. could-be-highest: no established highest route in this assigned test patch; the hypothetical product
isolation downgrade would require an actual fallback path, which the inspected all-or-nothing apply rejects.
possible_cover: none. Caveats: this is not a claim that cleanup can never fail or that a timed-out arbitrary backend
writer can never outlive its test; no such concrete changed path was established. No claim about unrelated deletion or
historical-store migration behavior.

Returned proposal (claims-024, event 12): **covered**, suggested bucket **other**.

The removed positive legacy-container behavior is explicitly retired. The replacement checks refusal without changing
the draft and explicit removal from stored fields. This is narrower than certifying all historical migration or
isolation paths.

Original stronger consequence: Losing a still-supported isolated-launch guarantee might permit an ordinary host launch
where isolation was requested. The latest history narrows the established scope to retirement/test cleanup and leaves
that security route unestablished.

Unresolved premise or limitation: An actual product fallback launching on the host despite retained requested isolation
would establish the stronger security concern. The inspected all-or-nothing application rejects that field; the assigned
test edits do not establish such a fallback. Cleanup can still fail; no universal backend-writer retirement claim is
made.

Proposed basis: Propose coverage only for deleting positive tests of the explicitly retired model and source refutation
of silent template fallback in the inspected caller. Send the original isolation consequence and this basis to the
independent drop check.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: spec-retired-old-container-positive-tests-and-source-refuted-silent-template-fallback. Parent
read full024. Earlierhost-supervisor/bindmountcontainerexperimentalmodel explicitlyretired;
positiveoldlaunchtestnotrequirednewPodmanisolation. Replacementretainsobject/refusalunchangeddraft/storedfieldremoval;
launcher rejectsnonnulloldcontainerbeforeclone/mutation, ordinaryinterceptedpayloadomitsoldfield.
Narrowexactretirementcoverandsilenttemplatefallbackrefutation, notallhistoricalmigration/isolation/runtimecleanup.
Originalpossiblehostexecutiondespiteisolationrequestpreservedfreshgate;
existingoldpositiveconfigoracledifferentnotcover.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g039 record](records/review-through-00de0b6c/groups/g039.json). Record ID: `c2`.
