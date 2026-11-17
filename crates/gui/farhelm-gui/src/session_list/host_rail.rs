//! The host rail: the column of hosts at the sidebar's left edge that narrows the session list to one host, and the
//! host names the list's other filter controls share with it (the chip under the heading and the empty sentence, both
//! drawn by [`super::filters`]).
//!
//! SPEC.md, "Session list" (the rail), ported from the old app's `hosts.rs` (`HostRailAvatar`, the rail's entries) and
//! the stylesheet's `.host-rail` rules at the reference commit.
//!
//! The rail lists, top to bottom: **All**, one avatar per registered host in the registry's order (a host a session
//! owns, a sandbox or container made for one session, has no avatar of its own: its sessions appear under All), a "?"
//! placeholder for a chosen host the registry no longer lists, a "?" status while the host list has never been read,
//! and **Add host** last. Each avatar is the host's chosen mark in its chosen color with its short name as a caption
//! under it; the caption repeats what the avatar's accessible name already says, so it has no node of its own for
//! assistive technology. Names, destinations and aliases are the host's own text, drawn and named through the
//! host-text element.
//!
//! Add host opens the Add host dialog ([`crate::hosts::add::open`]) with itself as the opener, so that closing the
//! dialog returns focus to it, as the gear does for Settings. It is never disabled: the old button was, while the old
//! page's one token for launches and host registry changes was held (`ops::OpLock` at the reference commit), but here
//! refusing belongs to the dialog's own Add, which refuses while a session launch is in flight, each side refusing its
//! own action while the other's work is in flight (decided by the migration's orchestrator on 2026-11-14).
//!
//! The rail's entries scroll on their own, and a focus move the keyboard or the app makes onto an entry below or above
//! the rail's fold scrolls it into view: the scrolling part is a focus coordinator scroll region
//! ([`focus::scroll_region`]). A press on All, an avatar or Add host scrolls nothing, as in a browser. The Add host
//! dialog names Add host as the control its closing returns focus to (`focus::ReturnTo::Control`), and that return is
//! an app's focus move, so a half-hidden Add host is scrolled into view when the dialog closes.
//!
//! Choosing is a query, not a filter the window applies: a press (or Space or Enter) on All or an avatar sends the UI
//! channel's session query with the host chosen and the status in force, and the helm then serves only that host's
//! sessions with their counts. The query, its two moments (the choice and the helm's answer) and its refusals are
//! [`super::filters`]'s. Removing the host filter from the chip or the empty sentence moves focus to the rail's All,
//! whose focus handle is the list's for that reason.
//!
//! The rail's foot ([`foot`]) is pinned under its entries, which scroll above it, and holds the notices bell at its top
//! ([`crate::rail_notices`]), Hosts under it above the Settings gear, the `?` Help button below the gear
//! ([`crate::help_menu`]), and the Expand control last, in the old foot's order. Hosts swaps the list's column for the
//! Hosts panel ([`crate::hosts::HostsPanel`]) and back, keeping focus on itself; choosing All or a host returns to
//! Sessions (SPEC.md: "All or a host avatar returns to Sessions"), and while the panel shows, the rail's chosen entry
//! wears a quieter look so the filled Hosts control marks the view on screen. Whether the panel shows belongs to the UI
//! session, like the choice. The gear opens the Settings dialog ([`crate::settings::open`]) with itself as the opener,
//! so that Escape and either close control bring focus back to it (SPEC.md: "Escape and either close control return
//! focus to the gear").
//!
//! The foot is drawn by the list after its own column rather than inside the rail, as the old app placed its
//! `.host-rail-footer` after the sidebar's content: GPUI's Tab order follows the order controls are drawn in, and the
//! old app kept "host choices before Settings" with the list's controls between them, so Tab goes from the rail
//! through the list (or the Hosts panel) to the foot. The rail leaves the foot's height free at its bottom
//! ([`FOOT_HEIGHT`]), and the foot is drawn over that room.
//!
//! Each avatar also shows its host's status: the connection dot, the update badge, the ring while work runs, the
//! warning mark and dimming, with the state's words in its name and hover help; Hosts carries an `↑` while any avatar
//! has an update, and its name and help count them. [`status`] draws them from what [`crate::hosts::status`] reads.
//!
//! **Expansion.** The foot's last control, named "Expand navigation rail" with » while the rail is collapsed and
//! "Collapse navigation rail" with « and the label Collapse while it is expanded (the old `#host-rail-toggle`), widens
//! the whole rail, its foot included, from 56 pixels to 220, and reports that state as expanded rather than open: it
//! keeps the rail's quiet look either way ([`crate::components::Button::discloses`]). Expanded, every entry is a row as
//! wide as the rail and one click target, its glyph or mark in the same 40-pixel icon box at the same place as in the
//! collapsed square, so nothing a user aims at moves: All reads "All hosts", an avatar shows its host's display name with
//! its destination smaller under it when the two differ (both cut short, the full identity staying in the avatar's
//! name and hover help) in place of its caption, Add host reads "Add host", and the foot's controls show Notifications,
//! Hosts, Settings, Help and Collapse beside their glyphs. Choosing, badges, rings, menus and actions work the same in
//! both widths: they are the same controls, and an avatar's status marks are anchored to its icon box rather than to
//! the button ([`status::icon_box`]). Whether the rail is expanded is the helm's `host_rail_open` preference, and
//! [`expansion`] has how it is read and saved; the sidebar adds the expanded rail's extra 164 pixels to its shown width
//! (`crate::sidebar`), so the list beside the rail keeps its width when the window has room.
//!
//! **An avatar's menu.** A secondary click on an avatar (a right press, or a Ctrl+click on macOS, which the platform
//! layer turns into one), or Shift+F10 while it has focus, opens that host's actions menu beside the avatar's icon box
//! through the host menu's entry point ([`crate::hosts::open_menu`]), the same menu as the Hosts panel row's ⋯ (SPEC.md:
//! "Host actions open on demand from the row menu or the same menu on its rail avatar"). It never chooses the host: the
//! old rail kept selection and the menu separate gestures so that opening a host's actions never changes which sessions
//! the list shows. The avatar is the menu's opener, so Escape, and every dialog the menu opens, return focus to it, and
//! it wears the open look while its menu is open (the old avatar's `aria-expanded`).
//!
//! NOTE: The old avatar also opened its menu with the keyboard's ContextMenu key. GPUI names no such key on either
//! platform, so only Shift+F10 is ported; the browser behaviors survey (`docs/gpui/browser-behaviors.md`, B2) lists
//! both.

pub(crate) mod expansion;
mod status;
#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
mod avatar_menu_tests;

use std::collections::HashMap;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{Availability, Collection, HostView};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    WeakEntity, Window, div, px, svg,
};

use super::SessionList;
use super::row::short_name;
use crate::assets::Icon;
use crate::components::{
    Button, HoverHelp, MenuStart, RAIL_ICON_BOX, Role, StatusMark, Tier, rail_entry,
};
use crate::focus::{self, Mark};
use crate::help_menu::HelpButton;
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::identity::{display_name, host_mark};
use crate::hosts::status::{HostStatus, host_status};
use crate::layers::{self, Anchor, LayerId};
use crate::rail_notices::NoticesButton;
use crate::settings;
use crate::theme::{self, Palette};

/// `.host-rail-hover`'s padding, vertical then horizontal, and the gap between its entries.
const RAIL_PAD: (f32, f32) = (8., 7.);
const RAIL_GAP: f32 = 8.;

/// `.host-rail-avatar .host-kind-icon`: the mark's size on an avatar.
const AVATAR_MARK: f32 = 24.;

/// `.host-rail-caption`: its width (the button's), size, line height and the space above it.
const CAPTION_WIDTH: f32 = 40.;
const CAPTION_SIZE: f32 = 10.;
const CAPTION_LINE: f32 = 14.;
const CAPTION_GAP: f32 = 2.;

/// `.host-filter-chip .host-kind-icon`: the mark's size in the chip and in the empty sentence ([`super::filters`]).
pub(super) const CHIP_MARK: f32 = 14.;

/// `.host-rail .btn`'s side: the square every rail and foot control is drawn in.
const RAIL_BUTTON: f32 = 40.;

/// `.host-rail-footer`'s padding: the bottom, and the sides that center a 40-pixel control in the 55 pixels left of the
/// rail beside its 1-pixel edge.
const FOOT_PAD_BOTTOM: f32 = 8.;
const FOOT_PAD_X: f32 = 7.5;

/// The foot's height: the old foot's 208 pixels, its five controls (the notices bell, Hosts, the gear, Help and the
/// Expand control) and the bottom padding.
pub(super) const FOOT_HEIGHT: f32 = 5. * RAIL_BUTTON + FOOT_PAD_BOTTOM;

/// `.host-rail-expanded`'s place and size in an expanded entry: 48 pixels from the button's left edge (8 past the icon
/// box), as wide as the rail less 72 pixels, its name over the destination 2 pixels apart.
const NAMES_LEFT: f32 = 48.;
const NAMES_INSET: f32 = 72.;
const NAMES_GAP: f32 = 2.;

/// `.host-rail-expanded-name` and `.host-rail-destination`: the expanded name's size and line height, and the smaller
/// destination's under it.
const NAME_SIZE: f32 = 12.;
const NAME_LINE: f32 = 16.;
const DESTINATION_SIZE: f32 = 10.;
const DESTINATION_LINE: f32 = 12.;

/// The Expand control's names, each its hover help too, and the label the expanded rail shows beside its «. The old
/// toggle's words (`sidebar-width.js`'s `updateLayout`).
pub(crate) const EXPAND_NAME: &str = "Expand navigation rail";
pub(crate) const COLLAPSE_NAME: &str = "Collapse navigation rail";
const COLLAPSE_LABEL: &str = "Collapse";

/// The Expand control's element id, the old `#host-rail-toggle`'s, which tests and the test bridge find it by.
pub(crate) const TOGGLE_ID: &str = "host-rail-toggle";

/// `SettingsIcon`'s size in the old foot's icon box.
const SETTINGS_GLYPH: f32 = 16.;

/// The old `ActionIcon`'s size: Add host's plus and Hosts' glyph, which were icon buttons' 14-pixel icons.
const ACTION_GLYPH: f32 = 14.;

/// The gear's hover help, the old app's words. The old app added ", and this app's automatic updates" when the desktop
/// app's updater was active; this app has no updater yet, so the words stop where the old app's did without one.
const SETTINGS_HELP: &str = "Settings: choices that apply to every host on this helm";

impl SessionList {
    /// The user pressed Hosts: show the Hosts panel in place of the list's column, or Sessions again if it shows. The
    /// query, the sort, the open session and the panel's own Details choice are untouched (SPEC.md: "opening or closing
    /// Hosts keeps the current query, sort, selected session and details choice"). Focus stays on Hosts (SPEC.md: "The
    /// control keeps focus"), also where a click focuses no button (macOS), as the old app focused it explicitly.
    /// `mark` is the one taken in the control's handler.
    fn toggle_hosts(&mut self, mark: &Mark, window: &mut Window, cx: &mut Context<Self>) {
        if self.current.current(cx).hosts_shown {
            self.hide_hosts(cx);
        } else {
            self.show_hosts(cx);
        }
        focus::focus_control(&self.hosts_focus, mark, window, cx);
    }

    /// Show Sessions in place of the Hosts panel, if the panel shows, without a focus move. The query, the sort and the
    /// open session are untouched. Every path that hides the panel goes through here, because the panel must hear of it
    /// ([`HostsPanel::hidden`]: the rail's warnings keep their own copy of whether Hosts shows): Hosts pressed again,
    /// SPEC.md's returns to Sessions ("All or a host avatar returns to Sessions, as does an accepted quick-switcher pick
    /// or New handoff"; [`Self::choose_host`], [`Self::picked_from_switcher`], [`super::open_new`], and
    /// [`super::show_sessions`] for a quick launch from the page and the session pane's ways into the launcher). Unguarded:
    /// none of those runs while the window is inert, and a reset shows Sessions on its own.
    ///
    /// [`HostsPanel::hidden`]: crate::hosts::HostsPanel::hidden
    pub(super) fn hide_hosts(&mut self, cx: &mut Context<Self>) {
        let current = self.current.current(cx);
        if current.hosts_shown {
            current.hosts_shown = false;
            self.hosts_panel.update(cx, |panel, cx| panel.hidden(cx));
            cx.notify();
        }
    }

    /// Show the Hosts panel in place of the list's column, if it does not show, without a focus move. The one path
    /// that shows it, because the panel must hear of it ([`HostsPanel::shown`], which acknowledges the hosts' warnings):
    /// Hosts pressed, and a golden scene, which has no focus handle of the foot to hand a mark to.
    ///
    /// [`HostsPanel::shown`]: crate::hosts::HostsPanel::shown
    pub(crate) fn show_hosts(&mut self, cx: &mut Context<Self>) {
        let current = self.current.current(cx);
        if !current.hosts_shown {
            current.hosts_shown = true;
            self.hosts_panel.update(cx, |panel, cx| panel.shown(cx));
            cx.notify();
        }
    }

    /// Whether the host rail is expanded as this window shows it ([`expansion::Expansion::shown`]): what the sidebar
    /// reads to add the expanded rail's width to its own, and what the rail and its foot draw from.
    pub(crate) fn rail_expanded(&self, cx: &App) -> bool {
        self.rail.expansion.read(cx).shown(cx)
    }

    /// The Expand or Collapse control was pressed: save the other width ([`expansion::Expansion::toggle`]). Focus stays
    /// where the press left it, on the control for a key or a Linux click, as the old ordinary button kept it.
    fn toggle_rail(&mut self, cx: &mut Context<Self>) {
        self.rail
            .expansion
            .update(cx, |expansion, cx| expansion.toggle(cx));
        cx.notify();
    }

    /// The rail's state for this frame, worked out before the frame borrows the store for the rest of its drawing:
    /// each avatar's anchor and focus handle, which are element state and need the window ([`Kept::avatars`]). Returns
    /// the host whose menu an avatar opened, while it is still open.
    pub(super) fn prepare_rail(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<i64> {
        let ids: Vec<i64> = crate::session::current_store(cx)
            .map(|store| {
                avatar_hosts(store.read(cx).collections())
                    .map(|host| host.id)
                    .collect()
            })
            .unwrap_or_default();
        self.rail.avatars = ids
            .into_iter()
            .map(|id| (id, AvatarParts::new(id, window, cx)))
            .collect();
        self.rail
            .menu
            .filter(|(_, layer)| layers::is_open(*layer, window, cx))
            .map(|(host, _)| host)
    }

    /// Open host `host`'s actions menu beside its avatar, for a secondary click or Shift+F10 on the avatar (the module
    /// docs): through the host menu's entry point, with the avatar as the opener its closing returns focus to. A menu an
    /// avatar opened before that is still open is closed first: the layer manager closes a popover on a press outside
    /// it but not on its own anchor, so a second right press on the same icon box would otherwise stack a second menu.
    /// Nothing opens for an avatar the last frame did not draw.
    fn open_avatar_menu(&mut self, host: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(parts) = self.rail.avatars.get(&host).cloned() else {
            return;
        };
        if let Some((_, layer)) = self.rail.menu.take()
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
        }
        // Taken in the handler, after the coordinator counted this press or key, so the menu opens for this gesture and
        // not for one the user has since moved on from.
        let mark = Mark::take(cx);
        // The avatar is no menu toggle (a secondary click or Shift+F10 opens it), so the menu always starts
        // on its first command.
        self.rail.menu = crate::hosts::open_menu(
            host,
            MenuStart::First,
            parts.anchor,
            &parts.focus,
            &mark,
            window,
            cx,
        )
        .map(|layer| (host, layer));
        cx.notify();
    }
}

// ---- Host names ----------------------------------------------------------------------------------------------------

/// The host's identity as the rail's avatar and the chip name it (the old `rail_identity`), escaped as the screen shows
/// host text, for an accessible name and a tooltip: its display name ("Local (this machine)" for the local host under
/// the registry's own name), then its destination in parentheses when the host is remote and the destination differs
/// from the name. The avatar adds its state after it ([`status::state_words`]); the chip shows the identity alone.
pub(super) fn identity(host: &HostView) -> SharedString {
    let local = host.is_local();
    // Only its accessible name is used, so the sentence is never drawn and its id never reaches a frame.
    let sentence = HostSentence::new("host-identity");
    let sentence = sentence.host(&display_name(host));
    match host
        .destination
        .as_deref()
        .filter(|destination| !local && *destination != host.name)
    {
        Some(destination) => sentence
            .text(" (")
            .host(&HostText::from_host(destination))
            .text(")"),
        None => sentence,
    }
    .accessible_name()
}

/// How a chosen host the registry no longer lists is named everywhere it shows.
pub(super) fn unregistered(id: i64) -> String {
    format!("Host {id} (no longer registered)")
}

/// Whether the registry was ever read: the hosts collection arrived and either is available or kept the hosts of an
/// earlier read. A failed first read holds no hosts, and the rail then invents none (SPEC.md: "a failed first read
/// invents no hosts").
fn registry_read(collections: &Collections) -> bool {
    collections
        .meta(Collection::Hosts)
        .is_some_and(|meta| meta.availability == Availability::Available || !meta.order.is_empty())
}

/// Whether the hosts shown are the last ones read: the registry was read once, and its refresh has failed since
/// (SPEC.md: "A failed registry refresh retains the avatars and names their state as last-read").
fn last_read(collections: &Collections) -> bool {
    collections
        .meta(Collection::Hosts)
        .is_some_and(|meta| meta.availability != Availability::Available && !meta.order.is_empty())
}

/// The hosts that have an avatar, in the registry's order: none before the registry was read, and never a host a
/// session owns (see [`rail`]).
fn avatar_hosts(collections: &Collections) -> impl Iterator<Item = &HostView> {
    registry_read(collections)
        .then(|| collections.ordered::<HostView>())
        .into_iter()
        .flatten()
        .filter(|host| !host.is_session_owned)
}

/// The avatars' update badges in the current UI session's store, counted for the Hosts control in the [`foot`]; none
/// while the window is inert.
fn updates(cx: &App) -> status::Updates {
    let mut updates = status::Updates::default();
    if let Some(store) = crate::session::current_store(cx) {
        for host in avatar_hosts(store.read(cx).collections()) {
            updates.add(host_status(host, cx).update);
        }
    }
    updates
}

/// The registered host `id`, if the registry lists it.
pub(super) fn registered(collections: &Collections, id: i64) -> Option<&HostView> {
    collections.get::<HostView>(&HostView::key(id))
}

// ---- The rail ------------------------------------------------------------------------------------------------------

/// What the rail keeps across frames, which the list holds for the window's life because the rail itself is rebuilt
/// on every draw.
pub(super) struct Kept {
    /// Add host's focus handle, a tab stop: the opener the Add host dialog takes focus from and gives it back to when
    /// it closes, so it must be the same handle from the frame that opened the dialog to the one that closes it.
    add_host_focus: FocusHandle,
    /// The scrolling part's scroll state, tracked through [`focus::scroll_region`]: a fresh handle per frame would
    /// put the rail back at its top on every draw.
    scroll: ScrollHandle,
    /// Whether the rail is expanded, and the write that changes it ([`expansion`]). An entity, so its save can reach
    /// it when the reply arrives, and so the sidebar can read it with only `&App`.
    expansion: Entity<expansion::Expansion>,
    /// The Expand control's focus handle, a tab stop, kept for the window's life like Hosts' and the gear's, so that
    /// focus stays on the control while its look and label change under it.
    toggle_focus: FocusHandle,
    /// Each avatar's anchor and focus handle as the last frame drew them, by host id: what a secondary click or
    /// Shift+F10 on the avatar opens its host's menu beside and returns focus to. Rebuilt at the top of every frame
    /// ([`SessionList::prepare_rail`]), so a host no longer drawn has none.
    avatars: HashMap<i64, AvatarParts>,
    /// The host whose menu an avatar opened last, with its layer; checked with the layer manager whenever it is read,
    /// so a menu closed some other way (Escape, a press outside, a chosen command) needs no bookkeeping here.
    menu: Option<(i64, LayerId)>,
}

impl Kept {
    /// The rail's expansion, which the foot's notices bell and Help button, views of other areas, are handed when the
    /// list makes them, and read as they draw.
    pub(super) fn expansion(&self) -> &Entity<expansion::Expansion> {
        &self.expansion
    }

    /// A new rail's state: scrolled to its top, nothing focused, no menu open, showing the stored expansion.
    pub(super) fn new(cx: &mut App) -> Self {
        Kept {
            add_host_focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            expansion: cx.new(|_| expansion::Expansion::new()),
            toggle_focus: cx.focus_handle().tab_stop(true),
            avatars: HashMap::new(),
            menu: None,
        }
    }
}

/// One avatar's anchor and focus handle, element state kept for as long as the avatar is drawn in consecutive frames:
/// the anchor is the avatar's icon box, which its menu opens beside (the old sheet: "Only the inner icon box anchors
/// corner marks and host menus"), and the focus handle is the avatar's, which the menu returns focus to.
#[derive(Clone)]
struct AvatarParts {
    anchor: Anchor,
    focus: FocusHandle,
}

impl AvatarParts {
    /// Host `id`'s avatar's parts, made or found again for this frame.
    fn new(id: i64, window: &mut Window, cx: &mut App) -> Self {
        AvatarParts {
            anchor: Anchor::for_element(
                SharedString::from(format!("host-rail-menu-anchor-{id}")),
                window,
                cx,
            )
            .beside(),
            focus: focus::control_handle(
                SharedString::from(format!("host-rail-host-{id}-focus-handle")),
                window,
                cx,
            ),
        }
    }
}

/// How the rail draws this frame, beside the hosts it lists.
pub(super) struct Look {
    /// The chosen host, `None` for All.
    pub(super) chosen: Option<i64>,
    /// Whether the chosen entry wears the quieter pressed look, while the Hosts panel shows.
    pub(super) quiet: bool,
    /// Whether the rail is expanded.
    pub(super) expanded: bool,
    /// The host whose avatar opened the menu that is open now, if any.
    pub(super) menu: Option<i64>,
}

/// The rail, for the hosts in `collections`, drawn as `look` says: the chosen entry pressed, in the quieter look while
/// the Hosts panel shows (which leaves the filled Hosts control as the mark of the view on screen), at the expanded or
/// collapsed width, and the avatar whose menu is open in the open look. Its entries choose through `list`, and All
/// tracks `all_focus`, the list's handle that removing the filter focuses. `kept` is the rail's state across frames
/// ([`Kept`]), its avatars' parts made for this frame already ([`SessionList::prepare_rail`]). Each avatar reads its
/// host's status, and the rail its palette, through `cx`.
///
/// It scrolls on its own when its entries overflow it, beside the list, which scrolls separately (`overflow: hidden
/// auto`); its padding and gaps are inside the scrolling element, so a wheel over them scrolls the rail. The scrolling
/// part stops [`FOOT_HEIGHT`] above the sidebar's bottom (the old rail's `bottom: 208px`), leaving room the list draws
/// the [`foot`] over, so no entry ever scrolls under the foot. It is a scroll region of the focus coordinator, so the
/// keyboard reaching an entry outside its visible part scrolls that entry into view.
pub(super) fn rail(
    collections: &Collections,
    list: &WeakEntity<SessionList>,
    all_focus: &FocusHandle,
    kept: &Kept,
    look: Look,
    cx: &App,
) -> impl IntoElement {
    let palette = theme::palette(cx);
    let Look {
        chosen,
        quiet,
        expanded,
        menu,
    } = look;
    let choose = |host: Option<i64>| {
        let list = list.clone();
        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
            let _ = list.update(cx, |list, cx| list.choose_host(host, cx));
        }
    };
    let all_chosen = chosen.is_none();
    // The old All showed its word in the icon box and "All hosts" beside it once expanded.
    let all_names = expanded.then(|| names(div().child("All hosts"), None, all_chosen, palette));
    let mut entries: Vec<AnyElement> = vec![
        Button::new(
            "host-rail-all",
            "All hosts",
            HoverHelp::tooltip("All hosts: show sessions from every host"),
            Role::Button,
        )
        .tier(Tier::Rail)
        .pressed(all_chosen)
        .quiet(quiet)
        .focus(all_focus)
        .when(expanded, Button::rail_row)
        .content(entry(div().child("All"), all_names))
        .on_click(choose(None))
        .into_any_element(),
    ];
    if registry_read(collections) {
        let last_read = last_read(collections);
        for host in avatar_hosts(collections) {
            let Some(parts) = kept.avatars.get(&host.id) else {
                // Made for every avatar host at the top of the frame, from the same store; never missing.
                continue;
            };
            entries.push(avatar(
                host,
                &host_status(host, cx),
                AvatarLook {
                    last_read,
                    chosen: chosen == Some(host.id),
                    quiet,
                    expanded,
                    menu_open: menu == Some(host.id),
                },
                parts,
                list,
                choose(Some(host.id)),
                palette,
            ));
        }
        if let Some(missing) = chosen.filter(|id| registered(collections, *id).is_none()) {
            // Pressed and inert: it keeps the removed host visibly chosen until the user picks All or another host.
            entries.push(
                Button::new(
                    SharedString::from(format!("host-rail-removed-{missing}")),
                    unregistered(missing),
                    HoverHelp::tooltip(format!(
                        "Host {missing} is no longer registered; choose All or another host"
                    )),
                    Role::Button,
                )
                .tier(Tier::Rail)
                .pressed(true)
                .quiet(quiet)
                .disabled(true)
                .when(expanded, Button::rail_row)
                .content(rail_entry(div().child("?"), None))
                .into_any_element(),
            );
        }
    } else {
        entries.push(unavailable(palette));
    }
    // Last, whatever the registry holds or failed to: adding a host needs no host list, and the old rail drew it even
    // while the list could not be read.
    entries.push(add_host(&kept.add_host_focus, expanded, palette));
    let scrolling = div()
        .id("host-rail")
        // Tests find the rail by this; it records nothing outside test builds.
        .debug_selector(|| "host-rail".into())
        .role(Role::Navigation)
        .aria_label("Hosts")
        .w_full()
        .flex_1()
        .min_h_0()
        .border_r_1()
        .border_color(theme::color(palette.border_row))
        .bg(theme::color(palette.bg_1))
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(RAIL_GAP))
                .py(px(RAIL_PAD.0))
                .px(px(RAIL_PAD.1))
                .children(entries),
        );
    div()
        .w(expansion::width(expanded))
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .child(focus::scroll_region(scrolling, &kept.scroll))
        // The foot's room: empty here, since the list draws the foot over it after its own column (see the module
        // docs for why the foot is not this element's child).
        .child(div().flex_none().h(px(FOOT_HEIGHT)))
}

/// What an entry shows: `glyph` in the rail's icon box, then, in the expanded rail, `names` beside it at the old
/// `.host-rail-expanded`'s place, 48 pixels into the button. The icon box ends a pixel short of its 40 pixels (its -1
/// margin), so the names sit 10 pixels after it.
fn entry(glyph: impl IntoElement, names: Option<AnyElement>) -> AnyElement {
    match names {
        None => rail_entry(glyph, None),
        Some(names) => div()
            .flex()
            .items_center()
            .min_w_0()
            .child(rail_entry(glyph, None))
            .child(div().ml(px(NAMES_LEFT - (RAIL_ICON_BOX - 2.))).child(names))
            .into_any_element(),
    }
}

/// An expanded entry's names (`.host-rail-expanded`): `name` over `destination`, if any, each on one line cut short with
/// an ellipsis, in the rail's width less 72 pixels. In the primary ink with the destination in the quiet one, except on a
/// `pressed` entry, where both take the button's own ink (the old `color: inherit`), the inverse on the accent fill and
/// the primary ink in the quieter look. No role: the button's name already says what they show.
fn names(
    name: gpui::Div,
    destination: Option<gpui::Div>,
    pressed: bool,
    palette: &Palette,
) -> AnyElement {
    let line = |text: gpui::Div, size: f32, height: f32| {
        text.w_full()
            .text_size(px(size))
            .line_height(px(height))
            .text_left()
            .truncate()
    };
    div()
        .w(expansion::width(true) - px(NAMES_INSET))
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(NAMES_GAP))
        .when(!pressed, |names| {
            names.text_color(theme::color(palette.fg_0))
        })
        .child(line(name, NAME_SIZE, NAME_LINE))
        .when_some(destination, |names, destination| {
            names.child(
                line(destination, DESTINATION_SIZE, DESTINATION_LINE)
                    .when(!pressed, |line| line.text_color(theme::color(palette.fg_1))),
            )
        })
        .into_any_element()
}

/// How one avatar draws, besides its host.
struct AvatarLook {
    /// The registry's refresh failed, so the avatar names its state as last read.
    last_read: bool,
    /// The host is the chosen one: the avatar is pressed.
    chosen: bool,
    /// The quieter pressed look, while the Hosts panel shows.
    quiet: bool,
    /// The rail is expanded: the avatar is a row with the host's names in place of its caption.
    expanded: bool,
    /// The avatar opened the host menu that is open now: it wears the open look.
    menu_open: bool,
}

/// One host's avatar: its mark in a rail button with the host's status (`reading`) drawn over it
/// ([`status::icon_box`]), named by its identity and its state's words, with its short name as a caption under it, or
/// expanded, its display name and destination beside the mark; `look` says how it draws. `parts` are its anchor and
/// focus handle, the menu's place and opener, and `list` opens the menu on a secondary click or Shift+F10 (the module
/// docs).
fn avatar(
    host: &HostView,
    reading: &HostStatus,
    look: AvatarLook,
    parts: &AvatarParts,
    list: &WeakEntity<SessionList>,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    palette: &Palette,
) -> AnyElement {
    let id = host.id;
    let name = SharedString::from(format!(
        "{}: {}",
        identity(host),
        status::state_words(host, reading, look.last_read)
    ));
    let mark = host_mark(host.icon, host.color, px(AVATAR_MARK), palette);
    let icon_box = status::icon_box(
        mark.into_any_element(),
        host,
        reading,
        look.chosen,
        look.quiet,
        parts.anchor.track().into_any_element(),
        palette,
    );
    // The old `.host-rail-expanded`: the display name, and under it the destination when it is not the name already.
    let expanded_names = look.expanded.then(|| {
        let destination = host
            .destination
            .as_deref()
            .filter(|destination| *destination != host.name)
            .map(|destination| host_text::element(&HostText::from_host(destination)));
        names(
            host_text::element(&display_name(host)),
            destination,
            look.chosen,
            palette,
        )
    });
    let (secondary, keyed) = (list.clone(), list.clone());
    let button = Button::new(
        SharedString::from(format!("host-rail-host-{id}")),
        name.clone(),
        HoverHelp::tooltip(name),
        Role::Button,
    )
    .tier(Tier::Rail)
    .pressed(look.chosen)
    .quiet(look.quiet)
    .expanded(look.menu_open)
    .focus(&parts.focus)
    .when(look.expanded, Button::rail_row)
    .content(entry(icon_box, expanded_names))
    .on_click(on_click)
    .on_secondary_click(move |_, window, cx| {
        let _ = secondary.update(cx, |list, cx| list.open_avatar_menu(id, window, cx));
    })
    // Shift+F10, the keyboard's way to a context menu, as the old avatar took it. Any other modifier is some other
    // chord's, and a held key does not open it again.
    .on_key_down(move |event, window, cx| {
        let keystroke = &event.keystroke;
        let modifiers = &keystroke.modifiers;
        if keystroke.key == "f10"
            && modifiers.shift
            && !(modifiers.control || modifiers.alt || modifiers.platform || modifiers.function)
            && !event.is_held
        {
            cx.stop_propagation();
            let _ = keyed.update(cx, |list, cx| list.open_avatar_menu(id, window, cx));
        }
    });
    let caption = HostText::from_host(short_name(host));
    div()
        .flex()
        .flex_col()
        .flex_none()
        .when(look.expanded, |entry| entry.w_full())
        .when(!look.expanded, |entry| entry.w(px(CAPTION_WIDTH)))
        .child(button)
        // No role, so no node for assistive technology: the avatar's name already says what the caption shows. The
        // expanded rail shows the names beside the mark in its place.
        .when(!look.expanded, |entry| {
            entry.child(
                host_text::element(&caption)
                    .id(SharedString::from(format!("host-rail-caption-{id}")))
                    .w(px(CAPTION_WIDTH))
                    .mt(px(CAPTION_GAP))
                    .text_size(px(CAPTION_SIZE))
                    .line_height(px(CAPTION_LINE))
                    .text_center()
                    .text_color(theme::color(palette.fg_1))
                    .truncate(),
            )
        })
        .into_any_element()
}

/// The rail's "?" while the host list has never been read (`.host-rail-unavailable`): a status, not a control, in the
/// amber of a warning, centered under the buttons, with the same words as its name and its hover help. It keeps its
/// place in the expanded rail, which the old rail gave it no label for.
fn unavailable(palette: &Palette) -> AnyElement {
    const WORDS: &str = "The host list could not be loaded";
    div()
        .w(px(CAPTION_WIDTH))
        .flex()
        .justify_center()
        .child(
            StatusMark::new(
                "host-rail-unavailable",
                WORDS,
                HoverHelp::tooltip(WORDS),
                Role::Status,
                "?",
            )
            .ink(palette.warn),
        )
        .into_any_element()
}

/// The rail's Add host (`.host-rail-add`): the old Add glyph, a plus, in a rail button, "Add host" beside it in the
/// `expanded` rail, which opens the Add host dialog with `handle`, its kept focus handle, as the opener. Never pressed,
/// so always the rail's quiet ink; the module docs say why it is never disabled.
fn add_host(handle: &FocusHandle, expanded: bool, palette: &Palette) -> AnyElement {
    let opener = handle.clone();
    let glyph = svg()
        .path(Icon::Add.path())
        .size(px(ACTION_GLYPH))
        .flex_none()
        .text_color(theme::color(palette.fg_1));
    let names = expanded.then(|| names(div().child("Add host"), None, false, palette));
    Button::new(
        "host-rail-add",
        "Add host",
        HoverHelp::tooltip(crate::hosts::add::ADD_HOST_HELP),
        Role::Button,
    )
    .tier(Tier::Rail)
    .focus(handle)
    .when(expanded, Button::rail_row)
    .content(entry(glyph, names))
    .on_click(move |_, window, cx| {
        // Taken in the handler, after the coordinator counted this press or key, so the dialog opens for this press and
        // not for one the user has since moved on from.
        let mark = Mark::take(cx);
        crate::hosts::add::open(&opener, &mark, window, cx);
    })
    .into_any_element()
}

/// The foot's controls the list keeps for the window's life: Hosts' and the gear's focus handles (Hosts keeps focus
/// when it toggles, and Settings gives focus back to the gear when it closes), and the notices bell and the `?` Help
/// button, views of their own areas.
pub(super) struct FootParts<'a> {
    pub(super) hosts_focus: &'a FocusHandle,
    pub(super) settings_focus: &'a FocusHandle,
    pub(super) notices: &'a Entity<NoticesButton>,
    pub(super) help: &'a Entity<HelpButton>,
}

/// The rail's foot (`.host-rail-footer`), for the list to draw after its own column, as its last child, over the room
/// [`rail`] leaves at its bottom: the notices bell, Hosts, the Settings gear, the `?` button and the Expand control
/// ([`FootParts`] and `kept`'s), in the old foot's order, pinned there while the rail's entries scroll above them, at
/// the rail's width and, when `expanded`, each a row with its label. Hosts is pressed and expanded while `hosts_shown`,
/// toggles the Hosts panel through `list`, and carries the avatars' update badges ([`updates`], read through `cx` like
/// the palette) as its `↑` and in its name and help. The Expand control toggles the rail's expansion through `list`.
///
/// The element places itself (absolutely, at the bottom left of the row the list draws it in), so that row must be
/// the one that holds the rail at its left edge and fills the sidebar's height.
pub(super) fn foot(
    list: &WeakEntity<SessionList>,
    parts: FootParts,
    kept: &Kept,
    hosts_shown: bool,
    expanded: bool,
    cx: &App,
) -> impl IntoElement {
    let palette = theme::palette(cx);
    let updates = updates(cx);
    let (hosts_list, toggle_list) = (list.clone(), list.clone());
    let gear = parts.settings_focus.clone();
    let label = |text: &'static str| expanded.then(|| text.into_any_element());
    div()
        .absolute()
        .left_0()
        .bottom_0()
        .w(expansion::width(expanded))
        .h(px(FOOT_HEIGHT))
        .flex()
        .flex_col()
        .items_start()
        .pb(px(FOOT_PAD_BOTTOM))
        .px(px(FOOT_PAD_X))
        .border_r_1()
        .border_color(theme::color(palette.border_row))
        .bg(theme::color(palette.bg_1))
        // The notices bell first, above Hosts (SPEC.md: "at the top of the navigation rail's footer, above Hosts").
        .child(parts.notices.clone())
        // The glyph at the old `ActionIcon`'s 14 pixels in the button's ink, which its pressed look turns to the
        // inverse. SPEC.md: Hosts "exposes its pressed and expanded state". GPUI's driver record has no expanded field,
        // so no in-process test reads the expanded state back; the pressed state is tested, and the button's own
        // `aria-expanded` wiring is the one every expanded button shares (the row menu's ⋯ among them).
        .child(
            Button::new(
                "hosts-panel-toggle",
                status::hosts_name(updates),
                HoverHelp::tooltip(status::hosts_help(updates)),
                Role::Button,
            )
            .tier(Tier::Rail)
            .when_some(status::hosts_badge(updates, palette), |button, badge| {
                button.badge(badge)
            })
            .pressed(hosts_shown)
            .expanded(hosts_shown)
            .focus(parts.hosts_focus)
            .when(expanded, Button::rail_row)
            .content(rail_entry(
                svg()
                    .path(Icon::Hosts.path())
                    .size(px(ACTION_GLYPH))
                    .flex_none()
                    .text_color(theme::color(if hosts_shown {
                        palette.on_fill
                    } else {
                        palette.fg_1
                    })),
                label("Hosts"),
            ))
            .on_click(move |_, window, cx| {
                // Taken in the handler, after the coordinator counted this press or key.
                let mark = Mark::take(cx);
                let _ = hosts_list.update(cx, |list, cx| list.toggle_hosts(&mark, window, cx));
            }),
        )
        .child(
            Button::new(
                "app-settings-toggle",
                "Settings",
                HoverHelp::tooltip(SETTINGS_HELP),
                Role::Button,
            )
            .tier(Tier::Rail)
            .focus(parts.settings_focus)
            .when(expanded, Button::rail_row)
            // Never pressed, so always the rail's quiet ink.
            .content(rail_entry(
                svg()
                    .path(Icon::Settings.path())
                    .size(px(SETTINGS_GLYPH))
                    .flex_none()
                    .text_color(theme::color(palette.fg_1)),
                label("Settings"),
            ))
            .on_click(move |_, window, cx| {
                // Taken in the handler, after the coordinator counted this press or key, so Settings opens for
                // this press and not for one the user has since moved on from.
                let mark = Mark::take(cx);
                settings::open(&gear, &mark, window, cx);
            }),
        )
        .child(parts.help.clone())
        // Last, below Help, as the old foot placed it. It widens the rail in place rather than opening anything, so it
        // reports its state without the open look (the module docs, "Expansion").
        .child({
            let name = if expanded { COLLAPSE_NAME } else { EXPAND_NAME };
            Button::new(TOGGLE_ID, name, HoverHelp::tooltip(name), Role::Button)
                .tier(Tier::Rail)
                .discloses(expanded)
                .focus(&kept.toggle_focus)
                .when(expanded, Button::rail_row)
                .content(rail_entry(
                    div().child(if expanded { "«" } else { "»" }),
                    label(COLLAPSE_LABEL),
                ))
                .on_click(move |_, _, cx| {
                    let _ = toggle_list.update(cx, |list, cx| list.toggle_rail(cx));
                })
        })
}
