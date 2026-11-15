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
//! refusing belongs to the dialog's own Add, each side refusing its own action while the other's work is in flight
//! (decided by the migration's orchestrator on 2026-11-14). That Add does not refuse during a launch yet (interim, in
//! the dialog's module docs), so until it does a host can be added while a session launches, which the old app did not
//! allow.
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
//! The rail's foot ([`foot`]) is pinned under its entries, which scroll above it, and holds Hosts above the Settings
//! gear, in the old foot's order. Hosts swaps the list's column for the Hosts panel ([`crate::hosts::HostsPanel`]) and
//! back, keeping focus on itself; choosing All or a host returns to Sessions (SPEC.md: "All or a host avatar returns
//! to Sessions"), and while the panel shows, the rail's chosen entry wears a quieter look so the filled Hosts control
//! marks the view on screen. Whether the panel shows belongs to the UI session, like the choice. The gear opens the
//! Settings dialog ([`crate::settings::open`]) with itself as the opener, so that Escape and either close control bring
//! focus back to it (SPEC.md: "Escape and either close control return focus to the gear").
//!
//! The foot is drawn by the list after its own column rather than inside the rail, as the old app placed its
//! `.host-rail-footer` after the sidebar's content: GPUI's Tab order follows the order controls are drawn in, and the
//! old app kept "host choices before Settings" with the list's controls between them, so Tab goes from the rail
//! through the list (or the Hosts panel) to the foot. The rail leaves the foot's height free at its bottom
//! ([`FOOT_HEIGHT`]), and the foot is drawn over that room.
//!
//! NOTE: Not drawn yet, each waiting for its area: the rest of the rail's foot (the notices bell, Help and the expand
//! toggle) for the help and notices areas; an avatar's right-click menu; every host status (connection dots, update
//! badges, progress rings, dimming and the state words that join the avatar's name, and the update count that joins
//! Hosts' name); and the rail's expansion to 220 pixels, with Add host's "Add host" label. The foot is sized for the
//! two controls it holds: the old foot was 208 pixels, five 40-pixel controls, and it grows back as the others arrive.

#[cfg(test)]
pub(crate) mod tests;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{Availability, Collection, HostView};
use gpui::{
    AnyElement, App, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Pixels,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, div, px,
    svg,
};

use super::SessionList;
use super::row::short_name;
use crate::assets::Icon;
use crate::components::{Button, HoverHelp, IconButton, Role, StatusMark, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::identity::{display_name, host_mark};
use crate::settings;
use crate::theme::{self, Palette};

/// `--host-rail-width` while the rail is collapsed, its only state so far: the rail's whole width, its right edge
/// included. The sidebar's width holds it, so the session list beside it is this much narrower.
const RAIL_WIDTH: Pixels = px(56.);

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

/// The foot's height while it holds Hosts and the gear: two controls and the bottom padding. The old foot's 208 pixels
/// held five controls; it grows by 40 pixels with each control that joins it.
pub(super) const FOOT_HEIGHT: f32 = 2. * RAIL_BUTTON + FOOT_PAD_BOTTOM;

/// `SettingsIcon`'s size in the old foot's icon box.
const SETTINGS_GLYPH: f32 = 16.;

/// Hosts' hover help, the old app's words while no host has an update (the update count that joins its name and help
/// comes with host status).
const HOSTS_HELP: &str = "Hosts: show host management";

/// The gear's hover help, the old app's words. The old app added ", and this app's automatic updates" when the desktop
/// app's updater was active; this app has no updater yet, so the words stop where the old app's did without one.
const SETTINGS_HELP: &str = "Settings: choices that apply to every host on this helm";

/// Add host's hover help, the old app's words, kept although the dialog it opens offers SSH alone for now (the
/// dialog's module docs record the missing Tensorlake choice).
const ADD_HOST_HELP: &str = "Add host: connect a machine or create a Tensorlake sandbox";

impl SessionList {
    /// The user pressed Hosts: show the Hosts panel in place of the list's column, or Sessions again if it shows. The
    /// query, the sort, the open session and the panel's own Details choice are untouched (SPEC.md: "opening or closing
    /// Hosts keeps the current query, sort, selected session and details choice"). Focus stays on Hosts (SPEC.md: "The
    /// control keeps focus"), also where a click focuses no button (macOS), as the old app focused it explicitly.
    /// `mark` is the one taken in the control's handler.
    fn toggle_hosts(&mut self, mark: &Mark, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.current.current(cx);
        current.hosts_shown = !current.hosts_shown;
        cx.notify();
        focus::focus_control(&self.hosts_focus, mark, window, cx);
    }

    /// Show the Hosts panel, as pressing Hosts does, without a press or a focus move: for a golden scene, which has no
    /// focus handle of the foot to hand a mark to.
    #[cfg(test)]
    pub(crate) fn show_hosts(&mut self, cx: &mut Context<Self>) {
        self.current.current(cx).hosts_shown = true;
        cx.notify();
    }
}

// ---- Host names ----------------------------------------------------------------------------------------------------

/// The host's identity as the rail's avatar and the chip name it (the old `rail_identity`), escaped as the screen shows
/// host text, for an accessible name and a tooltip: its display name ("Local (this machine)" for the local host under
/// the registry's own name), then its destination in parentheses when the host is remote and the destination differs
/// from the name. The state that SPEC.md also wants in the avatar's name joins it with the host status.
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
}

impl Kept {
    /// A new rail's state: scrolled to its top, nothing focused.
    pub(super) fn new(cx: &mut App) -> Self {
        Kept {
            add_host_focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
        }
    }
}

/// The rail, for the hosts in `collections` with `chosen` pressed; its entries choose through `list`, and All tracks
/// `all_focus`, the list's handle that removing the filter focuses. While `quiet` (the Hosts panel shows), the chosen
/// entry wears the quieter pressed look, which leaves the filled Hosts control as the mark of the view on screen.
/// `kept` is the rail's state across frames ([`Kept`]).
///
/// It scrolls on its own when its entries overflow it, beside the list, which scrolls separately (`overflow: hidden
/// auto`); its padding and gaps are inside the scrolling element, so a wheel over them scrolls the rail. The scrolling
/// part stops [`FOOT_HEIGHT`] above the sidebar's bottom (the old rail's `bottom: 208px`), leaving room the list draws
/// the [`foot`] over, so no entry ever scrolls under the foot. It is a scroll region of the focus coordinator, so the
/// keyboard reaching an entry outside its visible part scrolls that entry into view.
pub(super) fn rail(
    collections: &Collections,
    chosen: Option<i64>,
    list: &WeakEntity<SessionList>,
    all_focus: &FocusHandle,
    kept: &Kept,
    quiet: bool,
    palette: &Palette,
) -> impl IntoElement {
    let choose = |host: Option<i64>| {
        let list = list.clone();
        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
            let _ = list.update(cx, |list, cx| list.choose_host(host, cx));
        }
    };
    let mut entries: Vec<AnyElement> = vec![
        Button::new(
            "host-rail-all",
            "All hosts",
            HoverHelp::tooltip("All hosts: show sessions from every host"),
            Role::Button,
        )
        .tier(Tier::Rail)
        .pressed(chosen.is_none())
        .quiet(quiet)
        .focus(all_focus)
        .content(div().child("All"))
        .on_click(choose(None))
        .into_any_element(),
    ];
    if registry_read(collections) {
        for host in collections.ordered::<HostView>() {
            if host.is_session_owned {
                continue;
            }
            entries.push(avatar(
                host,
                chosen == Some(host.id),
                quiet,
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
                .content(div().child("?"))
                .into_any_element(),
            );
        }
    } else {
        entries.push(unavailable(palette));
    }
    // Last, whatever the registry holds or failed to: adding a host needs no host list, and the old rail drew it even
    // while the list could not be read.
    entries.push(add_host(&kept.add_host_focus));
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
        .w(RAIL_WIDTH)
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .child(focus::scroll_region(scrolling, &kept.scroll))
        // The foot's room: empty here, since the list draws the foot over it after its own column (see the module
        // docs for why the foot is not this element's child).
        .child(div().flex_none().h(px(FOOT_HEIGHT)))
}

/// One host's avatar: its mark in a rail button named by its identity, with its short name as a caption under it;
/// `quiet` as for [`rail`].
fn avatar(
    host: &HostView,
    chosen: bool,
    quiet: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    palette: &Palette,
) -> AnyElement {
    let id = host.id;
    let name = identity(host);
    let caption = HostText::from_host(short_name(host));
    div()
        .flex()
        .flex_col()
        .flex_none()
        .w(px(CAPTION_WIDTH))
        .child(
            Button::new(
                SharedString::from(format!("host-rail-host-{id}")),
                name.clone(),
                HoverHelp::tooltip(name),
                Role::Button,
            )
            .tier(Tier::Rail)
            .pressed(chosen)
            .quiet(quiet)
            .content(host_mark(host.icon, host.color, px(AVATAR_MARK), palette))
            .on_click(on_click),
        )
        // No role, so no node for assistive technology: the avatar's name already says what the caption shows.
        .child(
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
        .into_any_element()
}

/// The rail's "?" while the host list has never been read (`.host-rail-unavailable`): a status, not a control, in the
/// amber of a warning, centered under the buttons, with the same words as its name and its hover help.
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

/// The rail's Add host (`.host-rail-add`): the old Add glyph, a plus, in a rail button, which opens the Add host dialog
/// with `handle`, its kept focus handle, as the opener. Never pressed, so always the rail's quiet ink; the module docs
/// say why it is never disabled.
fn add_host(handle: &FocusHandle) -> AnyElement {
    let opener = handle.clone();
    IconButton::new(
        "host-rail-add",
        Icon::Add,
        "Add host",
        HoverHelp::tooltip(ADD_HOST_HELP),
        Role::Button,
    )
    .tier(Tier::Rail)
    .focus(handle)
    .on_click(move |_, window, cx| {
        // Taken in the handler, after the coordinator counted this press or key, so the dialog opens for this press and
        // not for one the user has since moved on from.
        let mark = Mark::take(cx);
        crate::hosts::add::open(&opener, &mark, window, cx);
    })
    .into_any_element()
}

/// The rail's foot (`.host-rail-footer`), for the list to draw after its own column, as its last child, over the room
/// [`rail`] leaves at its bottom: Hosts above the Settings gear, in the old foot's order, pinned there while the rail's
/// entries scroll above them. `hosts_focus` and `settings_focus` are their focus handles, which the list keeps for the
/// window's life: Hosts keeps focus when it toggles, and Settings gives focus back to the gear when it closes. Hosts is
/// pressed and expanded while `hosts_shown`, and toggles the Hosts panel through `list`.
///
/// The element places itself (absolutely, at the bottom left of the row the list draws it in), so that row must be
/// the one that holds the rail at its left edge and fills the sidebar's height.
pub(super) fn foot(
    list: &WeakEntity<SessionList>,
    hosts_focus: &FocusHandle,
    hosts_shown: bool,
    settings_focus: &FocusHandle,
    palette: &Palette,
) -> impl IntoElement {
    let list = list.clone();
    let gear = settings_focus.clone();
    div()
        .absolute()
        .left_0()
        .bottom_0()
        .w(RAIL_WIDTH)
        .h(px(FOOT_HEIGHT))
        .flex()
        .flex_col()
        .items_start()
        .pb(px(FOOT_PAD_BOTTOM))
        .px(px(FOOT_PAD_X))
        .border_r_1()
        .border_color(theme::color(palette.border_row))
        .bg(theme::color(palette.bg_1))
        // An icon button draws the glyph at the old `ActionIcon`'s 14 pixels in the button's own ink, which its pressed
        // look turns to the inverse. SPEC.md: Hosts "exposes its pressed and expanded state". GPUI's driver record has
        // no expanded field, so no in-process test reads the expanded state back; the pressed state is tested, and the
        // button's own `aria-expanded` wiring is the one every expanded button shares (the row menu's ⋯ among them).
        .child(
            IconButton::new(
                "hosts-panel-toggle",
                Icon::Hosts,
                "Hosts",
                HoverHelp::tooltip(HOSTS_HELP),
                Role::Button,
            )
            .tier(Tier::Rail)
            .pressed(hosts_shown)
            .expanded(hosts_shown)
            .focus(hosts_focus)
            .on_click(move |_, window, cx| {
                // Taken in the handler, after the coordinator counted this press or key.
                let mark = Mark::take(cx);
                let _ = list.update(cx, |list, cx| list.toggle_hosts(&mark, window, cx));
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
            .focus(settings_focus)
            // Never pressed, so always the rail's quiet ink.
            .content(
                svg()
                    .path(Icon::Settings.path())
                    .size(px(SETTINGS_GLYPH))
                    .flex_none()
                    .text_color(theme::color(palette.fg_1)),
            )
            .on_click(move |_, window, cx| {
                // Taken in the handler, after the coordinator counted this press or key, so Settings opens for
                // this press and not for one the user has since moved on from.
                let mark = Mark::take(cx);
                settings::open(&gear, &mark, window, cx);
            }),
        )
}
