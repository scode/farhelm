# Very short Trash frames may overcommit fixed chrome

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A very short Trash window may leave its content or controls unreachable; actual supported geometry and recovery remain
unverified.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The inspected fixed/minimum chrome budgets can exceed the remaining live CSS viewport. Saved outer-frame validation is
not a live viewport floor. Source/layout checks did not establish native minimum sizing, final scroll reachability and
keyboard recovery for a concrete supported frame. Existing reachable-scrolling acceptance does not automatically cover
unreachable controls.

Source locations: `crates/farhelm-ui/assets/app.css:8616-8627,8672-8679,8694-8717,9212-9219,9256-9273`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis complete against d6645f771bee8ed03a66486ef8755834937d759d. Same introduced
implementation8996f7213020f4fd10e7c5074c7e6eeadd57649c and assigned
landing-report8186c6b1839ae78ef25823efff5f6aef725dcab4. No new finding ID. Retain first-pass CSS overcommit: 360x220
gives a196px card cap; ignoring all header text, header padding/border33 + footer102.6(two32.8px answers,8px gap,28px
padding,1px border) + body minimum32 + confirmation minimum44 + card borders2 =213.6px. Actual footer/pointer exposure
and recovery remain unverified; do not describe all controls as inaccessible. Repository minimum-size reconciliation:
desktop.rs:222-246,309-332 supplies no live minimum. window_state.rs:257-297 rejects saved outer frames under320x240
physical pixels only;338-351 subtracts decorations and converts macOS size to logical pixels. This is no CSS viewport
floor and no live resize restriction. Cargo.lock pins dioxus-desktop0.7.10,tao0.34.8,wry0.53.5. Inspected local
version-matched source: Dioxus config.rs:183-190 replaces the supplied builder; webview.rs:215-264 supplies
default800x600 initial size but no minimum; Tao window.rs:294-353,1535-1613 defaults minimum constraints toNone; macOS
window.rs:563-574 calls minimum setter only for explicit minima; Linux window.rs:104-126 and util.rs:40-75 sets MIN_SIZE
hint only for explicit minima. Wry webkitgtk/mod.rs:587-623's size request is a bounds-based GtkFixed branch, not a
universal220px floor. No configured minimum refutes the candidate; OS/window-manager reservations and actual delivered
viewport remain outside this source-only check. Focus reconciliation: modal_dialog.rs:388-396 uses layout boxes, not
clipping, for eligibility;626-668 intentionally lets Tab wrapping use plain focus() and native focus scrolling.
ScrollRegion recovery uses preventScroll:true (452-460,531-545) and only manages stops/focus, not root size. Trash
supplies no root_scroll owner (trash.rs:953-979,1085-1107; modal_dialog.rs:925-975); initial and cancel focus suppress
scrolling; ConfirmBand explicitly calls scrollIntoView on answers (trash.rs:511), which can involve ancestors. Therefore
native focus/scrollIntoView may reveal clipped controls by moving an ancestor; neither complete inaccessibility nor
pinned-header preservation under that recovery is established. Existing whole-card overflow:auto fallback is scoped to
.host-settings-dialog.host-dialog-framed(app.css:4073-4076), not Trash. Checked specs still promise Trash's pinned
header/footer and reachable scroll regions(SPEC.md:1280-1285;SPEC_impl.md:551-559). Tests only establish source
assertions at300px or ordinary520px, not runtime evidence at220px. Missing evidence: a supported delivered viewport and
computed rectangles, root/region scroll offsets, pointer hit targets, native Tab/Home/PageDown before/after opening
at360x220. Experiments prohibited, so stop with that uncertainty. Suggested bucketother; confidence unverified after
investigation for user-facing access/recovery; no wrong target or user-work-loss route. proposed_drop:none.
possible_cover: no exact coverage found; TODO Planned and unlanded index empty; BUGS unrelated; other short-window queue
items are different dialogs. FILTER 'Dialog explanations out of view but reachable by scrolling' does not by itself
establish the missing recovery or cover an asserted footer-action/pinned-layout failure. a020 independently checks exact
filter and related exclusions. g017's resize/movement-pause lead remains separate and is not adopted here. Historical
runtime and approvals unauthenticated.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g018/f2`, `a020/g018-f2`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
