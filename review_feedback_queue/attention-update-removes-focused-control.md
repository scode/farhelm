# Attention update removes focused control

Reviewed commit: b30f9f6e6c429167404abe09111fe1c19b21bf53

## TLDR

Starting Update from the attention list removes the focused action without handing keyboard focus to a surviving
control.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/src/hosts/attention_list.rs:298 at b30f9f6e6c429167404abe09111fe1c19b21bf53`.

Source assessment (read with the later corrections below) (review record g079/l10, event 28):

Reviewed commit: b30f9f6e6c429167404abe09111fe1c19b21bf53. TLDR: Keyboard users can lose their visible focus after
starting a host update from the attention list, because the focused Update button disappears when progress begins.
Mechanism: attention_list.rs:298-300 only stops propagation and calls on_update. list/view.rs:914-921 only
validates/enqueues shown-host requests, with no focus handoff. The existing provisioning publisher inserts
Pending/Running progress into row_update_progress (provisioning.rs:3720-3735); attention_list.rs:74-89 then classifies
that host as Working. Its rendered match :295-312 replaces the Update button with a nonfocusable progress element. No
onmounted/drop/focus hook on this attention component repairs the removed control. The header Update all at :216-219
likewise unmounts once its eligible rows become Working, without a handoff. This is the same browser constraint
explicitly documented in SPEC_impl.md:803-804: a focused disabled/unmounted control drops focus to body. The ordinary
Hosts Update already handles this exact transition at hosts.rs:4710-4715 by moving focus to its persistent menu toggle;
the new attention action omitted an equivalent destination. Shared menu-focus.js:21-29,50-68 only tracks focus epochs
and executes requested menu focus moves, not generic removed-control repair. modal_dialog.rs:492-525 binds lost-focus
recovery to a captured modal/region; this section uses plain section/div markup (attention_list.rs:213-234), not that
modal recovery owner. Suggested fix: retain focus on a persistent unavailable action or hand focus to the same host's
persistent identity control before replacing the action; provide a stable header/region fallback for Update all.
Preserve independent focus chosen after dispatch. Suggested bucket: other, keyboard navigation/focus continuity defect,
with no established security/process/data-loss route. Confidence: confirmed by source and the documented browser focus
constraint; no runtime/browser experiment. Temporal qualification: persists at pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b attention_list.rs:299-315 and list/view.rs:998-1005. proposed_drop:false.
possible_cover:not searched, parent owns matching. Caveats: do not claim that the next Tab necessarily restarts at the
document beginning, since sequential-navigation behavior can vary; the established defect is removal of the focused
action with no app-owned replacement focus. Distinct from l9's accessible naming/description omission and from c7's
refuted bubbling hypothesis.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g079/l10`; supporting records
`g080/i10`, `g081/f3`. The retained private log preserves earlier wording and its corrections. This record does not
claim runtime reproduction.
