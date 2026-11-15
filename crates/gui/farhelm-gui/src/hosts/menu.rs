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
//! setup failed, which runs it again ([`super::setup::rerun`]), and on a host whose Update failed, which sends that
//! Update again ([`super::update::rerun`]); Update on a remote host, which updates it without asking (the same
//! request); Settings, which opens the host settings dialog ([`super::settings`]); on the helm's own machine the greyed
//! Update with the reason it is greyed ("Run the installer again to update this machine"), which sends nothing
//! (SPEC.md, "Session list"), as is a failed Update's Re-run there; and on an SSH host, behind a separator in the
//! danger red, Remove, which forgets the host ([`super::remove`]). Settings and Remove are unavailable while the host
//! has work in flight or a run going on.
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
//! (a rerun, Update, Settings, then Uninstall and Remove behind a separator), and the toggle keeps the old hover help,
//! which names them already. Not ported: the old menu's Retry and Adopt (the
//! migration's settings-and-hosts area left both out of the port's second milestone on 2026-11-14), opening the menu
//! with the arrow keys from the closed toggle, and the flyout's pointer notch.

use farhelm_proto::ui_channel::HostView;
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    WeakEntity, Window, div, px,
};

use super::identity::{display_name, phase};
use super::offers::{self, HostKind, Offers, Rerun, Update};
use crate::assets::Icon;
use crate::components::{HoverHelp, IconButton, MenuHeader, MenuItem, Role, Tier, menu_separator};
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
/// Focus starts on the menu's first command that can act, and the menu names `opener` as where its closing returns
/// focus, so closing it returns focus to `opener`, and the settings dialog opened from the menu returns it
/// there too (see the module docs). Nothing opens, and no focus moves, when the user has moved on since `mark`, the
/// window is inert, the registry does not list the host, or `anchor` is not drawn.
///
/// Why not simply the first command: the old menu started on its first command, but that was always Retry, which this
/// port leaves out, so the old local menu never opened on its greyed Update. Starting there now would put focus where
/// the menu draws no highlight (a greyed item takes none), so the user could not see where Enter goes. The arrow keys
/// still walk every command, the greyed Update included, as the old menu's did.
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
    // Focus starts on the first command that can act: an available Re-run, then the live Update, then Settings when
    // available. A greyed command (the local host's Update, or its Re-run of a failed Update) is never where it starts
    // (see `open_menu`'s docs); only a menu where nothing can act starts on its first command.
    let rerun_drawn = rerun(&view, &offered);
    let start = if rerun_drawn.is_some_and(|(_, enabled)| enabled) {
        rerun_focus.clone()
    } else if offered.update == Some(Update::Available) {
        update_focus.clone()
    } else if offered.settings_enabled {
        settings_focus.clone()
    } else if rerun_drawn.is_some() {
        rerun_focus.clone()
    } else if offered.update == Some(Update::Greyed) {
        update_focus.clone()
    } else {
        settings_focus.clone()
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
            traps_tab: true,
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

/// The Re-run the menu draws for `view`, and whether it can be chosen: the rerun of a failed setup on a kind whose setup
/// the rerun can run again so far ([`HostKind::reruns_setup`]), or of a failed Update, greyed where the panel cannot
/// update the host. `None` when it draws none.
fn rerun(view: &HostView, offered: &Offers) -> Option<(Rerun, bool)> {
    offered.rerun.filter(|(rerun, _)| match rerun {
        Rerun::Setup => HostKind::of(view).reruns_setup(),
        Rerun::Update => true,
    })
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
    /// Update's focus handle, where the menu starts when the live Update is its first command that can act, or the
    /// greyed one when no command in it can act.
    update_focus: FocusHandle,
    /// Settings' focus handle, where the menu starts when it offers no Re-run and Settings is available.
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
        let rerun = rerun(&view, &offered).map(|(which, enabled)| {
            let (menu, host) = (cx.weak_entity(), self.host);
            let help = match which {
                Rerun::Setup => "Re-run: try this host's failed setup again",
                Rerun::Update => "Re-run: try this host's failed update again",
            };
            MenuItem::new(
                "host-menu-rerun",
                Icon::Restart,
                "Re-run",
                HoverHelp::tooltip(help),
                Role::MenuItem,
            )
            .description(which.description())
            .disabled(!enabled)
            .focus(&self.rerun_focus)
            .on_click(move |_, window, cx| {
                // The mark of this click, taken before the menu closes, which moves focus back to the toggle; the
                // setup confirmation and an Update's refusal open only through it.
                let mark = Mark::take(cx);
                close(&menu, window, cx);
                // Checked again at the click: a run may have started, or work gone out, since the menu last drew.
                if let Some((view, offered)) = current(host, cx) {
                    match rerun(&view, &offered) {
                        Some((Rerun::Setup, true)) => super::setup::rerun(&view, mark, window, cx),
                        Some((Rerun::Update, true)) => super::update::rerun(host, mark, window, cx),
                        Some((_, false)) | None => {}
                    }
                }
            })
        });
        let update = offered.update.map(|offered| {
            let (menu, host) = (cx.weak_entity(), self.host);
            let help = match offered {
                Update::Available => "Update: install this helm's Farhelm version on this host",
                Update::Greyed => "Update: run the installer again to update this machine",
            };
            MenuItem::new(
                "host-menu-update",
                Icon::Update,
                "Update",
                HoverHelp::tooltip(help),
                Role::MenuItem,
            )
            .description(offered.description())
            .disabled(offered == Update::Greyed)
            .focus(&self.update_focus)
            .on_click(move |_, window, cx| {
                // As Re-run's: the mark of this click, before the menu closes. Not checked again here: a press that meets
                // work on the host's key sends nothing and says so under the row, and the helm refuses an Update of a
                // host whose run started meanwhile.
                let mark = Mark::take(cx);
                close(&menu, window, cx);
                super::update::start(host, mark, window, cx);
            })
        });
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
            close(&menu, window, cx);
            let _ = super::settings::open_settings(host, &opener, &mark, window, cx);
        });
        // Remove where the offers say, unavailable while the host is busy; the old menu's last command, in its
        // destructive group behind a separator.
        let remove = offered.remove.map(|enabled| {
            let (menu, host) = (cx.weak_entity(), self.host);
            MenuItem::new(
                "host-menu-remove",
                Icon::Trash,
                "Remove",
                HoverHelp::tooltip(super::remove::REMOVE_HELP),
                Role::MenuItem,
            )
            .description("Forget this host")
            .danger()
            .disabled(!enabled)
            .on_click(move |_, window, cx| {
                // The mark of this click, taken before the menu closes. Closing the menu returns focus to its
                // toggle, which the confirmation then returns focus to when it closes.
                let mark = Mark::take(cx);
                let layer = menu.upgrade().and_then(|menu| menu.read(cx).layer);
                if let Some(layer) = layer {
                    layers::close(layer, window, cx);
                }
                super::remove::start(host, &mark, window, cx);
            })
        });
        let removal = remove.map(|remove| {
            [
                menu_separator(cx).into_any_element(),
                remove.into_any_element(),
            ]
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
                .child(settings)
                .children(removal.into_iter().flatten()),
        )
    }
}

/// Close `menu`, as a chosen command does before it acts.
fn close(menu: &WeakEntity<HostMenu>, window: &mut Window, cx: &mut App) {
    let layer = menu.upgrade().and_then(|menu| menu.read(cx).layer);
    if let Some(layer) = layer {
        layers::close(layer, window, cx);
    }
}

/// The menu's header ([`MenuHeader`]): the host's name, then where it is and which Farhelm it runs on one line
/// ("dev@build-box · Farhelm 0.16.0"). The destination and the build are host values in the monospace face; the helm's
/// own machine reads "Local host", and a host that reports no build reads "not connected" in the app's own face.
///
/// NOTE: The old header fell back to the host's `remote_farhelm` when no build was reported, but that field is where
/// Farhelm is installed on the host (a path), not a version, so the line read as "Farhelm ~/.local/bin/farhelm". Only a
/// build the host reports now ([`phase::reported_build`]) is shown as one.
fn header(view: &HostView, name: &HostText) -> MenuHeader {
    let mut summary =
        HostSentence::new(SharedString::from(format!("host-menu-summary-{}", view.id)));
    summary = match &view.destination {
        Some(destination) => summary.mono(&HostText::from_host(destination.as_str())),
        None => summary.text("Local host"),
    };
    summary = summary.text(" · Farhelm ");
    summary = match phase::reported_build(&view.state) {
        Some(build) => summary.mono(&HostText::from_host(build)),
        None => summary.text("not connected"),
    };
    MenuHeader::new(name, summary)
}
