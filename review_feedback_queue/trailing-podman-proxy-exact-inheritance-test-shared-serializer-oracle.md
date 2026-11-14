# Proxy test's shared serializer cannot prove its exact-inheritance claim

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Proxy test's shared serializer cannot prove its exact-inheritance claim

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g066/l1`.

### g066/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b.

## TLDR

The new proxy privacy test claims to detect lossy handling of metacharacters and newlines, but computes its expected
bytes with the same serializer as production. A regression that changes those values inside that serializer can pass,
giving false assurance about exact inheritance. This is a test-oracle/documentation defect, not an observed production
proxy or credential failure.

## Details

The fixture value's comment explicitly promises that including shell metacharacters and a newline means a passing
privacy test 'proves that exact inheritance was not replaced with lossy sanitization' (failure_tests.rs:12-14). The test
cold_build_sends_exact_proxy_values_only_as_private_input creates the original value map at :987-990 but derives its
expected prefix by calling build::proxy_exports(&values) at :991. Its exact-value assertion at :1027-1029 only checks
input.starts_with(that prefix). Production Podman::script calls this same build::proxy_exports on the actual place
values (environment/podman.rs:356). The serializer is implemented at environment/podman/build.rs:421-429 and supplies
the shell quoting under examination.

Concrete competing mechanism, derived from source without executing or modifying anything: if proxy_exports replaced
newlines in each value with spaces before quoting, production's input and the test's expected prefix would both contain
the changed value. The marker would still appear exactly once in bootstrap (:1032-1038); argv would still contain no
marker (:999-1019); the PRIVATE_SCRIPT wrapper and import-environment name assertions (:1025, :1031, :1040-1058) would
be unchanged. Thus every assertion in this test can pass while its original multiline proxy value is changed. The
command fake parses phase-identifying script text and returns canned results instead of evaluating shell exports
(failure_tests.rs exec arm around :552-601), so it contributes no independent value-preservation observation. The
backend's install_bytes_checked also intentionally ignores _content (provisioning/e2e.rs:382-407). This does not
invalidate the test's meaningful private-input-versus-argv assertions, or require this workflow test to execute a real
container.

Contract: the supplied test-authoring checklist requires an observable that distinguishes competing mechanisms. The
distinct test claim concerns exact inheritance versus lossy sanitization; production correctness today does not
establish the claimed regression sensitivity. Use an independent expected literal/byte representation or a focused
semantic round trip over the original value, or narrow the comment/assertion message to say this test verifies transport
of the serializer's output.

Attribution/persistence: git log of the assigned file over the complete exclusive
6e423e30068b62c6cfa276c6d720ee35bb70387a through inclusive 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db range shows only its
introduction in 56659839785691f76da3af5d8f51b511cff4ba8b; no later patch changes this oracle or claim. Suggested bucket:
other, for false test assurance and misleading claimed coverage. Confidence: confirmed by source and counterfactual
assertion analysis; no execution, mutation, reproduction or runtime observation. proposed_drop: false. possible_cover:
none established; caller-supplied g065/l1 is a distinct production empty-value persistence defect, not this
shared-oracle consequence. Could-be-highest: false; no shipped security, credential exposure or loss-of-work route is
claimed. Material caveats: the current serializer may be correct, and the test does catch loss introduced outside that
shared helper as well as proxy values entering argv. This finding is restricted to the explicit stronger
exact-inheritance claim and missing independent oracle for transformations inside the helper. Original lead's
consequence is retained; all readily checkable premises are resolved and no uncertainty remains for this narrow finding.

Returned proposal (claims-042, event 7): **retain-verified**, suggested bucket **other**.

The test expressly promises sensitivity to lossy proxy transformations but obtains its expected prefix from production's
serializer. A newline-to-space transformation inside that shared helper changes both sides and preserves the remaining
assertions.

Original stronger consequence: False assurance that shell metacharacters/newlines survive unchanged. This is a
test-oracle/comment defect, not an observed product proxy failure or credential exposure. Current serialization may be
correct; the test still meaningfully checks private-input transport, absence from argv and transformations introduced
outside the shared helper. Supplied g065/l1 production empty-value persistence is a distinct consequence, not a
duplicate or cover.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record other with source-confirmed confidence. Use independent expected bytes/semantic observation, or
narrow the exact-inheritance claim to transport of the serializer's output.

Existing parent disposition (event 7): verified. Parent explicitly accepted named retain-verified proposal from
claims-042 after inspecting it. The test expressly promises sensitivity to lossy proxy transformations but obtains its
expected prefix from production's serializer. A newline-to-space transformation inside that shared helper changes both
sides and preserves the remaining assertions. Record other with source-confirmed confidence. Use independent expected
bytes/semantic observation, or narrow the exact-inheritance claim to transport of the serializer's output. Checked
source/results: ["56659839785691f76da3af5d8f51b511cff4ba8b:environment/podman/failure_tests.rs:12-14 promises exact
inheritance against lossy sanitization. At 987-991 it creates original values but computes exports with
build::proxy_exports; 1027-1029 compares input to those computed exports.","Production podman.rs:355-356 calls the same
proxy_exports, defined at podman/build.rs:420-429. Replacing newlines with spaces there would affect both expected and
delivered prefix; marker count at 1032-1038, no-marker argv checks at 999-1019 and import-name checks at 1040-1058 still
permit the changed value.","failure_tests.rs:545-597 parses script text into phases and returns canned output rather
than evaluating exports. provisioning/e2e.rs:382-408 ignores installed _content, so it supplies no independent
original-value observation.","Range log and diff show failure_tests.rs and build.rs introduced at
56659839785691f76da3af5d8f51b511cff4ba8b and unchanged through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.
Source/counterfactual analysis only; no mutation or execution."] Current coverage/results: {"SPEC.md":"Container
sessions at 4876-4882 promises deterministic creation-time proxy inheritance. This does not accept overstated regression
sensitivity in the test.","SPEC_impl.md":"Current 6082-6085 requires selected proxy values in environment.d. Read
surrounding Podman creation and prepared-artifact sections; no shared-oracle exception.","test-authoring contract":"Read
.agents/test-authoring.md in full; it requires an observable distinguishing competing mechanisms.","TODO.md Planned and
plans":"Empty/no unlanded entries.","BUGS.md":"Read; no matching test oracle acceptance.","queue and active
triage":"Searched proxy_exports, exact inheritance, lossy sanitization, test name and module, then broader proxy/test
assurance terms across queue/index and active ledger. No exact cover/alias found. Existing fixture proxy-privacy guards
concern different source strings and assertions.","FILTER.md":"No full match. The consequence is persistent false test
assurance, not an imprecise runtime diagnostic or self-correcting rare failure."} Exact cited cover: [] Original
stronger consequence and missing premise remain preserved: "False assurance that shell metacharacters/newlines survive
unchanged. This is a test-oracle/comment defect, not an observed product proxy failure or credential exposure. Current
serialization may be correct; the test still meaningfully checks private-input transport, absence from argv and
transformations introduced outside the shared helper. Supplied g065/l1 production empty-value persistence is a distinct
consequence, not a duplicate or cover."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g066 record](records/review-through-00de0b6c/groups/g066.json). Record ID: `l1`.
