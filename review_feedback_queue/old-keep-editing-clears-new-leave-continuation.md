# Old keep editing clears new leave continuation

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a

## TLDR

An earlier Keep editing answer can erase a later pending close request, so Save or Discard returns to the form without
closing its dialog.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-ui/src/tensorlake/recipe_editor.rs:483-489,739 at 8bd92e1336565dfc263fe5f3fa2fef413a82a87a`.

Source assessment (read with the later corrections below) (review record g058/g058-l4, event 10):

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a; persists at 8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR:
after a replacement close request, an earlier Keep editing answer can erase the close continuation, so the current
Save/Discard returns to the recipe/add-host form while the dialog remains open. This is interface reconciliation, not a
discovery. Supplied owner record g064:f3 independently confirms the exact duplicate, repeated-close producer and
navigation-only consequence. Source proof from preceding synthesis remains: recipe_editor.rs:739 unconditionally clears
pending_close after cancel_for(&shown), even when token rejection leaves the replacement prompt up; 483-489 stores a
continuation and opens a new question whenever the dirty editor holds an enclosing close. A dirty nonbusy editor can
show A; another enabled header Close/Escape opens B before queued Keep-editing-A is processed. Settings routes those
controls through request_close (settings.rs:309-314,341-354; footer 791-796); Add host routes them via
cancel/close_guard.request (reviewed add_host.rs:193-201,218-229). Role=status inline answers do not create a nested
modal and locked is only busy (recipe_editor.rs:399-401,726-763); header's close adapter is an ordinary unconditionally
enabled button (hosts/settings_dialog.rs:374-381). Source route is admitted under documented same-render events; no
completed-unmount callback survival is needed. Current Discard takes B but finds no continuation and calls on_cancel
(745-749); successful Save likewise has no close to invoke (451-457). g064 additionally checked outside-dialog isolation
and Settings callback navigation, consistent with this proof. Confidence: source-confirmed unconditional side effect,
reachable producer and conditional state transition under the application event contract. No exact native event
scheduling, observation frequency or dynamic reproduction claimed. Suggested bucket other, solely failure to close; no
credential/process/saved-data loss and no loss beyond a deliberate Discard asserted. proposed_drop false.
possible_cover: exact duplicate g064:f3; consolidate once. Fix gate pending_close clearing on take(&shown).is_some(),
preferably put the close continuation in its opening's payload. Temporal: predates assigned #3050 sticky-action change;
initial record locates it at the parent revision. Supplied g039:g039-l4 and g056:g056-l3 concern live draft Save with an
unestablished stale-visible-state premise; that uncertainty is preserved and not imported into this continuation proof.
Current Discard alone establishes the navigation consequence. c2/c3 refutations remain unchanged.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g058/g058-l4`; supporting
records `g064/f3`, `g056/g056-c5`. The retained private log preserves earlier wording and its corrections. This record
does not claim runtime reproduction.
