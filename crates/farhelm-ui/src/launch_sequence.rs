//! Template shortcut keys and the transient sequence queue.
//!
//! The physical keys and modifier masks live here so the capture listener
//! and every Rust hint receive the same definition. The queue is shared by the
//! sidebar relays and main pane's indicator. Enter captures the bindings shown
//! by that indicator before the dispatcher reads current template definitions.
//! A shortcut moved elsewhere cannot substitute a different template unseen.

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
/// Cmd-J uses the same serialized definition to open the template list.
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

/// Direct shortcuts resolve fresh bindings; a completed sequence supplies names
/// from its displayed snapshot. Both use current template definitions, so a
/// renamed or deleted template cannot launch from a stale captured definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TemplateReference {
    /// The helm-wide ⚡ mark, read at dispatch time.
    Bolt,
    /// A physical number key's stored value, resolved against fresh bindings.
    Number(String),
    /// A switcher row names a template from its snapshot; resolve it afresh.
    Name(String),
    /// A sequence showed an absent binding. Keep it absent after a fresh read;
    /// `Some` identifies a number key and `None` identifies the bolt mark.
    Unbound(Option<String>),
}

impl TemplateReference {
    /// Read the binding used by the indicator and by sequence capture alike.
    /// An explicitly unbound reference cannot gain authority from newer settings.
    fn name<'a>(&'a self, preferences: &'a crate::api::Preferences) -> Option<&'a String> {
        match self {
            Self::Bolt => preferences.quick_launch_template.as_ref(),
            Self::Number(key) => preferences.template_shortcuts.get(key),
            Self::Name(name) => Some(name),
            Self::Unbound(_) => None,
        }
    }
}

/// Capture the names shown at Enter, before any awaited preference read.
/// Keep order and duplicates, including absent bindings: a key newly assigned
/// elsewhere must still refuse if the indicator showed it as unbound. Names
/// remain subject to the dispatcher's current-catalog lookup.
pub(crate) fn capture_sequence_bindings(
    references: &[TemplateReference],
    preferences: &crate::api::Preferences,
) -> Vec<TemplateReference> {
    references
        .iter()
        .map(|reference| {
            if let Some(name) = reference.name(preferences) {
                TemplateReference::Name(name.clone())
            } else {
                match reference {
                    TemplateReference::Number(key) | TemplateReference::Unbound(Some(key)) => {
                        TemplateReference::Unbound(Some(key.clone()))
                    }
                    _ => TemplateReference::Unbound(None),
                }
            }
        })
        .collect()
}

/// One synchronous JS observation of dialog ownership and its opening epoch.
///
/// A modal can appear between separate awaited evaluations, including a
/// programmatic dialog that does not increment Farhelm's epoch. Keep the
/// context and epoch together while Rust crosses the JS boundary.
#[derive(Clone, Deserialize)]
pub(crate) struct ShortcutContextSnapshot {
    pub(crate) context: String,
    pub(crate) modal_opening: u64,
}

/// Take the listener's snapshot from the physical shortcut press.
/// The serialized worker may reach a later press only after an earlier fetch;
/// observing the DOM then would miss a dialog that opened between them.
pub(crate) async fn take_shortcut_press_snapshot() -> Option<ShortcutContextSnapshot> {
    document::eval("return window.farhelmTakeTemplateShortcutPress?.()")
        .join::<ShortcutContextSnapshot>()
        .await
        .ok()
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
            let name = reference.name(preferences)?;
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

/// The context where the prefix was accepted, for the sequence pill's verb.
/// A sequence is cancelled when dialog ownership changes, so this remains
/// stable through the digit presses it describes.
#[derive(Clone, Copy)]
pub(crate) struct LaunchSequenceInLauncher(pub Signal<bool>);

/// Explain a pending sequence without taking focus or changing pane layout.
/// This lives above the selected session so navigation and an empty fleet do
/// not strand its display in an unmounted terminal.
#[component]
pub(crate) fn LaunchSequenceIndicator() -> Element {
    let sequence = use_context::<LaunchSequence>().0;
    let in_launcher = use_context::<LaunchSequenceInLauncher>().0;
    let preferences = use_context::<SharedPreferences>();
    let Some(references) = sequence() else {
        return rsx! {};
    };
    let names = references
        .iter()
        .map(|reference| {
            let preferences = preferences.0.read();
            reference
                .name(&preferences)
                .map(|name| display_peer(name))
                .unwrap_or_else(|| match reference {
                    TemplateReference::Number(key) | TemplateReference::Unbound(Some(key)) => {
                        format!("unbound key {key}")
                    }
                    _ => "unbound ⚡".into(),
                })
        })
        .collect::<Vec<_>>();
    rsx! {
        div { class: "launch-sequence-indicator", role: "status",
            if references.is_empty() {
                if in_launcher() {
                    "Apply sequence: press number keys to queue templates, Enter to apply, Esc to cancel."
                } else {
                    "Launch sequence: press number keys to queue templates, Enter to launch, Esc to cancel."
                }
            } else {
                span { class: "launch-sequence-names",
                    for (index, name) in names.into_iter().enumerate() {
                        if index > 0 { " + " }
                        bdi { dir: "auto", "{name}" }
                    }
                }
                span { class: "launch-sequence-hint",
                    if in_launcher() { "Enter apply · Esc cancel" } else { "Enter launch · Esc cancel" }
                }
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

    /// Enter must keep the names shown by the pill when another client moves
    /// number and bolt bindings. Current definitions still apply by name, and
    /// the sequence retains its order and duplicate applications.
    #[test]
    fn sequence_uses_displayed_names_after_bindings_move() {
        use farhelm_proto::launcher::TemplateFields;

        let mut shown = crate::api::Preferences {
            quick_launch_template: Some("second".into()),
            ..Default::default()
        };
        shown.template_shortcuts.insert("1".into(), "first".into());
        let references = [
            TemplateReference::Number("1".into()),
            TemplateReference::Bolt,
            TemplateReference::Number("1".into()),
        ];
        let captured = capture_sequence_bindings(&references, &shown);
        let mut fresh = shown.clone();
        fresh.quick_launch_template = Some("first".into());
        fresh.template_shortcuts.insert("1".into(), "second".into());
        let templates = ["first", "second"].map(|name| LaunchTemplate {
            name: name.into(),
            fields: TemplateFields {
                name: Some(format!("current {name}")),
                ..Default::default()
            },
        });
        let names = |templates: Vec<LaunchTemplate>| {
            templates
                .into_iter()
                .map(|template| template.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(resolve_template_references(&references, &fresh, &templates).unwrap()),
            ["second", "first", "second"],
            "premise: fresh shortcut lookup would launch different templates"
        );
        assert_eq!(
            resolve_template_references(&captured, &fresh, &templates),
            Some(vec![
                templates[0].clone(),
                templates[1].clone(),
                templates[0].clone()
            ]),
        );
        assert!(
            resolve_template_references(&captured, &fresh, &templates[1..]).is_none(),
            "deleting or renaming a captured name must refuse the entire batch"
        );
    }

    /// A key shown as unbound must not launch a later assignment. Preserve its
    /// kind for the existing refusal hint, and refuse the whole mixed batch.
    #[test]
    fn sequence_unbound_references_cannot_gain_new_bindings() {
        use farhelm_proto::launcher::TemplateFields;

        let shown = crate::api::Preferences::default();
        let references = [
            TemplateReference::Number("1".into()),
            TemplateReference::Bolt,
        ];
        let captured = capture_sequence_bindings(&references, &shown);
        assert_eq!(
            captured,
            [
                TemplateReference::Unbound(Some("1".into())),
                TemplateReference::Unbound(None),
            ]
        );
        let mut fresh = shown;
        fresh.quick_launch_template = Some("new".into());
        fresh.template_shortcuts.insert("1".into(), "new".into());
        let templates = [LaunchTemplate {
            name: "new".into(),
            fields: TemplateFields::default(),
        }];
        assert!(
            resolve_template_references(&references, &fresh, &templates).is_some(),
            "premise: both previously absent bindings now resolve"
        );
        for unbound in captured {
            assert!(
                resolve_template_references(
                    &[TemplateReference::Name("new".into()), unbound,],
                    &fresh,
                    &templates
                )
                .is_none()
            );
        }
    }

    /// A Cmd-J row supplies only its name. A rename or deletion between the
    /// switcher snapshot and Enter cannot launch the old captured definition.
    #[test]
    fn name_reference_uses_the_current_catalog() {
        use farhelm_proto::launcher::TemplateFields;

        let preferences = crate::api::Preferences::default();
        let reference = [TemplateReference::Name("chosen".into())];
        let template = LaunchTemplate {
            name: "chosen".into(),
            fields: TemplateFields::default(),
        };
        assert_eq!(
            resolve_template_references(&reference, &preferences, std::slice::from_ref(&template)),
            Some(vec![template])
        );
        assert_eq!(
            resolve_template_references(&reference, &preferences, &[]),
            None
        );
    }
}
