# Masked secret fields can give plaintext to platform text services

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Farhelm covers tokens on screen, but the platform’s text input system can still receive the secret as ordinary text
while you edit it.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R6 Opus 1.

Source references: `crates/gui/farhelm-gui/src/components/text_input.rs:198–212`,
`vendor/gpui-pre-linux/src/linux/wayland/client.rs:721`, `vendor/gpui-pre-macos/src/window.rs:248`.

The masked constructor turns on GPUI Kit's display masking. In the pinned gpui-base 0.9.1 dependency,
src/input/base/state.rs:4561–4571 returns underlying text through text_for_range. On Wayland the input context declares
ContentPurpose::Normal. The Opus report also points to the macOS attributedSubstring implementation and observed no
Secure Event Input. That leaves the text-service route available for a Tensorlake key, ntfy token, or named credential.
Masked painting and Copy/Cut blocking work separately.

Opus rated it likely and noncritical. The source supports plaintext access by the text service. Whether a particular
input method actually learns or retains the token remains unverified. Highest reflects the shipped plaintext boundary.
SPEC.md “Credentials” calls these write-only secret fields. That covers saved-value handling and does not describe
disclosure to platform text services.

Keep secret text out of text_for_range, set password content purpose on Wayland, and use macOS Secure Event Input while
a secret field holds focus. Verify focused editing and the existing submission and masking behavior with fake secrets.
