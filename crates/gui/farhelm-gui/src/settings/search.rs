//! Settings searches the app's words. The catalog is independent of which sections happen to be mounted, so typing
//! never loads a section or queries a host. Only opening a result runs the section's usual reads.

#[cfg(test)]
mod tests;

use std::ops::Range;

use farhelm_input_mapping::Platform;
use gpui::{
    AnyElement, App, Context, FontWeight, HighlightStyle, InteractiveElement as _, IntoElement,
    ParentElement as _, Role, ScrollHandle, StatefulInteractiveElement as _, Styled as _,
    StyledText, Subscription, Window, div, prelude::FluentBuilder as _, px,
};

use super::{
    Choice, HOST_CHOICES_HELP, SOUNDS_HELP, SOUNDS_ID, SOUNDS_LABEL, Section, SettingsDialog,
    TrustAgent, agent_trust, app, appearance, checkout_field, checkouts, containers, credentials,
    file_types, ntfy, recipes, tensorlake, tensorlake_key, tensorlake_sandboxes,
    tensorlake_ssh_key, terminal,
};
use crate::actions::Dismiss;
use crate::components::{
    HoverHelp, OptionRow, TextInput, TextInputEvent, TextInputState, Vertical, option_heading,
};
use crate::focus::{self, Mark};
use crate::layers;
use crate::theme::{self, Palette, metrics};

/// Where a result lands after showing its section. The live view owns the handle and knows when it can take focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Top,
    Choice(Choice),
    Trust(TrustAgent),
    AppUpdates,
    AppCheck,
    Tunnels,
    FileTypes,
    CheckoutFolder,
    PostClone,
    SpareCheckout,
    NtfyField(ntfy::Field),
    NtfySwitch(ntfy::Switch),
    TensorlakeKey(tensorlake_key::Place),
    TensorlakeAsk,
}

/// An entry's stable element id refers to the setting or its group, never to the position in the results.
struct Entry {
    section: Section,
    id: &'static str,
    label: &'static str,
    help: Vec<String>,
    target: Target,
}

impl Entry {
    fn new(
        section: Section,
        id: &'static str,
        label: &'static str,
        help: impl IntoIterator<Item = impl Into<String>>,
        target: Target,
    ) -> Self {
        Self {
            section,
            id,
            label,
            help: help.into_iter().map(Into::into).collect(),
            target,
        }
    }

    fn choice(choice: Choice) -> Self {
        let shared_help = (choice.section() == Section::Hosts).then_some(HOST_CHOICES_HELP);
        Self::new(
            choice.section(),
            choice.id(),
            choice.label(),
            [choice.tooltip(), choice.help(false), choice.help(true)]
                .into_iter()
                .chain(shared_help),
            Target::Choice(choice),
        )
    }

    /// Terms may match across different fields of the same entry. A section-name term matches all of its entries.
    fn matches(&self, words: &[String]) -> bool {
        let fields: Vec<_> = [self.section.label(), self.synonyms(), self.label]
            .into_iter()
            .chain(self.help.iter().map(String::as_str))
            .map(fold)
            .collect();
        words
            .iter()
            .all(|word| fields.iter().any(|field| field.contains(word)))
    }

    /// Prefer the help line that actually contains the query. Keep a little context when the first match would sit
    /// at the clipped end of the row. Synonym-only matches show the first ordinary help line.
    fn excerpt(&self, words: &[String]) -> (&str, bool) {
        let mut best = ("", 0, None);
        for line in &self.help {
            if line.is_empty() {
                continue;
            }
            let folded = fold(line);
            let count = words
                .iter()
                .filter(|word| folded.contains(word.as_str()))
                .count();
            if best.0.is_empty() || count > best.1 {
                best = (
                    line,
                    count,
                    highlights(line, words).first().map(|range| range.start),
                );
            }
        }
        let (line, _, first) = best;
        let Some(prefix) = first.map(|start| &line[..start]) else {
            return (line, false);
        };
        let Some((near, _)) = prefix.char_indices().rev().nth(30) else {
            return (line, false);
        };
        let start = prefix[..near]
            .char_indices()
            .rev()
            .find(|(_, ch)| ch.is_whitespace())
            .map_or(near, |(space, ch)| space + ch.len_utf8());
        (&line[start..], start > 0)
    }

    /// A synonym names the setting it leads to. A section name is the way to list the whole section.
    fn synonyms(&self) -> &'static str {
        match (self.section, self.target) {
            (Section::Appearance, _) => "dark mode light theme color colour",
            (Section::Hosts, Target::Choice(Choice::AutoUpdateRemoteHosts)) => {
                "update upgrade server"
            }
            (Section::Recipes, _) => "template environment snapshot",
            (Section::Tensorlake, Target::TensorlakeKey(_)) => "token secret api key cloud",
            (Section::Containers, _) => "podman",
            (Section::Credentials, _) => "token secret api key login",
            (Section::Terminal, Target::Tunnels) => "tunnel",
            (Section::Terminal, Target::Top) if self.id == terminal::ID => "links browser",
            (Section::Checkouts, _) => "clone git",
            (Section::AgentTrust, _) => "trust",
            (Section::Notifications, Target::Choice(_)) => "sound bell",
            (Section::Notifications, Target::Top) if self.id == SOUNDS_ID => "sound bell",
            (Section::Notifications, _) => "push phone",
            (Section::App, _) => "update upgrade version",
            _ => "",
        }
    }
}

/// Assemble the authored sections in display order, interleaving the trust hints with their own agents just like the
/// Settings pane. Switch entries come from the same lists the dialog uses for their state.
///
/// Only settings this build draws are listed: the tunnel switch exists only where [`terminal::shown_on`] says so.
/// Sections this build hides (App without an active updater) are filtered when matching, since that depends on
/// live state the catalog does not see.
fn catalog(platform: Platform) -> Vec<Entry> {
    let mut entries = Vec::new();
    for &section in Section::ALL {
        let add = |id, label, help: &[&str], target| {
            Entry::new(section, id, label, help.iter().copied(), target)
        };
        if section == Section::Notifications {
            entries.push(add(SOUNDS_ID, SOUNDS_LABEL, &[SOUNDS_HELP], Target::Top));
        }
        if section != Section::AgentTrust {
            entries.extend(
                Choice::ALL
                    .iter()
                    .copied()
                    .filter(|choice| choice.section() == section)
                    .map(Entry::choice),
            );
        }
        match section {
            Section::Sessions | Section::Hosts => {}
            Section::Appearance => entries.push(Entry::new(
                section,
                appearance::ID,
                appearance::LABEL,
                [appearance::HELP, appearance::TOOLTIP].into_iter().chain(
                    appearance::CHOICES
                        .iter()
                        .flat_map(|&(_, label, help)| [label, help]),
                ),
                Target::Top,
            )),
            Section::Recipes => entries.push(add(
                "recipes",
                section.label(),
                &[recipes::HELP],
                Target::Top,
            )),
            Section::Tensorlake => {
                use tensorlake::{ASK_HELP, ASK_ID, ASK_LABEL, ASK_TOOLTIP};
                use tensorlake_key::Place::Tensorlake;
                use tensorlake_sandboxes as sandboxes;
                use tensorlake_ssh_key as ssh;
                entries.extend([
                    add(
                        "tensorlake-experimental",
                        tensorlake::EXPERIMENTAL_NAME,
                        &[tensorlake::EXPERIMENTAL_WARNING, tensorlake::INTRO],
                        Target::Top,
                    ),
                    add(
                        "tensorlake-key-storage-warning",
                        credentials::STORAGE_LABEL,
                        &[tensorlake_key::STORAGE_WARNING],
                        Target::Top,
                    ),
                    key_entry(section, Tensorlake),
                    add(ssh::ID, ssh::LABEL, &[ssh::HELP], Target::Top),
                    add(
                        sandboxes::ID,
                        sandboxes::HEADING,
                        &[sandboxes::HELP],
                        Target::Top,
                    ),
                    add(
                        ASK_ID,
                        ASK_LABEL,
                        &[ASK_HELP, ASK_TOOLTIP, tensorlake::USAGE_CREDITS_NOTE],
                        Target::TensorlakeAsk,
                    ),
                ]);
            }
            Section::Containers => entries.extend([
                add(
                    "containers-hosts",
                    section.label(),
                    &[containers::HELP],
                    Target::Top,
                ),
                add(
                    "containers-leftovers-heading",
                    containers::LEFTOVERS_LABEL,
                    &[containers::LEFTOVERS_HELP],
                    Target::Top,
                ),
            ]),
            Section::Credentials => {
                entries.extend([
                    add(
                        credentials::STORAGE_ID,
                        credentials::STORAGE_LABEL,
                        &[credentials::STORAGE_HELP, credentials::INTRO],
                        Target::Top,
                    ),
                    key_entry(section, tensorlake_key::Place::Credentials),
                ]);
                entries.extend(
                    credentials::search_lists()
                        .map(|(id, label, help)| add(id, label, &[help], Target::Top)),
                );
            }
            Section::Terminal => {
                use terminal::{TUNNEL_HELP, TUNNEL_ID, TUNNEL_LABEL, TUNNEL_TOOLTIP};
                entries.push(Entry::new(
                    section,
                    terminal::ID,
                    terminal::LABEL,
                    [terminal::HELP, terminal::TOOLTIP].into_iter().chain(
                        terminal::CHOICES
                            .iter()
                            .flat_map(|&(_, label, help)| [label, help]),
                    ),
                    Target::Top,
                ));
                entries.extend([
                    add(
                        TUNNEL_ID,
                        TUNNEL_LABEL,
                        &[TUNNEL_HELP, TUNNEL_TOOLTIP],
                        Target::Tunnels,
                    ),
                    add(
                        file_types::ID,
                        file_types::label(platform),
                        &[file_types::HELP],
                        Target::FileTypes,
                    ),
                    add(
                        "settings-file-types-warning-html",
                        file_types::HTML_HEADING,
                        &[file_types::HTML_TEXT],
                        Target::Top,
                    ),
                    add(
                        "settings-file-types-warning-command",
                        file_types::OTHER_HEADING,
                        &[file_types::OTHER_TEXT],
                        Target::Top,
                    ),
                ]);
            }
            Section::Checkouts => {
                let folder = &checkout_field::DEFAULT_FOLDER;
                let post_clone = &checkout_field::POST_CLONE;
                entries.extend([
                    add(
                        folder.id,
                        folder.label,
                        &[folder.help, checkouts::FOLDER_NOTE],
                        Target::CheckoutFolder,
                    ),
                    add(
                        post_clone.id,
                        post_clone.label,
                        &[post_clone.help, checkout_field::COMMAND_NOTE],
                        Target::PostClone,
                    ),
                    add(
                        checkouts::SPARE_ID,
                        checkouts::SPARE_LABEL,
                        &[checkouts::SPARE_HELP, checkouts::SPARE_TOOLTIP],
                        Target::SpareCheckout,
                    ),
                ]);
            }
            Section::AgentTrust => {
                for agent in TrustAgent::ALL {
                    entries.push(Entry::new(
                        section,
                        agent.id(),
                        agent.label(),
                        std::iter::once(agent.tooltip().to_owned()).chain(agent.help()),
                        Target::Trust(agent),
                    ));
                    if agent.has_caution() {
                        entries.push(add(
                            agent_trust::CAUTION_ID,
                            agent_trust::CAUTION_TITLE,
                            &agent_trust::CAUTION,
                            Target::Top,
                        ));
                    }
                    entries.extend(agent.hint().map(Entry::choice));
                }
            }
            Section::Notifications => {
                let switch = |switch: ntfy::Switch| {
                    let (id, label, help) = switch.words();
                    add(id, label, &[help, ntfy::INTRO], Target::NtfySwitch(switch))
                };
                entries.push(switch(ntfy::Switch::Enabled));
                for field in ntfy::Field::ALL {
                    let (id, label, help) = field.words();
                    let extra: &[&str] = match field {
                        ntfy::Field::Server => &[],
                        ntfy::Field::Topic => &[ntfy::TOPIC_HELP],
                        ntfy::Field::Token => &[ntfy::TOKEN_CHANGED_HELP, ntfy::TOKEN_CLEAR_HELP],
                    };
                    entries.push(Entry::new(
                        section,
                        id,
                        label,
                        std::iter::once(help).chain(extra.iter().copied()),
                        Target::NtfyField(field),
                    ));
                }
                entries.push(switch(ntfy::Switch::ClearToken));
                entries.push(Entry::new(
                    section,
                    "ntfy-priority",
                    ntfy::PRIORITY_LABEL,
                    std::iter::once(ntfy::PRIORITY_HELP).chain(
                        ntfy::PRIORITIES
                            .into_iter()
                            .map(|priority| priority.display_label()),
                    ),
                    Target::Top,
                ));
            }
            Section::App => entries.extend([
                add(
                    app::CHECK_ID,
                    app::CHECK_LABEL,
                    &[app::CHECK_HELP],
                    Target::AppCheck,
                ),
                add(
                    app::ID,
                    app::LABEL,
                    &[app::TOOLTIP, app::HELP_ON, app::HELP_OFF],
                    Target::AppUpdates,
                ),
            ]),
        }
    }
    entries.retain(|entry| entry.target != Target::Tunnels || terminal::shown_on(platform));
    entries
}

fn key_entry(section: Section, place: tensorlake_key::Place) -> Entry {
    use tensorlake_key::{DASHBOARD_HELP, ID, KEY_HELP, REPLACE_HELP};
    Entry::new(
        section,
        ID,
        place.label(),
        [KEY_HELP, REPLACE_HELP, DASHBOARD_HELP, place.elsewhere()],
        Target::TensorlakeKey(place),
    )
}

fn fold(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// Highlight ranges refer to the source's UTF-8 bytes. A lowercased character can occupy a different number of
/// bytes or expand into two characters, so substring offsets in the folded text cannot be passed to StyledText.
fn highlights(text: &str, words: &[String]) -> Vec<Range<usize>> {
    let mut folded = String::new();
    let mut original = Vec::new();
    for (start, ch) in text.char_indices() {
        for lower in ch.to_lowercase() {
            folded.push(lower);
            original.extend(std::iter::repeat_n(
                start..start + ch.len_utf8(),
                lower.len_utf8(),
            ));
        }
    }
    let mut hits = Vec::new();
    for word in words {
        // Adjacent matches can share bytes, as two matches of "ana" do in "banana".
        hits.extend(
            folded
                .char_indices()
                .filter(|&(start, _)| folded[start..].starts_with(word))
                .map(|(start, _)| original[start].start..original[start + word.len() - 1].end),
        );
    }
    hits.sort_unstable_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for hit in hits {
        if let Some(last) = merged.last_mut().filter(|last| hit.start <= last.end) {
            last.end = last.end.max(hit.end);
        } else {
            merged.push(hit);
        }
    }
    merged
}

fn marked(text: &str, words: &[String], selected: bool, palette: &Palette) -> StyledText {
    let style = HighlightStyle {
        color: Some(theme::color(if selected {
            palette.on_fill
        } else {
            palette.accent
        })),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    StyledText::new(text.to_owned()).with_highlights(
        highlights(text, words)
            .into_iter()
            .map(|range| (range, style)),
    )
}

/// Search takes over the right pane while the query has words. The dialog still owns the underlying section and its
/// drafts, even though the section is temporarily undrawn.
pub(super) struct Search {
    input: TextInputState,
    catalog: Vec<Entry>,
    words: Vec<String>,
    matches: Vec<usize>,
    selected: usize,
    scroll: ScrollHandle,
    _events: Subscription,
}

impl Search {
    pub(super) fn new(
        platform: Platform,
        window: &mut Window,
        cx: &mut Context<SettingsDialog>,
    ) -> Self {
        let input = TextInputState::new(window, cx);
        input.set_placeholder("Search", window, cx);
        let events = input.subscribe(window, cx, |dialog, event, window, cx| match event {
            TextInputEvent::Changed => dialog.search_edited(cx),
            TextInputEvent::Submitted { held: false, .. } => dialog.open_search_result(window, cx),
            TextInputEvent::Submitted { held: true, .. } => {}
        });
        Self {
            input,
            catalog: catalog(platform),
            words: Vec::new(),
            matches: Vec::new(),
            selected: 0,
            scroll: ScrollHandle::new(),
            _events: events,
        }
    }

    #[cfg(test)]
    pub(super) fn set_platform(&mut self, platform: Platform, cx: &App) {
        self.catalog = catalog(platform);
        self.edited(cx);
    }

    pub(super) fn active(&self) -> bool {
        !self.words.is_empty()
    }

    /// The navigation count describes the same rows as the results pane.
    pub(super) fn section_count(&self, section: Section) -> usize {
        self.matches
            .iter()
            .filter(|&&index| self.catalog[index].section == section)
            .count()
    }

    /// Every edit selects the first result, including one that replaces a query with no matches.
    pub(super) fn edited(&mut self, cx: &App) {
        self.words = self.input.text(cx).split_whitespace().map(fold).collect();
        // Repeated terms impose the same requirement and would otherwise repeat highlight work on every redraw.
        self.words.sort_unstable();
        self.words.dedup();
        self.matches = if self.active() {
            self.catalog
                .iter()
                .enumerate()
                // A section this build hides offers nothing to open, so its settings are not results.
                .filter(|(_, entry)| {
                    entry.section.listed(cx)
                        && (entry.target != Target::Choice(Choice::ShowWhatsNew)
                            || crate::desktop::updates::upgrade_notes(cx).is_some())
                })
                .filter_map(|(index, entry)| entry.matches(&self.words).then_some(index))
                .collect()
        } else {
            Vec::new()
        };
        self.selected = 0;
        self.scroll.scroll_to_item(0);
    }

    /// Programmatically replacing field text raises no input event, so reset the results here too.
    pub(super) fn clear(&mut self, window: &mut Window, cx: &mut App) {
        self.input.set_text("", window, cx);
        self.edited(cx);
    }

    /// Set the words the golden captures, using the same recomputation as a user's edit.
    #[cfg(test)]
    pub(super) fn set_query(&mut self, query: &str, window: &mut Window, cx: &mut App) {
        self.input.set_text(query.to_owned(), window, cx);
        self.edited(cx);
    }

    /// The selected result in display order, or none while the query is blank or has no matches.
    pub(super) fn chosen(&self) -> Option<(Section, Target)> {
        let entry = &self.catalog[*self.matches.get(self.selected)?];
        Some((entry.section, entry.target))
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        focus::focus_in_layer(&self.input.focus_handle(cx), &Mark::take(cx), window, cx);
    }

    /// Result headings are scroll children but not keyboard stops. Count the ones above the choice when revealing it.
    fn step(&mut self, direction: Vertical, cx: &mut Context<SettingsDialog>) {
        self.selected = match direction {
            Vertical::Up => self.selected.saturating_sub(1),
            Vertical::Down => (self.selected + 1).min(self.matches.len().saturating_sub(1)),
        };
        if !self.matches.is_empty() {
            let sections = self.matches[..=self.selected]
                .chunk_by(|&left, &right| self.catalog[left].section == self.catalog[right].section)
                .count();
            self.scroll.scroll_to_item(self.selected + sections);
            cx.notify();
        }
    }

    /// The field handles Escape only while it owns focus and has text. Header and footer dismissal never pass here.
    pub(super) fn field(&self, cx: &Context<SettingsDialog>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let step = weak.clone();
        div()
            .id("settings-search-field")
            .debug_selector(|| "settings-search-field".into())
            .flex_none()
            .on_action(move |_: &Dismiss, window, cx| {
                let cleared = weak
                    .update(cx, |dialog, cx| {
                        if dialog.search.input.text(cx).is_empty() {
                            return false;
                        }
                        dialog.search.clear(window, cx);
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if cleared {
                    // The held repeats belong to this clear, even though the next fresh press closes Settings.
                    focus::note_closing(true, window, cx);
                } else {
                    cx.propagate();
                }
            })
            .child(
                TextInput::new(
                    "settings-search",
                    &self.input,
                    "Search Settings",
                    HoverHelp::tooltip("Find settings by name or description"),
                )
                .when(self.active(), |input| {
                    input.on_vertical(move |direction, _, cx| {
                        let _ = step.update(cx, |dialog, cx| dialog.search.step(direction, cx));
                    })
                }),
            )
    }

    /// Only the rows scroll. The heading, help and field stay in view as the arrow keys reveal more results.
    /// The help edge lives outside the scroll element so the scroll handle's child indices still address the rows.
    pub(super) fn pane(&self, window: &mut Window, cx: &mut Context<SettingsDialog>) -> AnyElement {
        let palette = theme::palette(cx);
        let query = self.input.text(cx);
        let count = self.matches.len();
        let heading = match count {
            0 => format!("No settings match “{}”", query.trim()),
            1 => format!("1 setting matches “{}”", query.trim()),
            _ => format!("{count} settings match “{}”", query.trim()),
        };
        let mut rows = Vec::new();
        let mut previous = None;
        for (position, &index) in self.matches.iter().enumerate() {
            let entry = &self.catalog[index];
            if previous != Some(entry.section) {
                previous = Some(entry.section);
                rows.push(option_heading(
                    format!("settings-search-group-{}", entry.section.key()),
                    entry.section.label().to_uppercase(),
                    palette,
                ));
            }
            rows.push(self.row(entry, position == self.selected, palette, cx));
        }
        div()
            .relative()
            .min_w_0()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(layers::beside_edge("settings-search-help-edge", window, cx))
            .child(
                div()
                    .id("settings-search-heading")
                    .debug_selector(|| "settings-search-heading".into())
                    .role(Role::Heading)
                    .aria_label(heading.clone())
                    .flex_none()
                    .min_w_0()
                    .truncate()
                    .text_size(px(super::HEADING_TEXT))
                    .font_weight(FontWeight::BOLD)
                    .child(heading),
            )
            .child(
                div()
                    .flex_none()
                    .min_w_0()
                    .truncate()
                    .text_color(theme::color(palette.fg_1))
                    .child(if count == 0 {
                        "Try a setting name or a word from its description."
                    } else {
                        "↑ ↓ to choose · Enter to open"
                    }),
            )
            .child(
                div()
                    .id("settings-search-results")
                    .role(Role::ListBox)
                    .aria_label("Matching settings")
                    .track_scroll(&self.scroll)
                    .flex()
                    .flex_col()
                    .flex_shrink_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows),
            )
            .into_any_element()
    }

    fn row(
        &self,
        entry: &Entry,
        selected: bool,
        palette: &Palette,
        cx: &Context<SettingsDialog>,
    ) -> AnyElement {
        let weak = cx.weak_entity();
        let (section, target) = (entry.section, entry.target);
        let (excerpt, clipped) = entry.excerpt(&self.words);
        let excerpt = if clipped {
            format!("…{excerpt}")
        } else {
            excerpt.to_owned()
        };
        OptionRow::new(
            format!("settings-result-{}-{}", section.key(), entry.id),
            entry.label,
            HoverHelp::tooltip(
                entry
                    .help
                    .first()
                    .map_or(entry.label, String::as_str)
                    .to_owned(),
            ),
            Role::ListBoxOption,
        )
        .selected(selected)
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(metrics::TEXT_MD)
                .child(marked(entry.label, &self.words, selected, palette)),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(if selected {
                    palette.on_accent_dim
                } else {
                    palette.fg_1
                }))
                .child(marked(&excerpt, &self.words, selected, palette)),
        )
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |dialog, cx| {
                dialog.open_search_target(section, target, window, cx)
            });
        })
        .into_any_element()
    }
}
