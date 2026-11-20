# Session order can stay frozen after a Shift sweep ends outside the list

Reviewed commit: 1e7e2030cf0a3ddf9ae4b60ece65aedbbdfa8f3d

## TLDR

When you release a Shift-drag outside the sidebar, the list can keep showing its previous order for up to five seconds.
The marked sessions stay selected.

## Details

The list records when the pointer leaves its rows. A captured sweep move can then mark the pointer as back over the rows
even when it is in the adjacent pane. On release, the list can keep the order hold until its stillness timer expires.
`SPEC.md:2609–2617` says the hold can survive a pointer leave during an active sweep and should end once the sweep
finishes.

At `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see
`crates/gui/farhelm-gui/src/session_list/pin_drag.rs:944–980,1169–1176` and
`crates/gui/farhelm-gui/src/session_list/hold.rs:138–180,284–307,319–341`. The captured motion should renew the timer
only while the pointer is over the rows.

Suggested bucket **other**. Source-confirmed by trailing reviewer g063/l31. The delay needs an order update waiting when
the sweep ends.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g064/l8, g063/l31.
