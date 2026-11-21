# A tab reset can send a key to the primary shell before it appears

Reviewed commit: 05ae73f23eb2cbb7ab284ca42473d66a771207a9

## TLDR

A session-list update can send a key to the main shell while the extra tab is still on screen.

## Details

**high**. Confirmed by source and contract inspection.

If an older filtered listing temporarily drops a still-open session while its extra tab has focus, a key can reach the
main Bash or Zsh shell while the screen still shows that extra tab. Confirmed conditional source ordering, estimated
high for input routed to the wrong running program. No observed command execution, disclosure or loss is claimed. The
supported two-session omission is f13/tabs/f11 (filters.rs:278–294 and carried_tests.rs:269–326). The primary terminal
can already be live independently of the roster update: the set keeps its agent entity, sizes and attaches hidden panes
using the shown pane (terminal/set.rs:337–364,439–442,631–648 and attached.rs:982–1017), and test source
set/tests.rs:454–483 verifies the normal case of a live extra and primary before fallback, without testing presentation
order. On omission, terminal/set.rs:583–602 clears the chosen tab, :422–436 selects the main pane and its focus handle,
and session_pane.rs:448–463 updates window home. The previously drawn grid's `focus::track` holds the extra's focus
handle in its mouse listener (focus.rs:871–908), so set_home (:504–512) leaves focus there until its node disappears. A
key received before the scheduled native frame makes GPUI draw synchronously (vendor/gpui-pre/src/window.rs:6503–6509).
That draw swaps in the new dispatch tree and invokes focus-lost listeners in the same call (:3795–3817), and
focus.rs:476–484,724–731,1316–1326,1753–1759 moves focus to the primary handle in that new tree. GPUI then picks its key
path from the new focus ID and dispatches the same key (window.rs:6508–6509,6666–6673,6771–6797,6976–6983). The drawn
primary grid's attached.rs:2329–2337,1384–1401,1412–1441 checks focus, live attachment and inertness but has no
presented-frame check. The draw merely sets needs_present (window.rs:3849). Presentation runs separately
(:2060–2095,3887–3892), and Wayland schedules frames separately from dispatching input
(wayland/window.rs:1012–1057,1577–1584). A primary already live before the metadata change requires no replay during
this key's dispatch, unlike the opposite direction investigated by tabs/f7. SPEC.md:5422–5427 requires the shown target
or ignoring the key; :2259–2262 and SPEC_impl.md:400–413 also distinguish an unavailable tab from omitted metadata.
Possible_cover: the product permits moving focus after a tab actually exits, but does not permit sending the
pre-presentation input to a newly focused terminal. Separate dispatch consequence beyond f13's verified visible
selection reset. Can be presented alongside f13 with both mechanisms retained. Source only, no platform timing or input
was observed.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-presentation/f17`.
