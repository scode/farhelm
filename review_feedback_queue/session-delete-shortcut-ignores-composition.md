# The Delete shortcut can run during input-method composition

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Pressing the session Delete shortcut while composing text can delete the open session, including its processes, when
Skip session confirmations is enabled.

## Details

Bucket: **high**. GUI migration final whole-tree review, R2 Sol composing Delete (G).

Source references: `vendor/gpui-pre-linux/src/linux/wayland/client.rs:2235, 2364`,
`crates/gui/farhelm-gui/src/actions.rs:91–99, 152, 410`, `crates/gui/farhelm-gui/src/main_view.rs:239–247`,
`crates/gui/farhelm-gui/src/session_list/delete.rs:273` at the reviewed commit (main
`crates/gui/farhelm-gui/src/session_list/delete.rs:281`).

Wayland tracks preedit through a separate text-input route. If the input method forwards Ctrl+Shift+Backspace as a key
event while preedit is active, the physical-key interceptor dispatches Delete before the terminal's composing-key guard.
The root handler checks for a focus layer and can invoke header Delete directly on the ordinary skip path. Cmd+W has the
same app-side gap if a Mac input source forwards it.

Sol rated it likely and critical. Opus verification G confirmed the SPEC.md line 3418 gap and rated it noncritical given
the explicit chord and off-by-default preference. The old browser app checked isComposing, so this is a regression. High
records the app-side guard gap. A supported input method forwarding the chord while composition is active would
establish the process/data-loss route and move it to highest.

Have DeleteSession (and check RenameSession) refuse while the focused input has marked text. Test the physical-key route
with an active composition, with and without skipped confirmations.
