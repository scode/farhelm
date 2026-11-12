//! Marks describe bulk targets, never the session open in the main pane.
//!
//! Stored identities survive filters. Every count and action is projected from
//! the rows actually displayed, in the list's existing held order; this module
//! owns neither a second ordering nor a lifecycle or metadata request queue.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dioxus::prelude::*;

use super::lifecycle::LifecycleAction;
use crate::Session;
use crate::anchored_popovers::{
    AnchorRegion, Interest, MovementOwner, use_anchor_binding, use_registration,
};
use crate::icons::{ActionGlyph, ActionIcon};
use crate::menu_panel::{
    MenuFocusQueue, MenuHeader, MenuItemCopy, MenuOpenIntent, MenuOrder, MenuWiring,
    PanelPlacement, ROW_MENU_OUTSIDE_RELAY, cancel_menu_focus, closed_toggle_key_intent,
    focus_menu_toggle, forget_menu_focus, handle_menu_key, install_row_menu_outside_dismiss,
    measurement_outcome, menu_item_focus_key, record_menu_item_focus, remember_menu_item,
    session_menu_placement_style, session_menu_pointer_style, should_measure_on_mount,
    use_menu_instance,
};

/// Shift always requests a range; either platform's toggle modifier works everywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Gesture {
    Open,
    Toggle,
    Range,
}

impl Gesture {
    /// Alt does not change the two supported marking gestures.
    pub(super) fn from_modifiers(modifiers: Modifiers) -> Self {
        if modifiers.contains(Modifiers::SHIFT) {
            Self::Range
        } else if modifiers.intersects(Modifiers::META | Modifiers::CONTROL) {
            Self::Toggle
        } else {
            Self::Open
        }
    }
}

/// Change marks only for a displayed target; a hidden anchor yields one marked row.
///
/// Range replacement includes both endpoints and discards previous hidden marks.
/// Toggle preserves all other identities. Opening is handled by the caller's
/// navigation guard, which clears marks only after accepting that navigation.
pub(super) fn mark(
    stored: &mut HashSet<String>,
    displayed: &[Session],
    anchor: Option<&str>,
    target: &str,
    gesture: Gesture,
) {
    let Some(end) = displayed.iter().position(|row| row.id.raw() == target) else {
        return;
    };
    match gesture {
        Gesture::Open => {}
        Gesture::Toggle => {
            if !stored.remove(target) {
                stored.insert(target.to_owned());
            }
        }
        Gesture::Range => {
            let start = anchor
                .and_then(|id| displayed.iter().position(|row| row.id.raw() == id))
                .unwrap_or(end);
            *stored = displayed[start.min(end)..=start.max(end)]
                .iter()
                .map(|row| row.id.raw().to_owned())
                .collect();
        }
    }
}

/// Project targets from the visible list, preserving its held slots and pin order.
pub(super) fn effective(displayed: &[Session], stored: &HashSet<String>) -> Vec<Session> {
    displayed
        .iter()
        .filter(|row| stored.contains(row.id.raw()))
        .cloned()
        .collect()
}

/// These writes reuse the row callbacks and deliberately claim no lifecycle lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum MetadataAction {
    Pin,
    Unpin,
    Read,
    Unread,
}

impl MetadataAction {
    /// Match the row's own menu, including seen capability and live-status checks.
    /// Delete-only retired rows never enter a metadata batch.
    pub(super) fn applies(self, row: &Session) -> bool {
        if row.is_retired_container() {
            return false;
        }
        match self {
            Self::Pin => !row.pinned,
            Self::Unpin => row.pinned,
            Self::Read => row.status.is_live() && row.has_unseen_output() == Some(true),
            Self::Unread => row.status.is_live() && row.has_unseen_output() == Some(false),
        }
    }

    /// The command names describe explicit destinations rather than toggles.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Pin => "Pin",
            Self::Unpin => "Unpin",
            Self::Read => "Mark read",
            Self::Unread => "Mark unread",
        }
    }

    /// Explain why a mixed marking can offer both directions at once.
    fn description(self) -> &'static str {
        match self {
            Self::Pin => "Pin the marked sessions that are not pinned",
            Self::Unpin => "Unpin the marked sessions that are pinned",
            Self::Read => "Treat unseen output in eligible marked sessions as seen",
            Self::Unread => "Show new output again in eligible marked sessions",
        }
    }

    /// Reuse the existing metadata glyphs without adding another asset family.
    fn glyph(self) -> ActionGlyph {
        match self {
            Self::Pin | Self::Unpin => ActionGlyph::Pin,
            Self::Read | Self::Unread => ActionGlyph::MarkSeen,
        }
    }
}

/// Restrict a drawn command to still-marked, still-displayed identities.
///
/// The drawn rows carry the activity stamps the user acted on. A later render
/// may remove targets but must never add a newly marked session to an old click.
pub(super) fn admitted_targets(
    drawn: Vec<Session>,
    displayed: &[Session],
    stored: &HashSet<String>,
) -> Vec<Session> {
    let visible: HashSet<_> = displayed.iter().map(|row| row.id.raw()).collect();
    drawn
        .into_iter()
        .filter(|row| stored.contains(row.id.raw()) && visible.contains(row.id.raw()))
        .collect()
}

/// Which bulk lifecycle controls the toolbar offers and enables for the
/// effective marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LifecycleOffer {
    /// Stop is drawn: some mark is not a retired container (those are
    /// Delete-only, as in their own row).
    pub(super) stop_shown: bool,
    /// Some drawn Stop target is not in an environment being deleted.
    pub(super) stop_enabled: bool,
    /// Some mark is not in an environment being deleted.
    pub(super) delete_enabled: bool,
}

impl LifecycleOffer {
    /// `unavailable` holds the marks whose environment is being deleted
    /// (`sandbox_sessions::deleting_reason`). Those take no lifecycle action,
    /// so a control whose every target is one of them is disabled. A mixed
    /// selection keeps the control: the list leaves those rows out with
    /// their reason and acts on the rest.
    pub(super) fn new(targets: &[Session], unavailable: &[String]) -> Self {
        let available = |row: &&Session| !unavailable.iter().any(|id| id == row.id.raw());
        let mut stop = targets
            .iter()
            .filter(|row| !row.is_retired_container())
            .peekable();
        let stop_shown = stop.peek().is_some();
        Self {
            stop_shown,
            stop_enabled: stop.any(|row| available(&row)),
            delete_enabled: targets.iter().any(|row| available(&row)),
        }
    }
}

/// The tooltip of a bulk lifecycle control disabled because every target's
/// environment is being deleted.
const ALL_BEING_DELETED: &str =
    "The marked sessions' environments are being deleted, so their actions are unavailable";

/// Noncomposing Escape can clear marks without claiming the key's default action.
pub(super) fn clears_on_escape(event: &KeyboardEvent) -> bool {
    event.key() == Key::Escape && !event.is_composing() && event.default_action_enabled()
}

/// Return from a disappearing toolbar only when it still owns keyboard focus.
///
/// Clearing from a row or filter keeps that surviving control focused. A menu
/// action removes its own item, so its handback waits for the ordinary title to
/// mount and yields if another control has already received focus meanwhile.
pub(super) fn return_header_focus() {
    let destination = crate::modal_dialog::FocusTarget::new(".sort-title", None, false).script();
    return_header_focus_with_script(&destination);
}

/// Finish a batch's disappearing-toolbar return without reclaiming newer focus.
///
/// The batch settles independently of the toolbar. Its completion keeps the
/// explicit clear path's captured-source and post-removal timing, but the final
/// asynchronous handoff uses the shared current-owner guard.
pub(super) fn return_header_focus_after_async() {
    let destination =
        crate::modal_dialog::FocusTarget::new(".sort-title", None, false).async_script(None);
    return_header_focus_with_script(&destination);
}

/// Capture toolbar ownership before the caller removes its marked projection.
///
/// Both clear paths must request this before clearing marks: after rendering,
/// the removed source can no longer establish whether a return was owed.
fn return_header_focus_with_script(destination: &str) {
    document::eval(&format!(
        r#"(() => {{
        const source = document.activeElement;
        if (!source?.closest('.marked-bar, .marked-menu-flyout')) return;
        requestAnimationFrame(() => {{
            if (source.isConnected || (document.activeElement && document.activeElement !== document.body)) return;
            {destination}
        }});
    }})();"#
    ));
}

/// The header's marked count, lifecycle controls and ordinary metadata menu.
///
/// Targets are drawn snapshots. Dispatch belongs to the list, which checks live
/// locks and visibility before reusing row callbacks. Lifecycle completions and
/// their modal live with that persistent owner, never this conditional toolbar.
#[component]
pub(super) fn MarkedBar(
    targets: Vec<Session>,
    disabled: bool,
    /// The ids among `targets` whose environment is being deleted; they take
    /// no lifecycle action ([`LifecycleOffer`]).
    #[props(default)]
    unavailable: Vec<String>,
    layout_epoch: ReadSignal<u64>,
    on_action: EventHandler<(MetadataAction, Vec<Session>)>,
    on_lifecycle: EventHandler<(LifecycleAction, Vec<Session>)>,
    on_clear: EventHandler<()>,
    /// A fresh user opening owns this marking interaction even with unchanged
    /// targets. An older batch must not unmount its menu during completion.
    on_open: EventHandler<()>,
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
            MetadataAction::Pin,
            MetadataAction::Unpin,
            MetadataAction::Read,
            MetadataAction::Unread,
        ],
        |action| targets.iter().any(|row| action.applies(row)),
    );
    let wiring: MenuWiring<MetadataAction, (), 4> = MenuWiring {
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
        move || (*open.peek()).then(|| "list:marked".to_string()),
        move || wiring.close_menu.call(()),
    );
    // Regional content changes capture live anchor leases. Bind each opening
    // at measurement, just like a row menu, so an attention row moving this
    // toolbar retires its old fixed-position flyout.
    let ready = use_callback(move |()| *open.peek() && toggle_handle.peek().is_some());
    let bind_anchor = use_anchor_binding(registration.clone(), "list:marked".to_string(), ready);
    // An eligibility change can remove the focused command. Close the small
    // metadata menu rather than carrying a focus index into a different order.
    let mut previous_order = use_signal(|| order);
    use_effect(use_reactive((&order,), move |(order,)| {
        if *previous_order.peek() != order {
            open.set(false);
            previous_order.set(order);
        }
    }));
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
            focus_menu_toggle("data-row-menu-key", "list:marked", ".marked-menu-toggle");
        }
    });
    let measure = move || {
        bind_anchor.call(());
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
        on_open.call(());
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
    let tab_stop = focused()
        .and_then(|position| order.get(position))
        .or_else(|| order.get(0));
    let menu_is_open = open();
    let offer = LifecycleOffer::new(&targets, &unavailable);
    rsx! {
        div { class: "marked-bar", "data-marked-count": targets.len(),
            // The rendered open state survives synchronous child dismissal in
            // this event's bubbling pass: Escape belongs to that menu only.
            onkeydown: move |event| {
                if !menu_is_open && clears_on_escape(&event) {
                    return_header_focus();
                    on_clear.call(());
                }
            },
            // This surviving, nonsequential target receives the modal's
            // confirming handback while the admitted work runs independently.
            span { class: "marked-count", tabindex: "-1", "{targets.len()} marked" }
            if offer.stop_shown {button {
                r#type: "button", class: "btn btn-neutral marked-stop", disabled: disabled || !offer.stop_enabled,
                "data-tooltip": if offer.stop_enabled { "Stop the agents and close terminal tabs in the marked sessions currently shown" } else { ALL_BEING_DELETED },
                aria_haspopup: "dialog",
                onclick: { let drawn: Vec<_> = targets.iter().filter(|row| !row.is_retired_container()).cloned().collect(); move |_| {
                    on_lifecycle.call((LifecycleAction::Stop, drawn.clone()));
                } },
                "Stop"
            }}
            button {
                r#type: "button", class: "btn btn-danger marked-delete", disabled: disabled || !offer.delete_enabled,
                "data-tooltip": if offer.delete_enabled { "Delete the marked sessions currently shown" } else { ALL_BEING_DELETED },
                aria_haspopup: "dialog",
                onclick: { let drawn = targets.clone(); move |_| {
                    on_lifecycle.call((LifecycleAction::Delete, drawn.clone()));
                } },
                "Delete"
            }
            if order.get(0).is_some() {span { "data-row-menu-key": "list:marked",
                button {
                    r#type: "button", class: "btn btn-ghost bar-menu-toggle marked-menu-toggle",
                    aria_label: "Marked session actions", "data-tooltip": "Actions for marked sessions",
                    aria_haspopup: "menu", aria_expanded: open(), disabled,
                    onmounted: move |element| {
                        toggle_handle.set(Some(element.data()));
                        if should_measure_on_mount(open(), *placement.peek()) { measure(); }
                    },
                    onfocusin: move |_| forget_menu_focus(wiring),
                    onkeydown: move |event| {
                        if !open() {
                            if let Some(next) = closed_toggle_key_intent(&event.key()) {
                                if disabled { return; }
                                event.prevent_default();
                                begin_open(next);
                            }
                        } else { handle_menu_key(&event, None, wiring, &()); }
                    },
                    onclick: move |_| {
                        if !disabled {
                            if open() { open.set(false); } else { begin_open(MenuOpenIntent::First); }
                        }
                    },
                    span { "aria-hidden": "true", "⋯" }
                }
                button {
                    r#type: "button", class: ROW_MENU_OUTSIDE_RELAY,
                    "data-row-menu": "list:marked", hidden: true, tabindex: "-1",
                    onclick: move |_| open.set(false),
                }
            }}
            button {
                r#type: "button", class: "btn btn-ghost marked-clear", disabled,
                aria_label: "Clear marked sessions", "data-tooltip": "Clear marked sessions (Escape in the list)",
                onclick: move |_| { return_header_focus(); on_clear.call(()); },
                ActionIcon { glyph: ActionGlyph::Close }
            }
        }
        if open() {
            div { class: "host-row-menu-flyout marked-menu-flyout",
                "data-row-menu-key": "list:marked", style: session_menu_placement_style(placement()),
                if let Some(style) = session_menu_pointer_style(placement()) {
                    span { class: "host-row-menu-pointer", style, "aria-hidden": "true" }
                }
                div { class: "host-row-menu-panel bar-menu-panel",
                    MenuHeader { title: rsx! { "{targets.len()} marked" }, summary: rsx! { "Session metadata" } }
                    div { class: "host-row-menu-items session-row-menu-items", role: "menu", aria_label: "Marked session actions",
                        for action in order.actions() {
                            button {
                                r#type: "button", class: "btn menu-item session-row-menu-item host-row-menu-item",
                                role: "menuitem", aria_disabled: if disabled { "true" },
                                "data-menu-focus": menu_item_focus_key(wiring, action),
                                "data-tooltip": action.description(),
                                tabindex: if tab_stop == Some(action) { "0" } else { "-1" },
                                onmounted: move |element| remember_menu_item(wiring, action, element.data()),
                                onfocusin: move |_| record_menu_item_focus(wiring, action),
                                onfocusout: move |_| focused.set(None),
                                onkeydown: move |event| handle_menu_key(&event, order.position(action), wiring, &()),
                                onclick: {
                                    let drawn: Vec<Session> = targets.iter().filter(|row| action.applies(row)).cloned().collect();
                                    move |_| {
                                        if disabled { return; }
                                        open.set(false);
                                        return_header_focus();
                                        on_action.call((action, drawn.clone()));
                                    }
                                },
                                ActionIcon { glyph: action.glyph(), class: "menu-icon session-row-menu-icon" }
                                MenuItemCopy { label: rsx! { "{action.label()}" }, description: rsx! { "{action.description()}" } }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::row::row_specimen;
    use super::*;

    /// Keep routing identities explicit so order assertions cannot pass on titles.
    fn ids(rows: &[Session]) -> Vec<&str> {
        rows.iter().map(|row| row.id.raw()).collect()
    }

    /// Shift has one meaning even when either platform's toggle key is also held.
    #[farhelm_testtrace::test]
    fn modifier_precedence_accepts_both_platform_toggle_keys() {
        assert_eq!(Gesture::from_modifiers(Modifiers::empty()), Gesture::Open);
        assert_eq!(Gesture::from_modifiers(Modifiers::ALT), Gesture::Open);
        for toggle in [
            Modifiers::CONTROL,
            Modifiers::META,
            Modifiers::CONTROL | Modifiers::META,
        ] {
            assert_eq!(Gesture::from_modifiers(toggle), Gesture::Toggle);
            assert_eq!(
                Gesture::from_modifiers(toggle | Modifiers::SHIFT),
                Gesture::Range
            );
        }
    }

    /// Range marking follows the held display, including manual pin moves, rather
    /// than the server's current order. Repeated ranges replace earlier marks.
    #[farhelm_testtrace::test]
    fn ranges_use_displayed_order_include_endpoints_and_replace_marks() {
        let mut a = row_specimen("a");
        let mut b = row_specimen("b");
        a.pinned = true;
        b.pinned = true;
        a.pin_position = Some(0);
        b.pin_position = Some(1);
        let c = row_specimen("c");
        let d = row_specimen("d");
        let held = crate::rows::HeldOrder::capture(&[a.clone(), b.clone(), c.clone(), d.clone()]);
        a.pin_position = Some(1);
        b.pin_position = Some(0);
        let shown = crate::rows::held_display_order(Some(&held), &[b, a, d, c]);
        assert_eq!(
            ids(&shown),
            ["b", "a", "c", "d"],
            "premise: pins move within held slots while the tail stays held"
        );
        let mut stored = HashSet::from(["hidden".to_string()]);
        mark(&mut stored, &shown, Some("a"), "d", Gesture::Range);
        assert_eq!(ids(&effective(&shown, &stored)), ["a", "c", "d"]);
        assert!(!stored.contains("hidden"));
        mark(&mut stored, &shown, Some("a"), "b", Gesture::Range);
        assert_eq!(ids(&effective(&shown, &stored)), ["b", "a"]);
        mark(&mut stored, &shown, Some("a"), "a", Gesture::Range);
        assert_eq!(ids(&effective(&shown, &stored)), ["a"]);
    }

    /// A hidden anchor must not silently choose the first row as another endpoint;
    /// a no-longer-displayed clicked target cannot change any marks at all.
    #[farhelm_testtrace::test]
    fn hidden_or_missing_anchors_mark_only_the_clicked_row() {
        let shown = [row_specimen("first"), row_specimen("clicked")];
        for anchor in [None, Some("hidden")] {
            let mut stored = HashSet::from(["first".to_string()]);
            mark(&mut stored, &shown, anchor, "clicked", Gesture::Range);
            assert_eq!(stored, HashSet::from(["clicked".to_string()]));
            let unchanged = stored.clone();
            mark(&mut stored, &shown, anchor, "gone", Gesture::Range);
            assert_eq!(stored, unchanged);
        }
    }

    /// Toggling the open row is allowed and does not discard marks hidden by a
    /// filter. Effective counts are projections, never mutations of storage.
    #[farhelm_testtrace::test]
    fn toggle_and_effective_projection_preserve_hidden_identities() {
        let shown = [row_specimen("open"), row_specimen("other")];
        let mut stored = HashSet::from(["hidden".to_string()]);
        mark(&mut stored, &shown, Some("open"), "open", Gesture::Toggle);
        assert_eq!(ids(&effective(&shown, &stored)), ["open"]);
        assert!(stored.contains("hidden"));
        mark(&mut stored, &shown, Some("open"), "open", Gesture::Toggle);
        assert!(effective(&shown, &stored).is_empty());
        assert_eq!(stored, HashSet::from(["hidden".to_string()]));
        assert!(effective(&[], &stored).is_empty());
    }

    /// Mixed marks offer both directions, but ended rows and an old helm's
    /// absent seen capability never acquire a read/unread action.
    #[farhelm_testtrace::test]
    fn metadata_targets_follow_each_rows_seen_and_pin_eligibility() {
        let mut unread = row_specimen("unread");
        unread.status = crate::SessionStatus::Running;
        unread.last_activity_at = 123;
        unread.seen_activity_at = Some(None);
        unread.stale = true;
        let mut read = unread.clone();
        read.id = "read".into();
        read.pinned = true;
        read.seen_activity_at = Some(Some(123));
        let mut ended = unread.clone();
        ended.id = "ended".into();
        ended.status = crate::SessionStatus::Exited { exit_code: Some(0) };
        let mut legacy = unread.clone();
        legacy.id = "legacy".into();
        legacy.seen_activity_at = None;
        let rows = [unread, read, ended, legacy];
        assert_eq!(
            rows[0].has_unseen_output(),
            Some(true),
            "premise: stale live row has supported unseen output"
        );
        let targets = |action: MetadataAction| {
            ids(&rows)
                .into_iter()
                .zip(&rows)
                .filter_map(|(id, row)| action.applies(row).then_some(id))
                .collect::<Vec<_>>()
        };
        assert_eq!(targets(MetadataAction::Pin), ["unread", "ended", "legacy"]);
        assert_eq!(targets(MetadataAction::Unpin), ["read"]);
        assert_eq!(targets(MetadataAction::Read), ["unread"]);
        assert_eq!(targets(MetadataAction::Unread), ["read"]);
    }

    /// A stale command can lose targets when filtering or toggling happens first,
    /// but can neither gain a new mark nor read a later activity stamp.
    #[farhelm_testtrace::test]
    fn admission_intersects_current_visibility_without_retargeting_drawn_values() {
        let mut drawn = row_specimen("kept");
        drawn.last_activity_at = 123;
        let mut current = drawn.clone();
        current.last_activity_at = 456;
        let new_mark = row_specimen("new-mark");
        let unmarked = row_specimen("unmarked");
        let hidden = row_specimen("hidden");
        let admitted = admitted_targets(
            vec![drawn, unmarked.clone(), hidden],
            &[new_mark, current, unmarked],
            &HashSet::from([
                "kept".to_string(),
                "hidden".to_string(),
                "new-mark".to_string(),
            ]),
        );
        assert_eq!(ids(&admitted), ["kept"]);
        assert_eq!(admitted[0].last_activity_at, 123);
    }

    /// Composition and a nested menu's consumed Escape keep their existing owner;
    /// an ordinary list Escape remains available to the browser's default action.
    #[farhelm_testtrace::test]
    fn list_escape_never_claims_the_default_and_yields_to_nested_consumers() {
        /// Native builds need no browser serialization feature to supply a key.
        struct EscapeKey {
            composing: bool,
        }
        impl dioxus::html::ModifiersInteraction for EscapeKey {
            fn modifiers(&self) -> Modifiers {
                Modifiers::empty()
            }
        }
        impl dioxus::html::HasKeyboardData for EscapeKey {
            fn key(&self) -> Key {
                Key::Escape
            }
            fn code(&self) -> Code {
                Code::Escape
            }
            fn location(&self) -> Location {
                Location::Standard
            }
            fn is_auto_repeating(&self) -> bool {
                false
            }
            fn is_composing(&self) -> bool {
                self.composing
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
        let event = |composing| {
            KeyboardEvent::new(Rc::new(KeyboardData::from(EscapeKey { composing })), true)
        };
        let ordinary = event(false);
        assert!(clears_on_escape(&ordinary));
        assert!(ordinary.default_action_enabled());
        assert!(!clears_on_escape(&event(true)));
        let menu_owned = event(false);
        menu_owned.prevent_default();
        assert!(!clears_on_escape(&menu_owned));
    }

    /// The marked toolbar must not offer a lifecycle action whose every target
    /// is in an environment being deleted: those sessions' row and header
    /// actions are already unavailable. A mixed selection keeps the control
    /// enabled, since the list acts on the eligible rest. Retired containers
    /// stay Delete-only, as before.
    #[farhelm_testtrace::test]
    fn lifecycle_controls_disable_when_every_target_is_being_deleted() {
        let deleting = row_specimen("deleting");
        let other = row_specimen("other");
        let unavailable = vec![deleting.id.raw().to_owned()];
        assert_eq!(
            LifecycleOffer::new(std::slice::from_ref(&deleting), &unavailable),
            LifecycleOffer {
                stop_shown: true,
                stop_enabled: false,
                delete_enabled: false,
            }
        );
        assert_eq!(
            LifecycleOffer::new(&[deleting.clone(), other.clone()], &unavailable),
            LifecycleOffer {
                stop_shown: true,
                stop_enabled: true,
                delete_enabled: true,
            }
        );
        assert_eq!(
            LifecycleOffer::new(std::slice::from_ref(&other), &[]),
            LifecycleOffer {
                stop_shown: true,
                stop_enabled: true,
                delete_enabled: true,
            }
        );
    }

    /// The toolbar's visible count must come from the supplied effective targets;
    /// stored marks outside that projection cannot acquire a second count here.
    #[farhelm_testtrace::test]
    fn toolbar_renders_the_projected_count() {
        /// Supply the ordinary movement context without a browser measurement.
        fn app() -> Element {
            use_context_provider(MovementOwner::default);
            let epoch = use_signal(|| 0_u64);
            rsx! {
                MarkedBar {
                    targets: vec![row_specimen("one"), row_specimen("two")],
                    disabled: false, layout_epoch: epoch,
                    on_action: |_| {}, on_lifecycle: |_| {}, on_clear: |_| {}, on_open: |_| {},
                }
            }
        }
        let edits = VirtualDom::new(app).rebuild_to_vec().edits;
        let texts: Vec<_> = edits
            .iter()
            .filter_map(|edit| match edit {
                dioxus::core::Mutation::CreateTextNode { value, .. } => Some(value.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            texts.contains(&"2 marked"),
            "the toolbar renders the projected target count: {texts:?}"
        );
    }

    /// The count remains a programmatic target even in a disabled toolbar.
    /// Dialog handback must not rely on an action being enabled, and must not
    /// add the count to the ordinary sequential Tab order.
    #[farhelm_testtrace::test]
    fn locked_toolbar_keeps_a_programmatic_count_target() {
        use dioxus::core::{AttributeValue, DynamicNode, ScopeId, TemplateAttribute, TemplateNode};

        /// Exercise the component's disabled presentation without claiming a batch is pending.
        fn app() -> Element {
            use_context_provider(MovementOwner::default);
            let epoch = use_signal(|| 0);
            let target = row_specimen("pending");
            assert!(!target.is_retired_container(), "premise: Stop is offered");
            assert!(
                MetadataAction::Pin.applies(&target),
                "premise: metadata is offered"
            );
            rsx! { MarkedBar {
                targets: vec![target], disabled: true,
                layout_epoch: epoch, on_action: |_| {}, on_lifecycle: |_| {}, on_clear: |_| {}, on_open: |_| {},
            } }
        }

        /// Find the rendered count in the component's static template tree.
        fn count_target(node: &TemplateNode) -> bool {
            let TemplateNode::Element {
                tag,
                attrs,
                children,
                ..
            } = node
            else {
                return false;
            };
            let has = |name, value| {
                attrs.iter().any(|attr| matches!(attr,
                TemplateAttribute::Static { name: key, value: text, .. } if *key == name && *text == value
            ))
            };
            if has("class", "marked-count") {
                assert_eq!(*tag, "span");
                assert!(has("tabindex", "-1"));
                assert!(!attrs.iter().any(|attr| matches!(
                    attr,
                    TemplateAttribute::Static {
                        name: "hidden" | "disabled",
                        ..
                    }
                )));
                return true;
            }
            children.iter().any(count_target)
        }

        /// Find each actual toolbar control, including conditional fragments.
        /// Stop and the metadata toggle are conditional so delete-only retired
        /// rows cannot offer them. Their attributes belong to nested VNodes,
        /// not the toolbar's root attribute array; counting only that array
        /// silently stops checking their lock when the markup becomes conditional.
        fn locked_controls(
            dom: &VirtualDom,
            vnode: &VNode,
            found: &mut std::collections::BTreeMap<&'static str, bool>,
        ) {
            for (index, attrs) in vnode.dynamic_attrs.iter().enumerate() {
                let Some(disabled) = attrs.iter().find_map(|attr| match &attr.value {
                    AttributeValue::Bool(value) if attr.name == "disabled" => Some(*value),
                    _ => None,
                }) else {
                    continue;
                };
                let path = vnode.template.attr_paths[index];
                let mut node = &vnode.template.roots[usize::from(path[0])];
                for &step in &path[1..] {
                    let TemplateNode::Element { children, .. } = node else {
                        panic!("control attribute path crosses a non-element");
                    };
                    node = &children[usize::from(step)];
                }
                let TemplateNode::Element { tag, attrs, .. } = node else {
                    panic!("disabled control is an element");
                };
                let control = attrs.iter().find_map(|attr| match attr {
                    TemplateAttribute::Static {
                        name: "class",
                        value,
                        ..
                    } => (*value).split_whitespace().find(|class| {
                        matches!(
                            *class,
                            "marked-stop" | "marked-delete" | "marked-menu-toggle" | "marked-clear"
                        )
                    }),
                    _ => None,
                });
                if let Some(control) = control {
                    assert_eq!(*tag, "button");
                    assert!(
                        found.insert(control, disabled).is_none(),
                        "one {control} control"
                    );
                }
            }
            for (index, node) in vnode.dynamic_nodes.iter().enumerate() {
                match node {
                    DynamicNode::Fragment(children) => {
                        for child in children {
                            locked_controls(dom, child, found);
                        }
                    }
                    DynamicNode::Component(component) => {
                        if let Some(scope) = component.mounted_scope(index, vnode, dom) {
                            locked_controls(dom, scope.root_node(), found);
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let root = dom.get_scope(ScopeId::APP).unwrap().root_node();
        let DynamicNode::Component(bar) = &root.dynamic_nodes[0] else {
            panic!("toolbar mounted");
        };
        let rendered = bar.mounted_scope(0, root, &dom).unwrap().root_node();
        assert!(rendered.template.roots.iter().any(count_target));
        let mut controls = std::collections::BTreeMap::new();
        locked_controls(&dom, rendered, &mut controls);
        assert_eq!(
            controls,
            std::collections::BTreeMap::from([
                ("marked-stop", true),
                ("marked-delete", true),
                ("marked-menu-toggle", true),
                ("marked-clear", true),
            ]),
            "Stop, Delete, metadata menu and Clear all obey the same lock"
        );
    }
}
