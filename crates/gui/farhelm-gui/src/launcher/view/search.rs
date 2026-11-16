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
//! - **A `tl:` row** applies its template to the draft through the templates' own application path, as a template
//!   shortcut's press in the open launcher does (`crate::templates::dispatch::search_row_batch`); a template that
//!   does not apply changes nothing, and the launcher's refusal note says why above the box, where the old launcher
//!   said why a template did not apply. While the helm cannot give its templates, `tl:` shows the helm's words in
//!   the results instead of rows.
//!
//! - **Rows from the chosen host's launch history.** A saved folder becomes the folder as a pick from the folder list
//!   does (leaving a managed checkout, keeping the tab). A model from a past launch chooses the agent type it ran with
//!   and the model as that type's custom id. A recent setup fills the draft as a click on the strip's row does
//!   ([`Launcher::take_recent`]), drawn on two lines, and never launches, from Enter either: the old search row only
//!   filled, and SPEC.md's Enter that launches is the strip's focused row. A setup carrying a retired container choice,
//!   or a folder or setup from a history that is no longer the chosen host's, is refused above the box in the old
//!   words.
//!
//! What the box holds besides the draft (the open results, the highlight, the last refusal) is the launcher's alone;
//! what is typed is the field's own text, read when the rows are needed, never copied.

use farhelm_proto::session_requests::LaunchHistoryEntry;
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchPermission, RETIRED_CONTAINER_REASON};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, Window, div, px,
};

use super::Launcher;
use super::host_name_with_phase;
use super::recents::RecentRefusal;
use crate::components::{HoverHelp, OptionRow, Role, Vertical};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::{Field, Mode, Tab};
use crate::launcher::recents::{destination_label, permission_value, summary_before_permissions};
use crate::launcher::search::{Inputs, Results, Row, Scope, scoped, search};
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
/// Why a saved folder or recent setup from an earlier host's history changed nothing, as the old launcher said it.
const STALE_ROW: &str = "That result belongs to an earlier host; choose a current result";

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
        // Every template, as the templates' own rows give them, which the search filters; only a `tl:` query reads them.
        let templates = if scoped(&query).0 == Scope::Templates {
            crate::templates::rows::rows("", cx)
        } else {
            Ok(Vec::new())
        };
        // The chosen host's history rows name that host as its own field does, phase included.
        let placement = self
            .draft
            .host
            .and_then(|id| hosts.iter().find(|host| host.id == id))
            .map(host_name_with_phase);
        let history = self.launch_history(cx).zip(placement.as_ref());
        search(&Inputs {
            query: &query,
            tab: self.draft.tab,
            harness: self.draft.agent.harness,
            model: self.draft.agent.model.as_deref(),
            catalog: self.catalog.as_deref(),
            catalog_pending: self.catalog.is_none() && self.catalog_error.is_none(),
            hosts: &hosts,
            templates: templates.as_deref(),
            history,
        })
    }

    /// Highlight the row the box's text names again, as typing does: the rows have changed under the highlight (a
    /// history answer adds or drops rows), so the index the arrows left would now point at a row they never reached,
    /// and Enter would take it.
    pub(super) fn reset_search_highlight(&mut self, cx: &App) {
        self.search_box.active = self.search_results(cx).default;
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
        if results.rows.is_empty() && results.message.is_none() && results.reason.is_none() {
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
        if let Some(refusal) = self.search_row_refusal(&row, cx) {
            self.search_box.refusal = Some(refusal);
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

    /// Why taking `row` would change nothing, in the old launcher's words: a field this launcher holds fixed
    /// ([`Self::held_by_row`]), a recent setup carrying a retired container choice, or a saved folder or recent setup
    /// from a history that is no longer the chosen host's (the host changed after the row was drawn).
    fn search_row_refusal(&self, row: &Row, cx: &App) -> Option<String> {
        if let Some(field) = self.held_by_row(row) {
            // Only Replace with and Restart with hold fields; New and Clone never get here.
            let mode = match self.draft.mode {
                Mode::ReplaceWith { .. } => "replace",
                Mode::New | Mode::Clone | Mode::RestartWith(_) => "restart",
            };
            return Some(format!(
                "{field} cannot change here: this dialog keeps {field} fixed for {mode}"
            ));
        }
        let stale = match row {
            Row::Recent { entry, .. } => match self.recent_refusal(entry, cx)? {
                RecentRefusal::Retired => return Some(RETIRED_CONTAINER_REASON.to_owned()),
                RecentRefusal::Stale => true,
                // Not reached: `held_by_row` refused above what Restart with holds.
                RecentRefusal::Held => {
                    debug_assert!(
                        false,
                        "a held destination is refused before the setup is looked at"
                    );
                    false
                }
            },
            Row::Folder(path) => !self.launch_history(cx).is_some_and(|history| {
                history
                    .folders
                    .iter()
                    .any(|folder| folder.display_cwd == *path)
            }),
            _ => false,
        };
        stale.then(|| STALE_ROW.to_owned())
    }

    /// The field, in the old refusal's words, that taking `row` would change while this launcher holds it fixed: any
    /// agent type (a model too, unless it is one of the held agent type's own on the Agent tab), host, folder or name,
    /// even one naming the current value (SPEC.md, "Restart with": "Search results and recent setups that set a held
    /// field are still refused even when they name its current value").
    fn held_by_row(&self, row: &Row) -> Option<&'static str> {
        let (field, words) = match row {
            Row::Harness(_) => (Field::AgentType, "agent type"),
            Row::Model { harness, .. } | Row::HistoryModel { harness, .. }
                if self.draft.tab != Tab::Agent || self.draft.agent.harness != Some(*harness) =>
            {
                (Field::AgentType, "agent type")
            }
            Row::Host { .. } => (Field::Host, "host"),
            // A recent setup sets the agent type too, but the old refusal named the destination, which every mode that
            // holds the agent type also holds.
            Row::UsePath(_) | Row::Folder(_) | Row::Recent { .. } => {
                (Field::Destination, "destination")
            }
            Row::Name(_) => (Field::Name, "session name"),
            // A template's own fields are checked against what the launcher holds when it applies, which removes the
            // ones repeating a held value and refuses the rest (`crate::templates::apply`).
            Row::Model { .. }
            | Row::HistoryModel { .. }
            | Row::Effort(_)
            | Row::Permission(_)
            | Row::Template(_) => {
                return None;
            }
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
            // Through the templates' application path, as a template shortcut's press in the open launcher: it waits
            // for the catalog read if that is out, and a refusal changes nothing and is said above the box.
            Row::Template(name) => self.apply_batch(
                crate::templates::dispatch::search_row_batch(name),
                window,
                cx,
            ),
            Row::Model { id, harness } => {
                // A switch made from the Command tab raises no banner, as the draft's own rule says
                // (`LaunchDraft::escalation_since`): the tab is switched here first, for the model's sake, so the
                // edit's escalation is judged as if from the Agent tab and dropped.
                let from_command = self.draft.tab == Tab::Command;
                self.draft.choose_tab(Tab::Agent);
                let wrapper = self.remembered_wrapper(harness, cx);
                let mut edit = self.draft.choose_catalog_model(
                    harness,
                    &id,
                    self.catalog.as_deref(),
                    wrapper.as_deref(),
                );
                if from_command {
                    edit.escalation = None;
                }
                self.chose(edit, window, cx);
            }
            // A model from a past launch, owned as a custom id by the agent type it ran with.
            Row::HistoryModel { id, harness } => {
                // A switch made from the Command tab raises no banner, as the draft's own rule says
                // (`LaunchDraft::escalation_since`): the tab is switched here first, for the model's sake, so the
                // edit's escalation is judged as if from the Agent tab and dropped.
                let from_command = self.draft.tab == Tab::Command;
                self.draft.choose_tab(Tab::Agent);
                let wrapper = self.remembered_wrapper(harness, cx);
                let mut edit = self.draft.choose_history_model(
                    harness,
                    &id,
                    self.catalog.as_deref(),
                    wrapper.as_deref(),
                );
                if from_command {
                    edit.escalation = None;
                }
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
            // A saved folder is the host's history, and stays bound to its installation.
            Row::Folder(path) => {
                self.folder_picked();
                self.take_history_folder(path, cx);
                self.sync_fields(window, cx);
                self.changed(cx);
            }
            // The strip's fill, as a click on its row makes it: it never launches, from Enter here either (the old
            // search row did not). `search_row_refusal` has already said why a setup cannot be taken.
            Row::Recent { entry, .. } => {
                let _ = self.take_recent(&entry, window, cx);
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
        let field = self.launcher_input(
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
        // Above the box, where the old launcher said why a template did not apply (a taken `tl:` row, a shortcut's
        // press or sequence here, a quick launch that opened New instead), as its own alert.
        // One plain line in the warning ink, as the box's own refusal below it and the old template error were.
        let note = match &self.note {
            Some(super::Note::Refusal(sentence)) => Some(
                div()
                    .id("launcher-note")
                    .debug_selector(|| "launcher-note".into())
                    .role(Role::Status)
                    .aria_label(sentence.accessible_name())
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.warn))
                    .child(sentence.clone()),
            ),
            Some(super::Note::Banner(_)) | None => None,
        };
        // Above the box too, where the old launcher said why a taken row changed nothing, in the launcher's plain
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
            .children(note)
            .children(refusal)
            .child(
                div()
                    // Tests compare what is drawn above the box with the box itself.
                    .debug_selector(|| "launcher-search-box".into())
                    .relative()
                    .child(field)
                    .child(tracker),
            )
            .into_any_element()
    }
}

/// The results area's status line under `id`, in place of rows: the search's own message, or why the templates cannot
/// be listed. `label` is its accessible name.
fn status_line(
    id: &'static str,
    label: SharedString,
    ink: theme::Token,
    child: impl IntoElement,
) -> AnyElement {
    div()
        .id(id)
        .role(Role::Status)
        .aria_label(label)
        .p(px(6.))
        .text_size(metrics::TEXT_MD)
        .text_color(theme::color(ink))
        .child(child)
        .into_any_element()
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
            children.push(status_line(
                "launcher-search-message",
                message.text.into(),
                ink,
                message.text,
            ));
        }
        // Why `tl:` offers no rows while the helm cannot give its templates: the helm's words, escaped.
        if let Some(reason) = results.reason.clone() {
            let label = reason.accessible_name();
            children.push(status_line(
                "launcher-search-reason",
                label,
                palette.warn_bright,
                reason,
            ));
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
                let content = match row {
                    Row::Recent { entry, placement } => {
                        recent_lines(&id, entry, placement, index == active, palette)
                    }
                    _ => words.into_any_element(),
                };
                children.push(
                    OptionRow::new(id, label, help, Role::ListBoxOption)
                        .selected(index == active)
                        .child(content)
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

/// A recent setup's row content, on two lines as the old search row drew it: "Recent setup: " with where it started (in
/// the terminal font unless it is a repository) and the host, then, smaller and quieter, its model, effort and
/// permissions, with YOLO in the danger ink. The agent type is in the row's name and hover help, not drawn, as in the old
/// row. Both lines cut their text short rather than wrap.
fn recent_lines(
    id: &str,
    entry: &LaunchHistoryEntry,
    placement: &HostText,
    selected: bool,
    palette: &theme::Palette,
) -> AnyElement {
    let part = |name: &str| SharedString::from(format!("{id}-{name}"));
    let selection = &entry.selection;
    let yolo =
        selection.harness.request_permission(selection.permissions) == Some(LaunchPermission::Yolo);
    let destination = HostText::from_host(destination_label(entry));
    let first = HostSentence::new(part("destination")).text("Recent setup: ");
    let first = if entry.github_repo.is_none() {
        first.mono(&destination)
    } else {
        first.host(&destination)
    };
    let first = first.text(" · ").host(placement);
    // The old second line's `--fg-1`, and `--on-accent-dim` on the selected row's fill.
    let quiet = if selected {
        palette.on_accent_dim
    } else {
        palette.fg_1
    };
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .child(div().min_w_0().truncate().child(first))
        .child(
            div()
                .flex()
                .min_w_0()
                .truncate()
                .mt(px(2.))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(quiet))
                // A custom model id is host-supplied text, so the line is drawn escaped.
                .child(
                    HostSentence::new(part("selection"))
                        .host(&HostText::from_host(summary_before_permissions(selection)))
                        .text(" · Permissions: "),
                )
                .child(
                    div()
                        .flex_none()
                        .when(yolo, |word| word.text_color(theme::color(palette.danger)))
                        .child(permission_value(selection)),
                ),
        )
        .into_any_element()
}
