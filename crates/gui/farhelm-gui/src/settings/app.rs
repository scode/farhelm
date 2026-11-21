//! Settings → App shows the running version, the on-demand check and the app's automatic-updates switch. The switch
//! belongs to this app installation. The check uses the sidebar readout's updater and leaves its result there.
//!
//! SPEC.md, "Session list" (the version row and the app's own setting) and "Installation and updates" (the shared
//! on-demand check and its progress). The switch keeps the old app's label and hover help from its `settings-app`
//! section at reference commit `24bc0580`.
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

use farhelm_proto::BUILD_VERSION;
use gpui::{
    App, Div, FocusHandle, ParentElement as _, Role, SharedString, Styled as _, Window, div, px,
};

use super::{card, choices};
use crate::components::{Button, Checkbox, HoverHelp, StatusMark, Tier};
use crate::desktop::updates;
use crate::update_readout::{InlineStatus, inline_status};

/// The switch's element id, which tests and the test bridge find it by.
pub(super) const ID: &str = "settings-app-automatic-updates";

/// The failure line's element id.
pub(super) const ERROR_ID: &str = "settings-app-automatic-updates-error";

/// The version mark's id, distinct from the one in the sidebar.
pub(super) const VERSION_ID: &str = "settings-app-version";
/// The Settings check button's id, distinct from the other update controls.
pub(super) const CHECK_ID: &str = "settings-app-check-for-updates";
pub(super) const CHECK_LABEL: &str = "Check for updates";
pub(super) const CHECK_HELP: &str = "Check for and install a newer Farhelm release. The version at the top of the sidebar shows the result.";

pub(super) const LABEL: &str = "Install updates automatically";

/// The hover help, the old row's `data-tooltip`.
pub(super) const TOOLTIP: &str = "Install updates automatically: check for a new Farhelm release when the app starts and about \
                       once a day, and install it in the background";

/// The help under the switch while it is on.
pub(super) const HELP_ON: &str = "Farhelm checks for a new release when it starts and about once a day, and installs it in the \
                       background. Restarting Farhelm finishes the update.";

/// The help under the switch while it is off: the on-demand routes that still install a release.
pub(super) const HELP_OFF: &str = "Farhelm installs a new release only when you check for updates (above, from the version in \
                        the sidebar, the ? menu or this machine's update item) or run the installer yourself.";

/// What the open dialog keeps for the section: the switch's focus handle, so it stays the same control while the
/// section is hidden and shown again, and the reason the last press was not saved, until the next press.
pub(super) struct AppUpdates {
    pub(super) focus: FocusHandle,
    pub(super) check_focus: FocusHandle,
    failure: Option<SharedString>,
}

impl AppUpdates {
    pub(super) fn new(cx: &mut App) -> Self {
        AppUpdates {
            focus: cx.focus_handle().tab_stop(true),
            check_focus: cx.focus_handle().tab_stop(true),
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

/// Share the updater's published progress words with the sidebar, which owns check results and their lifetime.
fn busy_status(cx: &App) -> Option<InlineStatus> {
    let readout = updates::readout(cx)?;
    inline_status(&readout.activity, readout.update_ready, None).filter(|status| status.spinning())
}

/// Match the geometry of a Settings switch row. Keep the check button's focus through progress so a second keypress
/// cannot land on the automatic-updates switch.
fn version_group(focus: &FocusHandle, cx: &App) -> Div {
    let busy = busy_status(cx);
    let help = "The Farhelm release this app is running.";
    let version = StatusMark::new(
        VERSION_ID,
        BUILD_VERSION,
        HoverHelp::tooltip(help),
        Role::Label,
        BUILD_VERSION,
    )
    .monospace();
    let check = Button::new(
        CHECK_ID,
        busy.map_or(CHECK_LABEL, InlineStatus::text),
        HoverHelp::tooltip(CHECK_HELP),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .focus(focus)
    .busy(busy.is_some())
    .on_click(|_, _, cx| {
        // An update started elsewhere can reach the updater before Settings draws the disabled button.
        if busy_status(cx).is_none() {
            updates::check_now(cx);
        }
    });
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .min_w_0()
        .child(card::switch_copy("Version", help, cx))
        .child(
            div()
                .flex()
                .items_center()
                .flex_none()
                .gap(px(8.))
                .child(version)
                .child(check),
        )
}

/// Version and automatic updates are rows of the same card. The switch owns the updater's refusal line.
/// The App section only exists while the updater runs.
pub(super) fn section(
    state: &AppUpdates,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Option<[Div; 2]> {
    let on = updates::automatic_updates(cx)?;
    let switch = Checkbox::new(ID, LABEL, HoverHelp::tooltip(TOOLTIP), Role::Switch)
        .switch()
        .content(card::switch_copy(
            LABEL,
            if on { HELP_ON } else { HELP_OFF },
            cx,
        ))
        .checked(on)
        .focus(&state.focus)
        .on_toggle(on_toggle);
    let group = div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .min_w_0()
        .child(switch)
        .children(
            state
                .failure
                .clone()
                .map(|reason| choices::failure_line(ERROR_ID.to_owned(), reason, cx)),
        );
    Some([version_group(&state.check_focus, cx), group])
}
