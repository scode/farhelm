# Linux can accept a tunnel request its GUI never offers

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

The Linux desktop shares a helm route that will open a localhost tunnel for an authenticated caller. Its own terminal UI
only offers the action on macOS.

## Details

Bucket: **other**. GUI migration final whole-tree review, R10 Opus DONE (Linux tunnel).

Source references: `crates/farhelm-desktop/src/lib.rs:620–625`, `crates/farhelm-helm/src/tunnels.rs:350, 474, 533, 553`,
`crates/gui/farhelm-gui/src/terminal/attached.rs:2732–2746`.

The embedded desktop supplies a native link opener on both platforms. Tunnel admission uses the presence of that opener
as its desktop capability check. On Linux the UI does not offer the Cmd-click gesture, but a caller holding the device
credential can still reach the open route if its usual consent and session checks pass.

Opus recorded this as a source observation without a separate severity or verification verdict. Other reflects a
capability and product-contract mismatch. No shipped Linux UI action or unprivileged remote request was shown to open
one. SPEC.md lines 3151–3170 describes the Mac UI gesture and Mac loopback listener. SPEC_impl.md describes the embedded
helm listener in platform-neutral terms, so it is unclear whether the product restricts the backend capability or just
the offer.

If tunnels are intended to be Mac-only, gate with an explicit tunnel capability while keeping normal Linux URL opening.
Otherwise describe which desktop capabilities the authenticated helm route exposes.
