//! The transient template queue shared by the sidebar's launch relays and the
//! main pane's indicator. Only Enter resolves bindings; these labels explain
//! the current queue without granting a displayed name launch authority.

use crate::list::SharedPreferences;
use crate::peer::display_peer;
use dioxus::prelude::*;

/// Describe the prefix this build owns; Mac browsers keep the web chord.
pub(crate) const PREFIX_CHORD: &str = if crate::HAS_NEW_SESSION_SHORTCUT {
    "⌘X"
} else {
    "Ctrl+Shift+X"
};

/// `None` is idle, an empty vector is an accepted prefix, and each queued
/// reference is a digit or (`None`) the bolt. Repetition and ordering matter:
/// composing the same template twice is still two applications.
#[derive(Clone, Copy)]
pub(crate) struct LaunchSequence(pub Signal<Option<Vec<Option<String>>>>);

/// Explain a pending sequence without taking focus or changing pane layout.
/// This lives above the selected session so navigation and an empty fleet do
/// not strand its display in an unmounted terminal.
#[component]
pub(crate) fn LaunchSequenceIndicator() -> Element {
    let sequence = use_context::<LaunchSequence>().0;
    let preferences = use_context::<SharedPreferences>();
    let Some(references) = sequence() else {
        return rsx! {};
    };
    let names = references
        .iter()
        .map(|reference| {
            let preferences = preferences.0.read();
            match reference {
                Some(key) => preferences
                    .template_shortcuts
                    .get(key)
                    .map(|name| display_peer(name))
                    .unwrap_or_else(|| format!("unbound key {key}")),
                None => preferences
                    .quick_launch_template
                    .as_ref()
                    .map(|name| display_peer(name))
                    .unwrap_or_else(|| "unbound ⚡".into()),
            }
        })
        .collect::<Vec<_>>();
    rsx! {
        div { class: "launch-sequence-indicator", role: "status",
            if references.is_empty() {
                "Launch sequence: press number keys to queue templates, Enter to launch, Esc to cancel."
            } else {
                span { class: "launch-sequence-names",
                    for (index, name) in names.into_iter().enumerate() {
                        if index > 0 { " + " }
                        bdi { dir: "auto", "{name}" }
                    }
                }
                span { class: "launch-sequence-hint", "Enter launch · Esc cancel" }
            }
        }
    }
}
