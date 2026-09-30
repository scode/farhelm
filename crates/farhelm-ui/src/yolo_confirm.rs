//! The loud confirmation a YOLO launch on a sensitive host asks for.
//!
//! The helm refuses to start a YOLO session (an agent running without
//! approval prompts) on a host the user has not marked safe for YOLO launches,
//! and says so with a header only its own code sets
//! (`api::asks_yolo_confirmation`). Every GUI flow that can hit that refusal
//! (create, replace, replace with, restart with) shows this block instead of
//! an ordinary error, and resubmits with the explicit override only when the
//! user presses its danger button. Nothing was started when it appears.
//!
//! Deliberately hard to miss: accidental YOLO launches on the wrong machine
//! are the failure it exists to stop, and a subtle prompt is one people
//! click through.

use dioxus::prelude::*;

use crate::peer::{DetailPart, PeerLine};

/// The confirmation block. `message` is the helm's refusal, which names the
/// host (peer text, so it goes through [`PeerLine`]). `confirm_submits`
/// makes the danger button a form submit button, for a flow whose resubmit
/// is its surrounding form's own submit; otherwise the button calls
/// `on_confirm`.
#[component]
pub(crate) fn YoloConfirmation(
    message: String,
    busy: bool,
    confirm_submits: bool,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "yolo-confirmation",
            role: "alertdialog",
            aria_label: "confirm a YOLO launch on a sensitive host",
            p { class: "yolo-confirmation-heading", "YOLO launch on a sensitive host" }
            p { class: "yolo-confirmation-consequence",
                "The agent would run commands on this machine without asking for approval. Nothing has been started."
            }
            PeerLine {
                class: "yolo-confirmation-message".to_string(),
                parts: vec![DetailPart::Peer(message)],
            }
            div { class: "yolo-confirmation-actions",
                button {
                    r#type: if confirm_submits { "submit" } else { "button" },
                    class: "btn btn-danger yolo-confirm",
                    disabled: busy,
                    onclick: move |_| on_confirm.call(()),
                    "start YOLO session anyway"
                }
                button {
                    r#type: "button",
                    class: "btn btn-neutral yolo-cancel",
                    autofocus: true,
                    onclick: move |_| on_cancel.call(()),
                    "cancel"
                }
            }
        }
    }
}
