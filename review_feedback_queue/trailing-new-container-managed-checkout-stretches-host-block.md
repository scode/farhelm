# New container with Managed checkout stretches the Container host picker to 370 px tall

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

In the session launcher, choosing New container and Managed checkout inserts a large empty vertical gap (~300 pixels)
below the Container host picker, pushing the Repository field, Name, Folder line, and agent choices far down the dialog.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view.rs:2277-2332`.

At crates/gui/farhelm-gui/src/launcher/view.rs:137, `CONTAINER_HOST_MIN_WIDTH` is `370.`. At lines 2277-2292,
`container_host` is built with
`.flex_grow_1().flex_basis(px(CONTAINER_HOST_MIN_WIDTH)).min_w(px(CONTAINER_HOST_MIN_WIDTH))` before branching on
destination kind. In the `if let Some(folder) = folder` branch (lines 2298-2322, Folder mode), `container_host` is
placed in a horizontal wrapping flex row (`div().flex().flex_wrap()...`), where `flex_basis(px(370.))` sets its
horizontal width basis. In the `else` branch (lines 2323-2332, Managed checkout mode — modified directly by f249fdff
while compacting managed checkout), `container_host` is attached directly to `rows` (`rows.children(container_host)`),
which is a vertical flex column (`div().flex().flex_col().gap(px(ROW_GAP))` at lines 2293-2297) with unconstrained
height inside the scrolling dialog body. GPUI forwards `style.flex_basis` directly to Taffy
(`vendor/gpui-pre/src/taffy.rs:514`). In CSS Flexbox / Taffy, `flex_basis` applies along the parent's main axis, which
for `flex_col()` is height, so `container_host` is laid out at 370 px tall instead of its ~60 px content height. Fix:
apply `.flex_grow_1().flex_basis(px(CONTAINER_HOST_MIN_WIDTH)).min_w(px(CONTAINER_HOST_MIN_WIDTH))` only when placing
`container_host` inside the horizontal `flex_wrap` row in the `if let Some(folder)` branch, leaving it with default
`flex_basis: auto` when placed in the vertical `flex_col` `rows` in the `else` branch. Verify with a GPUI layout test
asserting `launcher-repository-row` sits immediately below `launcher-container-host-row` (within a normal row gap +
optional place note height, not ~300 px below).

Reviewer metadata (g24-launcher-compact-checkout/l3): suggested bucket: high (could not be in highest: purely
visual/layout degradation, no security or data-loss impact; materially degrades every New container + Managed checkout
launch by inserting ~300 px of empty vertical space that pushes Repository and the rest of the form down); confidence:
confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g24-launcher-compact-checkout/l3`.
