//! The rail bell shares helm-owned history and the permanent Trash owner.
//!
//! Toggle and panel are siblings of the rail's stacking context, as with Help.
//! One close path acknowledges exactly the last panel render's IDs. The shared
//! popover owner, outside relay and focus handback keep dismissal consistent
//! with session bells; this is a list of notices, not a roving command menu.

use std::{cell::RefCell, rc::Rc};

use dioxus::prelude::*;

use crate::HostId;
use crate::anchored_popovers::{
    AnchorRegion, Interest, MovementOwner, use_anchor_binding, use_registration,
};
use crate::api::NoticeContent;
use crate::hosts::HostsRead;
use crate::menu_panel::{PanelPlacement, measurement_outcome};
use crate::notices::NoticeReader;
use crate::peer::display_peer;
use crate::reader::Trigger;

use super::trash::TrashActions;

const KEY: &str = "bar:rail-notices";

/// ListView keeps these handles alive while either the panel or Trash closes.
/// A Retry reply therefore never calls a callback owned by an unmounted panel.
#[derive(Clone)]
pub(super) struct RailNotices {
    open: Signal<bool>,
    placement: Signal<PanelPlacement>,
    anchor: Signal<Option<Rc<MountedData>>>,
    shown: Rc<RefCell<Vec<i64>>>,
    pub(super) close: Callback<bool>,
    begin: Callback<()>,
    retry: Callback<(i64, HostId, String)>,
    retrying: Signal<Option<i64>>,
    error: Signal<Option<String>>,
}

impl PartialEq for RailNotices {
    fn eq(&self, other: &Self) -> bool {
        self.open == other.open
    }
}

/// Reuse the page's geometry and request owners without another poll or feed.
/// `false` handback is reserved for Open Trash, whose modal takes focus next.
pub(super) fn use_rail_notices(
    reader: NoticeReader,
    actions: Signal<Option<TrashActions>>,
    layout_epoch: ReadSignal<u64>,
) -> RailNotices {
    let mut open = use_signal(|| false);
    let mut placement = use_signal(|| PanelPlacement::Unmeasured);
    let anchor = use_signal(|| None::<Rc<MountedData>>);
    let mut generation = use_signal(|| 0_u64);
    let mut opened_epoch = use_signal(|| 0_u64);
    let shown = use_hook(|| Rc::new(RefCell::new(Vec::<i64>::new())));
    let closing_shown = shown.clone();
    let close = use_callback(move |handback: bool| {
        if !*open.peek() {
            return;
        }
        open.set(false);
        generation += 1;
        reader
            .mark_read
            .call(std::mem::take(&mut *closing_shown.borrow_mut()));
        if handback {
            crate::menu_panel::focus_menu_toggle("data-row-menu-key", KEY, ".rail-notices-toggle");
        }
    });
    let registration = use_registration(
        use_context::<MovementOwner>(),
        Interest::new(AnchorRegion::AppBar),
        move || (*open.peek()).then(|| KEY.to_owned()),
        move || close.call(true),
    );
    let ready = use_callback(|()| true);
    let bind = use_anchor_binding(registration.clone(), KEY.to_owned(), ready);
    let opening_shown = shown.clone();
    let begin = use_callback(move |_| {
        generation += 1;
        let current = *generation.peek();
        opened_epoch.set(*layout_epoch.peek());
        opening_shown.borrow_mut().clear();
        placement.set(PanelPlacement::Unmeasured);
        crate::menu_panel::install_row_menu_outside_dismiss();
        open.set(true);
        bind.call(());
        reader.request.call(Trigger::Explicit);
        spawn(async move {
            let measured = match anchor.peek().clone() {
                Some(anchor) => anchor.get_client_rect().await.ok(),
                None => None,
            };
            if let Some(outcome) = measurement_outcome(current, *generation.peek(), measured) {
                placement.set(outcome);
            }
        });
    });
    use_effect(move || {
        if layout_epoch() != *opened_epoch.peek() && *open.peek() {
            registration.close_if_open();
        }
    });
    let mut retrying = use_signal(|| None::<i64>);
    let mut error = use_signal(|| None::<String>);
    let finished = use_callback(move |result: Result<(), String>| {
        retrying.set(None);
        error.set(result.err());
    });
    let retry = use_callback(move |(notice, host, checkout): (i64, HostId, String)| {
        if retrying.peek().is_some() {
            return;
        }
        error.set(None);
        if let Some(actions) = *actions.peek() {
            retrying.set(Some(notice));
            actions.retry.call((host, checkout, finished));
        } else {
            error.set(Some(
                "Trash is still loading. Try again when it is ready.".into(),
            ));
        }
    });
    RailNotices {
        open,
        placement,
        anchor,
        shown,
        close,
        begin,
        retry,
        retrying,
        error,
    }
}

/// Anchor from the footer's bottom edge so a long list grows upward. Like
/// Help, a failed measurement uses a visible corner fallback without a pointer.
/// Visible states reset pointer-events explicitly: Dioxus preserves omitted
/// inline properties, including the unmeasured state's input suppression.
fn panel_style(placement: PanelPlacement) -> (String, Option<String>) {
    match placement {
        PanelPlacement::Unmeasured => ("opacity:0;pointer-events:none;".into(), None),
        PanelPlacement::Fallback => ("opacity:1;pointer-events:auto;left:8px;bottom:8px;max-width:calc(100vw - 16px);max-height:calc(100vh - 16px);".into(), None),
        PanelPlacement::Measured(rect) => {
            let left = rect.max_x() + 10.0;
            let bottom = rect.max_y() + 4.0;
            let pointer = (rect.max_y() - rect.min_y()) / 2.0 - 1.0;
            (format!("opacity:1;pointer-events:auto;right:auto;top:auto;--menu-left:max(8px,min({left}px,calc(100vw - 368px)));left:var(--menu-left);--menu-bottom:max(8px,calc(100vh - {bottom}px));bottom:var(--menu-bottom);max-width:calc(100vw - var(--menu-left) - 8px);max-height:calc(100vh - var(--menu-bottom) - 8px);"),
             Some(format!("top:auto;bottom:clamp(12px,{pointer}px,calc(100% - 18px));")))
        }
    }
}

/// The always-present footer button uses the existing bell geometry at rail
/// size. Count and accessible wording expose the alert without relying on red.
#[component]
pub(super) fn NoticesToggle(ui: RailNotices, reader: NoticeReader) -> Element {
    let unread = reader.state.read().unread();
    let label = format!("Notifications: {unread} unread");
    let mut anchor = ui.anchor;
    let close = ui.close;
    let begin = ui.begin;
    let open = ui.open;
    rsx! {
        span { class: "app-bar-menu", "data-bar-menu": "rail-notices", "data-row-menu-key": KEY,
            button {
                r#type: "button", class: if unread > 0 { "btn btn-neutral bar-menu-toggle rail-notices-toggle loud" } else { "btn btn-neutral bar-menu-toggle rail-notices-toggle" },
                aria_label: label.clone(), "data-tooltip": format!("{label}; open notices about your hosts and Farhelm"),
                aria_haspopup: "dialog", aria_expanded: open(), aria_controls: "rail-notices-panel",
                onclick: move |_| if *open.peek() { close.call(true) } else { begin.call(()) },
                onkeydown: move |event| {
                    if event.key() == Key::Escape && !event.is_composing() && *open.peek() {
                        event.prevent_default(); close.call(true);
                    }
                },
                span { class: "host-rail-icon", "aria-hidden": "true",
                    onmounted: move |element| anchor.set(Some(element.data())),
                    crate::icons::BellIcon { loud: unread > 0 }
                    if unread > 0 { span { class: "rail-notices-count", "{unread}" } }
                }
                span { class: "host-rail-label", "aria-hidden": "true", "Notifications" }
            }
            button { r#type: "button", class: crate::menu_panel::ROW_MENU_OUTSIDE_RELAY,
                "data-row-menu": KEY, hidden: true, tabindex: "-1", onclick: move |_| close.call(true),
            }
        }
    }
}

/// Render the current server history and remember only IDs this panel drew.
/// New notices arriving after this render are excluded from its close mark.
#[component]
pub(super) fn NoticesPanel(
    ui: RailNotices,
    reader: NoticeReader,
    hosts: Signal<HostsRead>,
    actions: Signal<Option<TrashActions>>,
    on_open_trash: Callback<()>,
) -> Element {
    if !(ui.open)() {
        return rsx! {};
    }
    let snapshot = reader.state.read();
    *ui.shown.borrow_mut() = snapshot.notices.iter().map(|notice| notice.id).collect();
    let resolved: Vec<_> = snapshot
        .notices
        .iter()
        .filter(|notice| notice.resolved)
        .map(|notice| notice.id)
        .collect();
    let (style, pointer) = panel_style((ui.placement)());
    let close = ui.close;
    let retry = ui.retry;
    let retrying = (ui.retrying)().is_some();
    let action_error = (ui.error)().or_else(|| (reader.action_error)());
    rsx! {
        div { class: "host-row-menu-flyout rail-notices-flyout", "data-bar-menu": "rail-notices", "data-row-menu-key": KEY,
            style,
            onkeydown: move |event| {
                if event.key() == Key::Escape && !event.is_composing() {
                    event.prevent_default(); event.stop_propagation(); close.call(true);
                }
            },
            onfocusout: move |_| close_when_focus_leaves(),
            if let Some(pointer) = pointer { span { class: "host-row-menu-pointer", style: pointer, "aria-hidden": "true" } }
            div { id: "rail-notices-panel", class: "host-row-menu-panel bar-menu-panel rail-notices-panel", role: "dialog", aria_label: "Notifications", tabindex: "-1",
                onmounted: move |element| { spawn(async move { let _ = element.data().set_focus(true).await; }); },
                crate::menu_panel::MenuHeader { title: rsx! { "Notifications" }, summary: rsx! { "About your hosts and Farhelm" } }
                if let Some(error) = &snapshot.error { p { class: "error", role: "status", "Could not refresh notifications: {error}" } }
                if let Some(error) = action_error { p { class: "error", role: "status", "{error}" } }
                if !snapshot.loaded { p { "Loading notifications…" } }
                else if snapshot.notices.is_empty() { p { class: "rail-notices-empty", "No notifications" } }
                ul { class: "session-bell-entries rail-notices-entries",
                    for notice in &snapshot.notices {
                        {
                            let host_name = hosts.read().hosts().unwrap_or_default().iter().find(|host| host.id == notice.host).map(|host| host.name.clone());
                            let title = host_name.map(|name| display_peer(name.raw())).unwrap_or_else(|| "Unknown host".into());
                            let NoticeContent::StuckTrash { checkout_id, name, repository, bytes, reason, at } = &notice.content;
                            let target = (notice.host, checkout_id.clone());
                            let can_retry = !notice.resolved && !retrying && actions.read().is_some_and(|actions| actions.retryable.read().contains(&target));
                            let retry_target = (notice.id, notice.host, checkout_id.clone());
                            rsx! {
                                li { key: "{notice.id}", class: if notice.resolved { "session-bell-entry rail-notice resolved" } else if notice.unread() { "session-bell-entry rail-notice new" } else { "session-bell-entry rail-notice" },
                                    "data-notice-id": "{notice.id}", "data-host-id": "{notice.host}",
                                    h3 { bdi { "{title}" } ": trash could not be freed" }
                                    if notice.resolved { span { class: "session-bell-resolved", "Resolved" } }
                                    p { "Farhelm could not finish removing " bdi { class: "peer-value", "{display_peer(name)}" } "." }
                                    p { class: "rail-notice-checkout", bdi { class: "peer-value", "{display_peer(repository)}" }
                                        if let Some(bytes) = bytes { " · {super::trash::space(*bytes, false)} last measured" }
                                        else { " · size unknown" }
                                    }
                                    p { class: "rail-notice-reason", bdi { "{display_peer(reason)}" } }
                                    p { class: "rail-notice-time", "Last failed " span { class: "font-mono", "{super::trash::utc_timestamp(*at)}" } }
                                    if !notice.resolved { p { "Farhelm tries again when this host’s supervisor restarts. You can also retry now." } }
                                    div { class: "rail-notice-actions",
                                        if !notice.resolved { button { class: "btn btn-neutral rail-notice-retry", r#type: "button", disabled: !can_retry,
                                            "data-tooltip": if can_retry { "Retry removing this checkout's files with the usual cleanup checks" } else { "Retry needs a connected host, a current stuck entry and no other operation in progress" },
                                            onclick: move |_| retry.call(retry_target.clone()), "Retry now"
                                        } }
                                        button { class: "btn btn-neutral rail-notice-open-trash", r#type: "button", disabled: actions.read().is_none_or(|actions| !(actions.can_open)()),
                                            "data-tooltip": "Open Trash to inspect archived checkouts on all hosts",
                                            onclick: move |_| { close.call(false); on_open_trash.call(()); }, "Open Trash"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "rail-notices-footer",
                    button { class: "btn btn-neutral rail-notices-clear", r#type: "button", disabled: resolved.is_empty() || (reader.clearing)(),
                        "data-tooltip": "Remove only the resolved notifications shown in this panel",
                        onclick: move |_| reader.clear.call(resolved.clone()), "Clear resolved"
                    }
                    button { class: "btn btn-neutral rail-notices-close", r#type: "button", "data-tooltip": "Close notifications and mark the shown items read", onclick: move |_| close.call(true), "Close" }
                }
            }
        }
    }
}

/// Match session bells' deferred focus-leave dismissal through the existing
/// keyed relay. Independent controls keep the focus the browser gave them.
fn close_when_focus_leaves() {
    document::eval(
        r#"setTimeout(() => {
        const flyout = document.querySelector('.rail-notices-flyout');
        const active = document.activeElement;
        if (!flyout || !active || active === document.body || flyout.contains(active)) return;
        if (active.closest('.rail-notices-toggle[aria-expanded="true"]')) return;
        document.querySelector('.row-menu-outside-relay[data-row-menu="bar:rail-notices"]')?.click();
    }, 0);"#,
    );
}
