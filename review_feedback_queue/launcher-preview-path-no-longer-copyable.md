# The launcher no longer lets users copy the checkout preview

Reviewed commit: f249fdff55a2bd63085e1f911757e61e41bd5b9f

## TLDR

The compact managed-checkout row shows the exact planned folder without the selection and copy control available before
the layout change.

## Details

`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1273` renders the ready path as a `HostSentence` in the Folder
row. The previous path display supported selecting and copying the exact destination before launch. The current launch
controls have no equivalent action.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`, finding `checkout-layout/f2`.
