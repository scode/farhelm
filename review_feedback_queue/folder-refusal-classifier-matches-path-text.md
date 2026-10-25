# Folder error classification also reads the submitted path

Reviewed commit: 1de332971ec47040f095b0fbc32903fbcb88ae73

## TLDR

A path containing familiar error phrases can be misclassified as missing rather than refused or non-directory. This
changes an advisory annotation, not launch authority; the rare-diagnostic filter's scope is ambiguous.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/create_form.rs:1292`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit: 1de332971ec47040f095b0fbc32903fbcb88ae73. TLDR: the launcher can say an existing path is missing when
it is actually a file, if its spelling contains a different folder-error phrase. create_form.rs:1292-1303 prioritizes
categories using contains over the whole refusal; service/core.rs:3544-3554 appends literal cwd after its correct
diagnosis. create_form.rs:5208-5217,1307-1318,6545-6547 supplies the wrong subordinate annotation.
draft.rs:326-333,626-634 preserves typed cwd, and api.rs:2092-2109 preserves the refusal body apart from surrounding
trim. The same parser survives at pinned tip 463cac1528b5c5e259404975380d321a3fff0412:create_form.rs:1300-1311, with
consumers at 3039-3044 and 5737-5746. Bucket OTHER; confidence confirmed conditional source mechanism, actual frequency
unmeasured. The correct original refusal remains visible; annotation has no action authority; no highest impact
established. possible_cover: rare-diagnostic FILTER ambiguity remains explicit; no exclusion accepted here.
proposed_drop: none. Provenance reconciliation: the parent verified and retained this cause, and identified g285/f1 and
g286/f1 as independent relative-path/inconclusive automatic-warning variants of the same cause. Preserve a035/d1,
g285/f1 and g286/f1 under one parent record. Those neighboring variants are attributed to the parent's reconciliation,
not claimed as separately reviewed by a035. This event adds provenance only and is not productive discovery beyond the
known parser defect.

Provenance: `a035/d1`, `g285/f1`, `g286/f1`. These are reviewer-local identities, not additional findings. Proposed
drop: none for the retained remainder.
