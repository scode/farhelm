//! The session launcher's command field turns a typed word into one choice in the draft. What a query offers is
//! [`crate::launcher::search`]'s. This module applies the choice and shares one results list between the left pane and
//! the narrow window popover. The empty pane's history and templates live in [`start`].
//!
//! SPEC.md, "Creation" (the launch-composer search sentences). Ported from the old launcher's search box and results
//! area (`list/create_form.rs`, the `.launch-composer-search` markup and its key handler, and `accept_search_result`
//! with `apply_search_result` in `list/create_form/draft.rs`) at reference commit `24bc0580`. Here's the TLDR:
//!
//! - **Typing** opens the pane's results or, in a narrow window, a non-focusing popover. Focus stays in the field,
//!   which preselects the row an exact word names ([`crate::launcher::search::Results::default`]).
//!   Up and Down move the highlighted row, wrapping, and the list keeps it in view. Escape, or a press outside, closes
//!   the results and keeps what was typed; Escape with them closed is the dialog's (it closes the launcher).
//! - **Enter** on an empty field presses the main action unless Down moved its highlight into the empty pane.
//!   In that case it takes the highlighted setup or template. With text, Enter reopens closed results, takes the highlighted row of open
//!   ones, and does nothing when they offer no default row (their message stays). A vanished explicit choice consumes
//!   Enter, keeps the query and shows an inline warning. A held Enter does nothing (SPEC.md: "A held Enter never
//!   launches"), so holding it after a pick cannot launch the pick.
//! - **Taking a row** (Enter or a click) makes the one edit it names through the same draft method a hand edit uses,
//!   then empties the box and closes the results, with focus back in the box for the next word; choosing Custom moves
//!   focus on to its flags, as choosing it by hand does. An agent type or model taken on the Command tab switches to
//!   the Agent tab, where they belong. A row that would change what this launcher holds fixed (Restart with's agent
//!   type, host, folder and name; Replace with's host) shows as disabled. A refusal that reaches apply keeps the query
//!   and closes results so its reason can show below the field.
//! - **A `tl:` row** applies its template to the draft through the templates' own application path, as a template
//!   shortcut's press in the open launcher does (`crate::templates::dispatch::search_row_batch`); a template that
//!   does not apply changes nothing, and the launcher's refusal note says why below the field.
//!   While the helm cannot give its templates, `tl:` shows the helm's words in
//!   the results instead of rows.
//!
//! - **Rows from the chosen host's launch history.** A saved folder becomes the folder as a pick from the folder list
//!   does (leaving a managed checkout, keeping the tab). A model from a past launch chooses the agent type it ran with
//!   and the model as that type's custom id. A recent setup fills the draft as a click on an empty-pane row does
//!   ([`Launcher::take_recent`]) and never launches, from Enter either: the old search row only
//!   filled, and SPEC.md's Enter that launches is the empty pane's highlighted row. A setup carrying a retired container choice,
//!   a new environment's setup in Replace with, or a folder or setup from a history that is no longer the chosen
//!   host's, is refused below the field. While a new environment is chosen, every folder row and Browse
//!   is refused there too, in the old words: the environment's folder is its fixed home (`environment_refusal`). A
//!   recent setup is still taken, with its own place.
//!
//! What the box holds besides the draft (the open results, the highlight, the last refusal) is the launcher's alone;
//! what is typed is the field's own text, read when the rows are needed, never copied.

use farhelm_proto::session_requests::LaunchHistoryEntry;
use farhelm_proto::ui_channel::HostView;
use std::cell::RefCell;

use farhelm_proto::{LaunchPermission, RETIRED_CONTAINER_REASON, join_permission_flags};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    ScrollHandle, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, WeakEntity,
    Window, div, px,
};

use super::Launcher;
use super::field_list::{Drawn, FieldList, step};
use super::host_name_with_phase;
use super::recents::{KEEPS_ENVIRONMENT, RecentRefusal, RecentSurface, recent_hover, wrapper_tail};
use crate::assets::{Icon, TERMINAL_FONT};
use crate::components::{
    HoverHelp, OptionRow, Role, ScrollBox, Vertical, chips_and_words, command_field,
    command_heading, command_icon_tile, key_chip, scroll_detail_page,
};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::short_name;
use crate::launcher::custom_registration::CustomRegistration;
use crate::launcher::draft::{Field, Install, Mode, Tab};
use crate::launcher::recents::{
    SetupKey, destination_label, permission_presentation, setup_key, summary_before_permissions,
    summary_without_wrapper, wrapper_summary,
};
use crate::launcher::request::capitalized;
use crate::launcher::search::{Inputs, Results, Row, Scope, scoped, search};
use crate::prompt::ActionFrame;
use crate::theme::{self, metrics};

mod start;

/// The input's name describes what typing there does and stays in place when a query hides its placeholder.
const SEARCH_LABEL: &str = "Choose an agent, host, folder, model or session name";

/// Keep the dialog's name and the empty field's opening words in step.
pub(super) const fn command_title(mode: &Mode) -> &'static str {
    match mode {
        Mode::New => "New session",
        Mode::Clone => "Clone session",
        Mode::ReplaceWith { .. } => "Replace session",
        Mode::RestartWith(_) => "Restart session",
    }
}

/// The same Return symbol appears beside what it does in the empty field and on the action itself.
pub(super) const RETURN_KEY: &str = "⏎";
/// Several lines of a message can stay visible without the header taking over a short window.
const COMMAND_NOTICE_HEIGHT: f32 = 88.;
/// The results' element id (`.launch-composer-search-results`), which tests find them by.
const SEARCH_RESULTS: &str = "launcher-search-results";
/// A row was offered when the user chose it, but that choice no longer exists.
const STALE_ROW: &str = "That choice is no longer available; choose a current result";

/// The right side still needs room for the address and several agent controls to share rows.
const MIN_WIDE_FORM_WIDTH: f32 = 650.;

/// Leave room for choices at the shortest supported window height. Longer flags page inside the footer.
const MAX_RECENT_FLAGS_HEIGHT: f32 = 140.;

/// The 250-pixel pane leaves at least 650 pixels for the address and agent controls on its right. Use the same
/// horizontal inset as DialogFrame so the two-column threshold is based on the card, not the whole window.
pub(super) fn command_pane_fits(window: &Window) -> bool {
    crate::desktop::linux_title_bar::content_area(window)
        .size
        .width
        >= px(start::PANE_WIDTH + MIN_WIDE_FORM_WIDTH) + metrics::DIALOG_SIDE_MARGIN * 2.
}

/// A remembered source and an empty pane's keyboard target both follow the row, never its current index.
#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "only the source, keyboard target and its shown snapshot hold setup identities"
)]
pub(super) enum SetupSource {
    Recent(Option<Install>, SetupKey),
    Template(String),
}

impl SetupSource {
    /// A host's setup belongs to the installation that offered it. New-environment history belongs to the helm.
    pub(super) fn recent(entry: &LaunchHistoryEntry, host: Option<&HostView>) -> Self {
        let install = if entry.target.is_none() {
            host.map(Install::of)
        } else {
            None
        };
        Self::Recent(install, setup_key(entry))
    }
}

/// Identity captured when an arrow or a presented row makes an explicit choice. Host history rows also carry the
/// installation that offered them, even when the next installation happens to offer an identical row.
#[derive(Clone)]
struct SearchChoice {
    row: Row,
    history_install: Option<Install>,
}

impl SearchChoice {
    /// Bind just the choices that depend on the host's history. A model already offered by the helm is independent
    /// of the installation, even if the same model was also in the current host's history.
    fn new(row: &Row, launcher: &Launcher, cx: &App) -> Self {
        let host_history = match row {
            Row::Folder(_) => true,
            Row::HistoryModel { id, harness } => !launcher.has_environment_model(*harness, id),
            Row::Recent { entry, .. } => entry.target.is_none(),
            _ => false,
        };
        let history_install = if host_history {
            launcher.chosen_host(cx).map(Install::of)
        } else {
            None
        };
        Self {
            row: row.clone(),
            history_install,
        }
    }

    /// Resolve against the current results without falling back to a different row or installation.
    fn position(&self, results: &Results, host: Option<&HostView>) -> Option<usize> {
        if let Some(install) = &self.history_install
            && host.map(Install::of).as_ref() != Some(install)
        {
            return None;
        }
        results
            .rows
            .iter()
            .position(|row| same_search_choice(row, &self.row))
    }
}

/// What the search box holds besides the draft and its own text. None of it is shared with another surface.
#[derive(Default)]
pub(super) struct SearchBox {
    /// The narrow results popover. Both presentations choose their default against the current rows, since a live
    /// update can insert a row ahead of an exact match.
    list: FieldList,
    /// Wide results are part of the dialog and need no floating layer. Only one presentation is open at a time.
    inline_results: bool,
    /// The field retains focus. Down explicitly starts navigating the empty pane and Up from the first row clears it.
    start_highlight: Option<SetupSource>,
    /// Enter uses the displayed start row or main action. A resize can withdraw the row before its next frame is shown.
    start_enter_frame: ActionFrame<Option<SetupSource>>,
    /// The last child revealed by keyboard navigation, so drawing after a mouse wheel does not undo the scroll.
    pub(super) pane_revealed: Option<usize>,
    /// The setup or last template that filled the form. Hand edits leave its provenance visible until Reset.
    pub(super) source: Option<SetupSource>,
    /// Why the last row taken changed nothing (a field this launcher holds fixed), said below the field until the next
    /// keystroke in it or the next row taken.
    refusal: Option<HostSentence>,
    /// A deliberate arrow pick survives live updates until Enter can take it or explain its disappearance.
    typed_choice: Option<SearchChoice>,
    /// The matching viewport can be drawn in a separate popover after the command field itself. Share its scroll
    /// handle with the field's key listener, indexed by the same setup identity as the keyboard highlight.
    recent_detail: RefCell<Option<RecentDetail>>,
}

/// Keep wheel and key offsets through redraws of the same choice. Another setup starts with its first flag, even
/// if it shares most of its words with this one.
struct RecentDetail {
    source: SetupSource,
    scroll: ScrollHandle,
}

impl Launcher {
    /// What the box's text offers now, against the draft, the catalog and the registry's hosts as they stand.
    pub(in crate::launcher) fn search_results(&self, cx: &App) -> Results {
        let query = self.search.text(cx);
        // The search reads hosts by value; a launcher sees a handful, so the copy is cheap.
        let hosts = self.hosts(cx);
        // Every template, as the templates' own rows give them, which the search filters; only a `tl:` query reads them.
        let templates = if scoped(&query).0 == Scope::Templates {
            crate::templates::rows::rows("", cx)
        } else {
            Ok(Vec::new())
        };
        // The chosen host's history rows name that host as its own field does, phase included; a new environment's
        // setup names its own placement.
        let host = self.chosen_host(cx).map(host_name_with_phase);
        let history = self.recent_history(cx);
        // Empty unless the query is `gh:` (`checkout`).
        let (repositories, repository_pending) = self.search_repositories(cx);
        search(&Inputs {
            query: &query,
            tab: self.draft.tab,
            harness: self.draft.agent.harness,
            model: self.draft.agent.model.as_deref(),
            catalog: self.catalog.as_deref(),
            custom_agents: &self
                .store
                .read(cx)
                .collections()
                .ordered::<farhelm_proto::custom_agents::CustomAgent>(),
            catalog_pending: self.catalog.is_none() && self.catalog_error.is_none(),
            hosts: &hosts,
            templates: templates.as_deref(),
            history: history.as_ref(),
            host: host.as_ref(),
            repositories: &repositories,
            repository_pending,
        })
    }

    /// Forget the keyboard's explicit result when starting a fresh search.
    pub(super) fn reset_search_highlight(&mut self) {
        self.search_box.typed_choice = None;
        self.search_box.pane_revealed = None;
        self.search_box.recent_detail.get_mut().take();
    }

    /// A sole result can look selected without an arrow pick. Rename fixtures require the explicit choice.
    #[cfg(test)]
    pub(crate) fn has_explicit_search_choice(&self) -> bool {
        self.search_box.typed_choice.is_some()
    }

    /// Whether the results are open: opened by the box and not since closed by the layer manager.
    pub(super) fn search_open(&self, window: &Window, cx: &App) -> bool {
        self.search_box.inline_results || self.search_box.list.is_open(window, cx)
    }

    /// Resolve a deliberate arrow pick by identity. A missing row takes Enter with it until another choice is made.
    fn search_highlight(&self, results: &Results, cx: &App) -> Option<usize> {
        if let Some(picked) = &self.search_box.typed_choice {
            return picked.position(results, self.chosen_host(cx));
        }
        (results.default < results.rows.len()).then_some(results.default)
    }

    /// The user typed in the box: forget the last refusal, preselect what the new text names, and show the results
    /// (or close them when the text offers nothing to show, an emptied box).
    pub(super) fn search_typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_box.refusal = None;
        self.reset_search_highlight();
        self.search_box.start_highlight = None;
        let results = self.search_results(cx);
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
        if self.search.text(cx).is_empty() && command_pane_fits(window) {
            self.search_box.refusal = None;
            self.step_start_choice(arrow, cx);
            return;
        }
        if !self.search_open(window, cx) {
            return;
        }
        let results = self.search_results(cx);
        let from = self.search_highlight(&results, cx);
        self.search_box.refusal = None;
        self.search_box.typed_choice = step(from, arrow, results.rows.len())
            .map(|index| SearchChoice::new(&results.rows[index], self, cx));
        cx.notify();
    }

    /// Enter in the box (see the module docs). `held` is an auto-repeat, which does nothing.
    pub(super) fn search_enter(&mut self, held: bool, window: &mut Window, cx: &mut Context<Self>) {
        if held {
            return;
        }
        // Literally empty, as the old check was: a box holding only spaces is a query, and never launches.
        if self.search.text(cx).is_empty() {
            let row = command_pane_fits(window)
                .then(|| self.search_box.start_highlight.clone())
                .flatten();
            if !matches!(self.draft.mode, Mode::RestartWith(_))
                && !self
                    .search_box
                    .start_enter_frame
                    .current()
                    .is_some_and(|frame| frame.shown() == Some(&row))
            {
                return;
            }
            if let Some(row) = row {
                self.take_start_choice(&row, true, window, cx);
                return;
            }
            self.close_search(window, cx);
            self.press(window, cx);
            return;
        }
        if !self.search_open(window, cx) {
            self.reset_search_highlight();
            self.open_search(window, cx);
            cx.notify();
            return;
        }
        let results = self.search_results(cx);
        if let Some(index) = self.search_highlight(&results, cx) {
            self.take_search_row(results.rows[index].clone(), window, cx);
        } else if let Some(choice) = &self.search_box.typed_choice
            && (self.save_panel.is_none() || !matches!(choice.row, Row::Template(_)))
        {
            let reason = self.withdrawn_row_reason(&choice.row, cx);
            self.refuse_stale_choice(reason, window, cx);
        }
    }

    /// A click can arrive after the frame that offered its row is already out of date. Reuse the current choice
    /// when its identity still matches, and otherwise preserve the query for another choice.
    fn take_drawn_search_row(
        &mut self,
        picked: &SearchChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut results = self.search_results(cx);
        if let Some(index) = picked.position(&results, self.chosen_host(cx)) {
            self.take_search_row(results.rows.swap_remove(index), window, cx);
        } else if self.save_panel.is_none() || !matches!(picked.row, Row::Template(_)) {
            let reason = self.withdrawn_row_reason(&picked.row, cx);
            self.refuse_stale_choice(reason, window, cx);
        }
    }

    /// A stale registration cannot reach the choice handler. Explain its absence using the live catalog,
    /// since a failed read cannot establish that the user needs to register it again.
    fn withdrawn_row_reason(&self, row: &Row, cx: &App) -> Option<HostSentence> {
        let Row::CustomAgent { id, .. } = row else {
            return None;
        };
        let registration = CustomRegistration::read(self.store.read(cx).collections(), id);
        super::choices::custom_registration_words(
            &registration,
            "That custom agent is no longer registered",
        )
    }

    /// Consume the stale choice. Closing results exposes the inline warning even in the narrow popover layout.
    fn refuse_stale_choice(
        &mut self,
        reason: Option<HostSentence>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search_box.refusal = Some(
            reason.unwrap_or_else(|| HostSentence::new("launcher-refusal-reason").text(STALE_ROW)),
        );
        self.close_search(window, cx);
        cx.notify();
    }

    /// Take `row`, from Enter or a click: refuse it below the field when it would change a field this launcher holds
    /// fixed, else make its one edit, empty the box, close the results and keep focus in the box (Custom's flags take
    /// it instead).
    fn take_search_row(&mut self, row: Row, window: &mut Window, cx: &mut Context<Self>) {
        // A template pick under the save panel leaves the query and focus alone, just like a template shortcut.
        if self.save_panel.is_some() && matches!(row, Row::Template(_)) {
            return;
        }
        let refusal = self.row_disabled(&row, cx).or_else(|| {
            self.search_row_refusal(&row, cx)
                .map(|words| HostSentence::new("launcher-refusal-reason").text(words))
        });
        if let Some(refusal) = refusal {
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

    /// Why taking `row` would change nothing, in the old launcher's words where it had some: a field this launcher
    /// holds fixed ([`Self::held_by_row`]), a folder row or Browse while a new environment is chosen
    /// ([`environment_refusal`]), a recent setup carrying a retired container choice or, in Replace with, a new
    /// environment ([`KEEPS_ENVIRONMENT`]), or a host, template or history row since withdrawn.
    fn search_row_refusal(&self, row: &Row, cx: &App) -> Option<String> {
        if let Some(reason) = self.held_row_reason(row) {
            return Some(reason);
        }
        if self.draft.target.is_some()
            && let Some(words) = environment_refusal(row)
        {
            return Some(words.to_owned());
        }
        let stale = match row {
            Row::Recent { entry, .. } => {
                match self.recent_refusal(entry, RecentSurface::Search, cx)? {
                    RecentRefusal::Retired => return Some(RETIRED_CONTAINER_REASON.to_owned()),
                    RecentRefusal::KeepsEnvironment => return Some(KEEPS_ENVIRONMENT.to_owned()),
                    RecentRefusal::Stale => true,
                    // Not reached: `held_by_row` refused above what Restart with holds.
                    RecentRefusal::Held => {
                        debug_assert!(
                            false,
                            "a held destination is refused before the setup is looked at"
                        );
                        false
                    }
                }
            }
            Row::Folder(path) => !self.launch_history(cx).is_some_and(|history| {
                history
                    .folders
                    .iter()
                    .any(|folder| folder.display_cwd == *path)
            }),
            Row::Host { id, .. } => self
                .store
                .read(cx)
                .collections()
                .get::<HostView>(&HostView::key(*id))
                .is_none_or(|host| host.is_session_owned),
            Row::Template(name) => self
                .store
                .read(cx)
                .collections()
                .get::<farhelm_proto::launcher::LaunchTemplate>(name)
                .is_none(),
            _ => false,
        };
        stale.then(|| STALE_ROW.to_owned())
    }

    /// Show a disabled choice with the existing refusal. A template can still repeat a held value as a no-op.
    fn row_disabled(&self, row: &Row, cx: &App) -> Option<HostSentence> {
        if let Row::Template(name) = row
            && self.draft.holds(Field::Host)
        {
            return crate::templates::dispatch::held_row_refusal(
                &self.draft,
                name,
                &self.hosts(cx),
                cx,
            );
        }
        self.held_row_reason(row)
            .or_else(|| {
                matches!(row, Row::Recent { entry, .. } if entry.target.is_some() && self.draft.holds(Field::Host))
                    .then(|| KEEPS_ENVIRONMENT.to_owned())
            })
            .map(|words| HostSentence::new("launcher-held-choice").text(words))
    }

    /// Use the same words in a held row's help and in the refusal if a keyboard pick or an outdated row reaches it.
    fn held_row_reason(&self, row: &Row) -> Option<String> {
        let field = self.held_by_row(row)?;
        let mode = if matches!(self.draft.mode, Mode::ReplaceWith { .. }) {
            "replace"
        } else {
            "restart"
        };
        Some(capitalized(&format!(
            "{field} cannot change here: this dialog keeps {field} fixed for {mode}"
        )))
    }

    /// The field, in the old refusal's words, that taking `row` would change while this launcher holds it fixed: any
    /// agent type (a model too, unless it is one of the held agent type's own on the Agent tab), host, folder or name,
    /// even one naming the current value (SPEC.md, "Restart with": "Search results and recent setups that set a held
    /// field are still refused even when they name its current value").
    fn held_by_row(&self, row: &Row) -> Option<&'static str> {
        let (field, words) = match row {
            Row::Harness(_) | Row::CustomAgent { .. } => (Field::AgentType, "agent type"),
            Row::Model { harness, .. } | Row::HistoryModel { harness, .. }
                if self.draft.tab != Tab::Agent || self.draft.agent.harness != Some(*harness) =>
            {
                (Field::AgentType, "agent type")
            }
            Row::Host { .. } => (Field::Host, "host"),
            // A recent setup sets the agent type too, but the old refusal named the destination, which every mode that
            // holds the agent type also holds.
            Row::UsePath(_)
            | Row::BrowsePath(_)
            | Row::Folder(_)
            | Row::Recent { .. }
            | Row::Github(_) => (Field::Destination, "destination"),
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
            Row::Host { id, .. } => self.pick_host(id, window, cx),
            // Choosing an agent type makes the tab Agent itself; a model of the agent already chosen does not, so the
            // model row switches it first.
            Row::Harness(harness) => self.choose_harness(harness, window, cx),
            Row::CustomAgent { id, .. } => self.choose_custom_agent(&id, window, cx),
            // Through the templates' application path, as a template shortcut's press in the open launcher: it waits
            // for the catalog read if that is out. A refusal changes nothing and is said below the field.
            Row::Template(name) => self.apply_batch(
                crate::templates::dispatch::search_row_batch(name),
                window,
                cx,
            ),
            Row::Model { id, harness, .. } => {
                let picked_agent =
                    self.draft.agent.harness != Some(harness) || self.draft.tab != Tab::Agent;
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
                if picked_agent && self.draft.agent.harness == Some(harness) {
                    self.check_hook_question(window, cx);
                }
            }
            // A model from a past launch, recognized or custom on the agent it ran with.
            Row::HistoryModel { id, harness } => {
                let picked_agent =
                    self.draft.agent.harness != Some(harness) || self.draft.tab != Tab::Agent;
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
                if picked_agent && self.draft.agent.harness == Some(harness) {
                    self.check_hook_question(window, cx);
                }
            }
            Row::Effort(effort) => self.choose_effort(effort, window, cx),
            Row::Permission(permission) => {
                self.choose_permissions(Some(permission.permission()), window, cx)
            }
            Row::UsePath(path) => {
                // A folder taken from search is a pick, not typing: the folder list shows every recent folder again.
                self.folder_picked();
                self.draft.edit_folder(path);
                self.sync_fields(window, cx);
                self.changed(cx);
            }
            // The typed path as the folder, as "Use this path" makes it, and then its listing (`browse`).
            Row::BrowsePath(path) => {
                self.folder_picked();
                self.draft.edit_folder(path);
                self.sync_fields(window, cx);
                self.changed(cx);
                self.browse(window, cx);
            }
            // A saved folder is the host's history, and stays bound to its installation.
            Row::Folder(path) => {
                self.folder_picked();
                self.take_history_folder(path, cx);
                self.sync_fields(window, cx);
                self.changed(cx);
            }
            // The empty pane's fill, as a click on its row makes it: it never launches, from Enter here either (the old
            // search row did not). `search_row_refusal` has already said why a setup cannot be taken.
            Row::Recent { entry, .. } => {
                let _ = self.take_recent(&entry, RecentSurface::Search, window, cx);
            }
            // A managed checkout of the repository, as the repository field's pick makes it (`checkout`).
            Row::Github(repo) => self.pick_repository(repo, window, cx),
        }
    }

    /// Use the pane at wide sizes, otherwise open under the field in a popover that never takes focus.
    fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if command_pane_fits(window) {
            self.search_box.inline_results = true;
            return;
        }
        self.search_box
            .list
            .open(SEARCH_RESULTS, draw_search_rows, window, cx);
    }

    /// Close the results if they are open. What was typed stays; the highlight goes, and the next opening sets it
    /// again.
    pub(super) fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_box.inline_results = false;
        self.search_box.start_highlight = None;
        self.reset_search_highlight();
        self.search_box.list.close(window, cx);
    }

    /// Give the command list its first Escape before the dialog closes. Its popover handles this itself when narrow.
    pub(super) fn dismiss_command_pane(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.search_box.inline_results && self.search_box.start_highlight.is_none() {
            return false;
        }
        if self.search_box.start_highlight.is_some() {
            // Holding Escape to clear the highlight leaves the draft open until a fresh press.
            focus::note_closing(true, window, cx);
        }
        self.close_search(window, cx);
        cx.notify();
        true
    }

    /// Move already-open results to the presentation that fits a resized window, retaining the query and choice.
    pub(super) fn reflow_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wide = command_pane_fits(window);
        if !wide {
            self.search_box.start_highlight = None;
        }
        let inline = self.search_box.inline_results;
        if self.search_open(window, cx) && wide != inline {
            let choice = self.search_box.typed_choice.take();
            self.close_search(window, cx);
            self.search_box.typed_choice = choice;
            self.open_search(window, cx);
        }
        if !matches!(self.draft.mode, Mode::RestartWith(_)) {
            if self.search.text(cx).is_empty() {
                self.search_box.start_enter_frame.capture(
                    self.search_box.start_highlight.clone(),
                    (),
                    window,
                );
            } else {
                self.search_box.start_enter_frame.clear();
            }
        }
    }

    /// The command field, the detail beneath it and the tracker for the whole row it shares with Close.
    ///
    /// It also forgets results the layer manager closed on its own (Escape, a press outside), which tells the box
    /// nothing; that relies on the launcher being drawn in every frame its dialog is, as the model field does.
    pub(super) fn command_header(
        &mut self,
        palette: &theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, Option<AnyElement>, AnyElement) {
        if self.search_box.list.follow(window, cx) {
            self.reset_search_highlight();
        }
        let flags_editor = cx.weak_entity();
        let tracker = self
            .search_box
            .list
            .track_anchor("launcher-search-anchor", window, cx);
        let weak = cx.weak_entity();
        let empty = self.search.text(cx).is_empty();
        let mark = command_icon_tile(Icon::Launch, palette)
            .debug_selector(|| "launcher-command-mark".into());
        let suffix = empty.then(|| {
            key_chip(
                format!("{RETURN_KEY} on empty launches"),
                palette.fg_1,
                palette,
            )
            .id("launcher-empty-launch-hint")
            .debug_selector(|| "launcher-empty-launch-hint".into())
            .role(Role::Label)
            .aria_label("Enter on an empty field launches")
        });
        let field = command_field(
            self.launcher_input(
                "launcher-search",
                &self.search,
                SEARCH_LABEL,
                HoverHelp::tooltip(
                    concat!(
                        "Type a name, host, folder, harness, model, effort or permission, then press Enter to choose it. ",
                        "Press Enter in an empty field to start this setup. ",
                        "Page Up and Page Down scroll long approval flags in a highlighted Recent setup."
                    ),
                ),
            ),
            mark,
            suffix,
        )
        .on_vertical(move |arrow, window, cx| {
            let _ = weak.update(cx, |launcher, cx| launcher.search_arrow(arrow, window, cx));
        });
        let field = div()
            .id("launcher-search-row")
            // Tests press the box by where its row is drawn, as they do the other fields' rows.
            .debug_selector(|| "launcher-search-row".into())
            // Kit submits both modified Enter actions in a single-line field. Only plain Enter picks or launches.
            .capture_action(|enter: &gpui_component::input::Enter, _, cx| {
                if enter.secondary || enter.shift {
                    cx.stop_propagation();
                }
            })
            .on_key_down(move |event, window, cx| {
                let scroll = flags_editor.upgrade().and_then(|launcher| {
                    let launcher = launcher.read(cx);
                    // A popover can close between frames. Do not give its last viewport the command field's keys.
                    let active = launcher.search_open(window, cx)
                        || (command_pane_fits(window)
                            && launcher.search.text(cx).is_empty()
                            && launcher.search_box.start_highlight.is_some());
                    if !active {
                        return None;
                    }
                    launcher
                        .search_box
                        .recent_detail
                        .borrow()
                        .as_ref()
                        .map(|detail| detail.scroll.clone())
                });
                if let Some(scroll) = scroll {
                    scroll_detail_page(event, &scroll, window, cx);
                }
            })
            .child(
                div()
                    .debug_selector(|| "launcher-search-box".into())
                    .relative()
                    .child(field),
            )
            .into_any_element();
        let guidance = match &self.draft.mode {
            Mode::New | Mode::Clone => empty.then(|| self.command_hints(palette, cx)),
            Mode::ReplaceWith { .. } => Some(command_summary(
                "Start a new session, then delete the source",
                palette,
            )),
            Mode::RestartWith(_) => Some(command_summary(
                "Resume this session with different settings",
                palette,
            )),
        };
        let notices = self.command_notices(palette);
        let detail = (guidance.is_some() || notices.is_some()).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(guidance)
                .children(notices)
                .into_any_element()
        });
        (field, detail, tracker.into_any_element())
    }

    /// Show command notices by the field even when the form below has scrolled away from its start. A refusal
    /// can name many competing templates, so the notice can scroll on its own without displacing the form or footer.
    fn command_notices(&self, palette: &theme::Palette) -> Option<AnyElement> {
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
            Some(super::Note::Banner(_) | super::Note::WakeRefusal(_)) | None => None,
        };
        let refusal = self.search_box.refusal.clone().map(|words| {
            div()
                .id("launcher-search-refusal")
                .role(Role::Status)
                .aria_label(words.accessible_name())
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.warn))
                .child(words)
        });
        (note.is_some() || refusal.is_some()).then(|| {
            let messages = div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(note)
                .children(refusal);
            ScrollBox::new(
                "launcher-command-notices",
                "Command field messages",
                HoverHelp::tooltip("Read messages from the command field"),
                Role::Region,
                messages,
            )
            .max_height(px(COMMAND_NOTICE_HEIGHT))
            .into_any_element()
        })
    }

    /// Keep the examples literal so they can be typed. The Host example uses a searchable short form of a real
    /// host name so an SSH login and domain do not take up the hint row. Use the default example on an empty fleet.
    fn command_hints(&self, palette: &theme::Palette, cx: &App) -> AnyElement {
        let host = self
            .store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .into_iter()
            .filter(|host| !host.is_local() && !host.is_session_owned)
            .find_map(|host| {
                let short = short_name(host);
                let name = short.raw();
                (!name.is_empty()
                    && !name.eq_ignore_ascii_case("local")
                    && name
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-.@_".contains(&c))
                    && host
                        .name
                        .to_ascii_lowercase()
                        .contains(&name.to_ascii_lowercase()))
                .then(|| name.to_owned())
            })
            .unwrap_or_else(|| "build-box".to_owned());
        let example_host = format!("host:{host}");
        div()
            .id("launcher-command-hints")
            .debug_selector(|| "launcher-command-hints".into())
            .flex()
            .flex_wrap()
            .gap_x(px(12.))
            .gap_y(px(4.))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_1))
            .children(
                [
                    ("claude", "pick an agent"),
                    (&example_host, "another host"),
                    ("gh:owner/repo", "clone a repo"),
                    ("tl:", "templates"),
                    ("fix login bug", "names the session"),
                ]
                .into_iter()
                .map(|(example, description)| {
                    chips_and_words([
                        key_chip(example.to_owned(), palette.accent, palette)
                            .font_family(TERMINAL_FONT)
                            .into_any_element(),
                        description.into_any_element(),
                    ])
                }),
            )
            .into_any_element()
    }
}

/// The destructive modes keep the reason for their action visible regardless of the command field's query.
fn command_summary(words: &'static str, palette: &theme::Palette) -> AnyElement {
    div()
        .id("launcher-mode-summary")
        .role(Role::Label)
        .aria_label(words)
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(words)
        .into_any_element()
}

/// Keep a keyboard target through a rename or other display update. The ID says which live choice the user meant.
fn same_search_choice(a: &Row, b: &Row) -> bool {
    match (a, b) {
        (Row::Host { id: a, .. }, Row::Host { id: b, .. }) => a == b,
        (Row::CustomAgent { id: a, .. }, Row::CustomAgent { id: b, .. }) => a == b,
        (
            Row::Model {
                id: a, harness: x, ..
            },
            Row::Model {
                id: b, harness: y, ..
            },
        ) => a == b && x == y,
        (Row::Recent { entry: a, .. }, Row::Recent { entry: b, .. }) => {
            a.host == b.host && setup_key(a) == setup_key(b)
        }
        _ => a == b,
    }
}

/// Why `row` cannot apply while a new environment is chosen, or `None` when it can (SPEC.md, "Container sessions":
/// Folder destinations "use its home, without Browse or recent folders"). A folder row would move the folder off the
/// environment's fixed home, and Browse would list a machine that does not exist yet; both are refused in the old
/// search's words. A recent setup applies: it carries its own place (SPEC.md, "Creation"), so taking one goes back to
/// that place ([`Launcher::take_recent`]), as the old search did.
fn environment_refusal(row: &Row) -> Option<&'static str> {
    match row {
        Row::UsePath(_) | Row::Folder(_) => {
            Some("A new sandbox's Folder is fixed to its home directory")
        }
        Row::BrowsePath(_) => Some("A new sandbox cannot be browsed before it exists"),
        Row::Recent { .. }
        | Row::Github(_)
        | Row::Name(_)
        | Row::Host { .. }
        | Row::Harness(_)
        | Row::CustomAgent { .. }
        | Row::Template(_)
        | Row::Model { .. }
        | Row::HistoryModel { .. }
        | Row::Effort(_)
        | Row::Permission(_) => None,
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

/// The open results (`field_list`): the search's message or the helm's reason if it has one, then each group's heading
/// and rows, with the highlighted row selected.
fn draw_search_rows(
    launcher: &Launcher,
    list: Stateful<Div>,
    weak: &WeakEntity<Launcher>,
    window: &Window,
    cx: &App,
) -> Drawn {
    let palette = theme::palette(cx);
    let inline = command_pane_fits(window);
    let text = launcher.search.text(cx);
    let (_, query) = scoped(&text);
    let results = launcher.search_results(cx);
    let harness = launcher.draft.agent.harness;
    let active = launcher.search_highlight(&results, cx);
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
    let collections = launcher.store.read(cx).collections();
    for (group, rows) in results.groups() {
        children.push(command_heading(
            SharedString::from(format!("launcher-search-group-{}", group.label())),
            group.label(),
            palette,
        ));
        for (index, row) in rows {
            let selected = Some(index) == active;
            if selected {
                active_child = Some(children.len());
            }
            let id = format!("launcher-search-option-{index}");
            let words = row.words(SharedString::from(format!("{id}-words")), collections);
            let label = words.accessible_name();
            let help = row.help(
                harness,
                SharedString::from(format!("{id}-help")),
                collections,
            );
            let refusal = launcher.row_disabled(row, cx);
            let disabled = refusal.is_some();
            let help = match row {
                Row::Recent { entry, .. } => {
                    recent_hover(help, entry.selection.wrapper_prefix.is_some(), refusal)
                }
                _ => HoverHelp::sentence(refusal.unwrap_or(help)),
            };
            let weak = weak.clone();
            let taken = SearchChoice::new(row, launcher, cx);
            let content = if inline {
                start::typed_choice(
                    launcher,
                    row,
                    words,
                    query,
                    selected && !disabled,
                    palette,
                    cx,
                )
            } else {
                match row {
                    Row::Recent { entry, placement } => {
                        recent_lines(&id, entry, placement, selected, query, palette, collections)
                    }
                    Row::Template(_) => words.underline_template_matches(query).into_any_element(),
                    _ => words.underline_matches(query).into_any_element(),
                }
            };
            children.push(
                OptionRow::new(id, label, help, Role::ListBoxOption)
                    .when(inline, OptionRow::compact)
                    .when(!inline, OptionRow::in_popover)
                    .selected(selected)
                    .disabled(disabled)
                    .child(content)
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |launcher, cx| {
                            launcher.take_drawn_search_row(&taken, window, cx)
                        });
                    })
                    .into_any_element(),
            );
        }
    }
    let recent = active.and_then(|index| match &results.rows[index] {
        Row::Recent { entry, .. } => Some(entry.as_ref()),
        _ => None,
    });
    Drawn {
        list,
        children,
        highlighted: active_child,
        footer: launcher.recent_permission_footer(recent, palette, cx),
    }
}

impl Launcher {
    /// Keep the exact saved arguments readable when two choices share a row summary. The footer sits outside
    /// the list, and paging inside it leaves the highlighted row and its Enter action alone.
    fn recent_permission_footer(
        &self,
        recent: Option<&LaunchHistoryEntry>,
        palette: &theme::Palette,
        cx: &App,
    ) -> Option<AnyElement> {
        let recent = recent.and_then(|entry| {
            let selection = &entry.selection;
            let flags = selection.permission_flags.as_deref()?;
            (selection.permissions == Some(LaunchPermission::Custom) && !flags.is_empty())
                .then_some((entry, flags))
        });
        let mut viewport = self.search_box.recent_detail.borrow_mut();
        let Some((entry, flags)) = recent else {
            viewport.take();
            return None;
        };
        let source = SetupSource::recent(entry, self.chosen_host(cx));
        if viewport
            .as_ref()
            .is_some_and(|detail| detail.source != source)
        {
            viewport.take();
        }
        let scroll = &viewport
            .get_or_insert_with(|| RecentDetail {
                source,
                scroll: ScrollHandle::new(),
            })
            .scroll;
        let flags = HostText::from_host(join_permission_flags(flags));
        Some(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .min_w_0()
                .gap(px(3.))
                .p(px(10.))
                .border_t_1()
                .border_color(theme::color(palette.border_dim))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .child("Custom approval flags")
                .child(
                    div()
                        .id("launcher-recent-flags-scroll")
                        .max_h(px(MAX_RECENT_FLAGS_HEIGHT))
                        .overflow_y_scroll()
                        .track_scroll(scroll)
                        .child(HostSentence::new("launcher-recent-permission-flags").mono(&flags)),
                )
                .into_any_element(),
        )
    }
}

/// Show the current name of the saved registration. When search matched its ID alone, show that ID beside the name
/// so a match has visible text to underline. A deleted registration still has its saved ID.
pub(super) fn recent_agent_words(
    sentence: HostSentence,
    selection: &farhelm_proto::LaunchSelection,
    query: &str,
    collections: &farhelm_helm_client::Collections,
) -> HostSentence {
    let Some(id) = selection.custom_agent.as_deref() else {
        return sentence
            .text(super::harness_label(selection.harness))
            .underline_matches(query);
    };
    let name = crate::harness::custom_name(id, collections);
    let term = query.to_lowercase();
    let sentence = sentence.host(&name);
    let sentence =
        if !term.is_empty() && id.contains(&term) && !name.raw().to_lowercase().contains(&term) {
            sentence.text(" (").host(&HostText::from_host(id)).text(")")
        } else {
            sentence
        };
    sentence.underline_template_matches(query)
}

/// Keep a registration's name ahead of the destination in narrow results, where a deep path could otherwise
/// hide the match. The quieter second line names the saved model, effort and permissions.
fn recent_lines(
    id: &str,
    entry: &LaunchHistoryEntry,
    placement: &HostText,
    selected: bool,
    query: &str,
    palette: &theme::Palette,
    collections: &farhelm_helm_client::Collections,
) -> AnyElement {
    let part = |name: &str| SharedString::from(format!("{id}-{name}"));
    let selection = &entry.selection;
    let (permission, yolo) = permission_presentation(selection);
    let destination = HostText::from_host(destination_label(entry));
    let first = HostSentence::new(part("destination")).text("Recent setup: ");
    let registered = selection.custom_agent.is_some();
    let first = if registered {
        recent_agent_words(first, selection, query, collections).text(" · ")
    } else {
        first
    };
    let first = if entry.github_repo.is_none() {
        first.mono(&destination)
    } else {
        first.host(&destination)
    };
    let first = first.text(" · ").host(placement);
    let first = if registered {
        first
    } else {
        recent_agent_words(first.text(" · "), selection, query, collections)
    };
    let shell = selection.harness.is_shell();
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
                .when(shell, |line| {
                    line.child(
                        HostSentence::new(part("selection"))
                            .host(&HostText::from_host(summary_without_wrapper(
                                selection,
                                collections,
                            )))
                            .underline_matches(query),
                    )
                })
                .when(!shell, |line| {
                    // A custom model id is host-supplied text, so the line is drawn escaped.
                    line.child(
                        HostSentence::new(part("selection"))
                            .host(&HostText::from_host(summary_before_permissions(selection)))
                            .text(" · Permissions: ")
                            .underline_matches(query),
                    )
                    .child(
                        div()
                            .flex_none()
                            .when(yolo, |word| word.text_color(theme::color(palette.danger)))
                            .child(
                                HostSentence::new(part("permission"))
                                    .text(permission)
                                    .underline_matches(query),
                            ),
                    )
                })
                .when_some(wrapper_summary(selection), |line, wrapper| {
                    line.child(wrapper_tail(
                        HostSentence::new(part("wrapper"))
                            .text(" · ")
                            .host(&HostText::from_host(wrapper))
                            .underline_matches(query),
                    ))
                }),
        )
        .into_any_element()
}
