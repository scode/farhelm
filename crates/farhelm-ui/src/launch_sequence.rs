//! Template shortcut keys and the transient sequence queue.
//!
//! The physical keys and modifier masks live here so the capture listener
//! and every Rust hint receive the same definition. The queue is shared by the
//! sidebar relays and main pane's indicator. Only Enter resolves bindings;
//! displayed names never grant launch authority.

use crate::list::SharedPreferences;
use crate::peer::display_peer;
use dioxus::prelude::*;
use farhelm_proto::launcher::LaunchTemplate;
use serde::{Deserialize, Serialize};

/// Select which client fact chooses a shortcut's modifier alternative.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ModifierScheme {
    /// The webview's operating system chooses Command or Ctrl+Shift.
    Platform,
    /// Only the native macOS desktop build takes Command; Mac browsers do not.
    MacDesktopBuild,
}

/// An exact modifier mask for one alternative of a shortcut.
///
/// A shortcut can change its modifiers without teaching the JS matcher or
/// Rust label renderer a new special case.
#[derive(Clone, Copy, Serialize)]
pub(crate) struct ModifierKeys {
    pub(crate) meta: bool,
    pub(crate) ctrl: bool,
    pub(crate) shift: bool,
    pub(crate) alt: bool,
}

/// The two exact chords and the client fact that chooses between them.
#[derive(Clone, Copy, Serialize)]
pub(crate) struct ShortcutModifiers {
    pub(crate) source: ModifierScheme,
    pub(crate) mac: ModifierKeys,
    pub(crate) other: ModifierKeys,
}

const COMMAND: ModifierKeys = ModifierKeys {
    meta: true,
    ctrl: false,
    shift: false,
    alt: false,
};

const CONTROL_SHIFT: ModifierKeys = ModifierKeys {
    meta: false,
    ctrl: true,
    shift: true,
    alt: false,
};

/// A physical key and its modifier rule, shared by the listener and labels.
#[derive(Clone, Copy, Serialize)]
pub(crate) struct ShortcutChord {
    pub(crate) code: &'static str,
    pub(crate) modifiers: ShortcutModifiers,
}

impl ShortcutChord {
    /// Show both platform alternatives when the web client's OS is unknown.
    pub(crate) fn label(self, mac_desktop: bool) -> String {
        chord_label(self.code, self.modifiers, mac_desktop)
    }
}

/// Render a physical code or a generic number placeholder by the same rule.
fn chord_label(code: &str, modifiers: ShortcutModifiers, mac_desktop: bool) -> String {
    let key = code
        .strip_prefix("Key")
        .or_else(|| code.strip_prefix("Digit"))
        .unwrap_or(code);
    let render = |mask| format!("{}{key}", modifier_label(mask));
    match modifiers.source {
        ModifierScheme::Platform => {
            format!("{} / {}", render(modifiers.mac), render(modifiers.other))
        }
        ModifierScheme::MacDesktopBuild if mac_desktop => render(modifiers.mac),
        ModifierScheme::MacDesktopBuild => render(modifiers.other),
    }
}

/// Derive the visible prefix from the same mask serialized to the listener.
fn modifier_label(mask: ModifierKeys) -> String {
    let parts = [
        mask.meta.then_some("⌘"),
        mask.ctrl.then_some("Ctrl"),
        mask.shift.then_some("Shift"),
        mask.alt.then_some("Alt"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if parts == ["⌘"] {
        "⌘".into()
    } else if parts.is_empty() {
        String::new()
    } else {
        format!("{}+", parts.join("+"))
    }
}

/// A number binding keeps its stored value separate from its physical key.
#[derive(Clone, Copy, Serialize)]
pub(crate) struct NumberShortcutKey {
    pub(crate) code: &'static str,
    pub(crate) value: &'static str,
}

/// The single source of physical template keys, modifiers, and binding order.
///
/// Rust renders labels from this value and serializes it to the JS listener.
/// Cmd-J is defined now but its action stays inactive until the switcher PR.
#[derive(Clone, Copy, Serialize)]
pub(crate) struct TemplateShortcutDefinition {
    pub(crate) mac_desktop: bool,
    pub(crate) bolt: ShortcutChord,
    pub(crate) number: NumberShortcutDefinition,
    pub(crate) prefix: ShortcutChord,
    pub(crate) template_list: ShortcutChord,
}

/// Keep the key order attached to the same data the listener receives.
#[derive(Clone, Copy, Serialize)]
pub(crate) struct NumberShortcutDefinition {
    pub(crate) keys: [NumberShortcutKey; 10],
    pub(crate) modifiers: ShortcutModifiers,
}

impl TemplateShortcutDefinition {
    /// Render a bound number's physical chord, or a generic number hint.
    /// An unknown value stays visible as given, so stale bindings are not
    /// silently presented as a different physical key.
    pub(crate) fn number_chord(self, value: &str) -> String {
        let code = self
            .number
            .keys
            .iter()
            .find(|key| key.value == value)
            .map_or(value, |key| key.code);
        chord_label(code, self.number.modifiers, self.mac_desktop)
    }
}

/// Current template keys, serialized once into the installed JS listener.
pub(crate) const TEMPLATE_SHORTCUTS: TemplateShortcutDefinition = TemplateShortcutDefinition {
    mac_desktop: crate::HAS_NEW_SESSION_SHORTCUT,
    bolt: ShortcutChord {
        code: "KeyP",
        modifiers: ShortcutModifiers {
            source: ModifierScheme::Platform,
            mac: COMMAND,
            other: CONTROL_SHIFT,
        },
    },
    number: NumberShortcutDefinition {
        keys: [
            NumberShortcutKey {
                code: "Digit1",
                value: "1",
            },
            NumberShortcutKey {
                code: "Digit2",
                value: "2",
            },
            NumberShortcutKey {
                code: "Digit3",
                value: "3",
            },
            NumberShortcutKey {
                code: "Digit4",
                value: "4",
            },
            NumberShortcutKey {
                code: "Digit5",
                value: "5",
            },
            NumberShortcutKey {
                code: "Digit6",
                value: "6",
            },
            NumberShortcutKey {
                code: "Digit7",
                value: "7",
            },
            NumberShortcutKey {
                code: "Digit8",
                value: "8",
            },
            NumberShortcutKey {
                code: "Digit9",
                value: "9",
            },
            NumberShortcutKey {
                code: "Digit0",
                value: "0",
            },
        ],
        modifiers: ShortcutModifiers {
            source: ModifierScheme::MacDesktopBuild,
            mac: COMMAND,
            other: CONTROL_SHIFT,
        },
    },
    prefix: ShortcutChord {
        code: "KeyX",
        modifiers: ShortcutModifiers {
            source: ModifierScheme::MacDesktopBuild,
            mac: COMMAND,
            other: CONTROL_SHIFT,
        },
    },
    template_list: ShortcutChord {
        code: "KeyJ",
        modifiers: ShortcutModifiers {
            source: ModifierScheme::Platform,
            mac: COMMAND,
            other: CONTROL_SHIFT,
        },
    },
};

/// A shortcut names a binding, not a template captured when its key was pressed.
/// The dispatcher resolves this against one fresh read so renamed or deleted
/// templates never inherit stale authority from a rendered hint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TemplateReference {
    /// The helm-wide ⚡ mark, read at dispatch time.
    Bolt,
    /// A physical number key's stored value, resolved against fresh bindings.
    Number(String),
}

/// One synchronous JS observation of dialog ownership and its opening epoch.
///
/// A modal can appear between separate awaited evaluations, including a
/// programmatic dialog that does not increment Farhelm's epoch. Keep the
/// context and epoch together while Rust crosses the JS boundary.
#[derive(Deserialize)]
pub(crate) struct ShortcutContextSnapshot {
    pub(crate) context: String,
    pub(crate) modal_opening: u64,
}

/// Ask the installed listener's one DOM rule before and after a fresh read.
/// A missing installer or failed evaluation grants no shortcut authority.
pub(crate) async fn shortcut_context_snapshot() -> Option<ShortcutContextSnapshot> {
    document::eval(
        "return { context: window.farhelmTemplateShortcutContext?.() || 'yield', modal_opening: window.__farhelmModalOpening || 0 }",
    )
    .join::<ShortcutContextSnapshot>()
    .await
    .ok()
}

/// Resolve an entire press or sequence from the same preference/template pair.
///
/// A missing binding or deleted template refuses the whole sequence. The
/// returned definitions retain order and duplicates for stacking on a draft.
pub(crate) fn resolve_template_references(
    references: &[TemplateReference],
    preferences: &crate::api::Preferences,
    templates: &[LaunchTemplate],
) -> Option<Vec<LaunchTemplate>> {
    references
        .iter()
        .map(|reference| {
            let name = match reference {
                TemplateReference::Bolt => preferences.quick_launch_template.as_ref(),
                TemplateReference::Number(key) => preferences.template_shortcuts.get(key),
            }?;
            templates
                .iter()
                .find(|template| &template.name == name)
                .cloned()
        })
        .collect()
}

/// `None` is idle; an empty vector is an accepted prefix. Repetition and
/// ordering matter: composing the same template twice is two applications.
#[derive(Clone, Copy)]
pub(crate) struct LaunchSequence(pub Signal<Option<Vec<TemplateReference>>>);

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
                TemplateReference::Number(key) => preferences
                    .template_shortcuts
                    .get(key)
                    .map(|name| display_peer(name))
                    .unwrap_or_else(|| format!("unbound key {key}")),
                TemplateReference::Bolt => preferences
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Moving keys or modifiers in the definition must move every displayed chord.
    /// A literal label elsewhere would let the listener and interface disagree.
    #[test]
    fn labels_follow_the_shared_definition() {
        let mut definition = TEMPLATE_SHORTCUTS;
        definition.bolt.code = "KeyQ";
        definition.prefix.code = "KeyZ";
        definition.number.keys[0].code = "KeyA";
        definition.template_list.code = "KeyU";
        definition.number.modifiers.other = ModifierKeys {
            meta: false,
            ctrl: true,
            shift: false,
            alt: true,
        };

        for mac_desktop in [false, true] {
            definition.mac_desktop = mac_desktop;
            assert_eq!(definition.bolt.label(mac_desktop), "⌘Q / Ctrl+Shift+Q");
            assert_eq!(
                definition.template_list.label(mac_desktop),
                "⌘U / Ctrl+Shift+U"
            );
            let prefix_modifier = if mac_desktop { "⌘" } else { "Ctrl+Shift+" };
            let number_modifier = if mac_desktop { "⌘" } else { "Ctrl+Alt+" };
            assert_eq!(
                definition.prefix.label(mac_desktop),
                format!("{prefix_modifier}Z")
            );
            assert_eq!(definition.number_chord("1"), format!("{number_modifier}A"));
        }
    }

    /// A sequence is one request: a stale middle binding must not launch the
    /// prefix or suffix, while repeated references must keep their order.
    #[test]
    fn resolver_keeps_sequence_atomic_and_ordered() {
        use farhelm_proto::launcher::TemplateFields;

        let template = |name: &str| LaunchTemplate {
            name: name.into(),
            fields: TemplateFields::default(),
        };
        let templates = vec![template("first"), template("second")];
        let mut preferences = crate::api::Preferences {
            quick_launch_template: Some("second".into()),
            ..Default::default()
        };
        preferences
            .template_shortcuts
            .insert("1".into(), "first".into());
        preferences
            .template_shortcuts
            .insert("2".into(), "gone".into());
        let number = |key: &str| TemplateReference::Number(key.into());
        let order = [number("1"), TemplateReference::Bolt, number("1")];
        assert_eq!(
            resolve_template_references(&order, &preferences, &templates)
                .unwrap()
                .iter()
                .map(|template| template.name.as_str())
                .collect::<Vec<_>>(),
            ["first", "second", "first"]
        );
        assert!(
            resolve_template_references(
                &[number("1"), number("2"), TemplateReference::Bolt],
                &preferences,
                &templates,
            )
            .is_none()
        );
    }
}
