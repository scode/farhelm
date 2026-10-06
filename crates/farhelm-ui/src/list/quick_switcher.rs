//! One activity-ordered fleet snapshot for keyboard navigation.
//!
//! The sidebar may show only one host, so the switcher reads independently.
//! Matching partitions that snapshot without changing the helm's order inside
//! each tier. Selection is a navigation intent; ListView keeps the ordinary
//! busy guard, preference write and terminal focus handoff.

use dioxus::prelude::*;

use crate::api::{ListSort, SessionFilter, fetch_sessions};
use crate::hosts::settings_dialog::install_dialog_with_selector;
use crate::launch_composer::harness_label;
use crate::peer::display_peer;
use crate::status::{StatusBadgeView, status_badge};
use crate::{ApiBase, Session, SessionLaunch, modal_isolation};

const DIALOG_SELECTOR: &str = ".quick-switcher-dialog";

/// Wait for the closing render before navigation lets a terminal take focus.
///
/// This task belongs to ListView, which survives the dialog. Releasing inert
/// alone is insufficient: terminal.js also refuses focus while any modal DOM
/// remains mounted. Focus restoration is a separate step: an accepted jump
/// must leave focus free for the new terminal, rather than restoring an
/// editable opener that would veto the terminal's reveal handoff.
pub(super) async fn finish_close() {
    let js = format!(
        r#"{}
        const dialog = document.querySelector('.quick-switcher-dialog');
        if (dialog) await new Promise(resolve => {{
            const observer = new MutationObserver(() => {{
                if (!dialog.isConnected) {{ observer.disconnect(); resolve(); }}
            }});
            observer.observe(document.body, {{ childList: true, subtree: true }});
        }});
        return true;"#,
        modal_isolation::release_js(DIALOG_SELECTOR)
    );
    let _ = document::eval(&js).join::<bool>().await;
}

/// Consume the opener after the navigation gate has decided whether to act.
///
/// Cancellation, a refused pick and the current session owe the old control
/// its focus. A different accepted session owes its terminal the keyboard;
/// restoring a select or input would prevent that terminal from taking it.
pub(super) fn restore_focus(restore: bool) {
    document::eval(&format!(
        r#"
        const previous = window.__farhelmQuickSwitcherFocus;
        delete window.__farhelmQuickSwitcherFocus;
        if ({restore} && previous?.isConnected) previous.focus({{ preventScroll: true }});
    "#
    ));
}

/// Original character positions of a case-insensitive subsequence.
///
/// Unicode lowercase can expand a character (İ becomes i plus a combining
/// dot). This is lowercase matching, without Unicode normalization or full
/// case folding. Keeping the original position with every folded character prevents
/// highlights from shifting or slicing a UTF-8 character in half.
fn matching_positions(text: &str, query: &str) -> Option<Vec<usize>> {
    let mut folded = text
        .chars()
        .enumerate()
        .flat_map(|(index, ch)| ch.to_lowercase().map(move |ch| (index, ch)));
    let mut positions = Vec::new();
    for wanted in query.chars().flat_map(char::to_lowercase) {
        let (index, _) = folded.find(|(_, ch)| *ch == wanted)?;
        if positions.last() != Some(&index) {
            positions.push(index);
        }
    }
    Some(positions)
}

/// The matches to render in each independently isolated peer-text column.
struct SessionMatch {
    index: usize,
    title: Vec<usize>,
    host: Vec<usize>,
    cwd: Vec<usize>,
}

/// Rank title matches above metadata-only matches, retaining activity ties.
///
/// Input is already newest-activity first. Two stable buckets deliberately
/// avoid an additional sort or score, and there is no client-side result cap.
fn session_matches(sessions: &[Session], query: &str) -> Vec<SessionMatch> {
    let query = query.trim();
    let mut title_matches = Vec::new();
    let mut metadata_matches = Vec::new();
    for (index, session) in sessions.iter().enumerate() {
        let title = matching_positions(&session.title, query);
        let host = session
            .host_name
            .as_deref()
            .and_then(|host| matching_positions(host, query));
        let cwd = matching_positions(&session.cwd, query);
        if title.is_none() && host.is_none() && cwd.is_none() {
            continue;
        }
        let title_matched = title.is_some();
        let matched = SessionMatch {
            index,
            title: title
                .and_then(|_| matching_positions(&display_peer(&session.title), query))
                .unwrap_or_default(),
            host: host
                .and_then(|_| matching_positions(&display_peer(host_name(session)), query))
                .unwrap_or_default(),
            cwd: cwd
                .and_then(|_| matching_positions(&display_peer(&session.cwd), query))
                .unwrap_or_default(),
        };
        if title_matched {
            title_matches.push(matched);
        } else {
            metadata_matches.push(matched);
        }
    }
    title_matches.extend(metadata_matches);
    title_matches
}

/// Use the listing's denormalized host name, including unreachable hosts.
fn host_name(session: &Session) -> &str {
    session.host_name.as_deref().unwrap_or("unknown host")
}

/// Name the stored launch's agent without guessing from command text.
fn agent_label(session: &Session) -> &'static str {
    match &session.launch {
        Some(SessionLaunch::Agent { selection, .. }) => harness_label(selection.harness),
        Some(SessionLaunch::Command(command)) => command.agent.map_or("command", harness_label),
        _ => session
            .agent_kind
            .proto()
            .and_then(|kind| kind.display_name())
            .unwrap_or("command"),
    }
}

/// Coalesce highlight runs so a long legal title does not create one DOM
/// node per character. Positions are sorted original-character indices.
fn highlight_runs(text: &str, positions: &[usize]) -> Vec<(bool, String)> {
    let mut positions = positions.iter().copied().peekable();
    let mut runs: Vec<(bool, String)> = Vec::new();
    for (index, ch) in text.chars().enumerate() {
        let marked = positions.peek() == Some(&index);
        if marked {
            positions.next();
        }
        if let Some((previous, value)) = runs.last_mut()
            && *previous == marked
        {
            value.push(ch);
        } else {
            runs.push((marked, ch.to_string()));
        }
    }
    runs
}

/// Highlight escaped display text while isolating the whole peer value.
#[component]
fn MatchedText(text: String, positions: Vec<usize>) -> Element {
    rsx! {
        bdi {
            for (marked, value) in highlight_runs(&display_peer(&text), &positions) {
                if marked { mark { "{value}" } } else { "{value}" }
            }
        }
    }
}

/// Search all hosts once per open; loading is distinct from an empty match.
///
/// The input retains focus while arrows change its active row, so subsequent
/// typing always edits the search. Parent callbacks close before acting.
#[component]
pub(super) fn QuickSwitcher(on_close: EventHandler<()>, on_pick: EventHandler<Session>) -> Element {
    let base = use_context::<ApiBase>().0;
    let listing = use_resource(move || {
        let base = base.clone();
        async move { fetch_sessions(&base, &SessionFilter::default(), ListSort::Activity).await }
    });
    let mut query = use_signal(String::new);
    let mut selected = use_signal(|| 0_usize);
    let read = listing.read();
    let sessions = read.as_ref().and_then(|result| result.as_ref().ok());
    let matches = sessions
        .map(|listing| session_matches(&listing.sessions, &query()))
        .unwrap_or_default();
    let selected_index = selected().min(matches.len().saturating_sub(1));
    let picked = sessions.and_then(|listing| {
        matches
            .get(selected_index)
            .map(|row| listing.sessions[row.index].clone())
    });
    let count = matches.len();
    let key_pick = picked.clone();
    let scroll_selection = move || {
        document::eval(
            "requestAnimationFrame(() => document.querySelector('.quick-switcher-row[aria-selected=\"true\"]')?.scrollIntoView({ block: 'nearest' }));",
        );
    };

    rsx! {
        div { class: "quick-switcher-backdrop", role: "presentation",
            onclick: move |_| on_close.call(()),
            div {
                class: "quick-switcher-dialog", role: "dialog", aria_modal: "true",
                aria_label: "quick switcher", tabindex: "-1",
                onclick: move |event| event.stop_propagation(),
                onmounted: move |_| install_dialog_with_selector(
                    DIALOG_SELECTOR, ".quick-switcher-search", Some(".quick-switcher-close")
                ),
                onkeydown: move |event: KeyboardEvent| {
                    if !event.is_composing() && event.key() == Key::Escape {
                        event.prevent_default(); on_close.call(());
                    }
                },
                input {
                onkeydown: move |event: KeyboardEvent| {
                    if event.is_composing() { return; }
                    match event.key() {
                        Key::ArrowDown if count > 0 => {
                            event.prevent_default();
                            selected.set((selected_index + 1).min(count - 1));
                            scroll_selection();
                        }
                        Key::ArrowUp if count > 0 => {
                            event.prevent_default();
                            selected.set(selected_index.saturating_sub(1));
                            scroll_selection();
                        }
                        Key::Enter => {
                            event.prevent_default();
                            if let Some(session) = key_pick.clone() { on_pick.call(session); }
                        }
                        _ => {}
                    }
                },
                    class: "quick-switcher-search", aria_label: "search sessions",
                    "data-tooltip": "search sessions across every host",
                    role: "combobox", aria_autocomplete: "list", aria_expanded: "true", aria_controls: "quick-switcher-results",
                    aria_activedescendant: picked.as_ref().map(|_| format!("quick-switcher-result-{selected_index}")),
                    placeholder: "jump to a session", value: query(),
                    oninput: move |event| { query.set(event.value()); selected.set(0); },
                }
                div { class: "quick-switcher-heading",
                    if query().trim().is_empty() { "recent sessions" } else { "sessions" }
                }
                div { class: "quick-switcher-results", id: "quick-switcher-results", role: "listbox",
                    aria_label: "sessions",
                    match read.as_ref() {
                        None => rsx! { p { role: "status", "loading sessions…" } },
                        Some(Err(reason)) => rsx! { p { role: "alert", "Couldn't load sessions: " bdi { "{display_peer(reason)}" } } },
                        Some(Ok(listing)) => rsx! {
                            if matches.is_empty() { p { role: "status", "no sessions match" } }
                            for (position, matched) in matches.iter().enumerate() {
                                { let session = listing.sessions[matched.index].clone();
                                  let badge = status_badge(&session.status, session.annotation.as_deref(), session.has_unseen_output());
                                  let ended = badge.as_ref().is_some_and(|badge| badge.visible);
                                  rsx! {
                                    button {
                                        key: "{session.id}", class: "quick-switcher-row", r#type: "button", role: "option", tabindex: "-1",
                                        id: "quick-switcher-result-{position}", aria_selected: position == selected_index,
                                        "data-tooltip": "open this session",
                                        onclick: move |_| on_pick.call(session.clone()),
                                        if let Some(badge) = badge {
                                            span { class: if ended { "quick-switcher-status quick-switcher-ended" } else { "quick-switcher-status" },
                                                StatusBadgeView { badge, dot_onclick: move |_| {}, dot_title: None }
                                            }
                                        }
                                        span { class: "quick-switcher-title", MatchedText { text: listing.sessions[matched.index].title.clone(), positions: matched.title.clone() } }
                                        span { class: "quick-switcher-agent", "{agent_label(&listing.sessions[matched.index])}" }
                                        span { class: "quick-switcher-host", MatchedText { text: host_name(&listing.sessions[matched.index]).to_string(), positions: matched.host.clone() } }
                                        span { class: "quick-switcher-cwd", MatchedText { text: listing.sessions[matched.index].cwd.clone(), positions: matched.cwd.clone() } }
                                    }
                                  }
                                }
                            }
                        },
                    }
                }
                if sessions.is_some_and(|listing| listing.truncated) {
                    p { class: "quick-switcher-cap", "Only the most recently active sessions were searched; the helm's listing limit was reached." }
                }
                footer { class: "quick-switcher-footer",
                    span { "↑↓ choose · Enter open · Escape close" }
                    button { class: "quick-switcher-close", r#type: "button", "data-tooltip": "close the quick switcher",
                        onclick: move |_| on_close.call(()), "close" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fuzzy matching keeps character positions meaningful across Unicode
    /// case expansion, and never accepts query characters in reverse order.
    #[test]
    fn subsequences_preserve_original_character_positions() {
        assert_eq!(
            matching_positions("API-refactor", "apr"),
            Some(vec![0, 1, 4])
        );
        assert_eq!(matching_positions("İstanbul", "İS"), Some(vec![0, 1]));
        assert_eq!(matching_positions("café", "FÉ"), Some(vec![2, 3]));
        assert_eq!(matching_positions("api", "pa"), None);
        assert_eq!(matching_positions("any title", ""), Some(vec![]));
    }

    /// Long peer values remain a bounded number of render runs, rather than
    /// a node per character, while adjacent matches share one highlight.
    #[test]
    fn long_unmatched_text_stays_one_run() {
        let long = "x".repeat(100_000);
        assert_eq!(highlight_runs(&long, &[]), [(false, long)]);
        assert_eq!(
            highlight_runs("API-refactor", &[0, 1, 4]),
            [
                (true, "AP".into()),
                (false, "I-".into()),
                (true, "r".into()),
                (false, "efactor".into()),
            ]
        );
    }

    /// Metadata matches must not outrank any title match. Activity order
    /// remains the tiebreak even when titles have different substring shapes.
    #[test]
    fn title_matches_precede_metadata_without_reordering_activity_ties() {
        let session = |id: &str, title: &str, host: &str, cwd: &str| {
            serde_json::from_value::<Session>(serde_json::json!({
                "id": id, "title": title, "cwd": cwd, "invocation": "command",
                "host": 1, "host_name": host
            }))
            .unwrap()
        };
        let sessions = vec![
            session("host", "first", "api-host", "/work"),
            session("scattered", "a project implementation", "build", "/work"),
            session("contiguous", "api", "build", "/work"),
            session("folder", "last", "build", "/api"),
        ];
        let ids = |query| {
            session_matches(&sessions, query)
                .iter()
                .map(|row| sessions[row.index].id.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids("api"), ["scattered", "contiguous", "host", "folder"]);
        assert_eq!(ids(""), ["host", "scattered", "contiguous", "folder"]);
        assert!(ids("absent").is_empty());
        assert!(session_matches(&[session("empty", "", "build", "/work")], "empty").is_empty());
    }
}
