# Uninstall inspection transforms newline-bearing identity text

Reviewed commit: c2aca60458145aa24d4736873b75d594118f1f1a

## TLDR

Inspection can transform newline-bearing path text before comparing identities. A supported incidental path and complete
layout defeating all removal guards remain missing; this is separate from the repaired ordinary path cases.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `crates/farhelm-helm/src/provisioning/backend.rs:2543-2544`. Locations and revisions in the assessment
below identify the reviewed source; later main is outside this review's pinned range.

Synthesis lead, not a confirmed product defect. Reviewed c2aca60458145aa24d4736873b75d594118f1f1a. resolve deletes all
newline bytes from readlink output at backend.rs:2543 before raw assignment and the -ef ancestor walk at 2568-2577.
Parser/service require nonempty canonical/raw values, not correspondence with the actual source path
(backend.rs:2857-2873; service.rs:1520-1540); therefore the ancestor-refusal proof assumes faithfully represented
resolved ancestors. This is distinct from g258/f2's source-refuted final mount check, which preserves interior newlines.
Missing source premises: a supported incidental source of newline-bearing resolved ancestors and a complete layout where
spelling checks, identity check and final mount refusal all miss and removal reaches retained data. No such layout is
established and no user-work loss is asserted. OTHER representation concern only; possibly highest only if the complete
uncovered data-loss route is established. Possible cover: pinned SPEC.md:3271-3278 explicitly excludes deliberately made
newline-target symlinks but preserves specific protections; ledger uninstall-newline at 6758-6771 addresses that
deliberate case. proposed_drop: no blanket exclusion; retain unresolved lead separately, static investigation
incomplete. Do not conflate with or withdraw narrow g260/f1 absent-leaf refutation. No reproductions authorized or
performed.

Provenance: `a038/f5`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
