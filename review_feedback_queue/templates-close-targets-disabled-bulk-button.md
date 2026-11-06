# Templates close targets disabled bulk button

Reviewed commit: 2d46033f3f3e89e97bbb9f8e110d8e5a93a20a7b

## TLDR

Closing Templates during a bulk operation can lose keyboard return focus because it targets a disabled button.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/src/list/templates.rs:455 at 2d46033f3f3e89e97bbb9f8e110d8e5a93a20a7b`.

Source assessment (read with the later corrections below) (review record g015/l4, event 13):

Reviewed commit: 2d46033f3f3e89e97bbb9f8e110d8e5a93a20a7b. TLDR: keyboard users closing Templates during a running bulk
Stop/Delete lose the intended return focus. Details: templates.rs:455 and trash.rs:30 unconditionally prefer
.marked-menu-toggle over their ordinary trigger. marking.rs:332-334 disables that button during accepted row work; the
surviving .marked-count at line 312 is intentionally programmatically focusable for precisely this state.
view.rs:3982-3985 quick-launch refusal checks ops.busy_now but not row_pending; an unbound shortcut goes to
templates_open.set(true) at line 4183, so Templates can open while bulk row work remains pending. Closing it at lines
4732-4736 calls DialogHandle.close with RETURN_FOCUS; modal_dialog.rs:141-146 schedules exactly that expression without
a disabled-target fallback. A disabled element cannot take focus, so the handback fails. Use the marked count while the
toolbar is locked, and the enabled toggle otherwise; apply the same selector policy to Trash. Suggested bucket other for
keyboard focus disruption, confidence confirmed for Templates; Trash shares the selector but its concurrent-opening
route was not independently established. proposed_drop: false. possible_cover: bulk-action implementation group; parent
checks coverage. Later source history through 8a8c9579e04299410652f0e4681975a7198dfc7b shows no further changes to
templates.rs, trash.rs or marking.rs after #3053. Caveat: no loss of sessions or destructive retargeting; this finding
covers only focus. The separate modified-key activation suspicion from the original l4 is tracked separately as c2
below.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g015/l4`; supporting records
`g088/x1`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
