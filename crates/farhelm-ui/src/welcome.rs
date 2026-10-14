//! A first-open introduction shared by every client of one helm.
//!
//! The authenticated preferences gate decides whether to offer it, once per
//! load and only after a successful read. The bar owns both this card and
//! feedback, so their handoff removes one modal before mounting the other.
//! Dismissal never waits for persistence: the existing preference queue keeps
//! the local choice even when its best-effort write fails.

use dioxus::prelude::*;

use crate::hosts::settings_dialog::install_dialog_with_focus;
use crate::{app_bar::open_external_page, modal_isolation};

/// The gate publishes its successful-read decision before children mount.
/// AppBar seeds its own open state from this once; later preference changes
/// cannot reopen a card or close one already open in another window.
#[derive(Clone, Copy)]
pub(crate) struct WelcomeOnLoad(pub(crate) Signal<bool>);

/// The page retains its startup decision across authentication remounts.
/// A failed authenticated read consumes it as suppressed; a request that asks
/// for sign-in does not, because startup has not finished authentication yet.
#[derive(Clone, Copy)]
pub(crate) struct WelcomeDecided(pub(crate) Signal<bool>);

/// Where dismissal hands the keyboard, based on how this card was opened.
pub(crate) enum CloseFocus {
    InitialPage,
    Help,
    Feedback,
}

const DIALOG_SELECTOR: &str = r#".welcome-dialog[role="dialog"]"#;
const GET_STARTED_URL: &str = "https://farhelm.io/docs/get-started/first-session/";

/// Release isolation before the parent removes the card or opens feedback.
/// A menu dismissal returns to its toggle after the closing render. First-open
/// dismissal restores the currently selected, revealed terminal. A terminal
/// still preparing takes focus on its ordinary reveal; an empty fleet keeps
/// its initial body focus. A feedback handoff lets the new modal own focus.
pub(crate) fn release(target: CloseFocus) {
    let focus = match target {
        CloseFocus::Help => {
            "requestAnimationFrame(() => document.querySelector('.app-help-toggle')?.focus({ preventScroll: true }));"
        }
        CloseFocus::InitialPage => {
            "requestAnimationFrame(() => window.farhelmTerm?.focusSelected());"
        }
        CloseFocus::Feedback => "",
    };
    document::eval(&format!(
        "{} {focus}",
        modal_isolation::release_js(DIALOG_SELECTOR)
    ));
}

/// The introduction uses the existing framed modal and safe Escape fallback.
/// Every dismissal is unconditional, including while a preference write fails;
/// only Get started leaves the card open. Its owner distinguishes first-open
/// dismissal from reopening and performs the shared write when needed.
#[component]
pub(crate) fn WelcomeDialog(on_close: EventHandler<()>, on_feedback: EventHandler<()>) -> Element {
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div {
                class: "app-settings-dialog welcome-dialog host-dialog-framed",
                role: "dialog", aria_modal: "true", aria_label: "welcome to farhelm", tabindex: "-1",
                onmounted: move |_| install_dialog_with_focus(DIALOG_SELECTOR, ".welcome-got-it", Some(".welcome-got-it"), true),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape && !event.is_composing() {
                        on_close.call(());
                    }
                },
                div { class: "host-settings-header welcome-header",
                    h2 { class: "host-settings-title", "welcome to farhelm" }
                    button {
                        r#type: "button", class: "host-settings-dismiss welcome-dismiss",
                        aria_label: "close welcome", "data-tooltip": "close: done with the welcome card",
                        onclick: move |_| on_close.call(()), "×"
                    }
                }
                div { class: "host-settings-body welcome-body",
                    p { class: "welcome-introduction",
                        "One window for every coding agent you run, on your Mac and on any Linux box you can ssh to. Each agent keeps running on its machine when you close the window, and you pick it up where it was."
                    }
                    div { class: "welcome-item",
                        strong { "New to it? " }
                        "The Get started guide walks through your first session and adding a remote host. "
                        a {
                            href: GET_STARTED_URL, target: "_blank", rel: "noopener",
                            "data-tooltip": "get started: open the first-session guide in your browser",
                            onclick: move |event| { event.prevent_default(); open_external_page(GET_STARTED_URL); },
                            "Read Get started ↗"
                        }
                    }
                    div { class: "welcome-item",
                        strong { "Tell us what you think. " }
                        "Send feedback, in the " strong { "?" }
                        " menu at the top of the sidebar, sends a private message straight to Farhelm's maintainer: bugs, rough edges, ideas. "
                        button {
                            r#type: "button", class: "welcome-feedback",
                            "data-tooltip": "send feedback now: close the welcome card and write a private message",
                            onclick: move |_| on_feedback.call(()), "Send feedback now"
                        }
                    }
                }
                div { class: "host-settings-footer welcome-footer",
                    span { "This card is in the ? menu as welcome if you want it again." }
                    button {
                        r#type: "button", class: "btn btn-primary welcome-got-it",
                        "data-tooltip": "got it: close the welcome card", onclick: move |_| on_close.call(()), "got it"
                    }
                }
            }
        }
    }
}
