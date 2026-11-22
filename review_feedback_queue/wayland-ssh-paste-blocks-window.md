# Pasting an SSH answer can block the Wayland GUI on another application's clipboard

Reviewed commit: aedc40c90b32779c35dbe8bfcd1637e85e7aad12

## TLDR

Pasting an SSH answer on Wayland can block the GUI while another application supplies clipboard data.

## Details

**high.** Source inspection.

aedc40c90b32779c35dbe8bfcd1637e85e7aad12 SPEC.md:3444–3448; 1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:3466–3472,
5769–5773; Cargo.lock:2827–2839, 2868–2880; crates/gui/farhelm-gui/src/sign_in_form.rs:35–48, 167–194;
crates/gui/farhelm-gui/src/components/text_input.rs:195–215, 744–760, 816–820; gpui-base 0.9.1
src/input/base/state.rs:3217–3231; vendor/gpui-pre-linux/src/linux/wayland/clipboard.rs:155–180, 269–284, 312–325;
vendor/gpui-pre-linux/src/linux/platform.rs:1329 onward

Pasting into an SSH answer field on Wayland can freeze the desktop window while the clipboard owner supplies bytes. The
field uses GPUI Kit's normal Paste action. The pinned gpui-base handler first calls the synchronous clipboard API on the
GUI thread, then takes its asynchronous fallback only if that call returns no item. An uncached external Wayland offer
takes the synchronous receive-and-poll path, whose poll timeout is four seconds. The terminal's separate async clipboard
path does not cover this field. The assigned revision broadened the exception from terminal paste to all user-initiated
macOS paste and explicitly retained responsiveness for other operations. The shared text-field route is inherited, but
is within that clarified UI-wide paste boundary. Suggested fix: use the asynchronous read directly on Wayland, with the
existing focus/document check for a delayed paste. The reviewer suggested high for a window-wide stall on a supported
frontend. Confidence: confirmed source route conditional on an external offer that takes time to respond. No runtime
test was run. Possible coverage: separate from CC-11 (macOS clipboard writes).

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `contract-corrections/CC-17`.
