//! The host rail: the column of hosts at the sidebar's left edge that narrows the session list to one host, and the
//! two places the list says which host it is narrowed to (the removable chip under its heading, and the empty state of
//! a host with no sessions).
//!
//! SPEC.md, "Session list" (the rail, the host chip, the empty filtered list), ported from the old app's `hosts.rs`
//! (`HostRailAvatar`, the rail's entries) and `list/view.rs` (`choose_host`, `FilterHostLabel`, the empty result) and
//! the stylesheet's `.host-rail`, `.host-filter-chip` and `.filter-empty` rules at the reference commit.
//!
//! The rail lists, top to bottom: **All**, one avatar per registered host in the registry's order (a host a session
//! owns, a sandbox or container made for one session, has no avatar of its own: its sessions appear under All), a "?"
//! placeholder for a chosen host the registry no longer lists, and a "?" status while the host list has never been
//! read. Each avatar is the host's chosen mark in its chosen color with its short name as a caption under it; the
//! caption repeats what the avatar's accessible name already says, so it has no node of its own for assistive
//! technology. Names, destinations and aliases are the host's own text, drawn and named through the host-text
//! element.
//!
//! Choosing is a query, not a filter the window applies: a press (or Space or Enter) on All or an avatar sends the UI
//! channel's session query (`PUT /ui/session-query`, `{"host": <id>}`, or `{}` for All), and the helm then serves only
//! that host's sessions with their counts. The choice is not persisted, and a choice equal to the current one sends
//! nothing. Status, sort and the open session stay as they are: the query names no sort, so the channel keeps following
//! the stored order, and hiding the open session changes neither the selection nor its terminal (the selection rules
//! never treat a filtered listing as proof that a session is gone).
//!
//! What the window shows follows two different moments, as in the old app ([`Choice`]): the rail's pressed entry and
//! the chip follow the user's choice at once, while the empty state names the host of the query the helm has
//! answered, so a held reply leaves the sentence describing the listing still on screen. A refused query puts the
//! previous choice back and shows the helm's words under the heading until the next choice.
//!
//! The choice belongs to the UI session ([`super::Current`]): a reset forgets it, as the helm's new channel starts
//! unfiltered (SPEC_impl.md, "One failure path: the reset").
//!
//! Removing the filter from the chip or the empty state's "Show all hosts" takes away the control the user just used,
//! so focus moves to the rail's All, as in the old app (SPEC.md: "After a clear button or the chip removes its focused
//! control, focus returns to the rail's All button"), through the focus coordinator's move for a user's action, which
//! does nothing once the user has moved on or while a layer is open. All's focus handle is the list's for that reason.
//!
//! NOTE: Not drawn yet, each waiting for its area: Add host and the rail's foot (Hosts, Settings, Help, the notices
//! bell and the expand toggle) for the settings-and-hosts, help and notices areas; an avatar's right-click menu; every
//! host status (connection dots, update badges, progress rings, dimming and the state words that join the avatar's
//! name); the rail's expansion to 220 pixels; and the status segments that share the chip's row. A keyboard focus move
//! onto an avatar below the rail's fold does not scroll it into view (the rail holds about seven hosts at the minimum
//! window height).

#[cfg(test)]
pub(crate) mod tests;

use farhelm_helm_client::{ApiPath, Collections, CommandError, Method};
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionMeta, HostView, Reply, SessionQuery,
};
use gpui::{
    AnyElement, App, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Pixels,
    SharedString, StatefulInteractiveElement, Styled, Svg, WeakEntity, Window, div, px, svg,
};

use super::SessionList;
use super::row::short_name;
use crate::assets::Icon;
use crate::components::{Button, HoverHelp, Role, StatusMark, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::session;
use crate::theme::{self, Palette, Token, metrics};

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

/// `.host-filter-chip .host-kind-icon`: the mark's size in the chip and in the empty state's sentence.
const CHIP_MARK: f32 = 14.;

/// `.host-filter-chip`'s gap between its mark, the host's name and the ✕.
const CHIP_GAP: f32 = 5.;

/// `.session-status-filter`'s padding (vertical then horizontal) and gap: the row under the heading that holds the chip.
const CHIP_ROW_PAD: (f32, f32) = (6., 8.);
const CHIP_ROW_GAP: f32 = 6.;

/// The empty state sentence's line height: fixed, so the mark can be centered on the first line of a wrapped name.
const EMPTY_LINE: f32 = 20.;

/// `.filter-empty-actions`'s gaps, vertical then horizontal, between the links that clear a filter.
const EMPTY_ACTIONS_GAP: (f32, f32) = (8., 12.);

/// The name the registry gives the local host when the user gave it no alias. Only this exact name is shown as
/// "Local (this machine)": an alias stays the user's word, and a remote host that happens to carry this name is not
/// the local one.
const LOCAL_NAME: &str = "this machine";

/// What the rail's choice is in one UI session: the host the user chose and the host of the query the helm last
/// accepted, which differ while a query is in flight, and the helm's words for a refused one.
///
/// Replies arrive in the order the queries were sent (the channel admits one command at a time), and each query
/// replaces the whole previous one on the helm's side, so the helm serves the last query it accepted: that is
/// `answered`. A refusal of the latest query puts the choice back to `answered`; a refusal of a query a later one
/// already superseded changes nothing, since the user has moved on from it and the later one is still on its way.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Choice {
    /// The host the user chose, `None` for All: what the rail presses and the chip names.
    chosen: Option<i64>,
    /// The host of the query the helm last accepted, `None` for All (as a new channel starts): what the listing on
    /// screen answers, and so what its empty state names.
    ///
    /// NOTE: It moves when the reply arrives, which the helm sends right after the listing it pushed for the query.
    /// Between the two, possibly for one frame, a new empty listing can be drawn naming the previous host. The
    /// listing's metadata does not say which query it answers, so nothing closer is known.
    answered: Option<i64>,
    /// How many queries this UI session sent; the last one's number.
    sent: u64,
    /// The helm's words for the latest query, when it refused it.
    refusal: Option<HostText>,
}

impl Choice {
    /// The host the user chose, `None` for All.
    pub(super) fn chosen(&self) -> Option<i64> {
        self.chosen
    }

    /// The host of the query the helm last accepted, `None` for All.
    pub(super) fn answered(&self) -> Option<i64> {
        self.answered
    }

    /// The helm's words for the latest query, if it refused it.
    pub(super) fn refusal(&self) -> Option<&HostText> {
        self.refusal.as_ref()
    }

    /// The user chose `host`: take it as the choice and return the number to send its query under, or `None` when it
    /// is the choice already, which sends nothing. A new choice drops the last refusal's words.
    pub(super) fn choose(&mut self, host: Option<i64>) -> Option<u64> {
        if self.chosen == host {
            return None;
        }
        self.chosen = host;
        self.refusal = None;
        self.sent += 1;
        Some(self.sent)
    }

    /// The helm answered query number `query`, which asked for `host`: accepted (`Ok`), or refused in its words.
    pub(super) fn settle(&mut self, query: u64, host: Option<i64>, answer: Result<(), HostText>) {
        match answer {
            Ok(()) => self.answered = host,
            Err(words) if query == self.sent => {
                self.chosen = self.answered;
                self.refusal = Some(words);
            }
            Err(_) => {}
        }
    }
}

impl SessionList {
    /// The user chose `host` (`None` for All) on the rail, the chip or "Show all hosts": press it at once and send the
    /// channel's session query for it, unless it is the choice already. The reply settles the choice ([`Choice`]).
    /// Nothing happens while the window is inert.
    pub(crate) fn choose_host(&mut self, host: Option<i64>, cx: &mut Context<Self>) {
        let Some(ui) = session::connection(cx).and_then(|c| c.read(cx).session().cloned()) else {
            return;
        };
        // Encoded before the choice is taken: a query that could not be sent must not leave a host pressed that no
        // reply will ever settle.
        let body = match serde_json::to_value(SessionQuery {
            host,
            ..SessionQuery::default()
        }) {
            Ok(body) => body,
            Err(error) => {
                tracing::warn!(%error, "could not encode the session query");
                return;
            }
        };
        let Some(query) = self.current.current(cx).hosts.choose(host) else {
            return;
        };
        cx.notify();
        let list = cx.weak_entity();
        ui.update(cx, |ui, cx| {
            ui.command(
                Method::Put,
                ApiPath::session_query(),
                Some(body),
                cx,
                move |_, result, cx| {
                    // A command the channel lost ends in a reset, which forgets the choice with its UI session.
                    let Some(answer) = answer(result) else {
                        return;
                    };
                    let _ = list.update(cx, |list, cx| {
                        list.current.current(cx).hosts.settle(query, host, answer);
                        cx.notify();
                    });
                },
            );
        });
    }

    /// The user removed the host filter from a control that goes with it (the chip, "Show all hosts"): choose All, and
    /// move focus to the rail's All in place of the control that is going away. `mark` is the one taken in the
    /// control's handler.
    fn clear_host(&mut self, mark: &Mark, window: &mut Window, cx: &mut Context<Self>) {
        self.choose_host(None, cx);
        focus::focus_control(&self.all_focus, mark, window, cx);
    }
}

/// A handler for a control that removes the host filter and goes away with it ([`SessionList::clear_host`]).
fn clear_from(
    list: &WeakEntity<SessionList>,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    let list = list.clone();
    move |_, window, cx| {
        // Taken in the handler, after the coordinator counted this press or key: the move is this action's own.
        let mark = Mark::take(cx);
        let _ = list.update(cx, |list, cx| list.clear_host(&mark, window, cx));
    }
}

/// What the helm's reply to a query says: accepted, or refused in its words ([`super::refusal_words`]), or `None` when
/// the channel lost the command, which only a reset follows.
fn answer(result: Result<Reply, CommandError>) -> Option<Result<(), HostText>> {
    let reply = result.ok()?;
    if (200..300).contains(&reply.status) {
        return Some(Ok(()));
    }
    Some(Err(super::refusal_words(
        reply.status,
        reply.body,
        "PUT /ui/session-query",
    )))
}

// ---- Host marks and names ------------------------------------------------------------------------------------------

/// A host's mark: its chosen shape `icon` at `size`, tinted with its chosen `color` (the ordinary foreground for the
/// default color). Decorative: what it stands for is in the accessible name of the control it sits in. The rows and
/// the quick switcher will draw a host's mark through this too.
pub(crate) fn host_mark(icon: HostIcon, color: HostColor, size: Pixels, palette: &Palette) -> Svg {
    svg()
        .path(glyph(icon).path())
        .size(size)
        .flex_none()
        .text_color(theme::color(tint(color, palette)))
}

/// The drawing of each host shape.
fn glyph(icon: HostIcon) -> Icon {
    match icon {
        HostIcon::Cloud => Icon::HostCloud,
        HostIcon::House => Icon::HostHouse,
        HostIcon::Flask => Icon::HostFlask,
        HostIcon::Database => Icon::HostDatabase,
        HostIcon::Chip => Icon::HostChip,
        HostIcon::Rocket => Icon::HostRocket,
        HostIcon::Gear => Icon::HostGear,
        HostIcon::Gem => Icon::HostGem,
        HostIcon::Hexagon => Icon::HostHexagon,
        HostIcon::Triangle => Icon::HostTriangle,
        HostIcon::Ring => Icon::HostRing,
        HostIcon::Square => Icon::HostSquare,
        HostIcon::Bug => Icon::HostBug,
        HostIcon::Factory => Icon::HostFactory,
        HostIcon::Castle => Icon::HostCastle,
        HostIcon::Laptop => Icon::HostLaptop,
    }
}

/// The token a host color tints with: the palette's `host` entry at the color's place in `HostColor::ALL` less one
/// (`--host-icon-<color>`), and the ordinary foreground for the default color, as the old mark's inline style did.
fn tint(color: HostColor, palette: &Palette) -> Token {
    match color {
        HostColor::Default => palette.fg_0,
        HostColor::Lavender => palette.host[0],
        HostColor::Orchid => palette.host[1],
        HostColor::Teal => palette.host[2],
        HostColor::Steel => palette.host[3],
        HostColor::Sand => palette.host[4],
        HostColor::Copper => palette.host[5],
        HostColor::Red => palette.host[6],
    }
}

/// The host's identity as the rail's avatar and the chip name it (the old `rail_identity`), escaped as the screen shows
/// host text, for an accessible name and a tooltip: its display name ("Local (this machine)" for the local host under
/// the registry's own name), then its destination in parentheses when the host is remote and the destination differs
/// from the name. The state that SPEC.md also wants in the avatar's name joins it with the host status.
fn identity(host: &HostView) -> SharedString {
    let local = host.is_local();
    // Only its accessible name is used, so the sentence is never drawn and its id never reaches a frame.
    let sentence = HostSentence::new("host-identity");
    let sentence = if local && host.name == LOCAL_NAME {
        sentence.text("Local (this machine)")
    } else {
        sentence.host(&HostText::from_host(host.name.as_str()))
    };
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
fn unregistered(id: i64) -> String {
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
fn registered(collections: &Collections, id: i64) -> Option<&HostView> {
    collections.get::<HostView>(&HostView::key(id))
}

// ---- The rail ------------------------------------------------------------------------------------------------------

/// The rail, for the hosts in `collections` with `chosen` pressed; its entries choose through `list`, and All tracks
/// `all_focus`, the list's handle that removing the filter focuses.
///
/// It scrolls on its own when its entries overflow it, beside the list, which scrolls separately (`overflow: hidden
/// auto`); its padding and gaps are inside the scrolling element, so a wheel over them scrolls the rail.
pub(super) fn rail(
    collections: &Collections,
    chosen: Option<i64>,
    list: &WeakEntity<SessionList>,
    all_focus: &FocusHandle,
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
                .disabled(true)
                .content(div().child("?"))
                .into_any_element(),
            );
        }
    } else {
        entries.push(unavailable(palette));
    }
    div()
        .id("host-rail")
        // Tests find the rail by this; it records nothing outside test builds.
        .debug_selector(|| "host-rail".into())
        .role(Role::Navigation)
        .aria_label("Hosts")
        .w(RAIL_WIDTH)
        .flex_none()
        .h_full()
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
        )
}

/// One host's avatar: its mark in a rail button named by its identity, with its short name as a caption under it.
fn avatar(
    host: &HostView,
    chosen: bool,
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

// ---- The chip, the refusal and the empty state ---------------------------------------------------------------------

/// The row under the list's heading while a host is chosen, holding the chip that removes the choice: the host's mark,
/// its short name (cut short when the row is narrow) and a ✕, named "Filtered to host <name>; remove filter" with the
/// host's identity as its help; for a host the registry no longer lists, "?" and "Host <id> (no longer registered)".
/// Pressing it chooses All. A plain wrapping row, so the status segments can join the chip in it.
pub(super) fn chip_row(
    collections: &Collections,
    chosen: i64,
    list: &WeakEntity<SessionList>,
    palette: &Palette,
) -> impl IntoElement {
    let (label, help, mark, name): (SharedString, SharedString, AnyElement, HostText) =
        match registered(collections, chosen) {
            Some(host) => {
                let name = HostText::from_host(short_name(host));
                let label = HostSentence::new("host-filter-chip-label")
                    .text("Filtered to host ")
                    .host(&name)
                    .text("; remove filter")
                    .accessible_name();
                let help = identity(host);
                let mark =
                    host_mark(host.icon, host.color, px(CHIP_MARK), palette).into_any_element();
                (label, help, mark, name)
            }
            None => {
                let name = unregistered(chosen);
                (
                    format!("Filtered to host {name}; remove filter").into(),
                    name.clone().into(),
                    div().flex_none().child("?").into_any_element(),
                    // The app's own words, drawn through the host-text element only to share the name's layout.
                    HostText::from_host(name),
                )
            }
        };
    let chip = Button::new(
        "host-filter-chip",
        label,
        HoverHelp::tooltip(help),
        Role::Button,
    )
    .tier(Tier::Chip)
    .content(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(CHIP_GAP))
            .min_w_0()
            .child(mark)
            .child(host_text::element(&name).min_w_0().truncate())
            .child(div().flex_none().child("✕")),
    )
    .on_click(clear_from(list));
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(CHIP_ROW_GAP))
        .py(px(CHIP_ROW_PAD.0))
        .px(px(CHIP_ROW_PAD.1))
        .min_w_0()
        .child(chip)
}

/// The host whose empty listing the list should name instead of drawing no rows, if any: the answered query's host,
/// when its listing is available, complete, and matched nothing (`shown` is the number of rows it holds). A listing the
/// helm's cap cut claims nothing about the host, and keeps the count line's warning.
pub(super) fn empty_host(
    meta: &CollectionMeta,
    shown: usize,
    answered: Option<i64>,
) -> Option<i64> {
    let matched_nothing = meta.availability == Availability::Available
        && meta
            .counts
            .is_some_and(|counts| !counts.truncated && counts.matching == Some(0));
    answered.filter(|_| shown == 0 && matched_nothing)
}

/// The empty state of the answered host `host`: "No sessions on <mark> <short name>." in the old `.status` line's
/// size and quiet ink, and under it the "Show all hosts" link, which chooses All. The sentence is one labelled node for
/// assistive technology; the host's name is its own text element in it, kept together with the period that ends it.
/// A long name wraps within its own element: the words before it and its mark stay on its first line, and the period
/// sits on its last, as the old inline sentence had them.
pub(super) fn empty_state(
    collections: &Collections,
    host: i64,
    list: &WeakEntity<SessionList>,
    palette: &Palette,
) -> impl IntoElement {
    let (mark, name) = match registered(collections, host) {
        Some(view) => (
            host_mark(view.icon, view.color, px(CHIP_MARK), palette).into_any_element(),
            HostText::from_host(short_name(view)),
        ),
        None => (
            div().flex_none().child("?").into_any_element(),
            HostText::from_host(unregistered(host)),
        ),
    };
    let label = HostSentence::new("host-filter-empty-label")
        .text("No sessions on ")
        .host(&name)
        .text(".")
        .accessible_name();
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .id("host-filter-empty")
                .role(Role::Label)
                .aria_label(label)
                .flex()
                .flex_row()
                .flex_wrap()
                .items_start()
                .gap_x(px(4.))
                .p(metrics::LIST_STATUS_PAD)
                .pb(px(8.))
                .text_size(metrics::TEXT_ROW_TITLE)
                .line_height(px(EMPTY_LINE))
                .text_color(theme::color(palette.fg_1))
                .child(div().child("No sessions on"))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .min_w_0()
                        // The mark sits centered on the name's first line, whatever the name's height.
                        .child(
                            div()
                                .flex_none()
                                .h(px(EMPTY_LINE))
                                .flex()
                                .items_center()
                                .child(mark),
                        )
                        .child(host_text::element(&name).min_w_0().ml(px(4.)))
                        .child(div().flex_none().self_end().child(".")),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_y(px(EMPTY_ACTIONS_GAP.0))
                .gap_x(px(EMPTY_ACTIONS_GAP.1))
                .px(metrics::LIST_STATUS_PAD)
                .pb(metrics::LIST_STATUS_PAD)
                .child(
                    Button::new(
                        "host-filter-show-all",
                        "Show all hosts",
                        HoverHelp::tooltip("Show all hosts: remove the host filter"),
                        Role::Button,
                    )
                    .tier(Tier::Link)
                    .on_click(clear_from(list)),
                ),
        )
}

/// The sentence for a refused query: the helm's words, which may quote a host, after the app's own.
pub(super) fn refusal_sentence(words: &HostText) -> HostSentence {
    HostSentence::new("host-filter-refusal")
        .text("Could not show that host's sessions: ")
        .host(words)
}
