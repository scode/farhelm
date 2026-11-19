//! The sidebar partitions the helm's ordered listing into global pins,
//! named groups and the ungrouped tail. The quick switcher reads the
//! helm's listing directly, so layout choices here never change its order.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Collections, Method, PathError};
use farhelm_proto::session_requests::{
    NewSessionGroup, SessionGroupMembership, SessionGroupTarget, UpdateSessionGroup,
};
use farhelm_proto::ui_channel::{Collection, SessionGroup, SessionRow};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, StatefulInteractiveElement as _, Styled as _,
    Subscription, Transformation, WeakEntity, Window, div, px, radians, svg,
};
use serde_json::json;

use super::{
    SessionList, menu, pin_drag, pins, queued_preference::QueuedPreference, selection::Listed,
    status,
};
use crate::assets::Icon;
use crate::components::{Button, HoverHelp, MenuItem, Role, StatusMark, Tier, submenu};
use crate::focus;
use crate::host_text::{self, HostSentence, HostText};
use crate::operations::{Command, NotStarted, OpKey, Operations, Work};
use crate::session;
use crate::theme::{self, metrics};

/// The content steps in under the group header. The grip, selection fill and marked stripe use the original row edge.
pub(super) const MEMBER_INDENT: Pixels = px(16.);

/// Put the guide beside the leading edge of the header's caret, clear of the member's indented status slot.
pub(super) fn member_guide(palette: &theme::Palette) -> gpui::Div {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(20.))
        .w(px(1.))
        .bg(theme::color(palette.border_nested))
}

/// The group header key is outside the session reference namespace. Menu
/// anchors, virtualized focus and drag handles can all use the same key.
pub(super) fn header_key(id: i64) -> String {
    format!("group/{id}")
}

/// Recover a group ID only from a header key.
pub(super) fn header_id(key: &str) -> Option<i64> {
    key.strip_prefix("group/")?.parse().ok()
}

/// The server includes every destination, even those whose members are all
/// hidden by a filter or a truncated listing.
pub(super) fn roster(collections: &Collections) -> &[SessionGroup] {
    collections
        .meta(Collection::Sessions)
        .map_or(&[], |meta| meta.session_groups.as_slice())
}

// Header controls

/// A collapsed group draws the mark of the first member at the highest
/// attention rank. Ties follow the same order as the member rows.
#[derive(Clone)]
pub(super) struct Attention {
    title: HostText,
    live: status::Live,
}

impl Attention {
    /// Use the highest attention rank among the rows this listing serves.
    pub(super) fn highest(rows: &[Listed]) -> Option<Self> {
        let mut best = None;
        for member in rows {
            let Some(live) = status::live(member.row) else {
                continue;
            };
            let Some(rank) = live.group_rank(member.row) else {
                continue;
            };
            if best.is_none_or(|(current, _, _)| rank > current) {
                best = Some((rank, live, member.row.info.title.as_str()));
            }
        }
        best.map(|(_, live, title)| Self {
            title: HostText::from_host(title),
            live,
        })
    }

    /// The header shares the list ticker when its winning mark runs.
    pub(super) fn pulses(&self) -> bool {
        self.live.pulses(false)
    }
}

/// A header's own drawing data, captured before the item callback borrows
/// the store and with collapsed members still available for its count.
#[derive(Clone)]
pub(super) struct Header {
    pub(super) id: i64,
    pub(super) name: String,
    pub(super) collapsed: bool,
    /// An old header can keep held rows in place after the group is gone.
    live: bool,
    pub(super) count: Option<String>,
    pub(super) attention: Option<Attention>,
    /// The title that otherwise has no selected row while this group is folded.
    open_member: Option<HostText>,
}

/// The same queue used by the sidebar preferences, with an entity owned
/// by each touched group. Its observer redraws the list when a save settles.
pub(super) struct Collapse {
    id: i64,
    queue: QueuedPreference<bool>,
}

/// Each group state owns its draft and the list subscribes to its changes.
pub(super) type CollapseWrites = HashMap<i64, (Entity<Collapse>, Subscription)>;

impl Collapse {
    /// The in-flight or latest queued toggle overrides the stored choice.
    pub(super) fn shown(writes: &CollapseWrites, group: &SessionGroup, cx: &App) -> bool {
        writes.get(&group.id).map_or(group.collapsed, |(state, _)| {
            state.read(cx).queue.shown(group.collapsed)
        })
    }

    /// A sparse write changes only this group's collapsed flag.
    fn choose(&mut self, value: bool, cx: &mut Context<Self>) {
        let id = self.id;
        QueuedPreference::choose_with_key(
            self,
            |state| &mut state.queue,
            value,
            OpKey::session_group(id),
            move |collapsed| {
                update(
                    id,
                    UpdateSessionGroup {
                        collapsed: Some(*collapsed),
                        ..Default::default()
                    },
                )
            },
            "group collapse",
            cx,
        );
    }
}

impl SessionList {
    /// Each group's toggle remains usable while its write is in flight, so
    /// a second press can immediately reverse the visible choice.
    fn toggle_group(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let Some(stored) = roster(store.read(cx).collections())
            .iter()
            .find(|group| group.id == id)
            .map(|group| group.collapsed)
        else {
            return;
        };
        let writes = &mut self.current.current(cx).group_collapse;
        let (entity, _) = writes.entry(id).or_insert_with(|| {
            let entity = cx.new(|_| Collapse {
                id,
                queue: QueuedPreference::new(stored),
            });
            let subscription = cx.observe(&entity, |_, _, cx| cx.notify());
            (entity, subscription)
        });
        let shown = entity.read(cx).queue.shown(stored);
        entity.update(cx, |state, cx| state.choose(!shown, cx));
        cx.notify();
    }
}

/// The toggle and menu are ordinary list controls registered inside the
/// virtualized item's focus handle. Their identities follow the group.
#[allow(
    clippy::too_many_arguments,
    reason = "the item callback's frame and focus state"
)]
pub(super) fn draw_header(
    header: &Header,
    item_focus: &FocusHandle,
    shared: &menu::SharedRows,
    drag: &pin_drag::Shared,
    list: &WeakEntity<SessionList>,
    pulse: u64,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let key = header_key(header.id);
    let name = HostText::from_host(header.name.as_str());
    let id = header.id;
    let label = format!("session-group-{id}-toggle");
    let toggle_focus = focus::control_handle(
        gpui::SharedString::from(format!("{label}-focus")),
        window,
        cx,
    );
    // The grip and the menu button show the way a row's do: while the pointer is over the header or focus is inside
    // it (an open menu keeps its button by itself). Always drawn, they cluttered every header with controls that do
    // nothing until reached for.
    let hover = super::row::RowHover::of(&key, window, cx);
    let active = hover.hovered(cx) || item_focus.contains_focused(window, cx);
    let group_menu = header
        .live
        .then(|| menu::menu_button(shared, list, &key, &header.name, false, active, window, cx));
    let drag = pin_drag::paint(drag, shared, list, &key, false, active, window, cx);
    let palette = theme::palette(cx);
    let words = if header.collapsed {
        "Expand group"
    } else {
        "Collapse group"
    };
    let name_label = format!("{words} {}", farhelm_proto::text::escaped(name.raw()));
    let toggle_help = header.open_member.as_ref().map_or_else(
        || HoverHelp::tooltip(words),
        |title| {
            HoverHelp::sentence(
                HostSentence::new(format!("session-group-{id}-open-help"))
                    .text(words)
                    .text(". Holds the open session: ")
                    .host(title),
            )
        },
    );
    let caret = svg()
        .path(Icon::ChevronDown.path())
        .size(px(12.))
        .flex_none()
        .text_color(theme::color(palette.fg_1))
        .with_transformation(Transformation::rotate(radians(if header.collapsed {
            -std::f32::consts::FRAC_PI_2
        } else {
            0.
        })));
    let toggle = Button::new(
        label,
        name_label,
        toggle_help.ring_radius(theme::metrics::RADIUS_SM - px(1.)),
        Role::Button,
    )
    .tier(Tier::Ghost)
    .compact()
    .discloses(!header.collapsed)
    .disabled(!header.live)
    .focus(&toggle_focus)
    .content(
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .child(caret)
            .child(host_text::element(&name).min_w_0().truncate()),
    )
    .on_click({
        let list = list.clone();
        move |_, _, cx| {
            let _ = list.update(cx, |list, cx| list.toggle_group(id, cx));
        }
    });
    let dot = header.attention.as_ref().map(|attention| {
        let hover = HostSentence::new(format!("session-group-{id}-attention-help"))
            .host(&attention.title)
            .text(" · ")
            .text(attention.live.word());
        StatusMark::new(
            format!("session-group-{id}-attention"),
            hover.accessible_name(),
            HoverHelp::sentence(hover).beside(),
            Role::Status,
            "",
        )
        .content(attention.live.mark(palette, false, false, pulse))
    });
    let refusal = session::current_operations(cx).and_then(|operations| {
        operations
            .read(cx)
            .failure(&OpKey::session_group(id))
            .filter(|failure| failure.work == Work::SessionGroup)
            .map(|failure| {
                super::failure_line(
                    HostSentence::new(format!("session-group-{id}-refusal")).host(&failure.message),
                    palette,
                )
            })
    });
    let mut head = div()
        .id(format!("session-group-{id}"))
        .relative()
        .flex()
        .items_center()
        .gap(px(6.))
        .pl(px(12.))
        .min_h(px(30.))
        .text_size(metrics::TEXT_SM)
        .bg(theme::color(if drag.membership_outline.is_some() {
            palette.marked_bg
        } else {
            palette.bg_2
        }))
        .on_hover(move |hovered, _, cx| hover.set(*hovered, cx))
        .child(div().min_w_0().child(toggle))
        .children(header.count.clone().map(|count| {
            div()
                .flex_none()
                .text_color(theme::color(palette.fg_1))
                .child(count)
        }))
        .children(dot)
        .child(div().flex_1())
        .children(group_menu)
        .children(drag.recorder)
        .children(drag.grip)
        .children(header.open_member.as_ref().map(|_| {
            super::row::leading_marker(palette.accent_fill)
                .debug_selector(|| format!("session-group-{id}-open"))
        }))
        .children(drag.membership_outline)
        .children(drag.drop_line);
    if drag.lifted {
        head = head
            .bg(pin_drag::lifted_fill(palette, false))
            .shadow(pin_drag::lifted_shadow(palette));
    }
    focus::track(
        div()
            .id(format!("session-group-{id}-item"))
            .child(head)
            .children(refusal),
        item_focus,
        focus::OnPress::Keep,
    )
    .into_any_element()
}

// Membership choices

/// The two row menus use one destination list and differ in how they
/// admit their currently displayed selection when the user chooses.
#[derive(Clone, Copy)]
pub(super) enum Destination {
    New,
    Existing(SessionGroupTarget),
}

/// The row menu names a pinned row's ungrouped destination Global pins.
pub(super) enum MoveTargets {
    Row(String),
    Marked(Vec<String>),
}

/// Offer destinations based on the whole roster, skipping a destination
/// only if every targeted session is already there. The submenu handles
/// focus and dismissal exactly as it does for Mute.
pub(super) fn move_menu(
    id: &'static str,
    focus: &FocusHandle,
    targets: MoveTargets,
    busy: bool,
    apply: impl Fn(Destination, &mut Window, &mut App) + Clone + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (keys, single) = match targets {
        MoveTargets::Row(key) => (vec![key], true),
        MoveTargets::Marked(keys) => (keys, false),
    };
    let item = MenuItem::new(
        id,
        Icon::Folder,
        "Move to group",
        HoverHelp::tooltip("Move sessions into or out of a group"),
        Role::MenuItem,
    )
    // Every other item in the row menu says in a line what it does; without one this item read as a bare label.
    .description("Keep related sessions together")
    .disabled(busy)
    .focus(focus);
    submenu(
        id.into(),
        item,
        focus,
        move |cx| {
            let Some(store) = session::current_store(cx) else {
                return Vec::new();
            };
            let collections = store.read(cx).collections();
            let targets: Vec<_> = keys
                .iter()
                .filter_map(|key| collections.get::<SessionRow>(key))
                .filter(|row| !super::retired::is(row))
                .collect();
            if targets.is_empty() {
                return Vec::new();
            }
            // Each destination's hover help says what choosing it does, rather than repeating its label.
            let item = |suffix: String, label: String, help: &str, destination| {
                let apply = apply.clone();
                MenuItem::new(
                    format!("{id}-{suffix}"),
                    Icon::Folder,
                    label,
                    HoverHelp::tooltip(help.to_owned()),
                    Role::MenuItem,
                )
                .on_click(move |_, window, cx| apply(destination, window, cx))
            };
            let mut destinations = vec![item(
                "new".into(),
                "New group…".into(),
                "Name a new group and move into it",
                Destination::New,
            )];
            destinations.extend(
                roster(collections)
                    .iter()
                    .filter(|group| targets.iter().any(|row| row.group != Some(group.id)))
                    .map(|group| {
                        item(
                            group.id.to_string(),
                            farhelm_proto::text::escaped(&group.name),
                            "Move into this group",
                            Destination::Existing(SessionGroupTarget::Group(group.id)),
                        )
                    }),
            );
            if targets.iter().any(|row| row.group.is_some()) {
                let (label, help) = if let [row] = targets.as_slice()
                    && row.pinned
                    && single
                {
                    (
                        "Global pins",
                        "Leave the group and stay pinned above all groups",
                    )
                } else {
                    ("Remove from group", "Leave the group")
                };
                destinations.push(item(
                    "remove".into(),
                    label.into(),
                    help,
                    Destination::Existing(SessionGroupTarget::Ungrouped),
                ));
            }
            destinations
        },
        window,
        cx,
    )
}

/// Recheck membership at acceptance so a stale menu never sends a
/// duplicate move. A destination can become empty and disappear meanwhile.
pub(super) fn move_member(key: &str, target: SessionGroupTarget, cx: &mut App) {
    if eligible_member(key, Some(target), cx) {
        super::menu::send(
            key,
            membership(
                key,
                SessionGroupMembership {
                    target,
                    pin_source: None,
                },
            ),
            cx,
        );
    }
}

/// Admit marked targets in display order for both the menu and a drop. An existing destination holds each eligible
/// row for a reply-paced membership write. A new group returns the admitted keys to name together in a single write.
/// Only a drop supplies its list, so only a drop carries expected pin areas and moves that window's held slots.
pub(super) fn move_marked<'a>(
    keys: impl IntoIterator<Item = &'a str>,
    destination: Destination,
    dropped_in: Option<WeakEntity<SessionList>>,
    cx: &mut App,
) -> Vec<String> {
    let target = match destination {
        Destination::Existing(target) => Some(target),
        Destination::New => None,
    };
    let Some(operations) = session::current_operations(cx) else {
        return Vec::new();
    };
    let mut naming = Vec::new();
    let mut pending = Vec::new();
    for key in keys {
        if !eligible_member(key, target, cx) {
            continue;
        }
        if menu::refused(key, cx) {
            super::bulk::not_started(key, Work::SessionGroup, "Move to group", cx);
        } else if let Some(target) = target {
            let op = OpKey::session(key);
            if operations
                .update(cx, |ops, cx| ops.hold(&op, Work::SessionGroup, cx))
                .is_err()
            {
                super::bulk::not_started(key, Work::SessionGroup, "Move to group", cx);
                continue;
            }
            let pin_source = dropped_in.as_ref().and_then(|_| {
                let store = session::current_store(cx)?;
                let row = store.read(cx).collections().get::<SessionRow>(key)?;
                row.pinned.then(|| row.group.into())
            });
            pending.push((
                key.to_owned(),
                SessionGroupMembership { target, pin_source },
            ));
        } else {
            naming.push(key.to_owned());
        }
    }
    if !pending.is_empty() {
        MarkedMoves {
            pending: Rc::new(RefCell::new(pending.into_iter())),
            operations: operations.downgrade(),
            dropped_in,
        }
        .advance(cx);
    }
    naming
}

/// The current reply owns the rest of the accepted batch. One write at a time keeps a large selection within the
/// helm's channel bound. Retiring the operation store drops the reply and its remaining held rows together.
#[derive(Clone)]
struct MarkedMoves {
    pending: Rc<RefCell<std::vec::IntoIter<(String, SessionGroupMembership)>>>,
    operations: WeakEntity<Operations>,
    dropped_in: Option<WeakEntity<SessionList>>,
}

impl MarkedMoves {
    /// Trade the next row's hold for its request in one update. Remote changes can make a queued row ineligible
    /// before its turn, and a refusal settles just that row. In both cases keep processing the rest.
    fn advance(&self, cx: &mut App) {
        let Some(operations) = self.operations.upgrade() else {
            return;
        };
        loop {
            let Some((key, request)) = self.pending.borrow_mut().next() else {
                return;
            };
            let op = OpKey::session(&key);
            operations.update(cx, |ops, cx| ops.release(&op, cx));
            if !eligible_member(&key, Some(request.target), cx) {
                continue;
            }
            if menu::refused(&key, cx) {
                super::bulk::not_started(&key, Work::SessionGroup, "Move to group", cx);
                continue;
            }
            let Ok(command) = membership(&key, request) else {
                continue;
            };
            let batch = self.clone();
            let moved = key.clone();
            let started = operations.update(cx, |ops, cx| {
                ops.start(&op, command, cx, move |settled, cx| {
                    if let Some(list) = &batch.dropped_in {
                        pin_drag::crossed_after(
                            list,
                            &moved,
                            request.target,
                            request.pin_source.is_some(),
                            None,
                            settled,
                            cx,
                        );
                    }
                    batch.advance(cx);
                })
            });
            match started {
                Ok(()) | Err(NotStarted::Retired) => return,
                Err(NotStarted::Busy(_)) => {
                    super::bulk::not_started(&key, Work::SessionGroup, "Move to group", cx)
                }
                Err(NotStarted::Path(_)) => {}
            }
        }
    }
}

/// Skip ineligible or already-correct members before reporting busy rows.
/// A new group has no destination to compare yet.
pub(super) fn eligible_member(key: &str, target: Option<SessionGroupTarget>, cx: &App) -> bool {
    session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<SessionRow>(key)
            .is_some_and(|row| {
                target.is_none_or(|target| row.group != target.group()) && !super::retired::is(row)
            })
    })
}

// List layout

/// Separate the leading global pins from the rest of the ungrouped rows.
/// While held, a session can still occupy its previous band.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Band {
    GlobalPins,
    Group(i64),
    Tail,
}

impl Band {
    /// The band a row occupies with no order hold.
    pub(super) fn served(row: &Listed) -> Self {
        match row.row.group {
            Some(id) => Self::Group(id),
            None if row.row.pinned => Self::GlobalPins,
            None => Self::Tail,
        }
    }
}

/// A single group at its displayed position with all its matching members.
pub(super) struct Block<'a> {
    pub(super) group: SessionGroup,
    pub(super) rows: Vec<Listed<'a>>,
}

/// The complete displayed layout before collapse. A hidden group's members
/// still have held slots, so an expand can reveal them in place.
#[derive(Default)]
pub(super) struct Layout<'a> {
    pub(super) global: Vec<Listed<'a>>,
    pub(super) groups: Vec<Block<'a>>,
    pub(super) tail: Vec<Listed<'a>>,
}

impl<'a> Layout<'a> {
    /// Place rows into the selected bands, with headers following the
    /// supplied group order. Empty blocks never get a header.
    pub(super) fn partition<'b>(
        rows: impl IntoIterator<Item = Listed<'a>>,
        roster: impl IntoIterator<Item = &'b SessionGroup>,
        band: impl Fn(&Listed<'a>) -> Band,
    ) -> Self {
        let mut layout = Self::default();
        let mut members: HashMap<i64, Vec<_>> = HashMap::new();
        for row in rows {
            match band(&row) {
                Band::GlobalPins => layout.global.push(row),
                Band::Group(id) => members.entry(id).or_default().push(row),
                Band::Tail => layout.tail.push(row),
            }
        }
        for group in roster {
            if members.is_empty() {
                break;
            }
            if let Some(rows) = members.remove(&group.id) {
                layout.groups.push(Block {
                    group: group.clone(),
                    rows,
                });
            }
        }
        // A row can be carried separately by the channel. If its group
        // metadata is missing, keep the row reachable in the ordinary list.
        let mut unrepresented: Vec<_> = members.into_values().flatten().collect();
        unrepresented.sort_by_key(|row| row.key);
        layout.tail.extend(unrepresented);
        layout
    }

    /// Apply the same stable pin partition to each band's served order.
    pub(super) fn served(rows: Vec<Listed<'a>>, roster: &[SessionGroup]) -> Self {
        let mut layout = Self::partition(rows, roster, Band::served);
        layout.global = pins::pinned_first(layout.global);
        for block in &mut layout.groups {
            block.rows = pins::pinned_first(std::mem::take(&mut block.rows));
        }
        layout
    }

    /// Members in this frame's band and row order, including collapsed
    /// members. The order hold must capture them before collapse is applied.
    pub(super) fn members(&self) -> impl Iterator<Item = (Band, Listed<'a>)> + '_ {
        self.global
            .iter()
            .map(|row| (Band::GlobalPins, *row))
            .chain(self.groups.iter().flat_map(|block| {
                block
                    .rows
                    .iter()
                    .map(|row| (Band::Group(block.group.id), *row))
            }))
            .chain(self.tail.iter().map(|row| (Band::Tail, *row)))
    }
}

/// The virtual list, marks and move targets all read one layout. Collapse
/// removes members from these projections without changing their held slots.
pub(super) struct Frame<'a> {
    pub(super) keys: Vec<String>,
    pub(super) rows: Vec<Listed<'a>>,
    pub(super) headers: HashMap<String, Header>,
    /// Displayed membership can lag a helm update while the list holds its order.
    pub(super) member_groups: HashMap<String, i64>,
    pub(super) separators: HashMap<String, pins::Separator>,
    pub(super) moves: HashMap<String, pins::Neighbors>,
    pub(super) drag: pin_drag::Targets,
}

impl<'a> Frame<'a> {
    /// Build the visible projection and every insertion band together. The
    /// same list items that show the drop line record its usable bounds.
    pub(super) fn new(
        layout: Layout<'a>,
        roster: &[SessionGroup],
        original_pins: Option<&std::collections::HashSet<String>>,
        collapse: &CollapseWrites,
        counts: Option<farhelm_proto::ui_channel::ListCounts>,
        open: Option<Listed<'a>>,
        cx: &App,
    ) -> Self {
        // While a hold keeps a displayed member in its former band, follow that band. A filtered-out open row still
        // comes through the channel outside the order and can mark its group when other members match the filters.
        let open_group = open.and_then(|open| {
            let band = layout
                .members()
                .find(|(_, member)| member.key == open.key)
                .map(|(band, _)| band)
                .unwrap_or_else(|| Band::served(&open));
            match band {
                Band::Group(id) => Some((id, open.row.info.title.as_str())),
                Band::GlobalPins | Band::Tail => None,
            }
        });
        let mut frame = Self {
            keys: Vec::new(),
            rows: Vec::new(),
            headers: HashMap::new(),
            member_groups: HashMap::new(),
            separators: HashMap::new(),
            moves: HashMap::new(),
            drag: pin_drag::Targets::default(),
        };
        let has_groups = !layout.groups.is_empty();
        frame.add_rows(
            &layout.global,
            Band::GlobalPins,
            original_pins,
            has_groups,
            None,
        );
        let mut headers = Vec::new();
        for Block { group, rows } in layout.groups {
            let key = header_key(group.id);
            let collapsed = Collapse::shown(collapse, &group, cx);
            let matching = rows.len() as u64;
            let count = counts.is_none_or(|c| !c.truncated).then(|| {
                let denominator = counts
                    .and_then(|c| c.matching)
                    .map(|_| group.members.max(matching));
                super::complete_count(matching, denominator)
            });
            let attention = collapsed.then(|| Attention::highest(&rows)).flatten();
            let open_member = open_group
                .filter(|(id, _)| collapsed && *id == group.id)
                .map(|(_, title)| HostText::from_host(title));
            let live = roster.iter().any(|live| live.id == group.id);
            frame.headers.insert(
                key.clone(),
                Header {
                    id: group.id,
                    name: group.name,
                    collapsed,
                    live,
                    count,
                    attention,
                    open_member,
                },
            );
            if live {
                headers.push(key.clone());
            }
            let header_index = frame.keys.len();
            frame.keys.push(key.clone());
            if !collapsed {
                let edge = live.then_some(pin_drag::Target {
                    neighbor: key,
                    placement: pins::Placement::After,
                });
                frame.add_rows(&rows, Band::Group(group.id), original_pins, false, edge);
            }
            if live {
                frame.drag.membership.push(pin_drag::MembershipBand {
                    target: SessionGroupTarget::Group(group.id),
                    items: frame.keys[header_index..].to_vec(),
                    append_pin: rows
                        .iter()
                        .filter(|row| !collapsed && !row.row.pinned)
                        .map(|row| row.key.to_owned())
                        .collect(),
                });
            }
        }
        if let Some(key) =
            pins::separator_after(&frame.keys, has_groups && !layout.tail.is_empty(), |_| true)
        {
            frame
                .separators
                .insert(key.clone(), pins::Separator::Groups);
        }
        frame.moves.extend(pins::ordered_neighbors(
            &headers.iter().map(String::as_str).collect::<Vec<_>>(),
        ));
        if let (Some(first), Some(global)) = (headers.first(), frame.drag.bands.first_mut())
            && !has_visible_pins(&layout.global, Band::GlobalPins)
        {
            global.empty = Some(pin_drag::Target {
                neighbor: first.clone(),
                placement: pins::Placement::Before,
            });
        }
        frame.drag.bands.push(pin_drag::Band {
            area: pin_drag::Area::Groups,
            items: headers,
            empty: None,
        });
        frame.add_rows(&layout.tail, Band::Tail, original_pins, false, None);
        let ungrouped: Vec<_> = layout
            .tail
            .iter()
            .filter(|row| !row.row.pinned)
            .map(|row| row.key.to_owned())
            .collect();
        if !ungrouped.is_empty() {
            frame.drag.membership.push(pin_drag::MembershipBand {
                target: SessionGroupTarget::Ungrouped,
                items: ungrouped,
                append_pin: Vec::new(),
            });
        }
        // With no headers between them, the global band and tail are one
        // contiguous run for the divider. A newly pinned row can touch the
        // run without becoming a movable pin during the hold.
        if !has_groups
            && let Some(row) = pins::separator_after(&frame.rows, false, |row| row.row.pinned)
        {
            frame
                .separators
                .insert(row.key.to_owned(), pins::Separator::GlobalPins);
        }
        frame
    }

    /// A drag of an unpinned group member still needs an ungrouping target if the list ends at a group.
    pub(super) fn offer_ungroup_strip(&mut self) {
        if !self
            .drag
            .membership
            .iter()
            .any(|band| band.target == SessionGroupTarget::Ungrouped)
        {
            self.keys.push(pin_drag::UNGROUP_STRIP.to_owned());
            self.drag.membership.push(pin_drag::MembershipBand {
                target: SessionGroupTarget::Ungrouped,
                items: vec![pin_drag::UNGROUP_STRIP.to_owned()],
                append_pin: Vec::new(),
            });
        }
    }

    /// A pin's keyboard neighbors come only from the band's leading
    /// movable run. A held row whose group changed cannot anchor a move.
    fn add_rows(
        &mut self,
        rows: &[Listed<'a>],
        band: Band,
        original: Option<&std::collections::HashSet<String>>,
        follows: bool,
        empty_edge: Option<pin_drag::Target>,
    ) {
        self.rows.extend_from_slice(rows);
        self.keys.extend(rows.iter().map(|row| row.key.to_owned()));
        self.drag.unpinned.extend(
            rows.iter()
                .filter(|row| !row.row.pinned && Band::served(row) == band)
                .map(|row| (row.key.to_owned(), row.row.group.into())),
        );
        let (separator, area) = match band {
            Band::GlobalPins => (pins::Separator::GlobalPins, SessionGroupTarget::Ungrouped),
            Band::Group(id) => {
                self.member_groups
                    .extend(rows.iter().map(|row| (row.key.to_owned(), id)));
                (
                    pins::Separator::GroupPins(id),
                    SessionGroupTarget::Group(id),
                )
            }
            Band::Tail => return,
        };
        if let Some(row) = pins::separator_after(rows, follows, |row| row.row.pinned) {
            self.separators.insert(row.key.to_owned(), separator);
        }
        let moves = pins::neighbors(rows, original, band);
        let movable = rows
            .iter()
            .filter(|row| moves.contains_key(row.key))
            .map(|row| row.key.to_owned())
            .collect();
        let empty = if has_visible_pins(rows, band) {
            None
        } else {
            empty_edge
        };
        self.moves.extend(moves);
        self.drag.bands.push(pin_drag::Band {
            area: pin_drag::Area::Pins(area),
            items: movable,
            empty,
        });
    }
}

/// A pinned row held in its former group is neither a neighbor nor
/// evidence that the band still has a visible pin.
fn has_visible_pins(rows: &[Listed], band: Band) -> bool {
    rows.iter()
        .any(|row| row.row.pinned && Band::served(row) == band)
}

// Route commands

/// A move via the menu or a drop with no visible pin neighbor uses the
/// same membership write. Only drag carries an expected pinned source.
pub(super) fn membership(key: &str, request: SessionGroupMembership) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::SessionGroup,
        Method::Put,
        ApiPath::api(&["sessions", key, "group"])?,
        Some(json!(request)),
        "PUT /api/sessions/[session]/group",
    ))
}

/// One write for the entire marked selection so there is one group even
/// when it has several members, and a refusal leaves all of them in place.
pub(super) fn create(name: &str, sessions: &[String]) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::SessionGroup,
        Method::Post,
        ApiPath::api(&["session-groups"])?,
        Some(json!(NewSessionGroup {
            name: name.to_owned(),
            sessions: sessions.to_vec()
        })),
        "POST /api/session-groups",
    ))
}

/// Compose a sparse shared write so collapse and rename from different
/// windows can settle in either order.
pub(super) fn update(id: i64, patch: UpdateSessionGroup) -> Result<Command, PathError> {
    Ok(Command::new(
        if patch.name.is_some() {
            Work::Rename
        } else {
            Work::SessionGroup
        },
        Method::Patch,
        ApiPath::api(&["session-groups", &id.to_string()])?,
        Some(json!(patch)),
        "PATCH /api/session-groups/[group]",
    ))
}

/// Ungroup retains members and any pins, so it needs no session deletes.
pub(super) fn ungroup(id: i64) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::SessionGroup,
        Method::Delete,
        ApiPath::api(&["session-groups", &id.to_string()])?,
        None,
        "DELETE /api/session-groups/[group]",
    ))
}
