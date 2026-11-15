//! A host's actions menu: the muted ⋯ toggle at the end of a Hosts panel row, and the menu it opens beside itself.
//!
//! Ported from the old app's `hosts.rs` (the row's `host-row-menu` toggle, `host_menu_label`,
//! `host_menu_summary_parts` and the menu's items) and the stylesheet's `.host-row-menu` and `.host-row-menu-panel`
//! rules at reference commit `24bc0580`. It follows the session row's menu (`session_list::menu`): a popover of the
//! layer manager anchored beside its toggle, the shared header ([`MenuHeader`]) naming the host, then the commands in a
//! [`Role::Menu`] of their own, each an icon, a label and a description line, walked with Up and Down.
//!
//! What the menu offers is [`offers::offers`], one pure function of the pushed host, its retained run and its work in
//! flight, read while the menu draws, so an open menu follows the host. So far it draws Re-run on an SSH host whose
//! setup failed, which runs it again ([`super::setup::rerun`]); Settings, which opens the host settings dialog
//! ([`super::settings`]); and on the helm's own machine the greyed Update with the reason it is greyed ("Run the
//! installer again to update this machine"), which sends nothing (SPEC.md, "Session list"). Settings is unavailable
//! while the host has work in flight or a run going on.
//!
//! [`open_menu`] is the entry point. The Hosts panel's row toggle calls it, and so will the host rail's avatar menu,
//! with its own anchor and focus handle. Focus goes back to that control on every platform, also where a click leaves a
//! button unfocused (macOS), as the old app returned it there (SPEC.md, "Opening and closing"): the menu and the
//! settings dialog opened from it both name the control as where their closing returns focus
//! ([`focus::ReturnTo::Control`]), so nothing focuses it before the menu opens. When the control is no longer drawn,
//! focus goes back to what held it before the menu opened, or, when that was the control itself (a click focuses it on
//! Linux), home.
//!
//! NOTE: Each of the other commands arrives with the piece of the port that sends it, at its place in the old order
//! (a rerun, Update, Settings, then Remove behind a separator; the rerun of a failed Update with Update), and the
//! toggle keeps the old hover help, which names them already. Not ported: the old menu's Retry and Adopt (the
//! migration's settings-and-hosts area left both out of the port's second milestone on 2026-11-14), opening the menu
//! with the arrow keys from the closed toggle, and the flyout's pointer notch.

use farhelm_proto::ui_channel::HostView;
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, px,
};

use super::identity::{display_name, phase};
use super::offers::{self, HostKind, Offers, Rerun, Update};
use crate::assets::Icon;
use crate::components::{HoverHelp, IconButton, MenuHeader, MenuItem, Role, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::operations::OpKey;
use crate::session;

#[cfg(test)]
mod tests;

/// `.host-row-menu-panel`'s width, the session row menu's.
const PANEL_WIDTH: f32 = 246.;

/// The toggle's hover help, the old toggle's words.
const TOGGLE_HELP: &str = "Host actions: settings, update, retry, remove and more";

/// The toggle's right margin (`.host-row-menu`), which with the row's gap before it is the row's trailing gutter.
const TOGGLE_MARGIN: f32 = 2.;

/// The anchor and focus handle of host `id`'s menu toggle, kept in keyed element state for as long as its row is drawn
/// in consecutive frames: made once per frame for each drawn row, before [`toggle`] draws it.
pub(super) fn toggle_parts(id: i64, window: &mut Window, cx: &mut App) -> (Anchor, FocusHandle) {
    let anchor = Anchor::for_element(
        SharedString::from(format!("host-menu-anchor-{id}")),
        window,
        cx,
    )
    .beside();
    let focus = focus::control_handle(
        SharedString::from(format!("host-menu-toggle-{id}-focus-handle")),
        window,
        cx,
    );
    (anchor, focus)
}

/// Host `id`'s menu toggle (named after the host's `name`), as a Hosts panel row draws it: the ghost ⋯ in the quiet ink,
/// always visible (the old `.host-row-menu` had no rule hiding it at rest), drawn pressed while its menu is open
/// (`open`), over `anchor`, the element the menu opens beside, with `focus` as its focus handle ([`toggle_parts`]).
pub(super) fn toggle(
    id: i64,
    name: &HostText,
    open: bool,
    (anchor, focus): &(Anchor, FocusHandle),
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let button = IconButton::new(
        format!("host-menu-toggle-{id}"),
        Icon::More,
        actions_label(name),
        HoverHelp::tooltip(TOGGLE_HELP),
        Role::Button,
    )
    .tier(Tier::RowMenu { on_fill: false })
    .expanded(open)
    .focus(focus)
    .on_click(move |_, window, cx| on_toggle(window, cx));
    div()
        .relative()
        .flex_none()
        .mr(px(TOGGLE_MARGIN))
        .child(button)
        .child(anchor.track())
        .into_any_element()
}

/// Open host `host`'s actions menu beside `anchor`, the control the user acted on, whose focus handle is `opener`:
/// the entry point the Hosts panel's row toggle calls, and the host rail's avatar menu will. `mark` is the [`Mark`]
/// taken in that control's handler.
///
/// Focus starts on the menu's first command, as the old menu's did, and the menu names `opener` as where its closing
/// returns focus, so closing it returns focus to `opener`, and the settings dialog opened from the menu returns it
/// there too (see the module docs). Nothing opens, and no focus moves, when the user has moved on since `mark`, the
/// window is inert, the registry does not list the host, or `anchor` is not drawn.
///
/// Returns the menu's layer, or `None` when nothing opened.
pub fn open_menu(
    host: i64,
    anchor: Anchor,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let (view, offered) = current(host, cx)?;
    if focus::has_moved_on(mark, cx) {
        return None;
    }
    let rerun_focus = cx.focus_handle().tab_stop(true);
    let update_focus = cx.focus_handle().tab_stop(true);
    let settings_focus = cx.focus_handle().tab_stop(true);
    // Focus starts on the first command, as the old menu's did.
    let start = if reruns_setup(&view, &offered) {
        rerun_focus.clone()
    } else {
        match offered.update {
            Some(Update::Greyed) => update_focus.clone(),
            Some(Update::Available) | None => settings_focus.clone(),
        }
    };
    let menu = cx.new(|_| HostMenu {
        host,
        opener: opener.clone(),
        rerun_focus,
        update_focus,
        settings_focus,
        layer: None,
    });
    let layer = layers::open_popover(
        Popover {
            view: menu.clone().into(),
            container: cx.focus_handle(),
            start: Some(start),
            anchor,
            // The toggle, not what held focus before: on macOS the click that opened the menu left it unfocused.
            returns_to: focus::ReturnTo::Control(opener.clone()),
        },
        mark,
        window,
        cx,
    )?;
    menu.update(cx, |menu, _| menu.layer = Some(layer));
    Some(layer)
}

/// Host `host` as the store holds it now, with what its menu offers: [`offers::offers`] over its pushed entry, its
/// retained run and whether work on it is in flight. `None` when the window is inert or the registry does not list it.
fn current(host: i64, cx: &App) -> Option<(HostView, Offers)> {
    let store = session::current_store(cx)?;
    let key = HostView::key(host);
    let collections = store.read(cx).collections();
    let view = collections.get::<HostView>(&key)?.clone();
    let in_flight = session::current_operations(cx)
        .is_some_and(|operations| operations.read(cx).in_flight(&OpKey::host(&key)).is_some());
    let offered = offers::offers(&view, collections.get(&key), in_flight);
    Some((view, offered))
}

/// Whether the menu draws Re-run for `view`: what it offers is the rerun of a failed setup, on a kind whose setup the
/// rerun can run again so far ([`HostKind::reruns_setup`]).
fn reruns_setup(view: &HostView, offered: &Offers) -> bool {
    matches!(offered.rerun, Some((Rerun::Setup, _))) && HostKind::of(view).reruns_setup()
}

/// The toggle's and the menu's accessible name for a host named `name`: "Host actions for" and the name escaped as the
/// screen shows it, cut short ([`MenuHeader::clamp_name`]). Every row has a toggle, so the name must say whose.
fn actions_label(name: &HostText) -> String {
    format!("Host actions for {}", MenuHeader::clamp_name(name))
}

/// The open menu of one host: drawn from the store while it is open, so its header and its commands follow the host.
pub(crate) struct HostMenu {
    host: i64,
    /// The control the menu was opened from, where the settings dialog returns focus.
    opener: FocusHandle,
    /// Re-run's focus handle, where the menu starts when it offers it.
    rerun_focus: FocusHandle,
    /// The greyed Update's focus handle, where the menu starts when it offers one.
    update_focus: FocusHandle,
    /// Settings' focus handle, where the menu starts otherwise.
    settings_focus: FocusHandle,
    /// Set right after the layer opens; what a chosen command closes.
    layer: Option<LayerId>,
}

impl Render for HostMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = layers::panel(cx)
            .id("host-menu")
            .debug_selector(|| "host-menu".into())
            .w(px(PANEL_WIDTH));
        // A host the registry stopped listing takes its row, and so the menu's anchor, with it; until that frame there
        // is nothing to show.
        let Some((view, offered)) = current(self.host, cx) else {
            return panel;
        };
        let name = display_name(&view);
        let rerun = reruns_setup(&view, &offered).then(|| {
            let (menu, host) = (cx.weak_entity(), self.host);
            MenuItem::new(
                "host-menu-rerun",
                Icon::Restart,
                "Re-run",
                HoverHelp::tooltip("Re-run: try this host's failed setup again"),
                Role::MenuItem,
            )
            .description(Rerun::Setup.description())
            .focus(&self.rerun_focus)
            .on_click(move |_, window, cx| {
                // The mark of this click, taken before the menu closes, which moves focus back to the toggle; the
                // setup confirmation opens only through it.
                let mark = Mark::take(cx);
                let layer = menu.upgrade().and_then(|menu| menu.read(cx).layer);
                if let Some(layer) = layer {
                    layers::close(layer, window, cx);
                }
                // Checked again at the click: a run may have started, or work gone out, since the menu last drew.
                if let Some((view, offered)) = current(host, cx)
                    && reruns_setup(&view, &offered)
                {
                    super::setup::rerun(&view, mark, window, cx);
                }
            })
        });
        // Only the helm's own machine's greyed Update is drawn so far; the live one comes with the Update piece.
        let update = match offered.update {
            Some(Update::Greyed) => Some(
                MenuItem::new(
                    "host-menu-update",
                    Icon::Update,
                    "Update",
                    HoverHelp::tooltip("Update: run the installer again to update this machine"),
                    Role::MenuItem,
                )
                .description(Update::Greyed.description())
                .disabled(true)
                .focus(&self.update_focus),
            ),
            Some(Update::Available) | None => None,
        };
        let (menu, host, opener) = (cx.weak_entity(), self.host, self.opener.clone());
        let settings = MenuItem::new(
            "host-menu-settings",
            Icon::Settings,
            "Settings",
            HoverHelp::tooltip("Settings: this host's connection, appearance and permissions"),
            Role::MenuItem,
        )
        .description("Edit this host's connection")
        .disabled(!offered.settings_enabled)
        .focus(&self.settings_focus)
        .on_click(move |_, window, cx| {
            // The mark of this click, taken before the menu closes; the dialog returns focus to the menu's opener when
            // it closes.
            let mark = Mark::take(cx);
            let layer = menu.upgrade().and_then(|menu| menu.read(cx).layer);
            if let Some(layer) = layer {
                layers::close(layer, window, cx);
            }
            let _ = super::settings::open_settings(host, &opener, &mark, window, cx);
        });
        panel.child(header(&view, &name)).child(
            div()
                .id("host-menu-items")
                .role(Role::Menu)
                .aria_label(actions_label(&name))
                .flex()
                .flex_col()
                .children(rerun)
                .children(update)
                .child(settings),
        )
    }
}

/// The menu's header ([`MenuHeader`]): the host's name, then where it is and which Farhelm it runs on one line
/// ("dev@build-box · Farhelm 0.16.0"). The destination and the build are host values in the monospace face; the helm's
/// own machine reads "Local host", and a host that reports no build the last one the helm recorded for it, or "not
/// connected".
fn header(view: &HostView, name: &HostText) -> MenuHeader {
    let mut summary =
        HostSentence::new(SharedString::from(format!("host-menu-summary-{}", view.id)));
    summary = match &view.destination {
        Some(destination) => summary.mono(&HostText::from_host(destination.as_str())),
        None => summary.text("Local host"),
    };
    summary = summary.text(" · Farhelm ");
    let build = phase::reported_build(&view.state).or(view.remote_farhelm.as_deref());
    summary = match build {
        Some(build) => summary.mono(&HostText::from_host(build)),
        None => summary.text("not connected"),
    };
    MenuHeader::new(name, summary)
}
