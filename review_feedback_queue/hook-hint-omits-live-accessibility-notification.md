# The hook hint has readable text but sends no live accessibility announcement

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

The hook setup hint exposes readable text but does not request a live announcement when it appears while focus stays in
the terminal.

## Details

**other.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b crates/gui/farhelm-gui/src/session_pane/hook_band.rs:192-207;
crates/gui/farhelm-gui/src/session_pane/trust_band.rs:398-415,427-445;
vendor/gpui-pre/src/elements/div.rs:1355-1364,1956-1966,3625-3635

The hook hint's text is exposed to a screen reader that navigates the pane, but the hint does not request an
announcement when it first appears while focus stays on the terminal. The shared strip promises announcement without
focus at trust_band.rs:398, but emits only an AccessKit Status role; the words are an ordinary child label. GPUI's
Div::role sets only the role (vendor/gpui-pre/src/elements/div.rs:1355-1364,1956-1966); the child aria_label is an
accessible name, not a live property. Cargo.lock pins AccessKit consumer 0.38.0, where src/node.rs:889-892 inherits a
supplied live property or falls back to Off independently of role. On macOS, AccessKit macos 0.26.3 src/node.rs:161,273
maps Status to AXGroup/AXApplicationStatus and src/event.rs:236,299 gates its live announcement code on live != Off. On
Linux, AccessKit AT-SPI common 0.19.1 src/node.rs:251,480-485 maps Status to StatusBar and Off to no live politeness.
Pinned SPEC.md:3641-3645 calls for background items to announce themselves without taking focus, with exact means left
to the feature; the shared strip explicitly documents its means. The older trust strip's role-only pattern predates the
patch, but the affected hook hint is new. The reviewer suggested other. Source confirms no live announcement is
requested by this path; any screen-reader-specific inference from role or tree changes is unverified. Preserve ordinary
navigation of the text and controls while emitting a supported live announcement. Adapter dispatch is explicit in pinned
vendored gpui-pre-macos/src/window.rs:2437-2446 and gpui-pre-linux/src/linux/wayland/window.rs:2283-2287. The matching
AccessKit AT-SPI adapter 0.19.1 src/adapter.rs:72-76 also sends an Announcement only with non-None politeness. The
trust-hint precedent at pinned SPEC.md:1760-1764 explicitly calls for announcement without focus; the new hook hint uses
the same strip.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-session/HS-15`.
