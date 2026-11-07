# Removed tunnel retains validation error

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a

## TLDR

Removing an invalid tunnel entry can leave its obsolete validation error displayed against the remaining draft.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-ui/src/tensorlake/recipe_editor.rs:617-619,654-655 at 68790603037bb51ccd8655e4a948c7ffb747760a`.

Source assessment (read with the later corrections below) (review record g064/f15, event 39):

Reviewed full hash: 68790603037bb51ccd8655e4a948c7ffb747760a; persistence verified at pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b. Novel verified pass-2 discovery. TLDR: the recipe editor continues showing a
tunnel-port error after the user removes the sole offending tunnel, even though no tunnels remain. Ordinary fully
rendered control sequence: open New recipe, give it a valid name with default valid size/packages source, Add tunnel
leaving its ports blank, Save, then Remove that blank row. to_save parses every row's ports (151-188); the local refusal
sets error and returns before busy (373-380). Remove is therefore enabled and removes the offending row, but its
callback at 619 never clears error. Nearby input callbacks at 608/612/616 do clear error when changing fields. Error is
still rendered at 654-655 after the row/list is gone. No server call or event-burst/timing premise is needed for the
initial failed validation and subsequent ordinary removal. Tip recheck: conversion:151-197; error set/early
return:422-429; Remove still only removes row:678-681; error still displayed in sticky actions:723-724. git log on
assigned paths shows only snapshot/UI-layout followups, and the source remains unrepaired. The assigned pure draft
validation test at recipe_editor_tests.rs:89-101 checks invalid numeric/tunnel input but does not exercise clearing
component-level error after a row is removed. Concrete consequence is a stale inline diagnostic; Save is not disabled by
error and a later Save validates the now-correct draft and clears it. Do not claim inability to save, lost input, wrong
target or incorrect helm state. Distinct from f4's index retargeting: this needs a single ordinary rendered Remove after
a local validation refusal. Suggested bucket other, diagnostics/convenience. Confidence: confirmed deterministic source
state transition, no runtime experiment. proposed_drop:false; possible_cover: unknown, parent handles coverage/filters;
no forbidden inventory searches. Suggested fix: clear obsolete local validation error when structural tunnel edits
change the draft, consistent with field mutations, or track/recompute errors by the field/row they describe. A
persistent transport/server refusal may merit separate presentation; no stronger requirement is inferred here. Discovery
search continued through temporal state, request success/failure guards and the assigned test boundary after the first
event; complete for this pass.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g064/f15`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
