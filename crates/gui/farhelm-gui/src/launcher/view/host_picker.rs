//! The address field's Host picker. Its button shows the host or new environment the draft will use. One popover
//! lists existing hosts and the new environments the launcher can create from here.
//!
//! How it behaves, as the old one did:
//!
//! - **Closed**, the button shows the chosen host's mark, its name, and its phase in parentheses while it is not
//!   connected. Enter on it presses the main action, under the choice controls' Enter rule; a click, Space or an arrow
//!   opens the list.
//! - **Open**, the list shows every host in registry order (hosts owned by an environment only when already chosen),
//!   then the offered new environments. Unavailable hosts still show their phase and can be taken. The list is the
//!   layer manager's non-focusing popover: focus and every key stay on the
//!   button. Up and Down move the highlight (stopping at the ends), Home and End jump, a typed letter jumps to the next
//!   host whose name starts with what was typed in the last second (the same letter again cycles among them), Enter or
//!   Space picks the highlighted host, and a click picks the one clicked. Escape, a press outside and focus leaving the
//!   button close it without a choice.
//! - **Held** (Restart with and Replace with keep the source's host), the row shows the host as text. Hover help
//!   has room for the name when the address clips it.
//!
//! Browsing never changes the draft. A pick of an existing host re-seeds an untouched folder through
//! [`LaunchDraft::choose_host`]. New environments use the draft's placement choice and start in the environment home.
//!
//! New uses the open session's folder when it is on the chosen installation. Otherwise it uses the folder from
//! the chosen host's latest ordinary folder launch, or `~` if none is recorded or the history read fails
//! (`Launcher::settle_folder_default`).
//!
//! [`LaunchDraft::choose_host`]: crate::launcher::draft::LaunchDraft::choose_host

use std::time::{Duration, SystemTime};

use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchTarget, ProviderKind};
use gpui::{
    AnyElement, App, AppContext as _, Context, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, Window, div, prelude::FluentBuilder as _, px, svg,
};

use super::Launcher;
use crate::assets::Icon;
use crate::components::{
    Button, HoverHelp, OptionRow, Role, Tier, option_heading, with_hover_help,
};
use crate::environment_ui::ProviderUi;
use crate::focus::{self, Mark};
use crate::host_text::HostSentence;
use crate::hosts::identity::{display_name, host_mark};
use crate::launcher::draft::{Field, RunsOn};
use crate::launcher::entry::default_folder;
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::local_state;
use crate::theme::{self, metrics};

/// The menu can be wide enough to name a longer host without widening the address field's Host segment.
const PICKER_MIN_WIDTH: f32 = 168.;
/// How long typed letters keep adding to one name prefix before the next letter starts a new one (the old picker's
/// one second).
const TYPEAHEAD: Duration = Duration::from_secs(1);
/// Leave room for the two group headings and both environment offers with three existing hosts. Longer menus scroll.
const LIST_MAX_HEIGHT: f32 = 280.;
/// The list's widest, the old `max-width: 30em` at 14-pixel text; a longer host name wraps in its row.
const LIST_MAX_WIDTH: f32 = 420.;
const LIST_PAD: f32 = 4.;
/// The host mark's size in the button and the rows (`.host-mark` in the launcher), and its gap to the name.
const MARK: f32 = 14.;
const MARK_GAP: f32 = 8.;

/// What the host picker holds besides the draft: the open list, the highlighted host, the typed prefix and the opening's
/// note. Nothing here is shared with another surface.
#[derive(Default)]
pub(super) struct HostPicker {
    /// The open list's layer, if open; the layer manager may close it on its own, so [`Launcher::host_list_open`] asks.
    layer: Option<LayerId>,
    /// The highlighted choice's identity. New hosts and account replies can change the rows while the list is open.
    active: Option<HostChoiceId>,
    /// What was typed for typeahead, lower-cased, and when its last letter came (the ticker's wall clock, which tests
    /// control).
    typed: Option<(String, SystemTime)>,
    /// Where the button is drawn, for the list to open under it; refreshed every frame.
    anchor: Option<Anchor>,
    /// Why the opening did not keep its source's host, said under the row until the user picks a host
    /// ([`super::open_noted`]): once they have, the host is their choice and the note no longer describes it, as the old
    /// launcher stopped saying it once the user took the host over.
    pub(super) note: Option<&'static str>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HostChoiceId {
    Existing(i64),
    New(ProviderKind),
}

/// A pick offered by the current store and account. Rendering and keys use the same ordered list.
#[allow(
    clippy::large_enum_variant,
    reason = "a short list built per frame for the Host menu, never stored; boxing would only add allocations"
)]
enum HostChoice {
    Existing(HostView),
    New(LaunchTarget),
}

impl HostChoice {
    fn id(&self) -> HostChoiceId {
        match self {
            Self::Existing(host) => HostChoiceId::Existing(host.id),
            Self::New(target) => HostChoiceId::New(target.provider()),
        }
    }

    /// Match a menu row by its name. Phase, description and qualification do not affect typeahead.
    fn typeahead_name(&self) -> String {
        match self {
            Self::Existing(host) => display_name(host).raw().to_lowercase(),
            Self::New(target) => ProviderUi(target.provider()).segment().to_lowercase(),
        }
    }
}

impl Launcher {
    /// The hosts the list offers, in the registry's order: every host but those a disposable environment owns, unless
    /// that one is chosen already (the old list's rule: an environment's host is not a place to start an unrelated
    /// session).
    fn host_options(&self, cx: &App) -> Vec<HostChoice> {
        self.store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .into_iter()
            .filter(|host| !host.is_session_owned || Some(host.id) == self.draft.host)
            .cloned()
            .map(HostChoice::Existing)
            .chain(
                self.offered_environments(cx)
                    .into_iter()
                    .map(HostChoice::New),
            )
            .collect()
    }

    /// Whether the host list is open.
    pub(super) fn host_list_open(&self, window: &Window, cx: &App) -> bool {
        self.host_picker
            .layer
            .is_some_and(|layer| layers::is_open(layer, window, cx))
    }

    /// Start a new browse at the current choice, or the first offering if the current one is unavailable.
    fn opening_highlight(&self, choices: &[HostChoice]) -> Option<HostChoiceId> {
        let chosen = self.draft.target.as_ref().map_or_else(
            || self.draft.host.map(HostChoiceId::Existing),
            |target| Some(HostChoiceId::New(target.provider())),
        );
        chosen
            .filter(|id| choices.iter().any(|choice| choice.id() == *id))
            .or_else(|| choices.first().map(HostChoice::id))
    }

    /// A key on the closed or open host button. Keys it takes stop here. Tab and Escape never reach it: they are key
    /// bindings, handled first (Tab moves focus, which closes the list through the button's blur; Escape closes the
    /// open list before the launcher's dialog sees it). A modified key other than Shift goes on, and so do
    /// Shift+Enter and Shift+Space (the shared key rule: a modified key is not an activation).
    fn host_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = &event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
            return;
        }
        let key = event.keystroke.key.as_str();
        if modifiers.shift && (key == "enter" || key == "space") {
            return;
        }
        let choices = self.host_options(cx);
        let open = self.host_list_open(window, cx);
        match key {
            "enter" if !open => {
                // The choice controls' Enter rule (`components::enter_choice`) with nothing to apply, since the button
                // shows the host in force: press the main action, once, and take the key, held or not. Done here rather
                // than through `enter_choice`, whose main action would update this launcher again from inside its own
                // update.
                cx.stop_propagation();
                if !event.is_held {
                    self.press(window, cx);
                }
            }
            "enter" | "space" => {
                cx.stop_propagation();
                if event.is_held {
                    return;
                }
                if open {
                    if let Some(id) = self.host_picker.active {
                        self.pick_host_choice(id, window, cx);
                    }
                    self.close_host_list(window, cx);
                } else {
                    let active = self.opening_highlight(&choices);
                    self.open_host_list(active, window, cx);
                }
            }
            "down" | "up" | "home" | "end" => {
                cx.stop_propagation();
                let last = choices.len().saturating_sub(1);
                let index = choices
                    .iter()
                    .position(|choice| Some(choice.id()) == self.host_picker.active);
                let active = match (key, open, index) {
                    ("home", _, _) => choices.first().map(HostChoice::id),
                    ("end", _, _) => choices.last().map(HostChoice::id),
                    ("up", true, Some(index)) => {
                        choices.get(index.saturating_sub(1)).map(HostChoice::id)
                    }
                    ("down", true, Some(index)) => {
                        choices.get((index + 1).min(last)).map(HostChoice::id)
                    }
                    _ => self.opening_highlight(&choices),
                };
                self.host_picker.typed = None;
                if open {
                    self.host_picker.active = active;
                } else {
                    self.open_host_list(active, window, cx);
                }
            }
            _ => {
                let Some(letter) = event
                    .keystroke
                    .key_char
                    .as_deref()
                    .filter(|text| text.chars().count() == 1)
                else {
                    return;
                };
                cx.stop_propagation();
                self.type_ahead(letter, &choices, open, window, cx);
            }
        }
        cx.notify();
    }

    /// A typed letter: extend the prefix typed within the last second (or start over), and highlight the next host
    /// whose name starts with it; the same letter again cycles among the hosts starting with it (old
    /// `typeahead_query`). Opens the list.
    fn type_ahead(
        &mut self,
        letter: &str,
        choices: &[HostChoice],
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let now = crate::ticker::wall_now(cx);
        let letter = letter.to_lowercase();
        if !open {
            // A closed list has no browsing highlight: start from the chosen host, so an unmatched prefix cannot
            // revive an old one.
            self.host_picker.active = self.opening_highlight(choices);
            self.host_picker.typed = None;
        }
        let previous = self
            .host_picker
            .typed
            .take()
            .filter(|(_, at)| now.duration_since(*at).unwrap_or_default() <= TYPEAHEAD)
            .map(|(prefix, _)| prefix)
            .unwrap_or_default();
        let repeated = !previous.is_empty() && previous == letter;
        let names: Vec<_> = choices.iter().map(HostChoice::typeahead_name).collect();
        let longest = names.iter().map(String::len).max().unwrap_or(0);
        let query = if repeated {
            letter
        } else {
            format!("{previous}{letter}")
        };
        let start = if repeated {
            choices
                .iter()
                .position(|choice| Some(choice.id()) == self.host_picker.active)
                .map_or(0, |index| index + 1)
        } else {
            0
        };
        if let Some(found) = (0..names.len())
            .map(|offset| (start + offset) % names.len())
            .find(|&index| names[index].starts_with(&query))
        {
            self.host_picker.active = Some(choices[found].id());
        }
        // A prefix longer than every name matches nothing, so the highlight stays put, but only what fits a name is
        // kept for the next letter (the old picker's bound on its stored prefix).
        let kept = if query.len() <= longest {
            query
        } else {
            previous
        };
        self.host_picker.typed = Some((kept, now));
        if !open {
            self.open_host_list(self.host_picker.active, window, cx);
        }
    }

    /// Resolve the pick against the live menu. A row that vanished while the list was open cannot take a new target
    /// merely because that target now occupies its old position.
    fn pick_host_choice(&mut self, id: HostChoiceId, window: &mut Window, cx: &mut Context<Self>) {
        match self
            .host_options(cx)
            .into_iter()
            .find(|choice| choice.id() == id)
        {
            Some(HostChoice::Existing(host)) => self.pick_host(host.id, window, cx),
            Some(HostChoice::New(target)) => self.pick_environment(target, window, cx),
            None => {}
        }
    }

    /// Choose `host` as the target. Picking the host already shown is still a pick: it makes the host the user's own
    /// choice ([`LaunchDraft::host_chosen`](crate::launcher::draft::LaunchDraft::host_chosen), which Save as template
    /// reads), as the old picker did, while [`LaunchDraft::choose_host`](crate::launcher::draft::LaunchDraft::choose_host)
    /// leaves the folder alone for the same host.
    pub(super) fn pick_host(&mut self, host: i64, window: &mut Window, cx: &mut Context<Self>) {
        let selected = local_state::selected(cx);
        let folder = default_folder(self.store.read(cx).collections(), selected.as_deref(), host);
        // Leaving an environment also resets the checkout's default-name sequence even when we return to the
        // same host. `choose_host` alone would see that host's old id and return early.
        self.draft.choose_runs_on(RunsOn::ExistingHost, None);
        self.draft.choose_host(host, folder);
        self.host_picker.note = None;
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Open the list under the button with the choice that started the browse highlighted.
    fn open_host_list(
        &mut self,
        active: Option<HostChoiceId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(anchor) = self.host_picker.anchor.clone() else {
            return;
        };
        self.host_picker.active = active;
        let launcher = cx.weak_entity();
        let rows = cx.new(|cx| HostRows::new(launcher, cx));
        let container = cx.focus_handle();
        let mark = Mark::take(cx);
        self.host_picker.layer = layers::open_popover(
            Popover::floating(rows, container, anchor),
            &mark,
            window,
            cx,
        );
    }

    /// Close the list if open, forgetting the browsing.
    pub(super) fn close_host_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.host_picker.layer.take()
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
        }
        self.host_picker.typed = None;
    }

    /// A click on the button: open the list, or close an open one. Opening also moves focus to the button, because the
    /// list is driven by the button's keys and closes when the button loses focus, and on macOS a click does not focus a
    /// button (the old picker focused it on click for the same reason).
    fn host_clicked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mark = Mark::take(cx);
        if self.host_list_open(window, cx) {
            self.close_host_list(window, cx);
        } else {
            let active = self.opening_highlight(&self.host_options(cx));
            self.open_host_list(active, window, cx);
            focus::focus_in_layer(&self.host_focus, &mark, window, cx);
        }
        cx.notify();
    }

    /// The address field's Host segment, either the picker or a held host. The owner supplies the address border.
    pub(super) fn host_control(
        &mut self,
        palette: &theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let chosen = self.chosen_host(cx).cloned();
        let mark = chosen
            .as_ref()
            .map(|host| host_mark(host.icon, host.color, px(MARK), palette).into_any_element());
        if self.draft.holds(Field::Host) {
            // Use the same host name in the address, the tooltip and the launch action.
            let host = self.target(cx).host;
            let words = HostSentence::new("launcher-host-name").host(&host);
            let id = SharedString::from("launcher-host-held");
            let segment = div()
                .id(id.clone())
                .debug_selector(|| "launcher-host-held".into())
                .flex()
                .items_center()
                .gap(px(MARK_GAP))
                .px(px(8.))
                .min_w_0()
                .text_size(metrics::TEXT_MD)
                .children(mark)
                .child(div().flex_1().min_w_0().truncate().child(words));
            // Escaped host text can need more lines than the short tooltip allows.
            let help =
                HoverHelp::paragraphs([HostSentence::new("launcher-host-held-help").host(&host)]);
            return with_hover_help(segment, &id, &help, false, None, window, cx)
                .into_any_element();
        }
        if self.host_picker.layer.is_some() {
            if !self.host_list_open(window, cx) {
                self.close_host_list(window, cx);
            } else if self.host_picker.active.is_none() {
                // A menu opened while hosts are loading starts browsing when its first rows arrive. Keep that
                // identity through later pushes so Enter still means the row the user saw highlighted.
                self.host_picker.active = self.opening_highlight(&self.host_options(cx));
            }
        }
        // Empty until the chosen host is in the registry (SPEC.md: the host's "value is empty before hosts arrive").
        let environment = self
            .draft
            .target
            .as_ref()
            .map(|target| ProviderUi(target.provider()));
        let (words, mark) = if let Some(provider) = environment {
            (
                Some(HostSentence::new("launcher-host-name").text(provider.segment())),
                Some(environment_mark(provider, palette.fg_1).into_any_element()),
            )
        } else {
            (
                chosen
                    .as_ref()
                    .map(|host| host_words("launcher-host-name", host)),
                mark,
            )
        };
        let label: SharedString = match &words {
            Some(words) => format!("Host: {}", words.accessible_name()).into(),
            None => "Host".into(),
        };
        // The address gives Host a fixed width. Let the menu open wider when it has longer names to show.
        let anchor = Anchor::for_element("launcher-host-anchor", window, cx);
        let tracker = anchor.track();
        self.host_picker.anchor = Some(anchor.clone());
        let (on_key, on_click) = (cx.weak_entity(), cx.weak_entity());
        // Keep the chevron and mark in view with long names or a phase suffix. Help and the accessible name retain
        // all the escaped words when the address runs out of room.
        let content = div()
            .flex()
            .items_center()
            .gap(px(MARK_GAP))
            .h(control_content_height())
            .w_full()
            .min_w_0()
            .text_size(metrics::TEXT_MD)
            .text_color(theme::color(palette.fg_0))
            .children(mark)
            .child(div().flex_1().min_w_0().truncate().children(words))
            .child(
                svg()
                    .path(Icon::ChevronDown.path())
                    .size(px(10.))
                    .flex_none()
                    .text_color(theme::color(palette.fg_1)),
            );
        let help = if let Some(provider) = environment {
            HoverHelp::tooltip(format!("{label}. {}", provider.launch_help()))
        } else if let Some(host) = &chosen {
            HoverHelp::sentence(
                HostSentence::new("launcher-host-help")
                    .text("Choose where to run the session. Current host: ")
                    .host(&super::host_name_with_phase(host)),
            )
        } else {
            HoverHelp::tooltip("Choose where to run the session.")
        };
        let button = Button::new("launcher-host", label, help, Role::ComboBox)
            .content(content)
            .tier(Tier::Ghost)
            // Report whether the list is open without painting its host mark and chevron on an accent fill.
            .discloses(self.host_list_open(window, cx))
            .focus(&self.host_focus)
            .on_key_down(move |event, window, cx| {
                let _ = on_key.update(cx, |launcher, cx| launcher.host_key(event, window, cx));
            })
            .on_click(move |_, window, cx| {
                let _ = on_click.update(cx, |launcher, cx| launcher.host_clicked(window, cx));
            });
        div()
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .child(button)
            .child(tracker)
            .into_any_element()
    }
}

/// The address's outer height includes its border. Inside it, both buttons share the folder input's height.
pub(super) fn control_content_height() -> gpui::Pixels {
    super::ADDRESS_HEIGHT - metrics::BUTTON_PAD.0 * 2. - px(4.)
}

/// A host's words in the list and the button, the footer's name for it ([`super::host_name_with_phase`]): its display
/// name, and its phase in parentheses while it is not connected (old `HostOption::label`), so an unavailable host is
/// seen as one before a press.
fn host_words(id: impl Into<gpui::ElementId>, host: &HostView) -> HostSentence {
    HostSentence::new(id).host(&super::host_name_with_phase(host))
}

/// Use the same environment mark the sidebar shows after a successful launch into this provider.
fn environment_mark(provider: ProviderUi, ink: theme::Token) -> impl IntoElement {
    svg()
        .path(provider.mark().path())
        .size(px(MARK))
        .flex_none()
        .text_color(theme::color(ink))
}

/// The open host list's rows, drawn from the launcher's state and kept scrolled to the highlighted host.
pub(super) struct HostRows {
    launcher: WeakEntity<Launcher>,
    scroll: ScrollHandle,
    /// The row last scrolled into view; the list scrolls only when the highlight moves.
    scrolled_to: Option<usize>,
    _redraw: Option<gpui::Subscription>,
}

impl HostRows {
    fn new(launcher: WeakEntity<Launcher>, cx: &mut Context<Self>) -> Self {
        let redraw = launcher
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()));
        HostRows {
            launcher,
            scroll: ScrollHandle::new(),
            scrolled_to: None,
            _redraw: redraw,
        }
    }
}

impl Render for HostRows {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let list = layers::surface(cx)
            .id("launcher-host-list")
            .debug_selector(|| "launcher-host-list".into())
            .role(Role::ListBox)
            .aria_label("Host")
            .flex()
            .flex_col()
            .p(px(LIST_PAD))
            .min_w(px(PICKER_MIN_WIDTH))
            .max_w(px(LIST_MAX_WIDTH))
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .text_size(metrics::TEXT_MD);
        let Some(launcher) = self.launcher.upgrade() else {
            return list;
        };
        let launcher_ref = launcher.read(cx);
        let choices = launcher_ref.host_options(cx);
        let active = launcher_ref.host_picker.active;
        if choices.is_empty() {
            return list.child(
                div()
                    .id("launcher-host-loading")
                    .role(Role::Status)
                    .aria_label("Loading hosts…")
                    .child("Loading hosts…"),
            );
        }
        let mut rows = Vec::new();
        let mut new_environment_heading = false;
        let mut scroll_to = None;
        if matches!(choices.first(), Some(HostChoice::Existing(_))) {
            rows.push(option_heading(
                "launcher-host-existing",
                "Existing host",
                palette,
            ));
        }
        for choice in choices {
            if matches!(choice, HostChoice::New(_)) && !new_environment_heading {
                let heading = div()
                    .when(!rows.is_empty(), |heading| {
                        heading
                            .mt(px(4.))
                            .border_t_1()
                            .border_color(theme::color(palette.border_dim))
                    })
                    .child(option_heading(
                        "launcher-host-new",
                        "New environment",
                        palette,
                    ));
                rows.push(heading.into_any_element());
                new_environment_heading = true;
            }
            let selected = Some(choice.id()) == active;
            if selected {
                // The scroll handle counts the two headings among its children. The keyboard never chooses them.
                scroll_to = Some(rows.len());
            }
            rows.push(
                self.choice_row(choice, selected, palette)
                    .into_any_element(),
            );
        }
        if self.scrolled_to != scroll_to {
            if let Some(index) = scroll_to {
                self.scroll.scroll_to_item(index);
            }
            self.scrolled_to = scroll_to;
        }
        list.children(rows)
    }
}

impl HostRows {
    /// All choices use one row role and take their target through the same live eligibility check as Enter.
    fn choice_row(
        &self,
        choice: HostChoice,
        selected: bool,
        palette: &theme::Palette,
    ) -> OptionRow {
        let pick = choice.id();
        let content = div().flex().items_center().gap(px(MARK_GAP)).min_w_0();
        let (id, label, help, content) = match choice {
            HostChoice::Existing(host) => {
                let id = format!("launcher-host-option-{}", host.id);
                let words = host_words(SharedString::from(format!("{id}-words")), &host);
                let mark = host_mark(host.icon, host.color, px(MARK), palette);
                // A host's own color never sits on the solid selection fill.
                let mark = mark.when(selected, |mark| {
                    mark.text_color(theme::color(palette.on_fill))
                });
                (
                    id,
                    words.accessible_name(),
                    "Run the session on this host",
                    content.child(mark).child(words),
                )
            }
            HostChoice::New(target) => {
                let provider = ProviderUi(target.provider());
                let name = provider.segment();
                let ink = if selected {
                    palette.on_fill
                } else {
                    palette.fg_1
                };
                let tag = provider.launch_offer_tag();
                let label = tag.map_or_else(|| name.to_owned(), |tag| format!("{name} · {tag}"));
                let content = content
                    .child(environment_mark(provider, ink))
                    .child(name)
                    .children(tag.map(|tag| {
                        div()
                            .text_size(metrics::TEXT_SM)
                            .text_color(theme::color(ink))
                            .child(tag)
                    }));
                (
                    format!("launcher-host-option-{name}"),
                    label.into(),
                    provider.segment_help(),
                    content,
                )
            }
        };
        let weak = self.launcher.clone();
        OptionRow::new(id, label, HoverHelp::tooltip(help), Role::ListBoxOption)
            .in_popover()
            .selected(selected)
            .child(content)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |launcher, cx| {
                    launcher.pick_host_choice(pick, window, cx);
                    launcher.close_host_list(window, cx);
                    cx.notify();
                });
            })
    }
}
