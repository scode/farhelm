# Save and Close can lose a conflicting-key precondition while waiting for a page save

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Save and Close can skip the conflicting-key check and write a second Tensorlake key after the first save completes.

## Details

The question counts both a pending key save and the other dirty key (settings/editing.rs:559–589.
Settings/tensorlake_key.rs:394–407). When a page batch already exists, the close answer records only
AfterBatch::SaveAndClose (:617–623). A committed key clears its input in a deferred update before delivering its receipt
(tensorlake_key.rs:344–357, :429–445. Editing.rs:66–88). The follow-up start_batch checks keys at editing.rs:342–353,
:375–387, :453–465, when one of the differently typed keys is already gone. SPEC.md:2454–2458 requires returning to
Credentials for different unsaved keys. Check that shared-store conflict on the close answer before waiting and retain
the rejected close decision. A target list and per-draft revision alone cannot enforce the cross-form conflict if the
initial precondition is never checked. footerEditors owns the Tensorlake outcome and any additional effects.

Confidence: confirmed by inspection, applying the close question's own classification of a still-pending key as an
unsaved change.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footer/l11`, `footerEditors/l3`.
