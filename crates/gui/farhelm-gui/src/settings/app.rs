//! Settings → App: the installed desktop app's automatic-updates switch, the one choice in the dialog that belongs to
//! this app installation rather than to the helm.
//!
//! SPEC.md, "Session list" ("The installed desktop app's dialog also has an **App** section with a switch for
//! installing updates automatically ... kept with the app rather than by the helm") and "Installation and updates" (on
//! by default; off stops the startup and daily checks and the background installs, while on-demand checks keep
//! working). Ported from the old app's `settings.rs` (the `settings-app` section) at reference commit `24bc0580`, with
//! its note, label, hover help and the two help lines, one for each state.
//!
//! The section exists only while the desktop app's updater runs (`desktop::updates::runs`): the dialog lists App only
//! then, as SPEC.md has it ("followed by **App** only in a desktop app whose updater is active"). The switch reads and
//! writes through the updater ([`crate::desktop::updates::automatic_updates`] and
//! [`crate::desktop::updates::set_automatic_updates`]), which keeps the choice in the app's own state file, never the
//! helm, so nothing here goes through the operation store or holds the preference row's key: it is neither refused
//! while a helm save is in flight nor able to refuse one.
//!
//! The write is synchronous and its answer is final, so there is no draft: the switch always shows the setting in force
//! as the updater last read or saved it. A failed write leaves the setting as it was and shows the updater's short
//! reason under the switch until the next press, as the old switch did ("Only a saved choice is shown as the choice").

use gpui::{
    App, Div, FocusHandle, ParentElement as _, Role, SharedString, Styled as _, Window, div, px,
};

use super::choices::{self, GROUP_GAP, LABEL_LINE_HEIGHT, LABEL_TEXT};
use crate::components::{Checkbox, HoverHelp};

/// The switch's element id, which tests and the test bridge find it by.
pub(super) const ID: &str = "settings-app-automatic-updates";

/// The failure line's element id.
pub(super) const ERROR_ID: &str = "settings-app-automatic-updates-error";

/// The note above the switch, the old section's: what sets this section apart from the rest of the dialog. The updater
/// runs only in the installed Mac app, so "this Mac" holds wherever the section is drawn.
const NOTE: &str = "This one belongs to this Farhelm app on this Mac.";

const LABEL: &str = "Install updates automatically";

/// The hover help, the old row's `data-tooltip`.
const TOOLTIP: &str = "Install updates automatically: check for a new Farhelm release when the app starts and about \
                       once a day, and install it in the background";

/// The help under the switch while it is on.
const HELP_ON: &str = "Farhelm checks for a new release when it starts and about once a day, and installs it in the \
                       background. Restarting Farhelm finishes the update.";

/// The help under the switch while it is off: the on-demand routes that still install a release.
const HELP_OFF: &str = "Farhelm installs a new release only when you check for updates from the ? menu or this \
                        machine's update item, or run the installer yourself.";

/// What the open dialog keeps for the section: the switch's focus handle, so it stays the same control while the
/// section is hidden and shown again, and the reason the last press was not saved, until the next press.
pub(super) struct AppUpdates {
    pub(super) focus: FocusHandle,
    failure: Option<SharedString>,
}

impl AppUpdates {
    pub(super) fn new(focus: FocusHandle) -> Self {
        AppUpdates {
            focus,
            failure: None,
        }
    }

    /// The user turned the switch to `on`: write it through the updater. A saved choice clears the last failure; a
    /// failed one keeps the setting as it was and keeps the updater's reason for the line under the switch.
    pub(super) fn set(&mut self, on: bool, cx: &mut App) {
        self.failure = crate::desktop::updates::set_automatic_updates(on, cx)
            .err()
            .map(SharedString::from);
    }
}

/// The section's content below its heading: the note, then the switch with its help and, after a failed press, the
/// updater's reason. `None` when no updater runs, which never lists the section, so there is nothing to draw.
pub(super) fn section(
    state: &AppUpdates,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Option<[Div; 2]> {
    let on = crate::desktop::updates::automatic_updates(cx)?;
    let switch = div()
        .text_size(px(LABEL_TEXT))
        .line_height(px(LABEL_LINE_HEIGHT))
        .child(
            Checkbox::new(ID, LABEL, HoverHelp::tooltip(TOOLTIP), Role::Switch)
                .switch()
                .checked(on)
                .focus(&state.focus)
                .on_toggle(on_toggle),
        );
    let group = div()
        .flex()
        .flex_col()
        .gap(px(GROUP_GAP))
        .min_w_0()
        .child(switch)
        .child(choices::help(if on { HELP_ON } else { HELP_OFF }, cx))
        .children(
            state
                .failure
                .clone()
                .map(|reason| choices::failure_line(ERROR_ID.to_owned(), reason, cx)),
        );
    Some([choices::help(NOTE, cx), group])
}
