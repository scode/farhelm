//! The launcher's single-choice host list, with the same focus ownership as
//! the model combobox. Browsing never changes the destination: only a pick
//! leaves this component, and the form owns every consequential transition.

use dioxus::prelude::*;
use web_time::Instant;

use super::shared::HostOption;
use crate::HostId;
use crate::icons::HostMark;
use crate::launch_controls::enter_choice;
use crate::peer::display_peer;

/// Keep repeated-key cycling distinct from a growing case-insensitive prefix.
///
/// `prefix` is the retained lowercase query and `key` is one typed character.
/// Lowercasing can expand that character (İ becomes i plus a combining dot),
/// so compare its complete folded spelling. A repeated key keeps one copy
/// of that spelling and tells the caller to search after the active host;
/// other keys extend the prefix and search from the beginning as before.
fn typeahead_query(prefix: &str, key: &str) -> (String, bool) {
    let letter = key.to_lowercase();
    let repeated = !prefix.is_empty() && prefix == letter;
    let query = if repeated {
        letter
    } else {
        format!("{prefix}{letter}")
    };
    (query, repeated)
}

/// Let an outside pointer press dismiss the open list even on inert padding.
///
/// Browsers do not blur a focused button when the pointer lands on a plain
/// container. The picker's Rust blur handler already discards provisional
/// browsing, so the document listener gives that handler the missing blur
/// without owning a second close path. It is installed once per document;
/// the launcher can unmount without leaving a reference to its component.
fn install_outside_blur() {
    document::eval(
        r#"if (!window.__farhelmHostPickerOutsideBlur) {
            window.__farhelmHostPickerOutsideBlur = true;
            document.addEventListener('pointerdown', (event) => {
                const host = document.querySelector('.launcher-host-picker > .create-session-host[aria-expanded="true"]');
                if (host && !host.parentElement.contains(event.target)) host.blur();
            }, true);
        }"#,
    );
}

/// A transient menu cannot outlive its launcher or the busy round trip.
///
/// The button owns keyboard focus and names its active option. Option rows
/// are not Tab stops; preventing pointer focus transfer preserves their click
/// until it commits. Blur, an outside pointer press, and Escape discard
/// browsing, while closed Enter retains the launcher's ordinary primary
/// action. Repeated letters cycle matching names; a pause starts a new prefix
/// without a timer or extra task.
#[component]
pub(super) fn HostPicker(
    hosts: Vec<HostOption>,
    selected: Option<HostId>,
    busy: bool,
    on_pick: EventHandler<HostId>,
    on_enter: EventHandler<()>,
) -> Element {
    let mut open = use_signal(|| false);
    let mut active = use_signal(|| 0_usize);
    let mut prefix = use_signal(String::new);
    let mut typed_at = use_signal(|| None::<Instant>);
    use_hook(install_outside_blur);
    use_effect(use_reactive!(|(busy,)| {
        if busy {
            open.set(false);
        }
    }));
    let chosen = hosts.iter().find(|host| Some(host.id) == selected);
    let committed_index = hosts
        .iter()
        .position(|host| Some(host.id) == selected)
        .unwrap_or(0);
    let index = active().min(hosts.len().saturating_sub(1));
    let showing = open() && !busy;
    let count = hosts.len();
    let keys = hosts.clone();
    use_effect(move || {
        if open() {
            let index = active();
            document::eval(&format!(
                "document.getElementById('launcher-host-option-{index}')?.scrollIntoView({{block:'nearest'}});"
            ));
        }
    });
    rsx! {
        div { class: "launcher-host-picker",
            button {
                class: "create-session-host", r#type: "button", role: "combobox",
                aria_label: "host", aria_haspopup: "listbox", aria_expanded: showing,
                aria_controls: "launcher-host-options",
                aria_activedescendant: (showing && count > 0).then(|| format!("launcher-host-option-{index}")),
                "data-host-id": selected.map(|id| id.to_string()).unwrap_or_default(),
                "data-tooltip": "host: the machine the session runs on",
                disabled: busy,
                onblur: move |_| { open.set(false); prefix.set(String::new()); },
                onclick: move |_| {
                    if busy { return; }
                    // Safari and the macOS desktop webview do not focus a
                    // button on click, and this menu closes on blur and is
                    // driven by the button's own keys. Without focus, a mouse
                    // opening would leave it deaf to arrows and Escape and
                    // unable to close on an outside click.
                    document::eval(
                        "document.querySelector('.launcher-host-picker > .create-session-host')?.focus()",
                    );
                    active.set(committed_index);
                    prefix.set(String::new());
                    open.set(!open());
                },
                onkeydown: move |event: KeyboardEvent| {
                    if event.is_composing() { return; }
                    let key = event.key();
                    if key == Key::Enter && !showing {
                        enter_choice(event, busy, || {}, Some(on_enter));
                        return;
                    }
                    if busy { return; }
                    if key == Key::Enter || key == Key::Character(" ".into()) {
                            event.prevent_default(); event.stop_propagation();
                            if event.is_auto_repeating() { return; }
                            if showing {
                                if let Some(host) = keys.get(index)
                                    && Some(host.id) != selected { on_pick.call(host.id); }
                                open.set(false);
                            } else { active.set(committed_index); open.set(true); }
                        return;
                    }
                    match key {
                        Key::Escape if showing => {
                            event.prevent_default(); event.stop_propagation(); open.set(false);
                        }
                        Key::ArrowDown | Key::ArrowUp | Key::Home | Key::End => {
                            event.prevent_default(); event.stop_propagation();
                            let next = match key {
                                Key::Home => 0,
                                Key::End => count.saturating_sub(1),
                                Key::ArrowUp if showing => index.saturating_sub(1),
                                Key::ArrowDown if showing => (index + 1).min(count.saturating_sub(1)),
                                _ => committed_index,
                            };
                            active.set(next); open.set(true); prefix.set(String::new());
                        }
                        Key::Character(key) if key.chars().count() == 1 && !event.modifiers().intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::META) => {
                            event.prevent_default();
                            // A closed menu has no browsing selection. Start at
                            // the committed host so an unmatched prefix cannot
                            // revive an old row and redirect Enter to it.
                            if !showing { active.set(committed_index); }
                            let now = Instant::now();
                            let fresh = typed_at().is_none_or(|previous| now.duration_since(previous).as_millis() > 1000) || !showing;
                            let old = if fresh { String::new() } else { prefix() };
                            let (query, repeated) = typeahead_query(&old, &key);
                            // The prefix is at most the longest offered label: longer
                            // input cannot match and need not grow retained UI state.
                            if query.len() <= keys.iter().map(|host| host.label().len()).max().unwrap_or(0) {
                                prefix.set(query.clone());
                            }
                            typed_at.set(Some(now));
                            let start = if repeated { index + 1 } else { 0 };
                            if let Some(found) = (0..count).map(|offset| (start + offset) % count)
                                .find(|position| keys[*position].label().to_lowercase().starts_with(&query)) {
                                active.set(found);
                            }
                            open.set(true);
                        }
                        _ => {}
                    }
                },
                if let Some(host) = chosen {
                    HostMark { icon: host.icon, color: host.color, local: host.local }
                    bdi { "{display_peer(&host.label())}" }
                }
                span { class: "launcher-host-chevron", aria_hidden: "true", "▾" }
            }
            if showing {
                div { id: "launcher-host-options", class: "launcher-host-options", role: "listbox", aria_label: "host",
                    if hosts.is_empty() { p { role: "status", "loading hosts…" } }
                    for (position, host) in hosts.iter().enumerate() {
                        button {
                            key: "{host.id}", id: "launcher-host-option-{position}",
                            class: "launcher-host-option", r#type: "button", role: "option", tabindex: "-1",
                            aria_selected: position == index, "data-host-id": "{host.id}",
                            disabled: busy,
                            onmousedown: move |event| event.prevent_default(),
                            onclick: { let id = host.id; move |_| {
                                if busy { return; }
                                if Some(id) != selected { on_pick.call(id); }
                                open.set(false);
                            } },
                            HostMark { icon: host.icon, color: host.color, local: host.local }
                            bdi { "{display_peer(&host.label())}" }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::typeahead_query;

    /// A lowercase expansion is still one typed key: the second İ must cycle
    /// rather than grow an unmatchable doubled query. ASCII repetition keeps
    /// the same contract, while distinct keys keep building a useful prefix.
    #[farhelm_testtrace::test]
    fn typeahead_cycles_complete_folded_keys_and_preserves_prefixes() {
        let first = typeahead_query("", "İ");
        assert_eq!(first, ("i\u{307}".into(), false));
        assert_eq!(typeahead_query(&first.0, "İ"), (first.0.clone(), true));

        let ascii = typeahead_query("", "A");
        assert_eq!(ascii, ("a".into(), false));
        assert_eq!(typeahead_query(&ascii.0, "a"), ("a".into(), true));
        assert_eq!(typeahead_query(&ascii.0, "B"), ("ab".into(), false));
        assert_eq!(typeahead_query("ab", "a"), ("aba".into(), false));
    }
}
