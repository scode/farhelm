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
    /// Describe an OS-following chord without changing any listener's matcher.
    pub(crate) const fn platform(code: &'static str) -> Self {
        Self {
            code,
            modifiers: ShortcutModifiers {
                source: ModifierScheme::Platform,
                mac: COMMAND,
                other: CONTROL_SHIFT,
            },
        }
    }

    /// Show both platform alternatives when the web client's OS is unknown.
    pub(crate) fn label(self, mac_desktop: bool) -> String {
        chord_label(self.code, self.modifiers, mac_desktop)
    }

    /// Show only this client's alternative, keeping OS and build decisions distinct.
    /// A Mac browser uses Command for platform chords but browser-safe modifiers
    /// for build-following ones. Callers read the platform once per opening.
    pub(crate) fn label_for_client(self, mac_desktop: bool, mac_platform: bool) -> String {
        let mac = match self.modifiers.source {
            ModifierScheme::Platform => mac_platform,
            ModifierScheme::MacDesktopBuild => mac_desktop,
        };
        let mask = if mac {
            self.modifiers.mac
        } else {
            self.modifiers.other
        };
        format!("{}{}", modifier_label(mask), key_label(self.code))
    }
}

/// Physical punctuation codes need their printed symbol in a key hint.
fn key_label(code: &str) -> &str {
    match code {
        "Slash" => "/",
        "Equal" => "+",
        "Minus" => "−",
        _ => code
            .strip_prefix("Key")
            .or_else(|| code.strip_prefix("Digit"))
            .unwrap_or(code),
    }
}

/// Render a physical code or a generic number placeholder by the same rule.
fn chord_label(code: &str, modifiers: ShortcutModifiers, mac_desktop: bool) -> String {
    let key = key_label(code);
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

/// Keep the initiating gesture through dispatch so a fallback explains the
/// action the user actually took. Only physical shortcuts own a JS press snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuickLaunchOrigin {
    Number,
    Bolt,
    TemplateList,
    QuickSwitcher,
    Sequence,
}

impl QuickLaunchOrigin {
    /// Physical relays have queued the press-time DOM snapshot; picker rows
    /// instead observe ownership after their dialog has finished closing.
    pub(crate) fn keyboard(self) -> bool {
        matches!(self, Self::Number | Self::Bolt | Self::Sequence)
    }

    /// Incomplete sequences already used the prefix; telling them to use it
    /// again conceals the actual problem, the combined setup still lacking a choice.
    /// Dispatch supplies a nonempty resolved batch; non-sequence origins hold one template.
    pub(crate) fn incomplete_banner(self, templates: &[LaunchTemplate]) -> PartialTemplateBanner {
        match self {
            Self::Sequence => PartialTemplateBanner::Sequence,
            Self::Number => PartialTemplateBanner::Number(templates[0].name.clone()),
            Self::Bolt | Self::TemplateList | Self::QuickSwitcher => {
                PartialTemplateBanner::Template(templates[0].name.clone())
            }
        }
    }
}

/// What the shortcut worker owes one relay click: nothing, only the press's
/// JS snapshot, or that snapshot plus a launch.
///
/// The page's listener saves one snapshot per keyboard relay it clicks (a
/// direct shortcut, or a sequence's Enter), and the worker takes snapshots
/// from that FIFO one per keyboard entry, in queue order. Nothing else ties a
/// snapshot to its press, so every keyboard click must produce exactly one
/// worker entry. An entry that cannot launch still has to take its snapshot,
/// or each later batch would take the one before it and could launch with
/// names an earlier press displayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShortcutAdmission {
    /// A picker row refused at admission. It never had a snapshot.
    Ignore,
    /// Take and discard this press's snapshot without launching anything.
    Drain,
    /// Take this press's snapshot and resolve the batch.
    Dispatch,
}

/// Decide what one relay click enqueues, given whether live state refuses
/// new work right now.
///
/// An empty batch only comes from a sequence's Enter whose queue the app never
/// filled: Begin was refused from live state while the page, rendered before
/// that state, still armed and queued entries. The page captured a snapshot
/// for that Enter all the same, so it drains like any refused keyboard press.
pub(crate) fn shortcut_admission(
    origin: QuickLaunchOrigin,
    references: &[TemplateReference],
    refused: bool,
) -> ShortcutAdmission {
    if !refused && !references.is_empty() {
        ShortcutAdmission::Dispatch
    } else if origin.keyboard() {
        ShortcutAdmission::Drain
    } else {
        ShortcutAdmission::Ignore
    }
}

/// An opening-owned hint, separate from launch refusal or remembered draft state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PartialTemplateBanner {
    Number(String),
    Template(String),
    Sequence,
}

impl PartialTemplateBanner {
    /// Render peer names visibly without letting their formatting impersonate
    /// the app's explanation. The existing banner owns expiry and dismissal.
    pub(crate) fn text(&self) -> String {
        match self {
            Self::Number(name) => format!(
                "{} cannot launch on its own. Use {} to combine template keys, then Enter to launch, or complete this setup below.",
                display_peer(name),
                TEMPLATE_SHORTCUTS
                    .prefix
                    .label(TEMPLATE_SHORTCUTS.mac_desktop)
            ),
            Self::Template(name) => format!(
                "{} needs more choices before it can launch; complete it below.",
                display_peer(name)
            ),
            Self::Sequence => {
                "This sequence doesn't make a complete setup; complete it below.".into()
            }
        }
    }
}

impl TemplateReference {
    /// Read the binding used by the indicator and by sequence capture alike.
    /// An explicitly unbound reference cannot gain authority from newer settings.
    /// Return a raw lookup key; the indicator still escapes this borrowed spelling.
    fn name<'a>(&'a self, preferences: &'a crate::api::Preferences) -> Option<&'a str> {
        match self {
            // References are exact lookup keys, including the DOM snapshot
            // captured at Enter; display escaping must not change that key.
            Self::Bolt => preferences
                .quick_launch_template
                .as_ref()
                .map(crate::peer::PeerText::raw),
            Self::Number(key) => preferences
                .template_shortcuts
                .get(key)
                .map(crate::peer::PeerText::raw),
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
                TemplateReference::Name(name.to_owned())
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
    /// JSON copied from the indicator's DOM at physical Enter, before Rust
    /// can observe a preference read whose render has not reached the page.
    pub(crate) sequence_bindings: Option<String>,
}

impl ShortcutContextSnapshot {
    /// Missing publication grants no sequence authority. Direct shortcuts
    /// never call this: they intentionally resolve fresh bindings instead.
    pub(crate) fn capture_sequence(
        &self,
        references: &[TemplateReference],
    ) -> Option<Vec<TemplateReference>> {
        let preferences =
            serde_json::from_str::<crate::api::Preferences>(self.sequence_bindings.as_deref()?)
                .ok()?;
        Some(capture_sequence_bindings(references, &preferences))
    }
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

/// Drop every press snapshot the page still holds for the shortcut worker.
///
/// The page's listener is installed once per window, but the worker that
/// takes its snapshots, and the queue of entries that owe them, belong to the
/// sidebar list and die when it unmounts (the token prompt). Call this from
/// that unmount so both sides of the one-to-one pairing restart empty
/// together. Fire and forget: nothing is left to receive a result.
pub(crate) fn clear_shortcut_presses() {
    document::eval("window.farhelmClearTemplateShortcutPresses?.()");
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
                .find(|template| template.name == name)
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
    let references = sequence();
    // One render publishes both names and their raw bindings. A separate
    // component could publish newer bindings before these labels arrived.
    let preferences = preferences.0.read();
    let bindings = serde_json::json!({
        "quick_launch_template": preferences.quick_launch_template,
        "template_shortcuts": preferences.template_shortcuts,
    })
    .to_string();
    let names = references
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|reference| {
            reference
                .name(&preferences)
                .map(display_peer)
                .unwrap_or_else(|| match reference {
                    TemplateReference::Number(key) | TemplateReference::Unbound(Some(key)) => {
                        format!("Unbound key {key}")
                    }
                    _ => "Unbound ⚡".into(),
                })
        })
        .collect::<Vec<_>>();
    rsx! {
        // Remain mounted while idle: rapid prefix/digit/Enter can complete
        // before the first pill render, using the last published bindings.
        span { class: "launch-sequence-bindings", hidden: true, "data-bindings": bindings }
        if let Some(references) = references {
        // Centered over the terminal above link destination previews
        // (app.css's z-index ladder) and transparent to the pointer, so a link
        // can still be clicked beside or under it: a link cover. A sequence
        // can be armed while a link press is held, so this can appear between
        // a link's hover and its release.
        div { class: "launch-sequence-indicator", role: "status", "data-link-cover": "true",
            "data-in-launcher": if in_launcher() { "true" } else { "false" },
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Physical Enter captures raw template names from the published DOM JSON.
    /// A visible escape in the indicator must not replace that lookup identity,
    /// and repeated references must still resolve to repeated applications.
    #[test]
    fn dom_binding_snapshot_captures_original_peer_names() {
        let snapshot = ShortcutContextSnapshot {
            context: "page".into(),
            modal_opening: 1,
            sequence_bindings: Some(
                r#"{"quick_launch_template":"bolt\u202e","template_shortcuts":{"1":"one\n"}}"#
                    .into(),
            ),
        };
        let references = [
            TemplateReference::Number("1".into()),
            TemplateReference::Bolt,
            TemplateReference::Number("1".into()),
        ];
        assert_eq!(
            snapshot.capture_sequence(&references),
            Some(vec![
                TemplateReference::Name("one\n".into()),
                TemplateReference::Name("bolt\u{202e}".into()),
                TemplateReference::Name("one\n".into()),
            ])
        );
    }

    /// A press carries only the two published preference fields, not the whole
    /// settings model. Later dispatch reads cannot bind a key the page showed
    /// as unbound, and a missing or malformed projection grants no authority.
    #[test]
    fn press_captures_only_published_bindings() {
        let press = ShortcutContextSnapshot {
            context: "page".into(),
            modal_opening: 0,
            sequence_bindings: Some(
                r#"{"quick_launch_template":"shown-bolt","template_shortcuts":{"1":"shown-one"}}"#
                    .into(),
            ),
        };
        let references = [
            TemplateReference::Number("1".into()),
            TemplateReference::Bolt,
            TemplateReference::Number("2".into()),
        ];
        assert_eq!(
            press.capture_sequence(&references),
            Some(vec![
                TemplateReference::Name("shown-one".into()),
                TemplateReference::Name("shown-bolt".into()),
                TemplateReference::Unbound(Some("2".into())),
            ])
        );
        for projection in [None, Some("not-json".into())] {
            assert!(
                ShortcutContextSnapshot {
                    sequence_bindings: projection,
                    ..press.clone()
                }
                .capture_sequence(&references)
                .is_none()
            );
        }
    }

    /// The same incomplete template needs different advice for a number key,
    /// an ordinary picker, and a sequence that already used the prefix.
    #[test]
    fn incomplete_hints_explain_the_initiating_action() {
        let templates = [LaunchTemplate {
            name: "model-only".into(),
            fields: Default::default(),
        }];
        let number = QuickLaunchOrigin::Number
            .incomplete_banner(&templates)
            .text();
        assert!(number.starts_with("model-only cannot launch on its own. Use "));
        assert!(
            number.contains(
                &TEMPLATE_SHORTCUTS
                    .prefix
                    .label(TEMPLATE_SHORTCUTS.mac_desktop)
            )
        );
        for origin in [
            QuickLaunchOrigin::Bolt,
            QuickLaunchOrigin::TemplateList,
            QuickLaunchOrigin::QuickSwitcher,
        ] {
            assert_eq!(
                origin.incomplete_banner(&templates).text(),
                "model-only needs more choices before it can launch; complete it below."
            );
        }
        for batch in [
            templates.to_vec(),
            vec![templates[0].clone(), templates[0].clone()],
        ] {
            assert_eq!(
                QuickLaunchOrigin::Sequence.incomplete_banner(&batch).text(),
                "This sequence doesn't make a complete setup; complete it below."
            );
        }
    }

    /// The worker pairs JS snapshots with entries purely by order, so a refused
    /// or empty sequence's Enter must still enqueue one entry that takes its
    /// own snapshot. Before this held, the app dropped such an Enter and the
    /// next batch launched with the names the refused sequence had shown.
    ///
    /// This pins only the admission decision: refused and empty keyboard
    /// clicks come back as `Drain`, never `Ignore`. The loop below imitates
    /// the worker's pairing to show what that decision implies; it does not run
    /// the launch relay or the worker. Their part is the browser test "a
    /// sequence refused at its prefix leaves later sequences their own
    /// bindings" in `e2e/tests/quick-launch.spec.ts`.
    #[test]
    fn refused_sequence_drains_its_own_snapshot() {
        let clicks = [
            // Begin was refused from live state while the page showed "1",
            // so the app's queue is empty at Enter.
            ("refused", QuickLaunchOrigin::Sequence, vec![], false),
            (
                "launched",
                QuickLaunchOrigin::Sequence,
                vec![TemplateReference::Number("2".into())],
                false,
            ),
            (
                "busy",
                QuickLaunchOrigin::Bolt,
                vec![TemplateReference::Bolt],
                true,
            ),
            (
                "bolt",
                QuickLaunchOrigin::Bolt,
                vec![TemplateReference::Bolt],
                false,
            ),
        ];
        let mut snapshots: std::collections::VecDeque<_> =
            clicks.iter().map(|(snapshot, ..)| *snapshot).collect();
        let mut launched = Vec::new();
        for (_, origin, references, refused) in &clicks {
            let admission = shortcut_admission(*origin, references, *refused);
            if admission == ShortcutAdmission::Ignore {
                continue;
            }
            let snapshot = snapshots.pop_front();
            if admission == ShortcutAdmission::Dispatch {
                launched.push(snapshot);
            }
        }
        assert_eq!(launched, [Some("launched"), Some("bolt")]);
        assert!(snapshots.is_empty(), "every snapshot is consumed once");
    }

    /// Picker rows read their context afresh rather than from the press FIFO,
    /// so a refused one must enqueue nothing that would take a snapshot.
    #[test]
    fn refused_picker_row_enqueues_nothing() {
        for origin in [
            QuickLaunchOrigin::TemplateList,
            QuickLaunchOrigin::QuickSwitcher,
        ] {
            assert_eq!(
                shortcut_admission(origin, &[TemplateReference::Bolt], true),
                ShortcutAdmission::Ignore
            );
            assert_eq!(
                shortcut_admission(origin, &[TemplateReference::Bolt], false),
                ShortcutAdmission::Dispatch
            );
        }
    }

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

    /// Mac browsers must keep browser-safe number keys while OS-following keys
    /// use Command. Test both independent facts rather than the test host's OS.
    #[test]
    fn client_labels_distinguish_platform_from_desktop_build() {
        for mac_desktop in [false, true] {
            for mac_platform in [false, true] {
                let keys = TEMPLATE_SHORTCUTS;
                assert_eq!(
                    keys.bolt.label_for_client(mac_desktop, mac_platform),
                    if mac_platform { "⌘P" } else { "Ctrl+Shift+P" }
                );
                assert_eq!(
                    keys.prefix.label_for_client(mac_desktop, mac_platform),
                    if mac_desktop { "⌘X" } else { "Ctrl+Shift+X" }
                );
            }
        }
        assert_eq!(
            crate::keyboard_shortcuts::SMALLER_TEXT.label_for_client(false, true),
            "⌘+Shift+−"
        );
        assert_eq!(
            crate::keyboard_shortcuts::LARGER_TEXT.label_for_client(false, true),
            "⌘+Shift++"
        );
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
