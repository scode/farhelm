# Environment Delete implementation spec still describes the desktop port as pending

Reviewed commit: 7e90bf0f704e68e400f5d042a7bef5b943d055c2

## TLDR

The implementation spec says the desktop does not yet send the environment Delete request, but it is already the shipped
GUI path.

## Details

**other.** Source inspection. Main reference `SPEC_impl.md:6249`.

SPEC_impl.md:6248–6253 says the operation will arrive with the port. At the same revision,
crates/gui/farhelm-gui/src/session_list/environment_delete.rs documents and implements this lane, and
session_list/bulk_tests.rs:1453–1514 asserts DELETE requests with ?delete_environment_session=true and the
retained-environment behavior. Update the spec to describe the implemented request and its late-notice outcome. Bucket:
other (implementation contract drift). Confidence: confirmed by pinned source and existing assertions. Possible
coverage: none.

Additional source record at `44137d0659d28b9cadf90a8ffe96a65930035aad`, `SPEC_impl.md:6263`.

notices/f4 documents the same sentence at 7e90bf0f704e68e400f5d042a7bef5b943d055c2 SPEC_impl.md:6248–6253 and the
existing bulk request assertions. At the assigned pin the sentence survives at SPEC_impl.md:6263–6264; GPUI
delete.rs:303–310 dispatches environment_delete.rs:206–216 and environment_delete_tests.rs:200–248 asserts the opted-in
request. Consolidate that cross-revision persistence with notices/f4. l1 is separate: it concerns SPEC.md:1648–1653
claiming an unanswered request had no decision, against the changed SPEC_impl.md:6258–6262 and helm
sessions/sandbox.rs:245–263. The same route can ship and correctly express uncertainty. Correcting its availability in
the implementation guide does not correct the product timeout guarantee. Bucket: other documentation. Confidence:
source-confirmed, existing assertions only. proposed_drop: set aside as a separate record because notices/f4 covers the
same sentence and shipped counterexample. Could not be highest. Possible coverage: notices/f4. L1 is independent.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `notices/f4`, `recovery_contract/l18`.
