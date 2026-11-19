//! A host's actions menu: the muted ⋯ toggle at the end of a Hosts panel row, and the menu it opens beside itself.
//!
//! Ported from the old app's `hosts.rs` (the row's `host-row-menu` toggle, `host_menu_label`,
//! `host_menu_summary_parts` and the menu's items) and the stylesheet's `.host-row-menu` and `.host-row-menu-panel`
//! rules at reference commit `24bc0580`. It follows the session row's menu (`session_list::menu`): a popover of the
//! layer manager anchored beside its toggle, the shared header ([`MenuHeader`]) naming the host, then the commands in a
//! [`Role::Menu`] of their own, each an icon, a label and a description line, walked with Up and Down.
//!
//! What the menu offers is [`offers::menu_offers`], one pure function of the pushed host, its retained run, its work in
//! flight and whether the desktop app's updater runs, read while the menu draws, so an open menu follows the host. It
//! draws Retry first, which tries the host's connection again now, then Adopt for a mismatched identity, naming the
//! identity it accepts as the host reported it when the menu opened, which is the one the click sends
//! ([`super::remedies`]), and which on a host whose machine's deletion failed deletes it again; then Re-run on an SSH
//! host or a Tensorlake sandbox whose setup failed, which runs it again ([`super::setup::rerun`]), and on a host whose
//! Update failed, which sends that Update again ([`super::update::rerun`]); Update on a remote host, which updates it
//! without asking (the same request); Settings, which opens the host settings dialog ([`super::settings`]); on the
//! helm's own machine the greyed Update with the reason it is greyed, this platform's way to update it
//! ([`Update::description`]), which sends nothing (SPEC.md, "Session list"), as is a failed Update's Re-run there; in
//! the installed desktop app, where the app's updater runs, that machine's Update is live instead, "Check for a newer
//! Farhelm and install it", and asks the updater to check now ([`crate::desktop::updates::check_now`]), whose progress
//! and outcome show beside the version readout, so the menu just closes, as the old one did; and on an SSH host, behind
//! a separator in the danger red, Uninstall, which removes Farhelm from the host ([`super::uninstall`]), and Remove,
//! which forgets the host ([`super::remove`]); on a Tensorlake sandbox or a container, Delete sandbox or Delete
//! container in Remove's place, which deletes the machine after a question that always asks (the same module). Retry,
//! Adopt, Settings, Uninstall, Remove and Delete are unavailable while the host has work in flight or a run going on.
//!
//! [`open_menu`] is the entry point. The Hosts panel's row toggle calls it, and so does the host rail's avatar menu,
//! with its own anchor and focus handle. Focus goes back to that control on every platform, also where a click leaves a
//! button unfocused (macOS), as the old app returned it there (SPEC.md, "Opening and closing"): the menu and the
//! settings dialog opened from it both name the control as where their closing returns focus
//! ([`focus::ReturnTo::Control`]), so nothing focuses it before the menu opens. When the control is no longer drawn,
//! focus goes back to what held it before the menu opened, or, when that was the control itself (a click focuses it on
//! Linux), home.
//!
//! On a sandbox or container, Suspend or Resume shows after Retry, at its place in the old order
//! ([`super::lifecycle`]). Down and Up on the closed row toggle open the menu on its first or last command, as the old
//! toggle did ([`MenuStart`]). Not ported: the flyout's pointer notch.

use farhelm_proto::ui_channel::HostView;
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    WeakEntity, Window, div, px,
};

use super::identity::{display_name, phase};
use super::offers::{self, HostKind, Lifecycle, Offers, Rerun, Update};
use crate::actions::THIS_PLATFORM;
use crate::assets::Icon;
use crate::components::{
    HoverHelp, IconButton, MenuHeader, MenuItem, MenuStart, Role, Tier, menu_separator,
    toggle_arrow_keys,
};
use crate::environment_ui::ProviderUi;
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

/// Host `id`'s menu toggle (named after the host's `name`), as a Hosts panel row draws it: the ghost ⋯ in the quiet
/// ink, always visible (the old `.host-row-menu` had no rule hiding it at rest), drawn pressed while its menu is open
/// (`open`), over `anchor`, the element the menu opens beside, with `focus` as its focus handle ([`toggle_parts`]).
/// A click calls `on_toggle`; Down or Up on the closed toggle calls `on_arrow` with the command the menu starts on.
pub(super) fn toggle(
    id: i64,
    name: &HostText,
    open: bool,
    (anchor, focus): &(Anchor, FocusHandle),
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    on_arrow: impl Fn(MenuStart, &mut Window, &mut App) + 'static,
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
    .on_key_down(toggle_arrow_keys(on_arrow))
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
/// the entry point the Hosts panel's row toggle calls, and the host rail's avatar menu too. `mark` is the [`Mark`]
/// taken in that control's handler.
///
/// Focus starts on the menu's first command that can act, or with [`MenuStart::Last`] (Up on the closed toggle) its
/// last, and the menu names `opener` as where its closing returns
/// focus, so closing it returns focus to `opener`, and the settings dialog opened from the menu returns it
/// there too (see the module docs). Nothing opens, and no focus moves, when the user has moved on since `mark`, the
/// window is inert, the registry does not list the host, or `anchor` is not drawn.
///
/// Why not simply the first command: the old menu started on its first command, which was always Retry, available
/// whenever the host was idle, so it never opened on a greyed command. Here Retry is first too and so is usually where
/// focus starts; when the host is busy every command but a greyed one may be unavailable, and starting on a greyed
/// command would put focus where the menu draws no highlight (a greyed item takes none), so the user could not see
/// where Enter goes. The arrow keys still walk every command, the greyed ones included, as the old menu's did. The
/// same holds from the other end: Up starts on the last command that can act, which is the destructive group's last
/// on an idle host. When no command can act, either end falls back to the command drawn there, greyed or not.
///
/// Returns the menu's layer, or `None` when nothing opened.
pub fn open_menu(
    host: i64,
    start: MenuStart,
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
    let retry_focus = cx.focus_handle().tab_stop(true);
    let adopt_focus = cx.focus_handle().tab_stop(true);
    let rerun_focus = cx.focus_handle().tab_stop(true);
    let update_focus = cx.focus_handle().tab_stop(true);
    let settings_focus = cx.focus_handle().tab_stop(true);
    let uninstall_focus = cx.focus_handle().tab_stop(true);
    let remove_focus = cx.focus_handle().tab_stop(true);
    let delete_focus = cx.focus_handle().tab_stop(true);
    // Focus starts on the first command that can act. Retry is offered in every phase but an environment's deletion
    // and can act whenever the host is idle, so that is almost always Retry, where the old menu always opened; Adopt,
    // which comes next, can act only when Retry can, so it is never where focus starts. While an environment is being
    // deleted (no Retry), it is the first of Re-run, the live Update and Settings that can act. A greyed command (the
    // local host's Update, or its Re-run of a failed Update) is never where it starts while another can act (see
    // `open_menu`'s docs). When none can act (the host is busy), it starts on the first command drawn: Retry, else a
    // greyed Re-run, Update or Settings. Up walks the same commands from the end, so on an idle host it starts on the
    // destructive group's last (Remove, or the provider's Delete). Adopt and Suspend or Resume are left out: the one
    // never starts (above), and the other sits between Retry and Adopt, so Up reaches it only when nothing after it
    // can act and Down never does. Settings can act exactly when the host is idle, as can each destructive command, so
    // none of those three is where Down starts while Settings, before them, cannot act.
    let mut walk: Vec<(&FocusHandle, bool)> = [
        (&retry_focus, offered.retry),
        (
            &rerun_focus,
            rerun(&view, &offered).map(|(_, enabled)| enabled),
        ),
        (&update_focus, offered.update.map(Update::acts)),
        (&settings_focus, Some(offered.settings_enabled)),
        (&uninstall_focus, offered.uninstall),
        (&remove_focus, offered.remove),
        (&delete_focus, offered.delete.map(|(_, enabled)| enabled)),
    ]
    .into_iter()
    .filter_map(|(focus, acts)| acts.map(|acts| (focus, acts)))
    .collect();
    if start == MenuStart::Last {
        walk.reverse();
    }
    // Settings is always drawn, so the walk is never empty.
    let start = walk
        .iter()
        .find(|(_, acts)| *acts)
        .or(walk.first())
        .map_or_else(|| settings_focus.clone(), |(focus, _)| (*focus).clone());
    let menu = cx.new(|_| HostMenu {
        host,
        opener: opener.clone(),
        retry_focus,
        adopt_focus,
        adopt_identity: offers::adoptable(&view.state).map(str::to_owned),
        takeover_target: super::remedies::TakeoverTarget::of(&view.state),
        rerun_focus,
        update_focus,
        settings_focus,
        uninstall_focus,
        remove_focus,
        delete_focus,
        layer: None,
    });
    let layer = layers::open_popover(
        Popover::menu(menu.clone(), cx.focus_handle(), anchor, start)
            // The toggle, not what held focus before: on macOS the click that opened the menu left it unfocused.
            .returns_to(focus::ReturnTo::Control(opener.clone())),
        mark,
        window,
        cx,
    )?;
    menu.update(cx, |menu, _| menu.layer = Some(layer));
    Some(layer)
}

/// Host `host` as the store holds it now, with what its menu offers: [`offers::menu_offers`] over its pushed entry, its
/// retained run, whether work on it is in flight and whether the desktop app's updater runs. `None` when the window is
/// inert or the registry does not list it.
fn current(host: i64, cx: &App) -> Option<(HostView, Offers)> {
    let store = session::current_store(cx)?;
    let key = HostView::key(host);
    let collections = store.read(cx).collections();
    let view = collections.get::<HostView>(&key)?.clone();
    let in_flight = session::current_operations(cx)
        .is_some_and(|operations| operations.read(cx).in_flight(&OpKey::host(&key)).is_some());
    let offered = offers::menu_offers(
        &view,
        collections.get(&key),
        in_flight,
        crate::desktop::updates::runs(cx),
    );
    Some((view, offered))
}

/// The Re-run the menu draws for `view`, and whether it can be chosen: the rerun of a failed setup on a kind whose
/// setup the rerun can run again ([`HostKind::setup_rerun`]), or of a failed Update, greyed where the panel cannot
/// update the host. `None` when it draws none.
fn rerun(view: &HostView, offered: &Offers) -> Option<(Rerun, bool)> {
    offered.rerun.filter(|(rerun, _)| match rerun {
        Rerun::Setup => HostKind::of(view).setup_rerun().is_some(),
        Rerun::Update => true,
    })
}

/// The toggle's and the menu's accessible name for a host named `name`: "Host actions for" and the name escaped as the
/// screen shows it, cut short ([`MenuHeader::clamp_name`]). Every row has a toggle, so the name must say whose.
fn actions_label(name: &HostText) -> String {
    format!("Host actions for {}", MenuHeader::clamp_name(name))
}

/// The open menu of one host: drawn from the store while it is open, so its header and its commands follow the host,
/// except the identity Adopt names, which the menu froze when it opened ([`HostMenu::adopt_identity`]).
pub(crate) struct HostMenu {
    host: i64,
    /// The control the menu was opened from, where the settings dialog returns focus.
    opener: FocusHandle,
    /// Retry's focus handle, where the menu starts whenever Retry can be chosen, and when nothing can act.
    retry_focus: FocusHandle,
    /// Adopt's focus handle. Never where the menu starts: Adopt can be chosen only when Retry, before it, can.
    adopt_focus: FocusHandle,
    /// The identity a mismatched host reported when the menu opened, raw: the one Adopt shows and sends for as long as
    /// the menu is open. Frozen at the opening rather than read while drawing, because a key press (Enter) is
    /// dispatched after a fresh frame (GPUI draws a dirty window before it dispatches a key), so Enter on an item that
    /// read the store while drawing could send an identity no frame the user saw showed. A host that has since moved on
    /// is refused by the helm, and opening the menu again shows what it reports now. A menu opened before the host
    /// became mismatched offers no Adopt until it is opened again.
    adopt_identity: Option<String>,
    /// Take over sends the incumbent and installation held at menu open.
    takeover_target: Option<super::remedies::TakeoverTarget>,
    /// Re-run's focus handle, where the menu starts when it offers no Retry (an environment being deleted) and Re-run
    /// can act, or is the first command drawn when nothing can.
    rerun_focus: FocusHandle,
    /// Update's focus handle, where the menu starts when it offers no Retry and the live Update is its first command
    /// that can act, or is the first command drawn when nothing can.
    update_focus: FocusHandle,
    /// Settings' focus handle, where the menu starts when it offers no Retry and Settings is its first command that
    /// can act, and the last fallback.
    settings_focus: FocusHandle,
    /// The destructive group's focus handles (Uninstall, Remove, the provider's Delete), where Up on the closed toggle
    /// starts: on the last of them that is drawn and can act.
    uninstall_focus: FocusHandle,
    remove_focus: FocusHandle,
    delete_focus: FocusHandle,
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
        // Retry, first in the old order, in every phase but an environment's deletion.
        let retry = offered.retry.map(|enabled| {
            let (menu, host) = (cx.weak_entity(), self.host);
            // On a failed deletion Retry deletes again, and says so in the words the row's details use for it.
            let (help, description) = if super::remedies::deletes_again(&view.state) {
                (
                    "Retry: finish deleting this host's machine",
                    "Finish the deletion",
                )
            } else {
                (
                    "Retry: try this host's connection again now",
                    "Try the connection again",
                )
            };
            MenuItem::new(
                "host-menu-retry",
                Icon::Restart,
                "Retry",
                HoverHelp::tooltip(help),
                Role::MenuItem,
            )
            .description(description)
            .disabled(!enabled)
            .focus(&self.retry_focus)
            .on_click(move |_, window, cx| {
                // The mark of this click, taken before the menu closes; a refusal opens only through it.
                let mark = Mark::take(cx);
                close(&menu, window, cx);
                super::remedies::retry(host, mark, window, cx);
            })
        });
        // Suspend or Resume, on a sandbox or container: after Retry, in the old order. `help` is `Some` wherever the
        // offer is, since both need the host's provider.
        let lifecycle = offered.lifecycle.and_then(|(verb, enabled)| {
            let help = super::lifecycle::help(&view, verb)?;
            let (menu, host) = (cx.weak_entity(), self.host);
            // The old menu drew Suspend with the stop glyph; here Pause's two bars name it (a recorded deviation).
            let (id, icon, label) = match verb {
                Lifecycle::Suspend => ("host-menu-suspend", Icon::Pause, "Suspend"),
                Lifecycle::Resume => ("host-menu-resume", Icon::Restart, "Resume"),
            };
            let item = MenuItem::new(id, icon, label, HoverHelp::tooltip(help), Role::MenuItem)
                .disabled(!enabled)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    close(&menu, window, cx);
                    match verb {
                        Lifecycle::Suspend => super::lifecycle::suspend(host, mark, window, cx),
                        Lifecycle::Resume => super::lifecycle::resume(host, mark, window, cx),
                    }
                });
            Some(item)
        });
        // Adopt, while the host is still mismatched: the identity it names and sends is the one the menu opened with
        // (`HostMenu::adopt_identity`), never one read again later (`super::remedies`).
        let adopt = offered
            .adopt
            .zip(self.adopt_identity.clone())
            .map(|(enabled, reported)| {
                let (menu, host) = (cx.weak_entity(), self.host);
                let shown = HostSentence::new("host-menu-adopt-identity")
                    .text("Accept identity ")
                    .mono(&HostText::identity(reported.as_str()));
                MenuItem::new(
                    "host-menu-adopt",
                    Icon::Adopt,
                    "Adopt",
                    HoverHelp::tooltip(
                        "Adopt: accept the identity shown; this host's five permissions go back to asking",
                    ),
                    Role::MenuItem,
                )
                .description_sentence(shown)
                .disabled(!enabled)
                .focus(&self.adopt_focus)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    close(&menu, window, cx);
                    super::remedies::adopt(host, reported.clone(), mark, window, cx);
                })
            });
        let take_over = self
            .takeover_target
            .clone()
            .filter(|_| super::remedies::TakeoverTarget::of(&view.state).is_some())
            .map(|target| {
                let (menu, host) = (cx.weak_entity(), self.host);
                let shown = HostSentence::new("host-menu-owner")
                    .text("Managed by helm ")
                    .mono(&HostText::from_host(target.owner_tag.as_str()));
                MenuItem::new(
                    "host-menu-take-over",
                    Icon::Adopt,
                    "Take over",
                    HoverHelp::tooltip("Manage this host and its sessions from this Farhelm"),
                    Role::MenuItem,
                )
                .description_sentence(shown)
                .disabled(offered.retry != Some(true))
                .focus(&self.adopt_focus)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    close(&menu, window, cx);
                    super::remedies::take_over(host, target.clone(), mark, window, cx);
                })
            });
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
            MenuItem::new(
                "host-menu-update",
                Icon::Update,
                "Update",
                HoverHelp::tooltip(offered.help(THIS_PLATFORM)),
                Role::MenuItem,
            )
            .description(offered.description(THIS_PLATFORM))
            .disabled(!offered.acts())
            .focus(&self.update_focus)
            .on_click(move |_, window, cx| {
                match offered {
                    Update::Available => {
                        // As Re-run's: the mark of this click, before the menu closes. Not checked again here: a press
                        // that meets work on the host's key sends nothing and says so under the row, and the helm
                        // refuses an Update of a host whose run started meanwhile.
                        let mark = Mark::take(cx);
                        close(&menu, window, cx);
                        super::update::start(host, mark, window, cx);
                    }
                    // The app's own check: its progress and outcome show beside the version readout, so the menu only
                    // closes. The updater runs one check at a time, so a press during one joins it.
                    Update::CheckForRelease => {
                        close(&menu, window, cx);
                        crate::desktop::updates::check_now(cx);
                    }
                    // Never reached: the greyed item takes no click. Nothing to do if it were.
                    Update::Greyed => {}
                }
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
            .focus(&self.remove_focus)
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
        // The provider's Delete in Remove's place, where removing the host deletes its machine: the old menu's one
        // removal item, labelled for the provider. Its question always asks, whatever "Remove hosts without asking"
        // says (`super::remove`).
        let delete = offered.delete.map(|(provider, enabled)| {
            let (menu, host) = (cx.weak_entity(), self.host);
            let words = ProviderUi(provider);
            MenuItem::new(
                "host-menu-delete",
                Icon::Trash,
                words.host_delete_label(),
                HoverHelp::tooltip(words.host_delete_help()),
                Role::MenuItem,
            )
            .description(words.host_delete_help())
            .danger()
            .disabled(!enabled)
            .focus(&self.delete_focus)
            .on_click(move |_, window, cx| {
                // As Remove's: the mark of this click, before the menu closes and returns focus to its toggle.
                let mark = Mark::take(cx);
                close(&menu, window, cx);
                super::remove::start(host, &mark, window, cx);
            })
        });
        // Uninstall where the offers say, before Remove in the destructive group, as the old menu ordered them.
        let uninstall = offered.uninstall.map(|enabled| {
            let (menu, host) = (cx.weak_entity(), self.host);
            MenuItem::new(
                "host-menu-uninstall",
                Icon::Uninstall,
                "Uninstall",
                HoverHelp::tooltip(super::uninstall::UNINSTALL_HELP),
                Role::MenuItem,
            )
            .description("Remove Farhelm from this host, keeping its data")
            .danger()
            .disabled(!enabled)
            .focus(&self.uninstall_focus)
            .on_click(move |_, window, cx| {
                // As Update's: the mark of this click, before the menu closes; the confirmation and a refusal open
                // only through it.
                let mark = Mark::take(cx);
                close(&menu, window, cx);
                super::uninstall::start(host, mark, window, cx);
            })
        });
        let destructive: Vec<AnyElement> = uninstall
            .map(IntoElement::into_any_element)
            .into_iter()
            .chain(remove.map(IntoElement::into_any_element))
            .chain(delete.map(IntoElement::into_any_element))
            .collect();
        let removal = (!destructive.is_empty())
            .then(|| std::iter::once(menu_separator(cx).into_any_element()).chain(destructive));
        panel.child(header(&view, &name)).child(
            div()
                .id("host-menu-items")
                .role(Role::Menu)
                .aria_label(actions_label(&name))
                .flex()
                .flex_col()
                .children(retry)
                .children(lifecycle)
                .children(adopt)
                .children(take_over)
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
