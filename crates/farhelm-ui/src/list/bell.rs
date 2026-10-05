//! The session row's notification bell and the list it opens (SPEC.md,
//! Status).
//!
//! ## Why the bell is laid over the row rather than inside it
//!
//! The row's whole first line lives inside its open button
//! (`.session-row-open`), and a button cannot contain another button: nested
//! interactive content is invalid HTML that engines and assistive technology
//! interpret inconsistently. So the bell is a sibling control in
//! `.session-row-main`, laid over the open button in the same grid cell
//! (app.css, `.session-bell-overlay`), and the open button reserves the
//! space it sits in with an empty slot of the same width just before the
//! activity time ([`BellSlot`]). The overlay carries an invisible copy of
//! the activity time after the bell, so right-aligning the overlay against
//! the button's right edge lands the bell exactly on the slot whatever width
//! the time has; the visible time stays the open button's own, with its own
//! hover text. Clicking the bell therefore never opens the session.
//!
//! ## Who owns what
//!
//! `ListView` owns which bell is open, the way it owns which row menu is
//! open, because the same things that detach a floating menu from its row
//! (a scroll, a resize, a row moving above it) must close the list too, and
//! because closing is where the list's entries become read: every close
//! path, whatever caused it, ends in `ListView`'s one read-on-close effect.
//! This component only draws the bell and, while open, the list, and reports
//! the user's toggle, Escape and clear.
//!
//! The list is not a menu of commands, so the row menus' roving-focus
//! machinery (`menu_panel::MenuOrder` and friends) does not apply; it shares
//! their floating placement, surface and outside-click dismissal instead.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::SessionNotification;
use crate::activity::{ACTIVITY_NOW, ActivityStamp};
use crate::icons::BellIcon;
use crate::menu_panel::{
    PanelPlacement, clamp_title, measurement_outcome, session_menu_placement_style,
    session_menu_pointer_style, should_measure_on_mount,
};
use crate::peer::{DetailPart, PeerLine, display_peer};

/// The bell's accessible name: which session it belongs to, and how many
/// notifications are unread, since a screen reader cannot hear the loud
/// colour.
pub(super) fn bell_label(title: &str, unread: usize) -> String {
    format!("{}: {}", list_label(title), unread_phrase(unread))
}

/// "notifications for <title>", the open list's accessible name and the
/// start of the bell's. The title is peer text (agents may rename any
/// session), so it is escaped before `menu_panel::clamp_title` cuts it, the
/// order that function's own doc requires.
fn list_label(title: &str) -> String {
    format!("notifications for {}", clamp_title(&display_peer(title)))
}

/// "1 unread", "3 unread" or "none unread", shared by the accessible name
/// and the hover text so the two never disagree about the count.
fn unread_phrase(unread: usize) -> String {
    match unread {
        0 => "none unread".to_string(),
        count => format!("{count} unread"),
    }
}

/// The space the open button keeps free for the bell, just before the
/// activity time. Rendered only on rows whose session has notifications, so
/// a row without them keeps its full title width.
#[component]
pub(super) fn BellSlot() -> Element {
    rsx! {
        span { class: "session-bell-slot", "aria-hidden": "true" }
    }
}

/// The bell control and, while `open` is `Some`, its notification list.
///
/// `open` carries the read mark as it stood when the list opened: entries
/// above it are the ones that arrived since the user last closed this list,
/// and stay marked "new" for as long as the list stays open, even if
/// another window closes its own list meanwhile and moves the shared mark.
#[component]
pub(super) fn NotificationBell(
    session_id: String,
    title: String,
    notifications: Vec<SessionNotification>,
    unread: usize,
    /// The row's displayed activity age, copied invisibly after the bell so
    /// the overlay's right-aligned layout puts the bell on its slot (see the
    /// module doc).
    activity_age: Option<String>,
    open: Option<u64>,
    /// The bell's click: `ListView` opens or closes this session's list.
    on_toggle: EventHandler<()>,
    /// Escape, from the bell or inside the list: closes it.
    on_close: EventHandler<()>,
    /// The list's clear button, with the newest sequence number shown.
    on_clear: EventHandler<u64>,
) -> Element {
    let mut bell_handle = use_signal(|| None::<Rc<MountedData>>);
    let mut placement = use_signal(|| PanelPlacement::Unmeasured);
    // Bumped on every open so a measurement that resolves after a newer open
    // is discarded, as in the row menus (`menu_panel::measurement_outcome`).
    let mut open_generation = use_signal(|| 0_u64);
    let is_open = open.is_some();
    let spawn_measurement = move || {
        let generation = *open_generation.peek();
        spawn(async move {
            let measured = match bell_handle.peek().clone() {
                Some(handle) => handle.get_client_rect().await.ok(),
                None => None,
            };
            if let Some(outcome) =
                measurement_outcome(generation, *open_generation.peek(), measured)
            {
                placement.set(outcome);
            }
        });
    };
    // A fresh open starts unmeasured and measures again: the bell can move
    // between opens, so a previous open's rectangle is not safe to reuse.
    // Driven by the prop rather than the click, because `ListView` is what
    // decides the list is open.
    use_effect(use_reactive((&is_open,), move |(is_open,)| {
        if is_open {
            open_generation += 1;
            placement.set(PanelPlacement::Unmeasured);
            spawn_measurement();
        }
    }));
    let label = bell_label(&title, unread);
    let tooltip = format!(
        "notifications: {}; click to list them",
        unread_phrase(unread)
    );
    let newest = notifications.iter().map(|n| n.seq).max().unwrap_or(0);
    let flyout_key = crate::menu_panel::row_menu_relay_key("bell", &session_id);
    let flyout_key_for_focus = flyout_key.clone();
    let clear_focus_id = session_id.clone();
    let focus_id = session_id.clone();
    // Escape closes the list. Opening it moved focus into it (the panel is
    // autofocused), so Escape reaches it however the bell was clicked: a
    // pointer click does not focus a button in every engine, which would
    // otherwise leave Escape with nowhere to land. Focus goes back to the
    // bell, as a row menu hands focus back to its toggle; `focus_menu_toggle`
    // leaves focus alone when it is anywhere else. An Escape that ends an
    // input method's composition is the composition's, not the list's.
    let escape = move || {
        let focus_id = focus_id.clone();
        move |evt: KeyboardEvent| {
            if evt.key() == Key::Escape && !evt.is_composing() {
                evt.prevent_default();
                crate::menu_panel::focus_menu_toggle(
                    "data-session-id",
                    &focus_id,
                    ".session-row-bell",
                );
                on_close.call(());
            }
        }
    };

    rsx! {
        span { class: "session-bell-overlay",
            button {
                r#type: "button",
                class: if unread > 0 { "btn session-row-bell loud" } else { "btn session-row-bell" },
                aria_label: "{label}",
                aria_expanded: is_open,
                aria_haspopup: "dialog",
                "data-tooltip": "{tooltip}",
                onmounted: move |element| {
                    bell_handle.set(Some(element.data()));
                    if should_measure_on_mount(is_open, *placement.peek()) {
                        spawn_measurement();
                    }
                },
                onclick: move |_| on_toggle.call(()),
                onkeydown: escape(),
                BellIcon { loud: unread > 0 }
            }
            // Invisible: it only gives the overlay the activity time's width
            // (see the module doc). The visible time is the open button's.
            span { class: "session-bell-time-spacer", "aria-hidden": "true",
                if let Some(age) = &activity_age {
                    "{age}"
                }
            }
        }
        if let Some(read_through) = open {
            div {
                class: "host-row-menu-flyout session-bell-flyout",
                // The outside-click dismissal finds this list's relay by
                // this key (`menu_panel::install_row_menu_outside_dismiss`).
                "data-row-menu-key": "{flyout_key}",
                style: session_menu_placement_style(placement()),
                onkeydown: escape(),
                // Focus leaving the list for anything but the bell (Tab out
                // of it, say) closes it, the way a pointer-down outside it
                // does: a list left open behind focus would no longer hear
                // Escape. Checked a task later, once the browser has moved
                // focus, through the same relay the outside-click dismissal
                // clicks; focus that went nowhere (the page body) is the
                // pointer-down path's to handle.
                onfocusout: move |_| close_when_focus_leaves(&flyout_key_for_focus),
                if let Some(pointer_style) = session_menu_pointer_style(placement()) {
                    span { class: "host-row-menu-pointer", style: pointer_style, "aria-hidden": "true" }
                }
                NotificationList {
                    title: title.clone(),
                    notifications: notifications.clone(),
                    read_through,
                    on_clear: move |_| {
                        // The bell and this list are about to go; the row's
                        // open button keeps keyboard focus in the sidebar.
                        crate::menu_panel::focus_menu_toggle(
                            "data-session-id",
                            &clear_focus_id,
                            ".session-row-open",
                        );
                        on_clear.call(newest);
                    },
                }
            }
        }
    }
}

/// The open list itself: a header, the entries newest first, and the clear
/// button.
///
/// Its own component so that reading the page clock ([`ACTIVITY_NOW`]),
/// which re-renders whoever reads it on every tick, re-renders only an open
/// list and never the rows.
#[component]
fn NotificationList(
    title: String,
    notifications: Vec<SessionNotification>,
    read_through: u64,
    on_clear: EventHandler<()>,
) -> Element {
    let now = *ACTIVITY_NOW.read();
    rsx! {
        div {
            class: "host-row-menu-panel session-bell-panel",
            role: "dialog",
            aria_label: "{list_label(&title)}",
            // Focus lands on the dialog itself when it opens, not on the
            // clear button, so a stray Enter cannot clear anything; see
            // `NotificationBell`'s Escape handling for why focus moves at all.
            // An explicit focus call rather than `autofocus`, which engines
            // skip when something else (the terminal, the bell) already has
            // focus. Best effort: a renderer that cannot focus leaves the
            // list reachable by Tab from the bell, and closable by a click.
            tabindex: "-1",
            onmounted: move |element| {
                spawn(async move {
                    let _ = element.data().set_focus(true).await;
                });
            },
            div { class: "session-row-menu-header",
                div { class: "session-row-menu-title", "notifications" }
            }
            ul { class: "session-bell-entries",
                for notification in notifications.iter() {
                    li {
                        key: "{notification.seq}",
                        class: if notification.seq > read_through { "session-bell-entry new" } else { "session-bell-entry" },
                        "data-notification-seq": "{notification.seq}",
                        span { class: "session-bell-entry-meta",
                            if notification.seq > read_through {
                                // A word as well as the colour, so the
                                // distinction survives without colour vision.
                                span { class: "session-bell-new", "new" }
                            }
                            if let Some(stamp) = ActivityStamp::new(now, notification.at) {
                                span {
                                    class: "session-bell-age",
                                    "data-tooltip": "{stamp.absolute}",
                                    if stamp.age == "now" { "now" } else { "{stamp.age} ago" }
                                }
                            }
                        }
                        PeerLine {
                            class: "session-bell-text".to_string(),
                            parts: vec![DetailPart::Peer(notification.text.clone())],
                        }
                    }
                }
            }
            div { class: "session-bell-actions",
                button {
                    r#type: "button",
                    class: "btn session-bell-clear",
                    "data-tooltip": "clear: remove these notifications for every window",
                    onclick: move |_| on_clear.call(()),
                    "clear all"
                }
            }
        }
    }
}

/// Close the list keyed `key` if, once the browser has settled focus, it is
/// neither inside the list nor on its bell; see the flyout's `onfocusout`.
/// Fire-and-forget, like the row menus' focus calls: a renderer that cannot
/// run it leaves the list open until a click or Escape closes it.
fn close_when_focus_leaves(key: &str) {
    let key_js = serde_json::to_string(key).expect("a string is serializable");
    document::eval(&format!(
        r#"setTimeout(() => {{
            const key = {key_js};
            const flyout = [...document.querySelectorAll('[data-row-menu-key]')]
                .find((node) => node.getAttribute('data-row-menu-key') === key);
            const active = document.activeElement;
            if (!flyout || !active || active === document.body) return;
            if (flyout.contains(active)) return;
            if (active.closest('.session-row-bell[aria-expanded="true"]')) return;
            for (const relay of document.querySelectorAll('.{relay}')) {{
                if (relay.getAttribute('data-row-menu') === key) {{
                    relay.click();
                    return;
                }}
            }}
        }}, 0);"#,
        relay = crate::menu_panel::ROW_MENU_OUTSIDE_RELAY,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Session titles are peer text, and an accessible name is read aloud,
    /// so the bell's name carries the escaped title: a bidi override or an
    /// invisible character in a title must not reach assistive technology
    /// raw, exactly as it must not reach the visible row.
    #[farhelm_testtrace::test]
    fn the_bell_name_escapes_the_title_and_counts_unread() {
        const SPOOF: &str = "build\u{200B} \u{202E}lanif";
        let label = bell_label(SPOOF, 2);
        assert_eq!(
            label,
            format!("notifications for {}: 2 unread", display_peer(SPOOF))
        );
        assert!(!label.contains('\u{202E}') && !label.contains('\u{200B}'));
        assert_eq!(
            bell_label("plain", 0),
            "notifications for plain: none unread"
        );
    }
}
