//! The composer search box at the top of the launcher's body: a text field with its results floating under it, which
//! turns a typed word into one choice in the draft. What a query offers is [`crate::launcher::search`]'s; this module
//! draws it and applies the row the user takes.
//!
//! SPEC.md, "Creation" (the launch-composer search sentences). Ported from the old launcher's search box and results
//! area (`list/create_form.rs`, the `.launch-composer-search` markup and its key handler, and `accept_search_result`
//! with `apply_search_result` in `list/create_form/draft.rs`) at reference commit `24bc0580`. Here's the TLDR:
//!
//! - **Typing** opens the results under the field, on the layer manager's non-focusing popover, so focus and every key
//!   stay in the field, and preselects the row an exact word names ([`crate::launcher::search::Results::default`]).
//!   Up and Down move the highlighted row, wrapping, and the list keeps it in view. Escape, or a press outside, closes
//!   the results and keeps what was typed; Escape with them closed is the dialog's (it closes the launcher).
//! - **Enter** on an empty box presses the main action, as the old form's own submit did: it launches a complete draft
//!   and says why not beside it otherwise. With text, Enter reopens closed results, takes the highlighted row of open
//!   ones, and does nothing when they offer no row (their message stays). A held Enter does nothing (SPEC.md: "A held
//!   Enter never launches"), so holding it after a pick cannot launch the pick.
//! - **Taking a row** (Enter or a click) makes the one edit it names through the same draft method a hand edit uses,
//!   then empties the box and closes the results, with focus back in the box for the next word; choosing Custom moves
//!   focus on to its flags, as choosing it by hand does. An agent type or model taken on the Command tab switches to
//!   the Agent tab, where they belong. A row that would change what this launcher holds fixed (Restart with's agent
//!   type, host, folder and name; Replace with's host) is refused above the box, even when it names the current value;
//!   the box keeps its text, and the results close so the reason shows.
//!
//! What the box holds besides the draft (the open results, the highlight, the last refusal) is the launcher's alone;
//! what is typed is the field's own text, read when the rows are needed, never copied.

use farhelm_proto::ui_channel::HostView;
use gpui::{
    AnyElement, App, AppContext as _, Context, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, Window, div, px,
};

use super::Launcher;
use crate::components::{HoverHelp, OptionRow, Role, TextInput, Vertical};
use crate::focus::{self, Mark};
use crate::launcher::draft::{Field, Mode, Tab};
use crate::launcher::search::{Inputs, Results, Row, search};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::theme::{self, metrics};

/// The box's accessible name, the old `aria-label`; the old end-to-end tests found the box by it.
pub(super) const SEARCH_LABEL: &str = "Search folders, harnesses, models, and efforts";
/// The hint the empty box shows, the old placeholder.
pub(super) const SEARCH_PLACEHOLDER: &str = "Search names, hosts, folders, harnesses, models…";
/// `.launch-composer-search-results`: `max-height: 260px; padding: 6px; gap: 2px`.
const LIST_MAX_HEIGHT: f32 = 260.;
const LIST_PAD: f32 = 6.;
const LIST_GAP: f32 = 2.;

/// What the search box holds besides the draft and its own text. None of it is shared with another surface.
#[derive(Default)]
pub(super) struct SearchBox {
    /// The open results' layer, if open. The layer manager closes it itself on Escape or a press outside, without
    /// telling the box, so [`Launcher::search_open`] asks it rather than trusting this.
    layer: Option<LayerId>,
    /// The highlighted row, as an index into the current rows; clamped when read, since the rows follow the store.
    active: usize,
    /// Why the last row taken changed nothing (a field this launcher holds fixed), said above the box until the next
    /// keystroke in it or the next row taken.
    refusal: Option<String>,
    /// Where the box is drawn, for the results to open under it; refreshed every frame.
    anchor: Option<Anchor>,
}

impl Launcher {
    /// What the box's text offers now, against the draft, the catalog and the registry's hosts as they stand.
    pub(in crate::launcher) fn search_results(&self, cx: &App) -> Results {
        let query = self.search.text(cx);
        // The search reads hosts by value; a launcher sees a handful, so the copy is cheap.
        let hosts: Vec<HostView> = self
            .store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .into_iter()
            .cloned()
            .collect();
        search(&Inputs {
            query: &query,
            tab: self.draft.tab,
            harness: self.draft.agent.harness,
            model: self.draft.agent.model.as_deref(),
            catalog: self.catalog.as_deref(),
            catalog_pending: self.catalog.is_none() && self.catalog_error.is_none(),
            hosts: &hosts,
        })
    }

    /// Whether the results are open: opened by the box and not since closed by the layer manager.
    pub(super) fn search_open(&self, window: &Window, cx: &App) -> bool {
        self.search_box
            .layer
            .is_some_and(|layer| layers::is_open(layer, window, cx))
    }

    /// The user typed in the box: forget the last refusal, preselect what the new text names, and show the results
    /// (or close them when the text offers nothing to show, an emptied box).
    pub(super) fn search_typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_box.refusal = None;
        let results = self.search_results(cx);
        self.search_box.active = results.default;
        if results.rows.is_empty() && results.message.is_none() {
            self.close_search(window, cx);
        } else if !self.search_open(window, cx) {
            self.open_search(window, cx);
        }
        cx.notify();
    }

    /// Up or Down in the box: move the highlighted row, wrapping, while the results are open and have rows.
    pub(super) fn search_arrow(
        &mut self,
        arrow: Vertical,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.search_results(cx).rows.len();
        if !self.search_open(window, cx) || count == 0 {
            return;
        }
        let active = self.search_box.active.min(count - 1);
        self.search_box.active = match arrow {
            Vertical::Down => (active + 1) % count,
            Vertical::Up => (active + count - 1) % count,
        };
        cx.notify();
    }

    /// Enter in the box (see the module docs). `held` is an auto-repeat, which does nothing.
    pub(super) fn search_enter(&mut self, held: bool, window: &mut Window, cx: &mut Context<Self>) {
        if held {
            return;
        }
        // Literally empty, as the old check was: a box holding only spaces is a query, and never launches.
        if self.search.text(cx).is_empty() {
            self.close_search(window, cx);
            self.press(window, cx);
            return;
        }
        if !self.search_open(window, cx) {
            let results = self.search_results(cx);
            self.search_box.active = results.default;
            self.open_search(window, cx);
            cx.notify();
            return;
        }
        let rows = self.search_results(cx).rows;
        if let Some(row) = rows.get(self.search_box.active.min(rows.len().saturating_sub(1))) {
            self.take_search_row(row.clone(), window, cx);
        }
    }

    /// Take `row`, from Enter or a click: refuse it above the box when it would change a field this launcher holds
    /// fixed, else make its one edit, empty the box, close the results and keep focus in the box (Custom's flags take
    /// it instead).
    fn take_search_row(&mut self, row: Row, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(field) = self.held_by_row(&row) {
            // Only Replace with and Restart with hold fields; New and Clone never get here.
            let mode = match self.draft.mode {
                Mode::ReplaceWith { .. } => "replace",
                Mode::New | Mode::Clone | Mode::RestartWith(_) => "restart",
            };
            self.search_box.refusal = Some(format!(
                "{field} cannot change here: this dialog keeps {field} fixed for {mode}"
            ));
            // The results would cover the reason; closing them keeps the query, and Enter opens them again.
            self.close_search(window, cx);
            cx.notify();
            return;
        }
        let mark = Mark::take(cx);
        self.search_box.refusal = None;
        self.search.set_text("", window, cx);
        self.close_search(window, cx);
        let custom = row == Row::Permission(crate::launcher::search::SearchPermission::Custom);
        if !custom {
            // The box keeps focus for the next word. A click on a row leaves it there anyway (the results never take
            // focus); this covers a row taken while something else held focus.
            focus::focus_in_layer(&self.search.focus_handle(cx), &mark, window, cx);
        }
        self.apply_search_row(row, window, cx);
        cx.notify();
    }

    /// The field, in the old refusal's words, that taking `row` would change while this launcher holds it fixed: any
    /// agent type (a model too, unless it is one of the held agent type's own on the Agent tab), host, folder or name,
    /// even one naming the current value (SPEC.md, "Restart with": "Search results and recent setups that set a held
    /// field are still refused even when they name its current value").
    fn held_by_row(&self, row: &Row) -> Option<&'static str> {
        let (field, words) = match row {
            Row::Harness(_) => (Field::AgentType, "agent type"),
            Row::Model { harness, .. }
                if self.draft.tab != Tab::Agent || self.draft.agent.harness != Some(*harness) =>
            {
                (Field::AgentType, "agent type")
            }
            Row::Host { .. } => (Field::Host, "host"),
            Row::UsePath(_) => (Field::Destination, "destination"),
            Row::Name(_) => (Field::Name, "session name"),
            Row::Model { .. } | Row::Effort(_) | Row::Permission(_) => return None,
        };
        self.draft.holds(field).then_some(words)
    }

    /// Make `row`'s edit through the method the matching hand edit uses, so it has every effect that edit has.
    fn apply_search_row(&mut self, row: Row, window: &mut Window, cx: &mut Context<Self>) {
        match row {
            Row::Name(name) => {
                self.draft.edit_name(name);
                self.sync_fields(window, cx);
                self.changed(cx);
            }
            // The rows were built from the store a moment ago; a host that has gone since is not chosen.
            Row::Host { id, .. } => {
                let offered = self
                    .store
                    .read(cx)
                    .collections()
                    .ordered::<HostView>()
                    .iter()
                    .any(|host| host.id == id && !host.is_session_owned);
                if offered {
                    self.pick_host(id, window, cx);
                }
            }
            // Choosing an agent type makes the tab Agent itself; a model of the agent already chosen does not, so the
            // model row switches it first.
            Row::Harness(harness) => self.choose_harness(harness, window, cx),
            Row::Model { id, harness } => {
                self.draft.choose_tab(Tab::Agent);
                let wrapper = self.remembered_wrapper(harness, cx);
                let edit = self.draft.choose_catalog_model(
                    harness,
                    &id,
                    self.catalog.as_deref(),
                    wrapper.as_deref(),
                );
                self.chose(edit, window, cx);
            }
            Row::Effort(effort) => self.choose_effort(effort, cx),
            Row::Permission(permission) => {
                self.choose_permissions(permission.permission(), window, cx)
            }
            Row::UsePath(path) => {
                // A folder taken from search is a pick, not typing: the folder list shows every recent folder again.
                self.folder_picked();
                self.draft.edit_folder(path);
                self.sync_fields(window, cx);
                self.changed(cx);
            }
        }
    }

    /// Open the results under the box, as a popover that never takes focus.
    fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(anchor) = self.search_box.anchor.clone() else {
            return;
        };
        let launcher = cx.weak_entity();
        let rows = cx.new(|cx| SearchRows::new(launcher, cx));
        let container = cx.focus_handle();
        let mark = Mark::take(cx);
        self.search_box.layer = layers::open_popover(
            Popover::floating(rows, container, anchor),
            &mark,
            window,
            cx,
        );
    }

    /// Close the results if they are open. What was typed stays.
    pub(super) fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.search_box.layer.take()
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
        }
    }

    /// The search box, full width at the top of the body, with the last refusal above it.
    ///
    /// It also forgets results the layer manager closed on its own (Escape, a press outside), which tells the box
    /// nothing; that relies on the launcher being drawn in every frame its dialog is, as the model field does.
    pub(super) fn search_row(
        &mut self,
        palette: &theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.search_box.layer.is_some() && !self.search_open(window, cx) {
            self.search_box.layer = None;
        }
        let anchor = Anchor::for_element("launcher-search-anchor", window, cx).matching_width();
        let tracker = anchor.track();
        self.search_box.anchor = Some(anchor.clone());
        let weak = cx.weak_entity();
        let field = TextInput::new(
            "launcher-search",
            &self.search,
            SEARCH_LABEL,
            HoverHelp::tooltip(
                "Search: type a name, host, folder, harness, model, effort or permission, then Enter to choose it",
            ),
        )
        .on_vertical(move |arrow, window, cx| {
            let _ = weak.update(cx, |launcher, cx| launcher.search_arrow(arrow, window, cx));
        });
        // Above the box, where the old launcher said why a taken row changed nothing, in the launcher's plain
        // warning ink like its other inline refusals.
        let refusal = self.search_box.refusal.clone().map(|words| {
            div()
                .id("launcher-search-refusal")
                .role(Role::Status)
                .aria_label(SharedString::from(words.clone()))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.warn))
                .child(words)
        });
        div()
            .id("launcher-search-row")
            // Tests press the box by where its row is drawn, as they do the other fields' rows.
            .debug_selector(|| "launcher-search-row".into())
            .flex()
            .flex_col()
            .gap(px(6.))
            .children(refusal)
            .child(div().relative().child(field).child(tracker))
            .into_any_element()
    }
}

/// The open results, drawn from the launcher's state, and kept scrolled so the highlighted row is in view.
pub(super) struct SearchRows {
    launcher: WeakEntity<Launcher>,
    scroll: ScrollHandle,
    /// The child last scrolled into view. The list is scrolled only when the highlight moves, so a wheel scroll is not
    /// pulled back to the highlighted row on the next redraw.
    scrolled_to: Option<usize>,
    /// Redraws the rows when the launcher changes (what is typed, the highlighted row, the draft).
    _redraw: Option<gpui::Subscription>,
}

impl SearchRows {
    fn new(launcher: WeakEntity<Launcher>, cx: &mut Context<Self>) -> Self {
        let redraw = launcher
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()));
        SearchRows {
            launcher,
            scroll: ScrollHandle::new(),
            scrolled_to: None,
            _redraw: redraw,
        }
    }
}

impl Render for SearchRows {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let list = layers::surface(cx)
            .id("launcher-search-results")
            // Tests find the results by where they are drawn; this records nothing outside test builds.
            .debug_selector(|| "launcher-search-results".into())
            .flex()
            .flex_col()
            .gap(px(LIST_GAP))
            .p(px(LIST_PAD))
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .text_size(metrics::TEXT_MD);
        let Some(launcher) = self.launcher.upgrade() else {
            return list;
        };
        let launcher_ref = launcher.read(cx);
        let results = launcher_ref.search_results(cx);
        let harness = launcher_ref.draft.agent.harness;
        let active = launcher_ref
            .search_box
            .active
            .min(results.rows.len().saturating_sub(1));
        // With rows the area is a list box; with none, it holds only its message, which is the status line the old
        // area was (`role="status"`).
        let list = if results.rows.is_empty() {
            list
        } else {
            list.role(Role::ListBox).aria_label(SEARCH_LABEL)
        };
        let mut children: Vec<AnyElement> = Vec::new();
        if let Some(message) = results.message {
            let ink = if message.warning {
                palette.warn_bright
            } else {
                palette.fg_1
            };
            children.push(
                div()
                    .id("launcher-search-message")
                    .role(Role::Status)
                    .aria_label(message.text)
                    .p(px(6.))
                    .text_size(metrics::TEXT_MD)
                    .text_color(theme::color(ink))
                    .child(message.text)
                    .into_any_element(),
            );
        }
        // The child index of the highlighted row, for scrolling it into view. Headings and rows are all the list's
        // own children, so the scroll handle can find any row; the old list nested each group in a `role="group"`
        // node, which this leaves out (a recorded deviation): a nested row would scroll only its whole group into view.
        let mut active_child = None;
        for (group, rows) in results.groups() {
            children.push(
                div()
                    .id(SharedString::from(format!(
                        "launcher-search-group-{}",
                        group.label()
                    )))
                    .role(Role::Label)
                    .aria_label(group.label())
                    .pt(px(4.))
                    .px(px(6.))
                    .pb(px(1.))
                    .text_size(metrics::TEXT_SM)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_1))
                    .child(group.label())
                    .into_any_element(),
            );
            for (index, row) in rows {
                if index == active {
                    active_child = Some(children.len());
                }
                let id = format!("launcher-search-option-{index}");
                let words = row.words(SharedString::from(format!("{id}-words")));
                let label = words.accessible_name();
                let help = HoverHelp::sentence(
                    row.help(harness, SharedString::from(format!("{id}-help"))),
                );
                let weak = self.launcher.clone();
                let taken = row.clone();
                children.push(
                    OptionRow::new(id, label, help, Role::ListBoxOption)
                        .selected(index == active)
                        // The highlighted row's words take the fill's ink, as the old selected row did.
                        .child(if index == active {
                            div()
                                .text_color(theme::color(palette.on_fill))
                                .child(words)
                                .into_any_element()
                        } else {
                            words.into_any_element()
                        })
                        .on_click(move |_, window, cx| {
                            let row = taken.clone();
                            let _ = weak.update(cx, |launcher, cx| {
                                launcher.take_search_row(row, window, cx)
                            });
                        })
                        .into_any_element(),
                );
            }
        }
        if active_child != self.scrolled_to {
            if let Some(child) = active_child {
                self.scroll.scroll_to_item(child);
            }
            self.scrolled_to = active_child;
        }
        list.children(children)
    }
}
