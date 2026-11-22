# Shift-click mixes a live range anchor with the displayed row order

Reviewed commit: 2dc0555c3db4a6d776b6ca5a9cc20ca6dc068f90

## TLDR

A Shift-click on a surviving row can mark a different range when the open session changes before the list redraws. The
click can add or omit local marks relative to the range displayed.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/session_list/row.rs:513`.

SPEC.md:2361–2368 defines the range from the open session in displayed order. One GUI at a time (SPEC.md:5588–5601)
permits a late event to use the shown target and permits reversible convenience state to lag. This route substitutes a
newer unseen selection for the shown range anchor. session_list.rs:477–482 and :557–621 can change selection on a store
update separately from the frame syncing the row order at :630–662. In production GPUI, app.rs:2201–2225 schedules the
frame. Its synchronous draw loop there is specific to test mode. On Wayland the scheduler queues a wake at
vendor/gpui-pre-linux/src/linux/wayland/window.rs:1043–1057, while client.rs:2626–2637 delivers MouseUp to input.
vendor/gpui-pre/src/window.rs:6112–6116, :6214–6219 and :6423–6457 dispatch it through the existing frame. The dirty
redraw at :6503–6506 is on the keyboard path. The separate callback draws and presents at :2060–2095. The row captures
the clicked key at row.rs:502–525. pin_drag.rs:1264–1301 and :1536–1558 checks that the clicked row still belongs to the
available listing and admits a stationary release. Its comment specifically calls out release before drawing a new
listing. None of those checks freezes the selection anchor. marking.rs:529–559 uses the live selected key with the last
list order, and :155–164 replaces the marked interval. The existing stable-anchor assertions at marking_tests.rs:483–545
do not cover this mismatch. Bind both the selected key and the order to the accepted row frame, as the shown-input
mechanism in prompt.rs:1149–1210 does for action data, while retaining stale-row refusal. Bucket: other. Direct
consequence is an incorrect, reversible local mark set. Later destructive acceptance is separate under
SPEC.md:2398–2409. Confidence: source-established for the native route and conditional effect. Frequency and exact
platform event order in an actual occurrence were not observed. Possible coverage: none.

Independent check of the proposed exclusion. AMBIGUOUS via 2dc0555c3 and df619914: session_list/row.rs:511–525.
2dc0555c3: SPEC.md:2359–2370,5586–5603 and session_list/pin_drag.rs:1262–1291.. The click handler matches at the
reviewed commit and tip. The parent diff and assigned patch excerpts are still needed to establish that the hover change
left range marking and release dispatch unaffected.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `status/f6`.
