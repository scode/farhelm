//! Discover the keys this client handles without changing their input owners.
//!
//! OS-following chords and build-following chords deliberately use different
//! client facts. The dialog reads the OS once per opening and refreshes only
//! template bindings; unrelated preferences and existing listeners stay owned
//! by their current components. AppBar survives dismissal and returns focus
//! after the modal root has left, just as the quick switcher does.

use crate::api::Preferences;
use crate::launch_sequence::{ShortcutChord, TEMPLATE_SHORTCUTS};
use crate::modal_dialog::{
    DialogFocus, DialogHandle, DialogHeader, ModalDialog, RecoveryDelay, RecoveryFocus, SavedFocus,
    ScrollRegion,
};
use dioxus::prelude::*;

/// Rust labels remain a second copy of the inline JS matcher; browser tests
/// exercise the real chords so a future listener edit cannot drift silently.
pub(crate) const QUICK_SWITCHER: ShortcutChord = ShortcutChord::platform("KeyK");
/// Shared with the text-size buttons' existing tooltip hints.
pub(crate) const SMALLER_TEXT: ShortcutChord = text_size_chord("Minus");
/// The listener uses the physical Equal key, including layouts that print `+`.
pub(crate) const LARGER_TEXT: ShortcutChord = text_size_chord("Equal");
const OVERVIEW: ShortcutChord = ShortcutChord::platform("Slash");

/// Terminal zoom requires Shift on Mac too; unshifted Command stays page zoom.
const fn text_size_chord(code: &'static str) -> ShortcutChord {
    let mut chord = ShortcutChord::platform(code);
    chord.modifiers.mac.shift = true;
    chord
}

/// The surviving bar releases this root before removing it and returning focus.
pub(crate) const DIALOG: DialogHandle =
    DialogHandle::new(r#".keyboard-shortcuts-dialog[role="dialog"]"#);
/// Both entry paths capture their own opener before the dialog takes focus.
pub(crate) const OPENER: SavedFocus = SavedFocus::new("__farhelmKeyboardShortcutsFocus");

/// Reserve only the overview chord, before xterm and after composition fencing.
/// The relay exists only in the authenticated, preference-seeded tree. A modal
/// retains the keyboard; repeat is swallowed without opening another dialog.
pub(crate) fn install_shortcut() {
    document::eval(
        r#"if (!window.__farhelmKeyboardShortcutsInstalled) {
            window.__farhelmKeyboardShortcutsInstalled = true;
            window.addEventListener('keydown', event => {
                const mac = /Mac/.test(navigator.platform || '');
                const chord = mac
                    ? event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey
                    : event.ctrlKey && event.shiftKey && !event.metaKey && !event.altKey;
                if (!chord || event.code !== 'Slash' || event.isComposing) return;
                if (document.querySelector('[role="dialog"][aria-modal="true"]')) return;
                const trigger = document.querySelector('.keyboard-shortcuts-trigger');
                if (!trigger) return;
                event.preventDefault();
                event.stopPropagation();
                if (event.repeat || trigger.disabled) return;
                window.__farhelmKeyboardShortcutsFocus = document.activeElement;
                trigger.click();
            }, true);
        }"#,
    );
}

/// Describe one section's rows; peers are escaped before joining authored text.
/// This is presentation data only, never a replacement shortcut registry.
fn rows(
    preferences: &Preferences,
    mac_platform: bool,
) -> [(&'static str, Vec<(String, String)>); 3] {
    let keys = TEMPLATE_SHORTCUTS;
    let label = |chord: ShortcutChord| chord.label_for_client(keys.mac_desktop, mac_platform);
    let mut sessions = vec![
        ("Quick switcher".into(), label(QUICK_SWITCHER)),
        (
            "Delete session".into(),
            crate::SESSION_DELETE_SHORTCUT.into(),
        ),
        (
            "Rename session".into(),
            crate::SESSION_RENAME_SHORTCUT.into(),
        ),
        ("Keyboard shortcuts".into(), label(OVERVIEW)),
    ];
    if crate::HAS_NEW_SESSION_SHORTCUT {
        sessions.insert(0, ("New session".into(), "⌘N".into()));
    }
    let marked = preferences.quick_launch_template.as_ref().map_or_else(
        || "Quick launch: none marked".to_owned(),
        |name| format!("Quick launch: ⚡ {}", name.escaped()),
    );
    let mut launching = vec![
        ("Templates".into(), label(keys.template_list)),
        (marked, label(keys.bolt)),
        // Unassigned direct keys still open Templates. Keep their chord
        // discoverable without filling the dialog with ten empty bindings.
        (
            "Launch a numbered template".into(),
            keys.number_chord("0–9"),
        ),
    ];
    for key in keys.number.keys {
        if let Some(name) = preferences.template_shortcuts.get(key.value) {
            launching.push((
                format!("{} · {}", key.value, name.escaped()),
                keys.number_chord(key.value),
            ));
        }
    }
    launching.extend([
        ("Begin launch sequence".into(), label(keys.prefix)),
        (
            "In a sequence: type a template name".into(),
            "Characters (digits continue a word)".into(),
        ),
        ("In a sequence: queue the word".into(), "Space".into()),
        ("In a sequence: complete a unique name".into(), "Tab".into()),
        ("In a sequence: edit the word".into(), "Backspace".into()),
        (
            "In a sequence: add a numbered template (no word in progress)".into(),
            "1–9, 0".into(),
        ),
        (
            "In a sequence: add the ⚡ template".into(),
            label(keys.bolt),
        ),
        ("In a sequence: launch".into(), "Enter".into()),
        ("In a sequence: cancel".into(), "Escape".into()),
    ]);
    [
        ("Sessions", sessions),
        ("Launching", launching),
        (
            "Terminal",
            vec![
                ("Smaller terminal text".into(), label(SMALLER_TEXT)),
                ("Larger terminal text".into(), label(LARGER_TEXT)),
            ],
        ),
    ]
}

/// One read-only opening owns one platform read and one best-effort binding read.
/// Failed binding reads retain the shared cached names. The resource never
/// subscribes to that cache, so writing refreshed bindings cannot loop the read.
#[component]
pub(crate) fn KeyboardShortcuts(on_close: EventHandler<()>) -> Element {
    let base = use_context::<crate::ApiBase>().0;
    let mut preferences = use_context::<crate::list::SharedPreferences>();
    let _bindings = use_resource(move || {
        let base = base.clone();
        async move {
            if let Ok(fresh) = crate::api::fetch_preferences(&base).await {
                crate::list::apply_template_bindings(
                    &mut preferences.0.write(),
                    crate::api::template_bindings_after_read(fresh),
                );
            }
        }
    });
    let platform = use_resource(move || async move {
        document::eval("return /Mac/.test(navigator.platform || '');")
            .await
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    });
    let sections = platform
        .read()
        .as_ref()
        .map(|mac| rows(&preferences.0.read(), *mac));
    rsx! {
        div { class: "host-settings-backdrop keyboard-shortcuts-backdrop", role: "presentation",
            onclick: move |_| on_close.call(()),
            ModalDialog {
                handle: DIALOG,
                focus: DialogFocus::remembered(".keyboard-shortcuts-close", Some(".keyboard-shortcuts-close"), false, None),
                class: "app-settings-dialog host-dialog-framed keyboard-shortcuts-dialog",
                role: "dialog", aria_modal: "true", aria_label: "Keyboard shortcuts", tabindex: "-1",
                onclick: move |event: MouseEvent| event.stop_propagation(),
                on_escape: move |event: KeyboardEvent| {
                    if !event.is_composing() { on_close.call(()); }
                },
                DialogHeader {
                    title: rsx! { "Keyboard shortcuts" },
                    close: rsx! {
                        button { r#type: "button", class: "btn btn-neutral keyboard-shortcuts-close",
                            aria_label: "Close keyboard shortcuts", "data-tooltip": "Close: return to where you opened keyboard shortcuts",
                            onclick: move |_| on_close.call(()), "Close" }
                    },
                }
                ScrollRegion {
                    handle: DIALOG, selector: ".keyboard-shortcuts-body", name: "Keyboard shortcuts",
                    recovery: RecoveryFocus::new(".keyboard-shortcuts-close", None, false, RecoveryDelay::OneFrame),
                    class: "dialog-body host-settings-body keyboard-shortcuts-body",
                    if let Some(sections) = sections {
                        for (heading, entries) in sections {
                            section { class: "keyboard-shortcuts-section",
                                h3 { "{heading}" }
                                for (action, chord) in entries {
                                    div { class: "keyboard-shortcuts-row",
                                        span { class: "peer-value", dir: "ltr", "{action}" }
                                        span { class: "quick-switcher-template-keys", kbd { "{chord}" } }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
