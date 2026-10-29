//! The toolbar's split-button menus reuse row-menu keyboard and focus rules.
//!
//! Lifecycle admission remains in SessionView. These menus choose an existing
//! action; opening one never takes the operation claim or authorizes a restart.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::icons::{ActionGlyph, ActionIcon};
use crate::menu_panel::{
    MenuFocusQueue, MenuHeader, MenuItemCopy, MenuOpenIntent, MenuOrder, MenuWiring,
    ROW_MENU_OUTSIDE_RELAY, cancel_menu_focus, closed_toggle_key_intent, focus_menu_toggle,
    forget_menu_focus, handle_menu_key, install_row_menu_outside_dismiss, menu_item_focus_key,
    record_menu_item_focus, remember_menu_item, use_menu_instance,
};

/// Each split button owns an independent menu and a persistent focus destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActionKind {
    Restart,
    Replace,
}

impl ActionKind {
    /// Namespace outside-click relays independently of a session row's menu.
    fn key(self) -> &'static str {
        match self {
            Self::Restart => "header:restart",
            Self::Replace => "header:replace",
        }
    }

    /// Name the operation, rather than any particular session's resume offer.
    fn label(self) -> &'static str {
        match self {
            Self::Restart => "Restart",
            Self::Replace => "Replace",
        }
    }

    /// Reuse the foundation's action vocabulary for the plain menu entry.
    fn glyph(self) -> ActionGlyph {
        match self {
            Self::Restart => ActionGlyph::Restart,
            Self::Replace => ActionGlyph::Replace,
        }
    }

    /// The launcher can return to this control after its menu item unmounts.
    fn toggle_class(self) -> &'static str {
        match self {
            Self::Restart => "restart-menu-toggle",
            Self::Replace => "replace-menu-toggle",
        }
    }

    /// Keep existing action selectors attached to their actual menu choices.
    fn with_class(self) -> &'static str {
        match self {
            Self::Restart => "restart-with-trigger",
            Self::Replace => "header-replace-with",
        }
    }
}

/// Plain and editable variants retain separate availability and explanations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Choice {
    Plain,
    With,
}

/// Menu descriptions use sentence case and no closing period, like the shared menu.
/// Only product-authored explanations enter here; this does not recase peer values.
fn menu_description(description: String) -> String {
    let description = description
        .replace("use Replace with", "use Replace with…")
        .replace("replace starts it over", "Replace starts it over");
    let description = description.trim_end_matches('.');
    let mut chars = description.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => description.to_string(),
    }
}

/// Offer the same admitted callbacks as the main halves of the split controls.
///
/// The caret stays mounted when the flyout closes, including while its With
/// action opens the launcher. A native-disabled busy state is separate from an
/// unavailable action: unavailable items remain keyboard reachable so their
/// reason is available without hovering a disabled native button.
#[component]
pub(super) fn ActionMenu(
    kind: ActionKind,
    mut open_menu: Signal<Option<ActionKind>>,
    busy: bool,
    plain_unavailable: bool,
    with_unavailable: bool,
    plain_description: String,
    with_description: String,
    on_plain: EventHandler<()>,
    on_with: EventHandler<()>,
) -> Element {
    let plain_description = menu_description(plain_description);
    let with_description = menu_description(with_description);
    let open = move || open_menu() == Some(kind);
    let close = use_callback(move |()| {
        if *open_menu.peek() == Some(kind) {
            open_menu.set(None);
        }
    });
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
    let order = MenuOrder::pack([Choice::Plain, Choice::With], |_| true);
    let wiring: MenuWiring<Choice, (), 2> = MenuWiring {
        order,
        handles,
        focus,
        generation,
        focused,
        requested,
        open_intent: intent,
        close_menu: close,
        instance: use_menu_instance(),
    };
    use_effect(use_reactive((&busy,), move |(busy,)| {
        if busy {
            close.call(());
        }
    }));
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
            focus_menu_toggle(
                "data-row-menu-key",
                kind.key(),
                &format!(".{}", kind.toggle_class()),
            );
        }
    });
    let mut begin_open = move |next_intent| {
        if busy {
            return;
        }
        generation += 1;
        handles.write().clear();
        cancel_menu_focus(focus);
        focused.set(None);
        requested.set(None);
        intent.set(Some(next_intent));
        install_row_menu_outside_dismiss();
        open_menu.set(Some(kind));
    };
    let tab_stop = focused()
        .and_then(|position| order.get(position))
        .or_else(|| order.get(0));
    let label = kind.label();
    let caret_description_id = format!("{}-options-description", kind.key());
    let caret_description = if plain_unavailable && with_unavailable {
        "Neither option is available. Open the menu for the reasons.".to_string()
    } else {
        format!("{label} options: use the current settings or edit them first")
    };
    rsx! {
        span { class: "header-action-options", "data-row-menu-key": kind.key(),
            button {
                r#type: "button", class: "btn btn-neutral header-action-caret bar-menu-toggle {kind.toggle_class()}",
                aria_label: "{label} options", aria_haspopup: "menu", aria_expanded: open(),
                aria_controls: format!("{}-menu", kind.key()),
                disabled: busy,
                "data-actions-unavailable": plain_unavailable && with_unavailable,
                "data-tooltip": caret_description.clone(), aria_describedby: caret_description_id.clone(),
                onfocusin: move |_| forget_menu_focus(wiring),
                onkeydown: move |event| {
                    if !open() {
                        if let Some(next) = closed_toggle_key_intent(&event.key()) {
                            event.prevent_default(); begin_open(next);
                        }
                    } else {
                        if event.key() == Key::Escape { event.stop_propagation(); }
                        handle_menu_key(&event, None, wiring, &());
                    }
                },
                onclick: move |_| {
                    if open() { close.call(()); } else { begin_open(MenuOpenIntent::First); }
                },
                ActionIcon { glyph: ActionGlyph::ChevronDown }
            }
            span { id: caret_description_id, class: "visually-hidden", "{caret_description}" }
            button {
                r#type: "button", class: ROW_MENU_OUTSIDE_RELAY, hidden: true, tabindex: "-1",
                "data-row-menu": kind.key(), onclick: move |_| close.call(()),
            }
            if open() {
                div { class: "host-row-menu-panel bar-menu-panel header-action-menu", "data-tooltip-panel": "true",
                    MenuHeader {
                        title: rsx! { "{label}" },
                        summary: rsx! {
                            if plain_unavailable && with_unavailable { "Unavailable for this session" }
                            else if kind == ActionKind::Restart { "Resume a conversation in this session" }
                            else { "Start a fresh conversation" }
                        },
                    }
                    div { class: "host-row-menu-items session-row-menu-items", id: format!("{}-menu", kind.key()),
                        role: "menu", aria_label: "{label}", "data-terminal-keyboard-owner": "true",
                        for choice in order.actions() {
                            button {
                                r#type: "button", role: "menuitem",
                                class: if choice == Choice::With { format!("btn menu-item host-row-menu-item {}", kind.with_class()) } else { "btn menu-item host-row-menu-item".into() },
                                aria_label: if choice == Choice::With { format!("{label} with…") } else { label.into() },
                                aria_disabled: if choice == Choice::With { with_unavailable } else { plain_unavailable },
                                aria_describedby: format!("{}-{choice:?}-description", kind.key()),
                                "data-tooltip": if choice == Choice::With { with_description.clone() } else { plain_description.clone() },
                                "data-tooltip-anchor": "panel",
                                "data-menu-focus": menu_item_focus_key(wiring, choice),
                                tabindex: if tab_stop == Some(choice) { "0" } else { "-1" },
                                onmounted: move |element| remember_menu_item(wiring, choice, element.data()),
                                onfocusin: move |_| record_menu_item_focus(wiring, choice),
                                onfocusout: move |_| focused.set(None),
                                onkeydown: move |event| {
                                    if event.key() == Key::Escape { event.stop_propagation(); }
                                    handle_menu_key(&event, order.position(choice), wiring, &());
                                },
                                onclick: move |_| {
                                    if busy || (if choice == Choice::With { with_unavailable } else { plain_unavailable }) { return; }
                                    close.call(());
                                    if choice == Choice::With { on_with.call(()); } else { on_plain.call(()); }
                                },
                                ActionIcon { glyph: if choice == Choice::With { ActionGlyph::Adjust } else { kind.glyph() }, class: "menu-icon" }
                                MenuItemCopy {
                                    label: rsx! { if choice == Choice::With { "{label} with…" } else { "{label}" } },
                                    description: rsx! { if choice == Choice::With { "{with_description}" } else { "{plain_description}" } },
                                    description_id: format!("{}-{choice:?}-description", kind.key()),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
