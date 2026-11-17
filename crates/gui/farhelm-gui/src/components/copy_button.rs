//! The copy control: a button that puts a fixed text on the clipboard.
//!
//! The old app had a copy button in a few places only (SPEC.md and the migration's decisions keep exactly those): the
//! host setup confirmation's Copy plan, SSH help's two commands, the Tensorlake key command and the uninstall notice's
//! path. Each copies text frozen when the button was drawn, so what lands on the clipboard is what the user was shown.
//! One shared control so they cannot drift apart: Copy plan uses it directly, and SSH help's commands and the Tensorlake
//! key command through the well they share ([`super::CommandWell`]).
//!
//! NOTE: No "copied" confirmation of its own. Neither the old Copy plan nor SSH help's Copy showed one (reference commit
//! `24bc0580`). The old Tensorlake key command's Copy did, as a line under the command ("Command copied") that its
//! section drew, so [`copy_button_then`] tells the owner of each copy and the owner draws the line. The details bar's
//! directory and command line confirm too, but they are a different control: a value drawn as itself rather than a
//! labelled button, which says "✓ Copied" for a moment after each click ([`super::CopyField`]). All of them write the
//! clipboard through [`write`], so they cannot drift apart on how.

use gpui::{App, ClipboardItem, SharedString};

use super::{Button, HoverHelp, Role, Tier};

/// A button labelled `label` with hover help `help` that copies `text` to the clipboard when activated, in the neutral
/// tier the old copy buttons used. It is an ordinary [`Button`], so the caller may restyle it (Copy plan is compact).
/// Its role is always [`Role::Button`], so the constructor does not take one: a copy control is a push button wherever
/// it sits, and none of the old ones was a menu item or a toggle.
pub fn copy_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    help: HoverHelp,
    text: impl Into<SharedString>,
) -> Button {
    copy_button_then(id, label, help, text, |_| {})
}

/// [`copy_button`], also running `copied` after each copy, for the one old copy button that confirmed a click with a
/// line of its own (Settings → Tensorlake's "Command copied"); the owner draws and times the line. The text is written
/// first, so `copied` follows a copy the user can already paste.
pub fn copy_button_then(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    help: HoverHelp,
    text: impl Into<SharedString>,
    copied: impl Fn(&mut App) + 'static,
) -> Button {
    let text = text.into();
    Button::new(id, label, help, Role::Button)
        .tier(Tier::Neutral)
        .on_click(move |_, _, cx| {
            write(&text, cx);
            copied(cx);
        })
}

/// Put `text` on the system clipboard, byte for byte. Every copy control writes through here.
///
/// It never fails as far as the caller can tell: GPUI's write returns nothing, and SPEC.md wants a failed clipboard
/// write to fail silently ("Clipboard writes use the system clipboard and fail silently"). So a control's confirmation
/// says the user clicked, never that the platform took the text, as the old app's `copy_to_clipboard` documented.
pub(super) fn write(text: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
}
