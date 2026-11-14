//! The copy control: a button that puts a fixed text on the clipboard.
//!
//! The old app had a copy button in a few places only (SPEC.md and the migration's decisions keep exactly those): the
//! host setup confirmation's Copy plan, SSH help's two commands, the Tensorlake key command and the uninstall notice's
//! path. Each copies text frozen when the button was drawn, so what lands on the clipboard is what the user was shown.
//! One shared control so they cannot drift apart; the first consumer is Copy plan.
//!
//! NOTE: No "copied" confirmation. The old Copy plan showed none (reference commit `24bc0580`); a consumer whose old
//! button did (SSH help) adds it here, through the ticker, when it is ported.

use gpui::{ClipboardItem, SharedString};

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
    let text = text.into();
    Button::new(id, label, help, Role::Button)
        .tier(Tier::Neutral)
        .on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()))
        })
}
