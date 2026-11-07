# Settings save replies arrive out of order

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246

## TLDR

An older settings-save reply can overwrite the displayed result of a newer save even though the persisted settings
remain correct.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/src/tensorlake_settings.rs:129 at da24fb6822d289a144e18677cdd248cc35093246`.

Source assessment (read with the later corrections below) (review record g060/f1, event 11):

Later-turn synthesis. Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246; persists at pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: Using two account controls concurrently can make a confirmed setting
appear to revert until Settings is reopened. Mechanism: original whole-snapshot assignments at
tensorlake_settings.rs:129,230,293 remain at pinned-tip :151,260,324. busy, creating and toggling are separate; the
switch is not locked by a key request and key controls are not locked by a switch request. A key reply built while the
switch is false can arrive after the switch's true reply and overwrite local settings with false. Rechecking pinned-tip
api.rs:2132-2140 confirms send_save passes dependent=false; :2362-2368 constructs/awaits a save barrier only for
dependent=true. Registration tracks outstanding requests but does not serialize these independent writes or JSON-body
decoding. Thus no hidden barrier refutes reply reordering. Operation-owned request fields remain api_key only,
clear_api_key only, or create_without_asking only. Config::saved in tensorlake/config.rs:48-74 preserves omitted
previous fields, and :131-153 applies that merge within the transaction. Preserve f3's exact refutation: this does not
overwrite unrelated persisted keys or permission, even though the reply replaces the local aggregate. Suggested bucket:
other, established impact is misleading local display and control availability; no proven disclosure or work/credential
loss. Confidence: confirmed from source; runtime tests are prohibited. proposed_drop: false. possible_cover: unknown;
parent will route account UI interfaces. Suggested correction: merge operation-owned reply fields or serialize/version
aggregate replies and reconcile. SPEC.md Settings requires the switch show the stored acknowledged choice; no accepted
limitation authorizes a late unrelated response rolling it back. Reproduction remains hold completed key-PUT response,
complete switch PUT, then release old reply and compare UI with GET. Caveat: persisted-versus-local distinction is
essential; do not attribute a highest consequence merely to stale display.

Later qualification (review record g060/f1, event 28):

Discovery pass 4 interface synthesis supplement to the complete existing f1 records, not new discovery. Exact supplied
owner g056:g056-l15@29, g056/findings.jsonl:29, reviews api.rs:657-674 at da24fb6822d289a144e18677cdd248cc35093246 and
necessary consumer/backend context, persists at pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. This is the same f1
root: independent field-owned account writes bypass the preceding-save barrier and their full Settings replies can
arrive out of order into one component signal. Preserve the owner’s ordinary sequential-click/out-of-order-network
condition and confirmed source-derived confidence without runtime/frequency claims. Backend patch isolation is complete:
Config::saved/transaction preserve unrelated persisted fields. A delayed key reply can replace the switch's later
acknowledged true with local false; a delayed switch reply can replace newly saved api_key_set=true with false,
affecting Test/Clear/RecipeList availability. No stored-key loss, unauthorized billing or disclosure claimed. Suggested
bucket other, proposed_drop false, possible_cover exact duplicate g056:g056-l15. Its inclusion of SSH-key whole-snapshot
replacement is the same reconciliation root, not a newly counted endpoint. Its c8 Save-then-Test refutation remains
narrow: dependent Test waits for tracked saves, whereas independent saves do not. The f3 exact persisted-field
refutation is preserved. f15 is a separate remount/completion-ownership gap: no older full reply is installed into the
new panel; its old completion is discarded instead. No other group's assigned inventory inspected or unavailable premise
reinvestigated.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g060/f1`; supporting records
`g056/g056-l15`, `g056/g056-c17`. The retained private log preserves earlier wording and its corrections. This record
does not claim runtime reproduction.
