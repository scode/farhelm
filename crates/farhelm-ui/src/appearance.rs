//! Device appearance has one owner shared by browser and desktop webview.
//!
//! The classic head asset resolves the palette before application content is
//! rendered. Settings supplies native controls but no Rust choice signal: a
//! system or storage event can change appearance while the dialog is closed,
//! and a refused storage write must still retain this page's selected choice.

use dioxus::prelude::*;

/// Keep native radio labels and keyboard behavior while JS owns checked state.
///
/// The head bootstrap must have installed `farhelmAppearance` before this
/// component mounts. Reopening Settings synchronizes its new inputs with the
/// surviving page owner; hiding a panel does not reset the device choice.
#[component]
pub(crate) fn AppearanceSettings() -> Element {
    rsx! {
        div {
            class: "appearance-settings",
            onmounted: move |_| {
                document::eval("window.farhelmAppearance.mountSettings();");
            },
            p {
                id: "appearance-help",
                class: "host-settings-help",
                "Choose the interface colors for this device. System follows your device's appearance. The terminal stays dark."
            }
            fieldset {
                class: "appearance-choices",
                aria_describedby: "appearance-help",
                legend { class: "appearance-legend", "Color theme" }
                for (choice, label) in [("system", "System"), ("dark", "Dark"), ("light", "Light")] {
                    label {
                        class: "appearance-choice",
                        input {
                            r#type: "radio",
                            name: "farhelm-appearance",
                            value: choice,
                            "data-appearance-choice": choice,
                        }
                        span { "{label}" }
                    }
                }
            }
        }
    }
}
