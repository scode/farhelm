//! The list's order: the heading's title, which names the order the list is in and opens the menu that changes it.
//!
//! SPEC.md, "Session list": "The chosen order is the list's title: **Recently active**, **Newest created** or **Title
//! A–Z**, with a menu offering those orders", and the order is "one preference shared by every client". Ported from the
//! old app's `list/sort_menu.rs` (the title toggle, the menu and its keys), `list/view.rs` (`apply_sort`,
//! `remember_sort`) and the stylesheet's `.sort-title` and `.sort-menu-item` rules at the reference commit.
//!
//! **The helm sorts.** The list draws the sessions in the order the helm pushes them, and the UI channel serves them in
//! the stored order for as long as the list's own query names no sort (SPEC_impl.md, "The UI channel"; the query in
//! [`super::filters`] never does). So choosing an order is a preference write and nothing else: a sparse `PUT
//! /api/preferences` with `{"list_sort": <word>}`, as Settings writes a preference, saved through a [`Draft`] under
//! the preference row's key ([`OpKey::preferences`]). The helm re-sorts the pushed list as part of that write, before
//! its reply, and an order chosen in another window or by the command line reaches this one the same way. Re-choosing
//! the order in force sends nothing.
//!
//! **What the title says.** While the write is in flight the title names the order being saved; at every other moment
//! it names the stored order, an unknown or missing word reading as Recently active (the helm's own default for the
//! channel). There is no copy of the order on this side.
//!
//! **A refused write is silent.** SPEC.md, "Errors and diagnostics", counts the list's order among the best-effort
//! preferences: a failed save is logged and nothing is shown, and the choice stays in effect only until the next read
//! from the helm. The old page re-sorted itself and kept that order until its next read. Under the channel the next
//! read is immediate, since the helm pushes the list in the stored order before it answers, so a refused write leaves
//! the list and the title in the stored order at once, and only the log says why (decided by the migration's
//! orchestrator on 2026-11-14). The operation store still records the refusal under the preference row's key, as for
//! any work on a key; nothing in the list draws it, and the Templates dialog, the one view that reads that key's
//! refusal, shows only one that its own write in the same opening left.
//!
//! **A choice while another write is in flight is dropped.** The operation store admits one request at a time on the
//! preference row's key, which Settings, the Templates dialog and this menu share, and the order's own draft refuses
//! an edit while its save is in flight. Either way the choice is logged and the title keeps the order it showed, as
//! Settings drops a click on its switches meanwhile (decided by the migration's orchestrator on 2026-11-14).
//!
//! **The menu.** The title is a ghost button named "Sort: <order>" that reports whether its menu is open. Its menu is a
//! popover of the layer manager below the title: the shared [`MenuHeader`] ("Session list"), then the three orders as
//! radio items with their descriptions, the one in force checked. A click opens it on its first item, and Down or Up on
//! the closed title on its first or last; inside it the layer manager's Up and Down walk the items, and Home and End
//! (the menu's own keys) go to the first and last. Escape closes it and focus goes back to the title, which the menu
//! names as its return target ([`focus::ReturnTo::Control`]), so the same happens on macOS, where the click that opened
//! it left the title unfocused. Choosing closes it the same way, so focus stays on the title under its new name.
//!
//! The order state belongs to the UI session (`super::Current`), an entity so the draft's save can reach it when the
//! reply arrives and the open menu can read it while it draws: a reset starts it over, with the helm's channel.
//!
//! NOTE: Not ported yet: Compact, the old menu's fourth item, which waits for compact rows so that no item changes
//! nothing visible; placing the menu beside the sidebar with a pointer notch (the layer manager places it below its
//! anchor, as for a row's menu); and `aria-haspopup`, which GPUI cannot report. Tab stays inside the open menu, as
//! SPEC.md's "Keyboard focus" has every menu keep focus, where the old Tab on an item closed the menu (a parity
//! deviation).

#[cfg(test)]
mod tests;

use farhelm_helm_client::{ApiPath, Collections, Method, PathError};
use farhelm_proto::ui_channel::Preferences;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement as _, Styled, WeakEntity,
    Window, div, px, svg,
};

use super::{SessionList, selection};
use crate::assets::Icon;
use crate::components::{Button, HoverHelp, MenuHeader, MenuItem, Role};
use crate::drafts::{self, Draft};
use crate::focus::{self, Mark};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::session;
use crate::theme::{self, metrics};

/// The title button's element id, which tests and the test bridge find it by.
pub(crate) const TITLE_ID: &str = "session-order";

/// The title's hover help, the old toggle's `data-tooltip`.
pub(crate) const TITLE_HELP: &str = "Sort: the order of the session list, shared by every client";

/// The menu's header, the old `MenuHeader`'s words. The summary still says "density": the old menu's Compact item comes
/// back with compact rows, and the header is the old one until then.
pub(crate) const MENU_TITLE: &str = "Session list";
pub(crate) const MENU_SUMMARY: &str = "Order and density, shared by every client";

/// The menu's accessible name, the old `div[role=menu][aria-label="Sort"]`'s.
pub(crate) const MENU_NAME: &str = "Sort";

/// The menu's width: the old flyout's, which it shared with a row's menu (`.host-row-menu-flyout`).
const MENU_WIDTH: f32 = 246.;

/// `.sort-title`'s gap between the order's name and its chevron, and the chevron's side (`.sort-title .action-glyph`).
const TITLE_GAP: f32 = 4.;
const CHEVRON: f32 = 12.;

/// One order of the list. The declaration order is the menu's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Order {
    /// Most recent activity first: the helm's default, and what an unknown stored word reads as.
    Activity,
    /// Newest created first.
    Created,
    /// By title.
    Title,
}

/// What an order is called and stored as, one row per [`Order`] in its order.
struct Row {
    /// The word the preference row stores (the `?sort=` vocabulary).
    word: &'static str,
    /// The order's name: the title's text, the menu item's label.
    label: &'static str,
    /// The menu item's description line, which is its hover help too (the old `sort_menu.rs`'s).
    description: &'static str,
    /// The menu item's element id.
    id: &'static str,
}

/// The orders' rows, indexed by [`Order`] as declared.
const ROWS: [Row; 3] = [
    Row {
        word: "activity",
        label: "Recently active",
        description: "Active sessions first, ordered by when work started",
        id: "session-order-activity",
    },
    Row {
        word: "created",
        label: "Newest created",
        description: "Most recently created sessions first",
        id: "session-order-created",
    },
    Row {
        word: "title",
        label: "Title A–Z",
        description: "Session titles in alphabetical order",
        id: "session-order-title",
    },
];

impl Order {
    /// Every order, in the menu's order.
    pub(crate) const ALL: [Order; 3] = [Order::Activity, Order::Created, Order::Title];

    fn row(self) -> &'static Row {
        &ROWS[self as usize]
    }

    /// The word the preference row stores for this order.
    pub(crate) fn word(self) -> &'static str {
        self.row().word
    }

    /// The order's name, as the title and the menu show it.
    pub(crate) fn label(self) -> &'static str {
        self.row().label
    }

    /// The menu item's description, which is also its hover help.
    pub(crate) fn description(self) -> &'static str {
        self.row().description
    }

    /// The menu item's element id.
    pub(crate) fn item_id(self) -> &'static str {
        self.row().id
    }

    /// The order the helm stored in `preferences`: a word this build does not know, or none, is most recent activity,
    /// as for the channel (SPEC_impl.md, "Shared preferences and announcements": the row outlives the build that
    /// validated it).
    pub(crate) fn stored(preferences: Option<&Preferences>) -> Order {
        match preferences.and_then(|preferences| preferences.list_sort.as_deref()) {
            Some("created") => Order::Created,
            Some("title") => Order::Title,
            _ => Order::Activity,
        }
    }

    /// The order stored in `collections`' preferences.
    pub(crate) fn stored_in(collections: &Collections) -> Order {
        Order::stored(selection::preferences(collections))
    }
}

/// `PUT /api/preferences` naming only `list_sort`, set to `order`'s word: a sparse write, so it never puts back a stale
/// copy of a field another client changed.
fn command(order: Order) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Save,
        Method::Put,
        ApiPath::api(&["preferences"])?,
        Some(serde_json::json!({ "list_sort": order.word() })),
        "PUT /api/preferences",
    ))
}

/// The order's state in one UI session: the draft its write is saved through, and the menu the title last opened.
pub(crate) struct OrderState {
    /// The order being saved while a write is in flight. Its value means nothing at any other moment: the title then
    /// reads the store.
    draft: Draft<Order>,
    /// The menu the title opened, checked with the layer manager when read, so a menu closed some other way (Escape, a
    /// press outside, the title no longer drawn) needs no bookkeeping.
    menu: Option<LayerId>,
}

impl OrderState {
    /// The order the title names and the menu checks, `stored` being the helm's: the one being saved while its write
    /// is in flight, else the stored one (see the module docs).
    fn shown(&self, stored: Order) -> Order {
        if self.draft.is_saving() {
            *self.draft.value()
        } else {
            stored
        }
    }
}

/// Which item a menu opened from the title starts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Start {
    /// The first order: a click, or Down on the closed title.
    First,
    /// The last order: Up on the closed title.
    Last,
}

impl SessionList {
    /// The current UI session's order state, made the first time it is asked for under the session.
    pub(super) fn order_state(&mut self, cx: &mut Context<Self>) -> Entity<OrderState> {
        let current = self.current.current(cx);
        if let Some(state) = &current.order {
            return state.clone();
        }
        let state = cx.new(|_| OrderState {
            draft: Draft::load(Order::Activity),
            menu: None,
        });
        current.order = Some(state.clone());
        state
    }

    /// Whether the title's menu is open.
    pub(crate) fn order_menu_open(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let state = self.order_state(cx);
        state
            .read(cx)
            .menu
            .is_some_and(|menu| layers::is_open(menu, window, cx))
    }

    /// The title was activated (a click, or Enter or Space while it has focus): open its menu on its first item, or
    /// close it if it is open. Visible to the crate for the screenshot goldens, which open the menu without a click.
    pub(crate) fn toggle_order_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.order_menu_open(window, cx) {
            let state = self.order_state(cx);
            if let Some(menu) = state.read(cx).menu {
                layers::close(menu, window, cx);
            }
            return;
        }
        self.open_order_menu(Start::First, window, cx);
    }

    /// Open the title's menu with focus on the item `start` names, unless it is open already or the title is not
    /// drawn. Nothing opens while the window is inert, or once the user has moved on since the key or click that asked.
    fn open_order_menu(&mut self, start: Start, window: &mut Window, cx: &mut Context<Self>) {
        if self.order_menu_open(window, cx) {
            return;
        }
        let state = self.order_state(cx);
        let Some(anchor) = self.order_anchor.clone() else {
            return;
        };
        let mark = Mark::take(cx);
        let items: [FocusHandle; 3] = std::array::from_fn(|_| cx.focus_handle().tab_stop(true));
        let start = match start {
            Start::First => items[0].clone(),
            Start::Last => items[2].clone(),
        };
        let list = cx.weak_entity();
        let view = cx.new(|_| OrderMenu {
            list,
            state: state.clone(),
            items,
        });
        let opened = layers::open_popover(
            Popover {
                view: view.into(),
                container: cx.focus_handle(),
                start: Some(start),
                anchor,
                // The title, not what held focus before: on macOS the click that opened the menu left the title
                // unfocused, and the old menu returned focus to the title when it closed.
                returns_to: focus::ReturnTo::Control(self.order_focus.clone()),
                // Tab stays inside the open menu (SPEC.md, "Keyboard focus": "Inside a dialog or menu, focus stays
                // inside"), where the old sort menu closed on Tab; a parity deviation, since SPEC names Tab closing a
                // menu only for the session view's caret menus.
                traps_tab: true,
            },
            &mark,
            window,
            cx,
        );
        state.update(cx, |state, _| state.menu = opened);
        cx.notify();
    }

    /// The user chose `order` in the menu: close the menu, which returns focus to the title, then save the order
    /// unless it is the one shown already. A choice that cannot be saved now (another preference write in flight, or no
    /// UI session) is logged and changes nothing, and a refused save is logged and shows nothing (the module docs).
    fn choose_order(&mut self, order: Order, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.order_state(cx);
        if let Some(menu) = state.read(cx).menu {
            layers::close(menu, window, cx);
        }
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let stored = Order::stored_in(store.read(cx).collections());
        if order == state.read(cx).shown(stored) {
            return;
        }
        state.update(cx, |state, cx| {
            if !state.draft.edit(order) {
                tracing::debug!(
                    ?order,
                    "the list's order was not saved: its last save is in flight"
                );
                return;
            }
            let started = drafts::save(
                state,
                |state: &mut OrderState| &mut state.draft,
                &OpKey::preferences(),
                |order| command(*order),
                cx,
                move |_, settled, _| {
                    if let Outcome::Refused(refusal) = &settled.outcome {
                        let words = &refusal.words;
                        tracing::warn!(?order, ?words, "the helm refused the list's order");
                    }
                },
            );
            if let Err(not_started) = started {
                tracing::debug!(?order, ?not_started, "the list's order was not saved");
            }
        });
        cx.notify();
    }

    /// The heading's title for the order shown, `stored` being the helm's: a ghost button named "Sort: <order>" showing
    /// the order's name and a chevron (`.sort-title`), reporting whether its menu is open, with the anchor its menu
    /// opens below. Drawn while the list draws its heading; it records the anchor for the title's handlers.
    pub(super) fn order_title(
        &mut self,
        stored: Order,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.order_state(cx);
        let anchor = Anchor::for_element("session-order-anchor", window, cx);
        self.order_anchor = Some(anchor.clone());
        let order = state.read(cx).shown(stored);
        let open = self.order_menu_open(window, cx);
        let palette = theme::palette(cx);
        // The open title wears the accent fill with the inverse ink (`.sort-title[aria-expanded="true"]`); at rest its
        // words are the primary ink, where a plain ghost button's are the quieter one.
        let ink = theme::color(if open { palette.on_fill } else { palette.fg_0 });
        let content = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(TITLE_GAP))
            .whitespace_nowrap()
            .text_size(metrics::TEXT_MD)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(ink)
            .child(order.label())
            .child(
                svg()
                    .path(Icon::ChevronDown.path())
                    .size(px(CHEVRON))
                    .flex_none()
                    .text_color(ink),
            );
        let (clicked, keyed) = (cx.weak_entity(), cx.weak_entity());
        let title = Button::new(
            TITLE_ID,
            format!("Sort: {}", order.label()),
            HoverHelp::tooltip(TITLE_HELP),
            Role::Button,
        )
        .heading()
        .expanded(open)
        .focus(&self.order_focus)
        .content(content)
        .on_key_down(move |event, window, cx| {
            // Down and Up open the closed menu on its first or last order, as the old toggle did; an open menu holds
            // focus, so these never reach the title while it is open.
            let start = match event.keystroke.key.as_str() {
                "down" => Start::First,
                "up" => Start::Last,
                _ => return,
            };
            if event.keystroke.modifiers.modified() {
                return;
            }
            cx.stop_propagation();
            if event.is_held {
                return;
            }
            let _ = keyed.update(cx, |list, cx| list.open_order_menu(start, window, cx));
        })
        .on_click(move |_, window, cx| {
            // The list may be gone with its window; then there is no menu to open.
            let _ = clicked.update(cx, |list, cx| list.toggle_order_menu(window, cx));
        });
        // `.sort-title-owner`: the title may give up width to the count and the actions beside it.
        div()
            .relative()
            .min_w_0()
            .child(title)
            .child(anchor.track())
            .into_any_element()
    }
}

/// The open menu of orders: drawn from the store and the order state while it is open, so the checked order follows a
/// change another client makes meanwhile.
pub(crate) struct OrderMenu {
    list: WeakEntity<SessionList>,
    state: Entity<OrderState>,
    /// The orders' focus handles, in their order: the menu starts on the first or the last, and Home and End move
    /// there.
    items: [FocusHandle; 3],
}

impl Render for OrderMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stored = session::current_store(cx).map_or(Order::Activity, |store| {
            Order::stored_in(store.read(cx).collections())
        });
        let shown = self.state.read(cx).shown(stored);
        let items = Order::ALL.iter().zip(&self.items).map(|(order, focus)| {
            let (order, list) = (*order, self.list.clone());
            MenuItem::new(
                order.item_id(),
                Icon::Adjust,
                order.label(),
                HoverHelp::tooltip(order.description()),
                Role::MenuItemRadio,
            )
            .description(order.description())
            .checked(order == shown)
            .focus(focus)
            .on_click(move |_, window, cx| {
                let _ = list.update(cx, |list, cx| list.choose_order(order, window, cx));
            })
        });
        let (first, last) = (self.items[0].clone(), self.items[2].clone());
        layers::panel(cx)
            .id("session-order-menu")
            .debug_selector(|| "session-order-menu".into())
            .w(px(MENU_WIDTH))
            // Home and End, the old menu's, handled here rather than by the layer manager, whose menu context binds
            // only Up and Down: a move to another item of the open menu, through the focus coordinator.
            .on_key_down(move |event, window, cx| {
                let target = match event.keystroke.key.as_str() {
                    "home" => &first,
                    "end" => &last,
                    _ => return,
                };
                if event.keystroke.modifiers.modified() {
                    return;
                }
                cx.stop_propagation();
                let mark = Mark::take(cx);
                focus::focus_in_layer(target, &mark, window, cx);
            })
            .child(MenuHeader::app(MENU_TITLE, MENU_SUMMARY))
            .child(
                div()
                    .id("session-order-items")
                    .role(Role::Menu)
                    .aria_label(MENU_NAME)
                    .flex()
                    .flex_col()
                    .children(items),
            )
    }
}
