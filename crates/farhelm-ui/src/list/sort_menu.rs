//! The list title also owns its shared ordering and density choices.
//!
//! Keep this menu mounted with the header: its focus queue and asynchronous
//! measurements must outlive the flyout they serve. The common menu helpers
//! supply the same keyboard, outside-pointer and focus rules as row menus.

use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;

use crate::anchored_popovers::{AnchorRegion, Interest, MovementOwner, use_registration};
use crate::api::ListSort;
use crate::icons::{ActionGlyph, ActionIcon};
use crate::menu_panel::{
    MenuFocusQueue, MenuHeader, MenuItemCopy, MenuOpenIntent, MenuOrder, MenuSeparator, MenuWiring,
    PanelPlacement, ROW_MENU_OUTSIDE_RELAY, cancel_menu_focus, closed_toggle_key_intent,
    focus_menu_toggle, forget_menu_focus, handle_menu_key, install_row_menu_outside_dismiss,
    measurement_outcome, menu_item_focus_key, record_menu_item_focus, remember_menu_item,
    session_menu_placement_style, session_menu_pointer_style, should_measure_on_mount,
    use_menu_instance,
};

/// Stable identities let focus follow an action independently of its checked state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Choice {
    Activity,
    Created,
    Title,
    Compact,
}

impl Choice {
    /// Compact changes density; the other choices use the helm's existing order keys.
    fn sort(self) -> Option<ListSort> {
        match self {
            Self::Activity => Some(ListSort::Activity),
            Self::Created => Some(ListSort::Created),
            Self::Title => Some(ListSort::Title),
            Self::Compact => None,
        }
    }

    /// User-facing order names stay separate from the wire vocabulary.
    fn label(self) -> &'static str {
        match self {
            Self::Activity => "Recently active",
            Self::Created => "Newest created",
            Self::Title => "Title A–Z",
            Self::Compact => "Compact",
        }
    }

    /// Explain the distinction without implying that output recency means liveness.
    fn description(self) -> &'static str {
        match self {
            Self::Activity => "Active sessions first, ordered by when work started",
            Self::Created => "Most recently created sessions first",
            Self::Title => "Session titles in alphabetical order",
            Self::Compact => "One line per session, with the folder shown on hover",
        }
    }
}

/// A title menu changes the existing shared preferences, never a client-side order.
#[component]
pub(super) fn SortMenu(
    sort: ListSort,
    compact: bool,
    layout_epoch: ReadSignal<u64>,
    on_sort: EventHandler<ListSort>,
    on_compact: EventHandler<bool>,
) -> Element {
    let mut open = use_signal(|| false);
    let mut opened_epoch = use_signal(|| *layout_epoch.peek());
    let mut toggle_handle = use_signal(|| None::<Rc<MountedData>>);
    let mut placement = use_signal(|| PanelPlacement::Unmeasured);
    let mut handles = use_signal(HashMap::new);
    let mut focused = use_signal(|| None);
    let mut requested = use_signal(|| None);
    let mut intent = use_signal(|| None);
    let mut generation = use_signal(|| 0_u64);
    let focus = MenuFocusQueue {
        target: use_signal(|| None),
        in_flight: use_signal(|| None),
        draining: use_signal(|| false),
    };
    let order = MenuOrder::pack(
        [
            Choice::Activity,
            Choice::Created,
            Choice::Title,
            Choice::Compact,
        ],
        |_| true,
    );
    let wiring: MenuWiring<Choice, (), 4> = MenuWiring {
        order,
        handles,
        focus,
        generation,
        focused,
        requested,
        open_intent: intent,
        close_menu: use_callback(move |()| open.set(false)),
        instance: use_menu_instance(),
    };
    let registration = use_registration(
        use_context::<MovementOwner>(),
        Interest::new(AnchorRegion::Sessions),
        move || (*open.peek()).then(|| "list:sort".to_string()),
        move || wiring.close_menu.call(()),
    );
    use_effect(move || {
        let epoch = layout_epoch();
        if *open.peek() && epoch != *opened_epoch.peek() {
            registration.close_if_open();
        }
    });
    use_effect(move || {
        if open() {
            return;
        }
        cancel_menu_focus(focus);
        let was_inside = focused.peek().is_some();
        focused.set(None);
        requested.set(None);
        intent.set(None);
        handles.write().clear();
        if was_inside {
            focus_menu_toggle("data-row-menu-key", "list:sort", ".sort-title");
        }
    });

    // Measurements belong to one opening. A late reply from its predecessor
    // cannot replace the new opening's placement or revive a closed flyout.
    let measure = move || {
        let measured_generation = *generation.peek();
        spawn(async move {
            let measured = match toggle_handle.peek().clone() {
                Some(handle) => handle.get_client_rect().await.ok(),
                None => None,
            };
            if let Some(outcome) =
                measurement_outcome(measured_generation, *generation.peek(), measured)
            {
                placement.set(outcome);
            }
        });
    };
    let mut begin_open = move |next_intent| {
        generation += 1;
        placement.set(PanelPlacement::Unmeasured);
        handles.write().clear();
        cancel_menu_focus(focus);
        focused.set(None);
        requested.set(None);
        intent.set(Some(next_intent));
        opened_epoch.set(*layout_epoch.peek());
        install_row_menu_outside_dismiss();
        open.set(true);
        measure();
    };
    let label = super::view::SORT_OPTIONS
        .into_iter()
        .find(|(candidate, _)| *candidate == sort)
        .map(|(_, label)| label)
        .expect("every offered order has a title");
    let tab_stop = focused()
        .and_then(|position| order.get(position))
        .or_else(|| order.get(0));

    rsx! {
        span { class: "sort-title-owner", "data-row-menu-key": "list:sort",
            button {
                r#type: "button", class: "btn btn-ghost sort-title bar-menu-toggle",
                aria_label: "Sort: {label}", "data-sort": sort.key(),
                "data-tooltip": "Sort: the order of the session list, shared by every client",
                aria_haspopup: "menu", aria_expanded: open(),
                onmounted: move |element| {
                    toggle_handle.set(Some(element.data()));
                    if should_measure_on_mount(open(), *placement.peek()) { measure(); }
                },
                onfocusin: move |_| forget_menu_focus(wiring),
                onkeydown: move |evt| {
                    if !open() {
                        if let Some(next) = closed_toggle_key_intent(&evt.key()) {
                            evt.prevent_default();
                            begin_open(next);
                        }
                    } else { handle_menu_key(&evt, None, wiring, &()); }
                },
                onclick: move |_| {
                    if open() { open.set(false); } else { begin_open(MenuOpenIntent::First); }
                },
                "{label}"
                ActionIcon { glyph: ActionGlyph::ChevronDown }
            }
            button {
                r#type: "button", class: ROW_MENU_OUTSIDE_RELAY,
                "data-row-menu": "list:sort", hidden: true, tabindex: "-1",
                onclick: move |_| open.set(false),
            }
        }
        if open() {
            div {
                class: "host-row-menu-flyout sort-menu-flyout",
                "data-row-menu-key": "list:sort",
                style: session_menu_placement_style(placement()),
                if let Some(style) = session_menu_pointer_style(placement()) {
                    span { class: "host-row-menu-pointer", style, "aria-hidden": "true" }
                }
                // These shared classes enroll this menu in the existing outside
                // dismissal and guarded focus restoration, without a new listener.
                div { class: "host-row-menu-panel bar-menu-panel sort-menu-panel",
                    MenuHeader {
                        title: rsx! { "Session list" },
                        summary: rsx! { "Order and density, shared by every client" },
                    }
                    div { class: "host-row-menu-items session-row-menu-items", role: "menu", aria_label: "Sort",
                        for choice in order.actions() {
                            if choice == Choice::Compact { MenuSeparator {} }
                            button {
                                r#type: "button",
                                class: "btn menu-item session-row-menu-item host-row-menu-item sort-menu-item",
                                role: if choice == Choice::Compact { "menuitemcheckbox" } else { "menuitemradio" },
                                aria_checked: choice.sort().map_or(compact, |candidate| candidate == sort),
                                "data-menu-focus": menu_item_focus_key(wiring, choice),
                                "data-tooltip": choice.description(),
                                tabindex: if tab_stop == Some(choice) { "0" } else { "-1" },
                                onmounted: move |element| remember_menu_item(wiring, choice, element.data()),
                                onfocusin: move |_| record_menu_item_focus(wiring, choice),
                                onfocusout: move |_| focused.set(None),
                                onkeydown: move |evt| handle_menu_key(&evt, order.position(choice), wiring, &()),
                                onclick: move |_| {
                                    open.set(false);
                                    if let Some(next) = choice.sort() { on_sort.call(next); }
                                    else { on_compact.call(!compact); }
                                },
                                ActionIcon { glyph: if choice == Choice::Compact { ActionGlyph::Sessions } else { ActionGlyph::Adjust }, class: "menu-icon session-row-menu-icon" }
                                MenuItemCopy {
                                    label: rsx! { "{choice.label()}" },
                                    description: rsx! { "{choice.description()}" },
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
