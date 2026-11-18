# Terminal Paste can freeze the window on macOS

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

When macOS stalls while supplying clipboard contents, pasting into the terminal can freeze the whole Farhelm window.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The stalled condition is
conditional and was checked by source inspection.

Terminal Paste calls the native read on the GUI thread before spawning its task. The macOS implementation reads the
payload before returning a ready task, so a hung pasteboard stalls the whole window. Fix the read at the platform
boundary so the window stays responsive and keep the terminal's ordering for completed pastes.

Reviewed locations: `crates/gui/farhelm-gui/src/terminal/attached.rs:1610-1636`,
`vendor/gpui-pre-macos/src/platform.rs:1212-1219`, `SPEC.md:3115-3119`. Haiku out-P8.md §4. Suggested bucket **high**.

SPEC.md:3115–3119 allows failed copies to be silent and requires the rest of Farhelm to remain responsive. The Wayland
clipboard-order and file-paste queue items concern different platform operations.
