# A late text commit can reach the tab you just left

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Text from an input method or dead key can go to the shell tab you just left if the platform is still using that tab's
input handler.

## Details

Bucket: **high**. GUI migration final whole-tree review, R2 Opus 3.

Source references: `crates/gui/farhelm-gui/src/terminal/attached.rs:2203–2218, 4099–4105, 4133–4137, 4186–4194`.

The GPUI frame may retain A's input handler during a switch from tab A to tab B. A late replace_text_in_range reaches
A's commit_text and sends to A's current attachment without checking that A still owns window focus. The ordinary key
listener makes that check. Ordinary Mac characters can also use a platform text commit.

Opus rated this possible and noncritical. The source confirms the focus check is absent. The platform delivery window
across an actual tab switch remains unverified. An established harmful cross-terminal route, such as a commit reaching a
different program and ending user work, would warrant highest.

Bind commit and marked-text callbacks to the focused terminal. Check focus and attachment ownership before accepting
platform text. Exercise a callback from the previous input handler after a tab switch.
