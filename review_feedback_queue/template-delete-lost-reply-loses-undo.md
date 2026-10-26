# A lost Delete reply removes the Undo offer

Reviewed commit: 463cac1528b5c5e259404975380d321a3fff0412

## TLDR

An ordinary template deletion can commit but offer no Undo when its success reply is lost. Manual recovery from the
mounted editor remains possible until it closes; the unknown-outcome exception's scope is ambiguous.

## Details

Suggested bucket: **highest**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/templates.rs:2040`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit 463cac1528b5c5e259404975380d321a3fff0412. TLDR: if an ordinary template deletion commits but its success
reply is lost, the template disappears without the Undo the control promised. Source route: templates.rs:1962-1970
clones the stored original, 1980 creates the request, and 1989-2014 installs Undo only on Ok; Err at 2040 merely
displays the error and drops the task-owned snapshot. Helm templates.rs:160-170 replies only after remove_template's
detached task commits (179-208), so client cancellation/transport failure does not roll back deletion. API send_save at
1660-1668 and send_inner_ordered at 1854-1877 propagate timeout/transport errors; execute_with_receipt at1796-1823
merely logs one client.execute, with no outcome reconciliation. delete_template at3967-3978 passes that error through.
Start with no existing Undo and let deletion commit followed by response loss: no restoration notice is created. A
second Delete sees a missing record and cannot produce that first snapshot. Closing the dialog unmounts its local state
(list/view.rs:4177-4189); reopening cannot recover the deleted catalog record. Bucket suggested highest, with
recoverability caveat: while the failed editor stays mounted, a clean draft can be saved again, or Duplicate can copy
its stored baseline (templates.rs:1395-1407). This means the bytes are not erased from all client memory immediately;
the defect is absence of the ordinary restoration offer, and loss once the user leaves without that manual recovery.
Unsaved draft edits need not equal the deleted stored original. Confidence confirmed conditional source mechanism;
actual frequency unmeasured. This code path predates the kind-binding change (also present in
b0e4eafe9fc2f2cae93e4cdbfbebc5824be4d783:1843-1868,1894), not a claim that that patch introduced it. possible_cover:
SPEC.md:3434-3437 says failed GUI saves are reported then discarded, and SPEC_impl.md:240-248 accepts unknown remote
outcome after a lost save reply. It is ambiguous whether that waives the explicit original-data Undo promise at
SPEC.md:758-762; the discarded attempted change is distinct from the original snapshot needed to undo a destructive
operation. SPEC.md:3383-3399 accepts browser sign-in-expiry state reset; this finding has an authenticated
transport-loss route and is not limited to sign-in expiry. Partial deletion SPEC.md:3232-3238 is about sessions and
attachment cleanup, not template Undo. Six pinned coverage sources and matching queue content searched; no exact
Planned/BUGS/queue/ledger item found. proposed_drop none; preserve possibly-highest consequence and acceptance
uncertainty for independent parent audit. No reproduction, tests or runtime experiments.

Provenance: `a028/delete-reply-loses-undo`. These are reviewer-local identities, not additional findings. Proposed drop:
none for the retained remainder.
