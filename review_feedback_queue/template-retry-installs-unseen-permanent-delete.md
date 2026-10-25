# Retry can install Permanent Delete before its label changes

Reviewed commit: 463cac1528b5c5e259404975380d321a3fff0412

## TLDR

After partial failures involving an invalid older template, desktop Retry can leave ordinary Delete visible while its
click permanently deletes the template without the promised warning or Undo. Release-upgrade filtering covers at most
part of this conditional route.

## Details

Suggested bucket: **highest**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/templates.rs:2261`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit 463cac1528b5c5e259404975380d321a3fff0412. Refinement of the same retained a028/unknown-list-retry-audit;
no new defect. TLDR: after Retry, an ordinary Delete still displayed with an Undo promise can execute the new Permanent
native handler, permanently removing the original saved template without its required advance warning. The concrete
route uses the editor's ordinary Save(None) at2263, not a Save-and-leave answer whose failure can retain its departure
prompt. Invalid command text is read-only but has a shipped remove control at1298-1310; Field::remove clears command
at634-645, so repair/rename N can succeed without preserving A's old command. Following failed preferences read retains
previous A and restarts the list at1798-1855. Failed-list classification/Retry/button are
at1528-1536,1727-1736,2257-2275; handler and successful reset at1955-2026,1409-1413. Locked Dioxus source establishes
the new-listener-before-raf-display route as recorded in prior events. undo_after_delete returns None for Permanent
at1551-1555. Important harm boundary: no claim that merely offering the ordinary save-based Undo could successfully
restore invalid A. That separate unusable-Undo schedule needs its own production route, and the
unknown-list-to-OLD-closure schedule remains refuted. This finding is that the user receives neither the advance
Permanent warning required for an unrestorable stored value nor the Undo promised by the still-displayed ordinary
control. The Permanent handler deletes A and clears its baseline; N is not necessarily a backup. SPEC.md:758-762 and
SPEC_impl.md:786-797 bind the destructive promise to what was drawn. Suggested highest, confirmed conditional source
mechanism; actual occurrence/frequency unverified. Preconditions remain invalid stored A, ordinary editor Save,
successful new-name PUT plus failed preferences/list reads, then Retry and narrow native input timing. possible_cover:
older-release-plus-rare FILTER.md:137-156 remains AMBIGUOUS for proven release-upgrade subcases; general legacy-invalid
input does not establish that provenance, and development-built earlier source also permits the state. No categorical
template-catalog exemption is inferred from affected-sessions wording. Ledger13434-13467 requires restoration or advance
warning, not this missing warning. proposed_drop: none. All earlier events and narrow withdrawals preserved.

The dependency premise is the locked Dioxus 0.7.10 native bridge: dioxus-desktop/src/webview.rs:537–589 polls and
installs the VDOM changes; dioxus-core/src/diff/node.rs:38–78 replaces the mounted listener;
dioxus-interpreter-js/src/ts/native.ts:409–501 applies display edits on requestAnimationFrame and acknowledges them.
Event delivery uses the mounted native listener before those display edits. The inspected archive checksums match
Cargo.lock, and local members match the archives. This establishes a conditional source schedule, not measured pixels or
an attested running binary.

Provenance: `a074/a028-unknown-list-retry-audit`, `a028/unknown-list-retry-audit`, `g230/unknown-list-retry-audit`.
These are reviewer-local identities, not additional findings. Proposed drop: none for the retained remainder.
