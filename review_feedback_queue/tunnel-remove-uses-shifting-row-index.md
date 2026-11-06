# Tunnel remove uses shifting row index

Reviewed commit: 8a8c9579e04299410652f0e4681975a7198dfc7b

## TLDR

Remove clicks from one rendered tunnel list can use shifted indices and remove a different unsaved row.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-ui/src/tensorlake/recipe_editor.rs:665-680 at 8a8c9579e04299410652f0e4681975a7198dfc7b`.

Source assessment (read with the later corrections below) (review record g064/f4, event 16):

Reviewed full hash: 8a8c9579e04299410652f0e4681975a7198dfc7b; same assigned implementation introduced at
68790603037bb51ccd8655e4a948c7ffb747760a:604-619. TLDR: tunnel controls from one drawn list can remove/edit another
unsaved row if an earlier removal shifts the live vector before that render's next event. Rechecked stable identity:
there is none. key is index (666), row is cloned for rendering from draft.tunnels (540,665), inputs call live
get_mut(index) (669,673,677), and Remove calls live remove(index) after only a bounds check (680). No row id,
expected-row comparison, busy claim, prompt token or editor unmount occurs on a row removal. Same-render transition
[A,B,C] with A Remove(index 0), then B Remove(index 1), yields [B] because the second index is C after the first
removal. Two Remove(index 0) events drawn for A yield [C]. Bounds checking avoids panics but not retargeting. Dioxus
0.7.10 runtime.rs:369-379 explicitly permits multiple handle_event calls before render; :408-475 looks up current
mounted listeners and calls synchronously. Before rendering, first removal changes only the signal; the old row
listeners remain current, making this event-order proof consistent with actual listener lifetime. After a render,
listeners/DOM reflect the changed list; do not claim a stale captured handler survives an arbitrary unmount or that two
normally rendered clicks necessarily mis-target. Native desktop handles each event synchronously (interpreter
native.ts:353-407,566-597; desktop webview.rs:101-195), while render/push is separately polled (:537-590); no guarantee
that two actual native clicks precede a flush has been dynamically demonstrated. Thus confidence is confirmed state
transition for the repository's supported same-render event model; actual native event scheduling/frequency is not
runtime evidence. Suggested bucket remains other: only wrong unsaved tunnel-row changes are established. No demonstrated
credential forwarding, saved-data loss or other highest consequence; Save is a later separate action and may display the
changed rows. possible_cover: unchanged, parent handles. proposed_drop: false. Fix using stable editor-local row ids so
removed-row events are ignored and later-row events locate their original row. This synthesis narrows the initial
rapid-click wording to the precise same-render premise and retains the limited unsaved-row consequence.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g064/f4`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
