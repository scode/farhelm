//! The Host row's picker: a button showing the chosen host, with a single-choice list of hosts floating under it,
//! ported from the old launcher's `list/host_picker.rs` at reference commit `24bc0580`.
//!
//! How it behaves, as the old one did:
//!
//! - **Closed**, the button shows the chosen host's mark, its name, and its phase in parentheses while it is not
//!   connected. Enter on it presses the main action, under the choice controls' Enter rule; a click, Space or an arrow
//!   opens the list.
//! - **Open**, the list shows every host in the registry's order (hosts a disposable environment owns only when chosen
//!   already), each with its mark, name and phase, so an unavailable host can be seen and still be chosen, and Launch
//!   then says why it cannot go. The list is the layer manager's non-focusing popover: focus and every key stay on the
//!   button. Up and Down move the highlight (stopping at the ends), Home and End jump, a typed letter jumps to the next
//!   host whose name starts with what was typed in the last second (the same letter again cycles among them), Enter or
//!   Space picks the highlighted host, and a click picks the one clicked. Escape, a press outside and focus leaving the
//!   button close it without a choice.
//! - **Held** (Restart with and Replace with keep the source's host), the row shows the host as text instead.
//!
//! Browsing never changes the draft: only a pick does, through [`LaunchDraft::choose_host`], which re-seeds a folder
//! the user has not typed or picked.
//!
//! A pick re-seeds from New's per-host folder default (`entry::default_folder`: the open session's folder when it is on
//! that host's install, else `~`). NOTE: the rest of SPEC.md's default, the newest recent setup's folder on that host,
//! arrives with recent setups.
//!
//! [`LaunchDraft::choose_host`]: crate::launcher::draft::LaunchDraft::choose_host

use std::time::{Duration, SystemTime};

use farhelm_proto::ui_channel::HostView;
use gpui::{
    AnyElement, App, AppContext as _, Context, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, Window, div, px,
};

use super::{Launcher, row};
use crate::components::{Button, HoverHelp, OptionRow, Role, Tier};
use crate::focus::{self, Mark};
use crate::host_text::HostSentence;
use crate::hosts::identity::{display_name, host_mark};
use crate::launcher::draft::Field;
use crate::launcher::entry::default_folder;
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::local_state;
use crate::theme::{self, metrics};

/// How long typed letters keep adding to one name prefix before the next letter starts a new one (the old picker's
/// one second).
const TYPEAHEAD: Duration = Duration::from_secs(1);
/// `.launcher-host-options`: `max-height: 240px; padding: 4px`, scrolling past that, its rows touching.
const LIST_MAX_HEIGHT: f32 = 240.;
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
    /// The highlighted host's position in the list.
    active: usize,
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

impl Launcher {
    /// The hosts the list offers, in the registry's order: every host but those a disposable environment owns, unless
    /// that one is chosen already (the old list's rule: an environment's host is not a place to start an unrelated
    /// session).
    fn host_options(&self, cx: &App) -> Vec<HostView> {
        self.store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .into_iter()
            .filter(|host| !host.is_session_owned || Some(host.id) == self.draft.host)
            .cloned()
            .collect()
    }

    /// Whether the host list is open.
    pub(super) fn host_list_open(&self, window: &Window, cx: &App) -> bool {
        self.host_picker
            .layer
            .is_some_and(|layer| layers::is_open(layer, window, cx))
    }

    /// The chosen host's position in the list, or the first.
    fn chosen_host_index(&self, hosts: &[HostView]) -> usize {
        hosts
            .iter()
            .position(|host| Some(host.id) == self.draft.host)
            .unwrap_or(0)
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
        let hosts = self.host_options(cx);
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
                    if let Some(host) = hosts.get(self.host_picker.active) {
                        self.pick_host(host.id, window, cx);
                    }
                    self.close_host_list(window, cx);
                } else {
                    self.open_host_list(&hosts, window, cx);
                }
            }
            "down" | "up" | "home" | "end" => {
                cx.stop_propagation();
                let last = hosts.len().saturating_sub(1);
                let active = self.host_picker.active;
                self.host_picker.active = match (key, open) {
                    ("home", _) => 0,
                    ("end", _) => last,
                    ("up", true) => active.saturating_sub(1),
                    ("down", true) => (active + 1).min(last),
                    _ => self.chosen_host_index(&hosts),
                };
                self.host_picker.typed = None;
                if !open {
                    let active = self.host_picker.active;
                    self.open_host_list(&hosts, window, cx);
                    self.host_picker.active = active;
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
                self.type_ahead(letter, &hosts, open, window, cx);
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
        hosts: &[HostView],
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let now = crate::ticker::wall_now(cx);
        let letter = letter.to_lowercase();
        if !open {
            // A closed list has no browsing highlight: start from the chosen host, so an unmatched prefix cannot
            // revive an old one.
            self.host_picker.active = self.chosen_host_index(hosts);
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
        let longest = hosts
            .iter()
            .map(|host| display_name(host).raw().len())
            .max()
            .unwrap_or(0);
        let query = if repeated {
            letter
        } else {
            format!("{previous}{letter}")
        };
        let start = if repeated {
            self.host_picker.active + 1
        } else {
            0
        };
        let names: Vec<String> = hosts
            .iter()
            .map(|host| display_name(host).raw().to_lowercase())
            .collect();
        if let Some(found) = (0..names.len())
            .map(|offset| (start + offset) % names.len())
            .find(|&index| names[index].starts_with(&query))
        {
            self.host_picker.active = found;
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
            let active = self.host_picker.active;
            self.open_host_list(hosts, window, cx);
            self.host_picker.active = active;
        }
    }

    /// Choose `host` as the target. Picking the host already shown is still a pick: it makes the host the user's own
    /// choice ([`LaunchDraft::host_chosen`](crate::launcher::draft::LaunchDraft::host_chosen), which Save as template
    /// reads), as the old picker did, while [`LaunchDraft::choose_host`](crate::launcher::draft::LaunchDraft::choose_host)
    /// leaves the folder alone for the same host.
    pub(super) fn pick_host(&mut self, host: i64, window: &mut Window, cx: &mut Context<Self>) {
        let selected = local_state::selected(cx);
        let folder = default_folder(self.store.read(cx).collections(), selected.as_deref(), host);
        self.draft.choose_host(host, folder);
        self.host_picker.note = None;
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Open the list under the button with the chosen host highlighted.
    fn open_host_list(&mut self, hosts: &[HostView], window: &mut Window, cx: &mut Context<Self>) {
        let Some(anchor) = self.host_picker.anchor.clone() else {
            return;
        };
        self.host_picker.active = self.chosen_host_index(hosts);
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
            let hosts = self.host_options(cx);
            self.open_host_list(&hosts, window, cx);
            focus::focus_in_layer(&self.host_focus, &mark, window, cx);
        }
        cx.notify();
    }

    /// The Host row: the picker, or the held host as text.
    pub(super) fn host_row(
        &mut self,
        palette: &theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hosts = self.host_options(cx);
        let chosen = self
            .draft
            .host
            .and_then(|id| hosts.iter().find(|host| host.id == id).cloned());
        let mark = chosen
            .as_ref()
            .map(|host| host_mark(host.icon, host.color, px(MARK), palette).into_any_element());
        if self.draft.holds(Field::Host) {
            // The footer's words: the registry's name with its phase, else the source session's own host name.
            let words = HostSentence::new("launcher-host-name").host(&self.target(cx).host);
            let held = div()
                .pt(px(8.))
                .flex()
                .items_center()
                .gap(px(MARK_GAP))
                .text_size(metrics::TEXT_MD)
                .children(mark)
                .child(words);
            return row("launcher-host-row", "Host", held, palette).into_any_element();
        }
        if self.host_picker.layer.is_some() && !self.host_list_open(window, cx) {
            self.close_host_list(window, cx);
        }
        // Empty until the chosen host is in the registry (SPEC.md: the host's "value is empty before hosts arrive").
        let words = chosen
            .as_ref()
            .map(|host| host_words("launcher-host-name", host));
        let label: SharedString = match &words {
            Some(words) => format!("Host: {}", words.accessible_name()).into(),
            None => "Host".into(),
        };
        let anchor = Anchor::for_element("launcher-host-anchor", window, cx).matching_width();
        let tracker = anchor.track();
        self.host_picker.anchor = Some(anchor.clone());
        let (on_key, on_click) = (cx.weak_entity(), cx.weak_entity());
        let content = div()
            .flex()
            .items_center()
            .gap(px(MARK_GAP))
            .min_w_0()
            .children(mark)
            .child(div().flex_1().min_w_0().children(words))
            .child(
                div()
                    .flex_none()
                    .text_color(theme::color(palette.fg_1))
                    .child("▾"),
            );
        // No expanded look: the old button looked the same open or closed, and the pressed look would put the mark and
        // the chevron on the accent fill.
        let button = Button::new(
            "launcher-host",
            label,
            HoverHelp::tooltip("Host: the machine the session runs on"),
            Role::ComboBox,
        )
        .content(content)
        .tier(Tier::Neutral)
        .focus(&self.host_focus)
        .on_key_down(move |event, window, cx| {
            let _ = on_key.update(cx, |launcher, cx| launcher.host_key(event, window, cx));
        })
        .on_click(move |_, window, cx| {
            let _ = on_click.update(cx, |launcher, cx| launcher.host_clicked(window, cx));
        });
        let control = div().relative().w_full().child(button).child(tracker);
        row("launcher-host-row", "Host", control, palette).into_any_element()
    }
}

/// A host's words in the list and the button, the footer's name for it ([`super::host_name_with_phase`]): its display
/// name, and its phase in parentheses while it is not connected (old `HostOption::label`), so an unavailable host is
/// seen as one before a press.
fn host_words(id: impl Into<gpui::ElementId>, host: &HostView) -> HostSentence {
    HostSentence::new(id).host(&super::host_name_with_phase(host))
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
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .text_size(metrics::TEXT_MD);
        let Some(launcher) = self.launcher.upgrade() else {
            return list;
        };
        let launcher_ref = launcher.read(cx);
        let hosts = launcher_ref.host_options(cx);
        let active = launcher_ref.host_picker.active;
        if hosts.is_empty() {
            return list.child(
                div()
                    .id("launcher-host-loading")
                    .role(Role::Status)
                    .aria_label("Loading hosts…")
                    .child("Loading hosts…"),
            );
        }
        if self.scrolled_to != Some(active) {
            self.scroll.scroll_to_item(active);
            self.scrolled_to = Some(active);
        }
        let rows = hosts.into_iter().enumerate().map(|(index, host)| {
            let id = format!("launcher-host-option-{}", host.id);
            let words = host_words(SharedString::from(format!("{id}-words")), &host);
            let label = words.accessible_name();
            let weak = self.launcher.clone();
            let picked = host.id;
            let selected = index == active;
            // The highlighted row gives its words the fill's ink, but the mark draws in the host's own color, so it
            // takes that ink here (`.launcher-host-option[aria-selected="true"] .host-kind-icon`), and a host's color
            // never sits on the accent fill.
            let mark = host_mark(host.icon, host.color, px(MARK), palette);
            let mark = if selected {
                mark.text_color(theme::color(palette.on_fill))
            } else {
                mark
            };
            let content = div()
                .flex()
                .items_center()
                .gap(px(MARK_GAP))
                .min_w_0()
                .child(mark)
                .child(words);
            OptionRow::new(
                id,
                label,
                HoverHelp::tooltip("Run the session on this host"),
                Role::ListBoxOption,
            )
            .selected(selected)
            .child(content)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |launcher, cx| {
                    launcher.pick_host(picked, window, cx);
                    launcher.close_host_list(window, cx);
                    cx.notify();
                });
            })
        });
        list.children(rows)
    }
}
