//! Pins: Pin to top, Unpin, Move up and Move down in a row's menu, the order pinned rows are drawn in, the muted pin
//! glyph before a pinned row's title, and the heavier separator under the leading pinned run.
//!
//! SPEC.md, "Session list". Ported from the old app's `rows.rs` (`pinned_first`, `pin_order_key`,
//! `has_manual_pin_order`, `pin_move_neighbors`, `pin_separator_after`), `list/row.rs` (the menu items, their wording,
//! the glyph and the separator), `list/view.rs` (`on_pin`, `on_move_pin`) and the stylesheet's `.session-pin` and
//! `.session-pin-separator` rules at the reference commit. Here's the TLDR of the rules:
//!
//! - **The helm owns the pins.** Membership and the manual order are shared by every client and kept by the helm, which
//!   serves each row's `pinned` and `pin_position` and leaves its served sort alone (the quick switcher keeps the
//!   helm's order). Only the sidebar groups pins first ([`pinned_first`]), so nothing here is a copy of the helm's
//!   state: the list sorts what the store holds every time it draws.
//! - **Pins first, in their shared order; the rest in the helm's order.** A stable partition, then the leading pinned
//!   block sorted by position, host and bare session id (the helm's tie break: Replace keeps both rows at one position
//!   for a moment). A listing in which some pinned row has no position keeps the plain partition and offers no Move
//!   items, as the old app did for an older helm. Filters apply first, so pins keep their relative order under a host
//!   or status filter.
//! - **Move up and Move down name the displayed neighbor** ([`neighbors`]): the pin drawn above or below inside the
//!   displayed leading run of movable pins, never a position. The helm takes that neighbor and a placement, so a pin a
//!   filter hides is never a target. An unavailable direction is not drawn; an unpinned row offers neither, and so does
//!   a row pinned while the order hold is on ([`super::hold`]), until the hold ends.
//! - **One request each, through the operation store,** under the session's key ([`Work::Pin`], [`Work::MovePin`]),
//!   so the session's other actions wait for it as they wait for Mark read. Each closes the menu at acceptance, which
//!   returns focus to the row's menu button (SPEC.md: a Move "keeps keyboard focus on the moved row's menu button"),
//!   and nothing is drawn in advance: the helm pushes the new order before it replies. A refusal shows on the row as
//!   "Pin: …" or "Move pin: …" ([`super::outcome`]).
//! - **The glyph and the separator** are display only: the glyph is a status mark named and helped "Pinned" just before
//!   the title, and the row's accessible name starts with "Pinned"; the separator follows the last row of the displayed
//!   leading pinned run, and only when an unpinned row follows ([`separator_after`]), so a pin the hold keeps lower down
//!   does not move it.
//!
//! Dragging a movable pin by its grip is the pointer's way to make the same move ([`super::pin_drag`]): it sends one
//! Move through [`send_move`], with the pin it was dropped on as the neighbor.
//!
//! NOTE: The old noncompact identity line wrapped its title under the pin when the sidebar was narrow; here the title
//! truncates beside the glyph as it does on every row, and the glyph's line box is held to its own size so that a
//! pinned row is no taller than an unpinned one.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::SessionRef;
use farhelm_proto::session_requests::{PinMoveAreas, SessionGroupTarget};
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, Role, SharedString,
    StatefulInteractiveElement as _, Styled, WeakEntity, div, px, relative,
};
use serde_json::json;

use super::row_marks::HelpPlace;
use super::selection::Listed;
use super::{SessionList, groups};
use crate::assets::Icon;
use crate::components::{HoverHelp, MenuItem, StatusMark};
use crate::operations::{Command, OpKey, Work};
use crate::theme::{self, Palette, Token, metrics};

// ---- The display order -----------------------------------------------------------------------------------------------

/// Whether `rows` carry the shared manual order: every pinned row has a position. Only then are pins sorted by it and
/// offered Move items. A mixed listing must not invent ranks for the rows without one.
pub(super) fn manual_order(rows: &[Listed]) -> bool {
    rows.iter()
        .filter(|session| session.row.pinned)
        .all(|session| session.row.pin_position.is_some())
}

/// What the manual order sorts pins by: the shared position, then the host's registry id, then the session's bare id
/// (the helm's documented tie break). A key that is not a full session reference compares whole.
fn order_key<'a>(session: &Listed<'a>) -> (i64, i64, &'a str) {
    let key: &'a str = session.key;
    // A full reference is the entry's UUID, a colon and the bare id; `split_once` takes the first colon, which is the
    // separator, since a UUID holds none (the bare id itself may).
    let bare = match key.parse::<SessionRef>() {
        Ok(_) => key.split_once(':').map_or(key, |(_, bare)| bare),
        Err(_) => key,
    };
    (
        session.row.pin_position.unwrap_or_default(),
        session.row.host,
        bare,
    )
}

/// Compare two pins by the helm's total order, including ties left by Replace.
pub(super) fn by_position(a: &Listed, b: &Listed) -> Ordering {
    order_key(a).cmp(&order_key(b))
}

/// `rows`, in the helm's order, with the pinned rows moved ahead of the rest: a stable partition, then the leading pinned
/// block in the shared manual order when the listing carries it ([`manual_order`]). Unpinned rows keep the helm's
/// order, whatever the chosen sort.
pub(super) fn pinned_first(mut rows: Vec<Listed<'_>>) -> Vec<Listed<'_>> {
    // `sort_by_key` is stable, so each group keeps the helm's order.
    rows.sort_by_key(|session| !session.row.pinned);
    if manual_order(&rows) {
        let leading = rows.iter().take_while(|session| session.row.pinned).count();
        rows[..leading].sort_by(by_position);
    }
    rows
}

/// A movable pin's displayed neighbors, by session key: the target of its Move up and its Move down. A missing side is a
/// direction the menu does not draw.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Neighbors {
    up: Option<String>,
    down: Option<String>,
}

/// The Move neighbors of every movable pin in `shown`, the rows as displayed: the displayed leading run of rows that are
/// pinned and, while the order hold is on, were pinned when it began (`original`, the hold's original pins; `None`
/// with no hold). A row outside that run (unpinned, pinned during the hold, or a pin below a gap) has no entry and
/// offers no Move items, and neither does any row of a listing without the manual order. A lone movable pin has an
/// entry with neither side.
pub(super) fn neighbors(
    shown: &[Listed],
    original: Option<&HashSet<String>>,
    band: groups::Band,
) -> HashMap<String, Neighbors> {
    if !manual_order(shown) {
        return HashMap::new();
    }
    let movable: Vec<&str> = shown
        .iter()
        .take_while(|session| {
            session.row.pinned
                && groups::Band::served(session) == band
                && original.is_none_or(|original| original.contains(session.key))
        })
        .map(|session| session.key)
        .collect();
    ordered_neighbors(&movable)
}

/// Neighbors of every item in one displayed manual order. The edges have
/// only one direction, and a lone item still has an entry for its grip.
pub(super) fn ordered_neighbors(movable: &[&str]) -> HashMap<String, Neighbors> {
    movable
        .iter()
        .enumerate()
        .map(|(index, key)| {
            let up = index.checked_sub(1).map(|above| movable[above].to_owned());
            let down = movable.get(index + 1).map(|below| (*below).to_owned());
            ((*key).to_owned(), Neighbors { up, down })
        })
        .collect()
}

/// Draw after the displayed leading run, only when another item follows.
/// A header can follow a pin run and a session can follow the groups.
pub(super) fn separator_after<T>(
    shown: &[T],
    another_band: bool,
    in_run: impl Fn(&T) -> bool,
) -> Option<&T> {
    let leading = shown.iter().take_while(|item| in_run(item)).count();
    (leading > 0 && (another_band || leading < shown.len())).then(|| &shown[leading - 1])
}

// ---- The menu items --------------------------------------------------------------------------------------------------

/// Which side of the neighbor it names a move puts the pin: the pin-order route's `placement`. A Move up is always
/// before the pin above and a Move down after the pin below; a drag names either side of any movable pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Placement {
    /// Just above the neighbor.
    Before,
    /// Just below the neighbor.
    After,
}

impl Placement {
    /// The route's word for it.
    pub(super) fn word(self) -> &'static str {
        match self {
            Placement::Before => "before",
            Placement::After => "after",
        }
    }
}

/// The pin item's element id in a row's menu.
const PIN_ITEM_ID: &str = "session-menu-pin";

/// The pin item's label and accessible name for a row that is `pinned` or not: the helm's current membership, never a
/// pin made earlier in this window.
fn pin_label(pinned: bool) -> &'static str {
    if pinned { "Unpin" } else { "Pin to top" }
}

/// The line the pin item shows under its label (the old item's description).
fn pin_description(pinned: bool) -> &'static str {
    if pinned {
        "Sort this session with the others again"
    } else {
        "Keep this session above unpinned ones"
    }
}

/// The pin item's hover help, worded like its neighbors' ("Mark read: treat the latest output as seen").
fn pin_help(pinned: bool) -> &'static str {
    if pinned {
        "Unpin: sort this session with the others again"
    } else {
        "Pin to top: keep this session above unpinned ones"
    }
}

/// Which way a Move goes among the pins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    /// Before the pin drawn above.
    Up,
    /// After the pin drawn below.
    Down,
}

impl Direction {
    /// The item's element id in a row's menu.
    fn item_id(self) -> &'static str {
        match self {
            Direction::Up => "session-menu-move-up",
            Direction::Down => "session-menu-move-down",
        }
    }

    /// The item's label and accessible name.
    fn label(self) -> &'static str {
        match self {
            Direction::Up => "Move up",
            Direction::Down => "Move down",
        }
    }

    /// The line the item shows under its label (the old item's description).
    fn description(self, group: bool) -> &'static str {
        match (self, group) {
            (Direction::Up, false) => "Place before the pinned session above",
            (Direction::Down, false) => "Place after the pinned session below",
            (Direction::Up, true) => "Place before the group above",
            (Direction::Down, true) => "Place after the group below",
        }
    }

    /// The item's hover help.
    fn help(self, group: bool) -> &'static str {
        match (self, group) {
            (Direction::Up, false) => "Move up: place before the pinned session above",
            (Direction::Down, false) => "Move down: place after the pinned session below",
            (Direction::Up, true) => "Move the group up",
            (Direction::Down, true) => "Move the group down",
        }
    }

    /// The item's icon.
    fn icon(self) -> Icon {
        match self {
            Direction::Up => Icon::MoveUp,
            Direction::Down => Icon::MoveDown,
        }
    }

    /// Where this direction puts the pin, relative to the neighbor it names.
    fn placement(self) -> Placement {
        match self {
            Direction::Up => Placement::Before,
            Direction::Down => Placement::After,
        }
    }

    /// The neighbor this direction targets among `neighbors`, if the row has one that way.
    fn target(self, neighbors: &Neighbors) -> Option<&str> {
        match self {
            Direction::Up => neighbors.up.as_deref(),
            Direction::Down => neighbors.down.as_deref(),
        }
    }
}

/// The pin items of the menu of `key`'s row, a session whose row is `row`, for the menu to draw right after Mark read:
/// Pin to top or Unpin, then Move up and Move down where the row has a displayed neighbor that way (`list`'s Move
/// neighbors, as the list last drew them). Every item is unavailable while `busy` ([`super::menu::refused`]), as the
/// menu's others are. Each closes the menu before its request goes out.
pub(super) fn menu_items(
    list: &WeakEntity<SessionList>,
    key: &str,
    row: &SessionRow,
    busy: bool,
    cx: &App,
) -> Vec<MenuItem> {
    let pinned = row.pinned;
    let neighbors = list
        .upgrade()
        .and_then(|list| list.read(cx).moves.get(key).cloned())
        .unwrap_or_default();
    let pin = {
        let (list, key) = (list.clone(), key.to_owned());
        MenuItem::new(
            PIN_ITEM_ID,
            Icon::Pin,
            pin_label(pinned),
            HoverHelp::tooltip(pin_help(pinned)),
            Role::MenuItem,
        )
        .description(pin_description(pinned))
        .disabled(busy)
        .on_click(move |_, window, cx| {
            let _ = list.update(cx, |list, cx| {
                list.close_menu(&key, window, cx);
                super::menu::send(&key, pin_command(&key, !pinned), cx);
            });
        })
    };
    let area = row
        .group
        .map_or(SessionGroupTarget::Ungrouped, SessionGroupTarget::Group);
    std::iter::once(pin)
        .chain(move_items(list, key, &neighbors, busy, Some(area)))
        .collect()
}

/// Group menus have no Pin item. Their Move actions use the same neighbors
/// and send the same relative placement as a pinned row.
pub(super) fn group_move_items(
    list: &WeakEntity<SessionList>,
    key: &str,
    busy: bool,
    cx: &App,
) -> Vec<MenuItem> {
    let neighbors = list
        .upgrade()
        .and_then(|list| list.read(cx).moves.get(key).cloned())
        .unwrap_or_default();
    move_items(list, key, &neighbors, busy, None).collect()
}

/// Build the Move actions from the neighbors drawn on screen. Both
/// directions use the same focus return and relative placement.
fn move_items<'a>(
    list: &'a WeakEntity<SessionList>,
    key: &'a str,
    neighbors: &'a Neighbors,
    busy: bool,
    area: Option<SessionGroupTarget>,
) -> impl Iterator<Item = MenuItem> + 'a {
    [Direction::Up, Direction::Down]
        .into_iter()
        .filter_map(move |direction| {
            let neighbor = direction.target(neighbors)?.to_owned();
            let (list, key) = (list.clone(), key.to_owned());
            Some(
                MenuItem::new(
                    direction.item_id(),
                    direction.icon(),
                    direction.label(),
                    HoverHelp::tooltip(direction.help(area.is_none())),
                    Role::MenuItem,
                )
                .description(direction.description(area.is_none()))
                .disabled(busy)
                .on_click(move |_, window, cx| {
                    let _ = list.update(cx, |list, cx| {
                        // Closing returns focus to this row's menu button, where SPEC.md keeps it after a Move,
                        // before the request goes out; the reply's new order moves the row and the focus with it.
                        list.close_menu(&key, window, cx);
                        let areas = area.map(|area| PinMoveAreas {
                            source: area,
                            destination: area,
                        });
                        send_move(&key, &neighbor, direction.placement(), areas, cx);
                    });
                }),
            )
        })
}

// ---- The requests ----------------------------------------------------------------------------------------------------

/// Send `key`'s Pin to top (`pinned`) or Unpin through the operation store, as the row menu's item does: the marked
/// bar's Pin and Unpin send each marked row's own request through here, so the request's shape stays in this file.
pub(super) fn send_pin(key: &str, pinned: bool, cx: &mut gpui::App) {
    super::menu::send(key, pin_command(key, pinned), cx);
}

/// The session's Pin to top (`pinned`) or Unpin: `PUT /api/sessions/{id}/pin` with `{"pinned": <bool>}`, which the helm
/// requires whole (a missing value is refused, never read as an unpin).
fn pin_command(session: &str, pinned: bool) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Pin,
        Method::Put,
        ApiPath::api(&["sessions", session, "pin"])?,
        Some(json!({ "pinned": pinned })),
        "PUT /api/sessions/[session]/pin",
    ))
}

/// Send `key`'s move to `placement` of `neighbor`, a displayed movable pin, through the operation store, as the menu's
/// Move up and Move down and a drop of the row's grip do: one request, under the session's key, with nothing drawn in
/// advance (the helm pushes the new order before it replies, and the order hold lets it through at once).
pub(super) fn send_move(
    key: &str,
    neighbor: &str,
    placement: Placement,
    areas: Option<PinMoveAreas>,
    cx: &mut gpui::App,
) {
    if let Some(group) = groups::header_id(key) {
        let Some(neighbor) = groups::header_id(neighbor) else {
            return;
        };
        super::menu::send_for(
            &OpKey::session_group(group),
            relative_command(
                Work::SessionGroup,
                &["session-groups", &group.to_string(), "order"],
                json!(neighbor),
                placement,
                None,
                "PUT /api/session-groups/[group]/order",
            ),
            cx,
        );
    } else {
        super::menu::send(key, move_command(key, neighbor, placement, areas), cx);
    }
}

/// The session's move: `PUT /api/sessions/{id}/pin-order` naming the displayed `neighbor` by its full key and the
/// placement relative to it. The helm refuses it when either is no longer pinned.
pub(super) fn move_command(
    session: &str,
    neighbor: &str,
    placement: Placement,
    areas: Option<PinMoveAreas>,
) -> Result<Command, PathError> {
    relative_command(
        Work::MovePin,
        &["sessions", session, "pin-order"],
        json!(neighbor),
        placement,
        areas,
        "PUT /api/sessions/[session]/pin-order",
    )
}

/// Only the resource and neighbor key differ between the two order routes.
fn relative_command(
    work: Work,
    path: &[&str],
    neighbor: serde_json::Value,
    placement: Placement,
    areas: Option<PinMoveAreas>,
    label: &'static str,
) -> Result<Command, PathError> {
    let mut body = json!({ "neighbor": neighbor, "placement": placement.word() });
    if let Some(areas) = areas {
        body["areas"] = json!(areas);
    }
    Ok(Command::new(
        work,
        Method::Put,
        ApiPath::api(path)?,
        Some(body),
        label,
    ))
}

// ---- The glyph and the separator -------------------------------------------------------------------------------------

/// The pin glyph's text: the Nerd Font thumbtack (U+F08D) the old row drew, which the terminal's bundled JetBrains Mono
/// Nerd Font carries.
const PIN_GLYPH: &str = "\u{f08d}";

/// The glyph's id suffix, after the row's open control id (`session-row-<key>-pin`).
const GLYPH_ID_SUFFIX: &str = "-pin";

/// The glyph's text size: `.session-pin` set none, so it took the open button's inherited 16 pixels, not the title's 14.
const GLYPH_SIZE: f32 = 16.;

/// The old identity copy's gap between the glyph and the title (`.session-identity-copy`'s `gap: 4px`), against the 6
/// pixels between the identity line's tracks here.
const GLYPH_GAP: f32 = 4.;

/// A pinned row's glyph, drawn just before its title in the row's secondary ink (`--fg-1`, or `--on-accent-dim` on the
/// selected row): a status mark named and helped "Pinned", with nothing to press, so a press on it opens the row as a
/// press on the title does. `row_id` is the row's open control id and `gap` the identity line's gap after it, which the
/// glyph takes back down to the old one. Its help shows where `place` says: beside the sidebar, as every row hover.
pub(super) fn glyph(
    row_id: &SharedString,
    ink: Token,
    gap: gpui::Pixels,
    place: HelpPlace,
) -> impl IntoElement {
    div().flex_none().mr(px(GLYPH_GAP) - gap).child(
        StatusMark::new(
            format!("{row_id}{GLYPH_ID_SUFFIX}"),
            "Pinned",
            place.place(HoverHelp::tooltip("Pinned")),
            Role::Image,
            PIN_GLYPH,
        )
        .ink(ink)
        .monospace()
        .described(false)
        // One line box of the glyph's own size, so the larger face does not make a pinned row taller.
        .content(
            div()
                .text_size(px(GLYPH_SIZE))
                .line_height(relative(1.))
                .child(PIN_GLYPH),
        ),
    )
}

/// The separator's element id. One list has at most one.
const SEPARATOR_ID: &str = "session-pin-separator";

/// The heavier separator under the leading pinned run (`.session-pin-separator`): two pixels of `--border-row` under
/// the last pinned row's own one-pixel border, then a 6-pixel gap before the first unpinned row, named "Pinned
/// sessions" as the old `role="separator"` was. There is no "Pinned" heading.
#[derive(Clone, Copy)]
pub(super) enum Separator {
    GlobalPins,
    GroupPins(i64),
    Groups,
}

pub(super) fn separator(kind: Separator, palette: &Palette) -> impl IntoElement {
    let (id, label) = match kind {
        Separator::GlobalPins => (SEPARATOR_ID.to_owned(), "Pinned sessions"),
        Separator::GroupPins(id) => (
            format!("session-group-{id}-pin-separator"),
            "Pinned group members",
        ),
        Separator::Groups => ("session-groups-separator".to_owned(), "Session groups"),
    };
    let grouped = matches!(kind, Separator::GroupPins(_));
    let (weight, gap) = if grouped { (1., 3.) } else { (2., 6.) };
    let separator = div()
        .id(SharedString::from(id.clone()))
        // Tests find it by this; it records nothing outside test builds.
        .debug_selector(move || id.clone())
        .role(Role::Splitter)
        .aria_label(label)
        .w_full();
    let stroke = theme::color(palette.border_row);
    if grouped {
        // Continue the guide through the divider between a group's last pin and its first unpinned member.
        separator
            .relative()
            .pb(px(gap))
            .child(
                div()
                    .ml(metrics::ROW_PAD.1 + super::groups::MEMBER_INDENT)
                    .border_t(px(weight))
                    .border_color(stroke),
            )
            .child(super::groups::member_guide(palette))
    } else {
        separator
            .border_t(px(weight))
            .border_color(stroke)
            .mb(px(gap))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_list::tests::{Fixture, session};
    use farhelm_proto::ui_channel::SessionRow;

    /// Each fixture's row, kept alive for the listing that borrows it.
    fn rows(fixtures: &[Fixture]) -> Vec<(&'static str, SessionRow)> {
        fixtures
            .iter()
            .map(|fixture| (fixture.key, fixture.row()))
            .collect()
    }

    fn listed<'a>(rows: &'a [(&'static str, SessionRow)]) -> Vec<Listed<'a>> {
        rows.iter().map(|(key, row)| Listed { key, row }).collect()
    }

    fn keys<'a>(shown: &[Listed<'a>]) -> Vec<&'a str> {
        shown.iter().map(|session| session.key).collect()
    }

    /// SPEC.md: pinned sessions appear first "in a manual order shared by every client … independent of the list's
    /// chosen sort", and "Unpinned rows keep the chosen list order below"; the helm breaks a position tie by host and
    /// then bare session id. Getting it wrong reorders every client's sidebar differently. Specified: served as u1, p3,
    /// u2, p1, p2 (pins at positions 3, 1, 1, the last two on hosts 2 and 1), the list draws p2 (host 1), p1, p3, then
    /// u1, u2 in the helm's order; full session references tie-break on their bare id, not the entry prefix.
    #[test]
    fn pins_lead_in_their_shared_order_and_the_rest_keep_the_helms() {
        let fixtures = [
            session("u1"),
            session("p3").pinned(3),
            session("u2"),
            session("p1").pinned(1).host(2),
            session("p2").pinned(1).host(1),
        ];
        let rows = rows(&fixtures);
        assert_eq!(
            keys(&pinned_first(listed(&rows))),
            ["p2", "p1", "p3", "u1", "u2"]
        );

        // Two full references on one host at one position: the bare ids decide, though the entry prefixes sort the
        // other way.
        let late = "ffffffff-ffff-4fff-8fff-ffffffffffff:aaa";
        let early = "00000000-0000-4000-8000-000000000000:bbb";
        let row = session("x").pinned(1).row();
        let refs = [(late, row.clone()), (early, row)];
        assert_eq!(keys(&pinned_first(listed(&refs))), [late, early]);
    }

    /// The old app kept an older helm's stable pin partition and offered no Move items when some pinned row had no
    /// position, since inventing ranks would move pins the helm never moved. Specified: with one pin missing its
    /// position, pins still lead in the helm's order, and no row has Move neighbors.
    #[test]
    fn a_listing_without_every_position_keeps_the_plain_partition_and_no_moves() {
        let fixtures = [
            session("u").title("u"),
            session("b").pinned(2),
            session("a").pinned(1),
        ];
        let mut rows = rows(&fixtures);
        rows[1].1.pin_position = None;
        let shown = pinned_first(listed(&rows));
        assert_eq!(keys(&shown), ["b", "a", "u"]);
        assert!(neighbors(&shown, None, groups::Band::GlobalPins).is_empty());
    }

    /// The items' words are the old menu's, and each request is its own kind of work on its own route, so a refusal
    /// carries the right name. Specified: each label, description and help; a pin is [`Work::Pin`] on the session's pin
    /// route and a move [`Work::MovePin`] on its pin-order route. The bodies are pinned where the window sends them
    /// (`pins_tests.rs`).
    #[test]
    fn the_items_are_worded_as_the_old_menu_and_send_the_routes_bodies() {
        assert_eq!(
            (pin_label(false), pin_description(false), pin_help(false)),
            (
                "Pin to top",
                "Keep this session above unpinned ones",
                "Pin to top: keep this session above unpinned ones"
            )
        );
        assert_eq!(
            (pin_label(true), pin_description(true), pin_help(true)),
            (
                "Unpin",
                "Sort this session with the others again",
                "Unpin: sort this session with the others again"
            )
        );
        assert_eq!(
            (
                Direction::Up.label(),
                Direction::Up.description(false),
                Direction::Up.help(false)
            ),
            (
                "Move up",
                "Place before the pinned session above",
                "Move up: place before the pinned session above"
            )
        );
        assert_eq!(
            (
                Direction::Down.label(),
                Direction::Down.description(false),
                Direction::Down.help(false)
            ),
            (
                "Move down",
                "Place after the pinned session below",
                "Move down: place after the pinned session below"
            )
        );
        let pin = pin_command("a", true).unwrap();
        assert_eq!(
            (pin.work(), pin.path().as_str()),
            (Work::Pin, "/api/sessions/a/pin")
        );
        let moved = move_command("a", "b", Direction::Up.placement(), None).unwrap();
        assert_eq!(
            (moved.work(), moved.path().as_str()),
            (Work::MovePin, "/api/sessions/a/pin-order")
        );
        assert_eq!(
            (
                Direction::Up.placement().word(),
                Direction::Down.placement().word()
            ),
            ("before", "after")
        );
    }

    /// SPEC.md: "A heavier separator with a small gap below follows the displayed leading contiguous run of pinned rows,
    /// only when an unpinned row follows … A newly pinned row held below that run does not move the separator there".
    /// No window test holds a new pin below an unpinned row, so this is that case's guard. Specified: for p and q pinned, u unpinned, then r
    /// pinned (held below), the separator follows q, not r, and is still drawn; none when every row is pinned or none
    /// is.
    #[test]
    fn the_separator_follows_the_leading_pinned_run_only_before_an_unpinned_row() {
        let fixtures = [
            session("p").pinned(1),
            session("q").pinned(2),
            session("u"),
            session("r").pinned(3),
        ];
        let rows = rows(&fixtures);
        let shown = listed(&rows);
        assert_eq!(
            separator_after(&shown, false, |row| row.row.pinned).map(|row| row.key),
            Some("q"),
            "a pin held below moves nothing"
        );
        assert_eq!(
            separator_after(&shown[..2], false, |row| row.row.pinned).map(|row| row.key),
            None,
            "nothing unpinned follows"
        );
        assert_eq!(
            separator_after(&shown[2..3], false, |row| row.row.pinned).map(|row| row.key),
            None,
            "nothing is pinned"
        );
    }

    /// The operation store's two classifications decide whether a launch and a host registry change refuse each other
    /// while this work is in flight; a pin or a move is neither, so it must hold up neither side. Pinned here, in the
    /// area's own tests, as the store's extension point asks of a new kind of work. Specified: neither kind creates a
    /// session or changes the host registry.
    #[test]
    fn pins_and_moves_neither_launch_nor_change_the_host_registry() {
        for work in [Work::Pin, Work::MovePin] {
            assert!(!work.creates_a_session(), "{work:?}");
            assert!(!work.changes_host_registry(), "{work:?}");
        }
    }
}
