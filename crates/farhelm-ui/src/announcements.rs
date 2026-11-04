//! The queue's presentation, independent of the producers that fill it.
//!
//! AppBar owns the modal lifetime and calls `read_and_show` after its initial
//! welcome decision. Its existing modal-admission boundary keeps a late read
//! from stacking on another dialog. A later trigger can call the same reader;
//! nothing here depends on a page-load event.

use dioxus::prelude::*;

use crate::api::{self, Announcement};
use crate::modal_dialog::{
    DialogFocus, DialogHandle, ModalDialog, RecoveryDelay, RecoveryFocus, ScrollRegion,
};

/// AppBar releases this exact modal before returning focus or saving shown ids.
pub(crate) const DIALOG: DialogHandle =
    DialogHandle::new(r#".announcements-dialog[role="dialog"]"#);

/// The current batch and known shared history have separate lifetimes.
/// History reopening never drains pending content or records another showing.
#[derive(Default)]
pub(crate) struct AnnouncementState {
    pub(crate) displayed: Vec<Announcement>,
    pub(crate) history: Vec<Announcement>,
    pub(crate) from_history: bool,
}

/// Read the queue and show its pending batch, newest first, if one exists.
/// Its owner mounts through the existing modal-admission boundary, so a read
/// finishing behind another dialog waits for that owner to close. A failed read
/// is logged and leaves the page usable; this function schedules no retry and
/// subscribes to no event feed. History retains the server's shown order.
pub(crate) async fn read_and_show(base: &str, mut state: Signal<AnnouncementState>) {
    match api::fetch_announcements(base).await {
        Ok(mut queue) => {
            queue.pending.reverse();
            state.set(AnnouncementState {
                displayed: queue.pending,
                history: queue.history,
                from_history: false,
            });
        }
        Err(detail) => {
            dioxus::logger::tracing::warn!(target: "announcements", "could not read announcements: {detail}");
        }
    }
}

/// Render plain text and optional display-ready extras in the welcome frame.
/// Closing is unconditional; its owner decides whether this is a pending batch
/// to record or a history view to close without a write.
#[component]
pub(crate) fn AnnouncementsDialog(
    announcements: Vec<Announcement>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            ModalDialog {
                handle: DIALOG,
                focus: DialogFocus::remembered(
                    ".announcements-got-it", Some(".announcements-got-it"), true, Some(".announcements-got-it"),
                ),
                class: "app-settings-dialog announcements-dialog host-dialog-framed",
                role: "dialog", aria_modal: "true", aria_label: "Announcements", tabindex: "-1",
                on_escape: move |event: KeyboardEvent| {
                    if !event.is_composing() {
                        on_close.call(());
                    }
                },
                crate::modal_dialog::DialogHeader {
                    title: rsx! { "Announcements" },
                    summary: rsx! { p { class: "announcements-summary", "Updates and information from Farhelm." } },
                    close: rsx! { button {
                        r#type: "button", class: "dialog-close host-settings-dismiss announcements-dismiss",
                        aria_label: "Close announcements", "data-tooltip": "Close: done with these announcements",
                        onclick: move |_| on_close.call(()), "×"
                    } },
                }
                // Linkless batches still need keyboard entry into their text;
                // the pinned closing buttons sit outside this scroll owner.
                ScrollRegion {
                    handle: DIALOG, selector: ".announcements-body", name: "Announcement content",
                    recovery: RecoveryFocus::new(".announcements-got-it", None, true, RecoveryDelay::OneFrame),
                    class: "dialog-body host-settings-body announcements-body",
                    for announcement in announcements {
                        article { key: "{announcement.id}", class: "announcement",
                            h3 { "{announcement.title}" }
                            if let Some(detail) = announcement.detail {
                                p { class: "announcement-detail", "{detail}" }
                            }
                            for paragraph in announcement.body.split("\n\n").filter(|part| !part.trim().is_empty()) {
                                p { class: "announcement-paragraph", "{paragraph}" }
                            }
                            if let Some(link) = announcement.link {
                                a {
                                    href: link.url.clone(), target: "_blank", rel: "noopener",
                                    "data-tooltip": format!("{}: open in your browser", link.label),
                                    onclick: move |event| {
                                        event.prevent_default();
                                        crate::app_bar::open_external_page(&link.url);
                                    },
                                    "{link.label} ↗"
                                }
                            }
                        }
                    }
                }
                crate::modal_dialog::DialogFooter {
                    crate::modal_dialog::DialogActions {
                        button {
                            r#type: "button", class: "btn btn-primary announcements-got-it",
                            "data-tooltip": "Got it: close these announcements",
                            onclick: move |_| on_close.call(()), "Got it"
                        }
                    }
                }
            }
        }
    }
}
