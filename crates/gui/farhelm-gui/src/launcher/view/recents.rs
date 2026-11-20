//! The launcher's recent setups strip: up to three of the chosen host's earlier launches that match the draft, between
//! the search box and the destination, each a row that fills the launcher with that setup. Which setups and in what
//! order is [`crate::launcher::recents`]'; this module draws them and takes the one the user picks.
//!
//! SPEC.md, "Creation" ("Recent setups ... A recent row fills every saved choice and destination; clicking it never
//! launches, and pressing Enter on a focused row launches the filled setup through the ordinary Launch path ... Recent-
//! setup rows appear only when the current filter actually matches history; the dialog reserves no space for them when
//! it does not"). Ported from the old launcher's `.launch-composer-recents` strip and `accept_recent`
//! (`list/create_form.rs` and `list/create_form/draft.rs` at reference commit `24bc0580`). Here's the TLDR:
//!
//! - **Shown only with rows.** No heading and no space while nothing matches, and never in Restart with, which keeps
//!   its setup. The rows read the draft as it stands, on either tab, so they narrow as the user chooses, and they show
//!   while a new environment is chosen as on a host.
//! - **A row** shows the agent type, where it started, where it ran (the chosen host, or a new environment's recipe and
//!   place, which needs no host connected: [`crate::launcher::recents::placement`]), and what it set explicitly
//!   ("Defaults" when it set nothing but the agent's default Guarded mode; YOLO in the danger ink), with "⏎ Launch" ("⏎
//!   Replace" in Replace with) at its end while it has keyboard focus. Its accessible name and hover help say the whole
//!   setup. The cells are fixed tracks that cut their text short, so the columns line up from row to row.
//! - **Focus follows the setup**, not the row's place: taking a row re-filters the rows, and a row keeps its keyboard
//!   focus only while it still shows the same setup, so a fill never leaves Enter on a setup the user did not move to.
//!   (A setup that leaves the strip takes its focus with it, and the focus coordinator recovers it as for any control
//!   that goes away.)
//! - **Taking a row** (a click, or Enter on a focused row) makes the setup's placement, choices and folder or
//!   repository the draft's through the draft's own methods
//!   ([`LaunchDraft::choose_placement`](crate::launcher::draft::LaunchDraft::choose_placement),
//!   [`LaunchDraft::restore_recent`](crate::launcher::draft::LaunchDraft::restore_recent)), on the Agent tab. A click
//!   stops there; Enter then presses the main action, so the setup launches, or says why not, exactly as a press of
//!   Launch would. A held Enter does nothing. A setup carrying a retired container choice is refused under the agent
//!   choices with the shared reason, and so is a new environment's in Replace with, which keeps the existing one; a row
//!   whose setup is no longer in the history (the host changed under it) does nothing.

use farhelm_proto::session_requests::LaunchHistoryEntry;
use farhelm_proto::{LaunchPermission, RETIRED_CONTAINER_REASON};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    px,
};

use super::{Launcher, host_name_with_phase};
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::focus;
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::{DestinationKind, Field, Mode, harness_label};
use crate::launcher::recents::{
    ALL_DEFAULTS, DraftDestination, RecentFilter, SetupKey, destination_label,
    explicit_before_permissions, permission_value, placement, selection_summary, setup_key, strip,
    uses_guarded_default,
};
use crate::layers;
use crate::theme::{self, metrics};

/// Why a new environment's setup is not taken in Replace with, the old launcher's words for a placement where the mode
/// keeps the source's (decided by the launcher area on 2026-11-16: the row is shown and refused, not left out).
pub(super) const KEEPS_ENVIRONMENT: &str = "Restart and Replace keep the existing environment";

/// `.launch-composer-recent-slots > button`'s tracks, in the strip's 13-pixel text: the agent's name (`9ch`), the folder
/// (at least `12ch`, taking what is left), the host (`22ch`, shrinking only when the row is narrow), the explicit choices
/// (`34ch`, likewise) and the hint (`9ch`), `2ch` apart, so the cells line up from row to row.
const HARNESS_TRACK: f32 = 72.;
const FOLDER_MIN: f32 = 96.;
const HOST_TRACK: f32 = 176.;
const SELECTION_TRACK: f32 = 272.;
const HINT_TRACK: f32 = 72.;
const TRACK_GAP: f32 = 16.;

/// The strip's rows' focus handles, each kept with the setup its row shows, so a row keeps keyboard focus across
/// redraws while it shows that setup and loses it when the setup leaves the strip (module docs). Only the drawn rows'
/// handles are kept.
#[derive(Default)]
pub(super) struct RecentsStrip {
    handles: Vec<(SetupKey, FocusHandle)>,
}

impl RecentsStrip {
    /// `key`'s handle: the one it had, or a new tab stop.
    fn handle_for(&mut self, key: SetupKey, cx: &mut App) -> FocusHandle {
        if let Some((_, handle)) = self.handles.iter().find(|(held, _)| *held == key) {
            return handle.clone();
        }
        let handle = cx.focus_handle().tab_stop(true);
        self.handles.push((key, handle.clone()));
        handle
    }

    /// Forget the handles of setups no longer drawn.
    fn keep_only(&mut self, drawn: &[SetupKey]) {
        self.handles.retain(|(key, _)| drawn.contains(key));
    }
}

/// Why a recent setup was not taken; the strip and the search each say it in their own place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecentRefusal {
    /// It carries a container choice: containers are retired, and the setup cannot be launched again as it was.
    Retired,
    /// It is no longer in the chosen host's history as read now: the host or its history changed after the row was
    /// drawn.
    Stale,
    /// This launcher holds the agent type and destination the setup would change (Restart with).
    Held,
    /// The setup made a new environment, and this launcher keeps the source's host (Replace with):
    /// [`KEEPS_ENVIRONMENT`].
    KeepsEnvironment,
}

impl Launcher {
    /// The draft's filter for the recent setups: its agent type, model and effort, and its permission only when the
    /// user chose it (`recents`' module docs).
    fn recent_filter(&self) -> RecentFilter {
        let agent = &self.draft.agent;
        RecentFilter {
            harness: agent.harness,
            model: agent.model.clone(),
            effort: agent.effort,
            permissions: agent.permissions.filter(|_| agent.permissions_explicit),
        }
    }

    /// The strip's rows now, each with where it ran as its row names it: none in Restart with, else up to three setups
    /// matching the draft.
    fn recent_rows(&self, cx: &App) -> Vec<(LaunchHistoryEntry, HostText)> {
        if matches!(self.draft.mode, Mode::RestartWith(_)) {
            return Vec::new();
        }
        let Some(history) = self.recent_history(cx) else {
            return Vec::new();
        };
        let destination = match &self.draft.checkout {
            None => DraftDestination::Folder(self.draft.folder.value()),
            Some(checkout) => match &checkout.repo {
                Some(repo) => DraftDestination::Repository(repo),
                None => DraftDestination::RepositoryInput,
            },
        };
        let host = self.chosen_host(cx).map(host_name_with_phase);
        let hosts = self.hosts(cx);
        strip(&history, &self.recent_filter(), destination)
            .into_iter()
            .filter_map(|entry| Some((entry.clone(), placement(entry, host.as_ref(), &hosts)?)))
            .collect()
    }

    /// Why `entry` cannot be taken now, if it cannot. The strip and the search say it in different places, so this only
    /// decides.
    pub(super) fn recent_refusal(
        &self,
        entry: &LaunchHistoryEntry,
        cx: &App,
    ) -> Option<RecentRefusal> {
        if entry.selection.container.is_some() {
            return Some(RecentRefusal::Retired);
        }
        // The row was drawn from the history as it was; a host change since has dropped that history, and with it the
        // row's claim to this host's folders.
        if !self.recent_offered(entry, cx) {
            return Some(RecentRefusal::Stale);
        }
        // Restart with holds the agent type and destination a setup would change; the strip is not drawn there, and
        // the search refuses the row before it gets here, so this only guards.
        if self.draft.holds(Field::AgentType) {
            return Some(RecentRefusal::Held);
        }
        (entry.target.is_some() && self.draft.holds(Field::Host))
            .then_some(RecentRefusal::KeepsEnvironment)
    }

    /// Take `entry`: fill the draft with its setup (see the module docs), or change nothing and say why
    /// ([`Self::recent_refusal`]). Enter on a strip row needs to know it filled before it presses the main action.
    pub(super) fn take_recent(
        &mut self,
        entry: &LaunchHistoryEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), RecentRefusal> {
        if let Some(refusal) = self.recent_refusal(entry, cx) {
            return Err(refusal);
        }
        // A recent setup carries its place (SPEC.md, "Creation": recipe and place together): a new environment's
        // restores that recipe and place, and any other goes back to Existing host on the host whose history it came
        // from, as the old launcher's did, rather than filling a host's folder into an environment that cannot use it.
        self.draft.choose_placement(entry.target.clone());
        self.draft
            .restore_recent(&entry.selection, self.catalog.as_deref());
        // A setup taken replaces the agent choices, so the escalation banner goes, as the old strip took it away: its
        // Undo would throw this setup away.
        self.follow_escalation(true, None, None, cx);
        match (&entry.github_repo, &entry.target) {
            (Some(repo), _) => self.draft.choose_repository(repo.clone()),
            // The placement just made the folder the new environment's home, which binds no installation; only a
            // managed checkout chosen before gives way to it.
            (None, Some(_)) => self.draft.choose_destination(DestinationKind::Folder),
            (None, None) => {
                self.take_history_folder(entry.cwd.clone(), cx);
                self.folder_picked();
            }
        }
        self.notice = None;
        self.sync_fields(window, cx);
        self.changed(cx);
        Ok(())
    }

    /// Take a strip row's `entry`: a retired container choice is said under the agent choices, where the old strip said
    /// it, and so is a new environment's in Replace with; a stale row does nothing. Returns whether it filled.
    fn take_strip_recent(
        &mut self,
        entry: &LaunchHistoryEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let words = match self.take_recent(entry, window, cx) {
            Ok(()) => return true,
            Err(RecentRefusal::Retired) => RETIRED_CONTAINER_REASON,
            Err(RecentRefusal::KeepsEnvironment) => KEEPS_ENVIRONMENT,
            Err(RecentRefusal::Stale | RecentRefusal::Held) => return false,
        };
        self.notice = Some(words.to_owned());
        cx.notify();
        false
    }

    /// The recent setups strip, or `None` while it has no rows (it then takes no room: SPEC.md).
    pub(super) fn recents_strip(
        &mut self,
        palette: &theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let rows = self.recent_rows(cx);
        if rows.is_empty() {
            return None;
        }
        let replacing = matches!(self.draft.mode, Mode::ReplaceWith { .. });
        let verb = if replacing { "Replace" } else { "Launch" };
        let mut slots = div()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(metrics::RADIUS_MD)
            .border_1()
            .border_color(theme::color(palette.border))
            .bg(theme::color(palette.well))
            .child(layers::beside_edge(
                "launcher-recents-help-edge",
                window,
                cx,
            ));
        let drawn: Vec<SetupKey> = rows.iter().map(|(entry, _)| setup_key(entry)).collect();
        for (index, (entry, placement)) in rows.into_iter().enumerate() {
            let handle = self.recents.handle_for(drawn[index].clone(), cx);
            // The hint, like the ring, shows for keyboard focus only.
            let focused = focus::shows_ring(&handle, window, cx);
            let destination = HostText::from_host(destination_label(&entry));
            let summary = selection_summary(&entry.selection);
            // The whole setup, host values escaped: the row's hover help, and its accessible name after "Replace: " in
            // Replace with.
            let whole =
                HostSentence::new(SharedString::from(format!("launcher-recent-{index}-whole")))
                    .host(&destination)
                    .text(" · ")
                    .host(&placement)
                    .text(" · ")
                    // The choices are host text too: a custom model id is whatever the user typed.
                    .host(&HostText::from_host(summary));
            let name = format!(
                "{}{}",
                if replacing { "Replace: " } else { "" },
                whole.accessible_name()
            );
            let content = row_cells(
                index,
                &entry,
                &destination,
                &placement,
                focused,
                verb,
                palette,
            );
            let (clicked, entered) = (entry.clone(), entry);
            let (on_click, on_key) = (cx.weak_entity(), cx.weak_entity());
            let row = Button::new(
                format!("launcher-recent-{index}"),
                name,
                HoverHelp::sentence(whole).beside_list(false),
                Role::Button,
            )
            .tier(Tier::RecentSetup)
            .focus(&handle)
            .content(content)
            // Enter fills, then presses the main action; a held Enter does neither. Taken here, so the button's own
            // Enter activation (on the key's press) does not fill it a second time. Not the components' shared choice
            // Enter rule (`components::enter_choice`): its fill cannot refuse, and a refused fill (a retired container,
            // a row gone stale) must not press the main action.
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key != "enter" || event.keystroke.modifiers.modified() {
                    return;
                }
                cx.stop_propagation();
                if event.is_held {
                    return;
                }
                let _ = on_key.update(cx, |launcher, cx| {
                    if launcher.take_strip_recent(&entered, window, cx) {
                        launcher.press(window, cx);
                    }
                });
            })
            .on_click(move |_, window, cx| {
                let _ = on_click.update(cx, |launcher, cx| {
                    launcher.take_strip_recent(&clicked, window, cx);
                });
            });
            // `button + button { border-top: 1px solid var(--border-row) }`.
            let row = div()
                .when(index > 0, |row| {
                    row.border_t_1()
                        .border_color(theme::color(palette.border_row))
                })
                .child(row);
            slots = slots.child(row);
        }
        self.recents.keep_only(&drawn);
        Some(
            div()
                .id("launcher-recents")
                .debug_selector(|| "launcher-recents".into())
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(palette.fg_1))
                        .child("Recent setups"),
                )
                .child(slots)
                .into_any_element(),
        )
    }
}

/// Row `index`'s cells (`.launch-composer-recent-*`): the agent's name in the primary ink and bold; the folder
/// (monospace, unless a repository) and the host; the explicit choices with the permission (YOLO in the danger ink), or
/// "Defaults"; and the hint while the row has keyboard focus. Every cell is a fixed track that cuts its text short with
/// an ellipsis rather than wrapping or widening the row. The agent and choices cells are labelled for the test bridge
/// and assistive technology's browsing of the row's parts; the row's own name says the whole setup.
fn row_cells(
    index: usize,
    entry: &LaunchHistoryEntry,
    destination: &HostText,
    placement: &HostText,
    focused: bool,
    verb: &str,
    palette: &theme::Palette,
) -> AnyElement {
    let id = |part: &str| SharedString::from(format!("launcher-recent-{index}-{part}"));
    let selection = &entry.selection;
    let explicit = explicit_before_permissions(selection).join(" · ");
    let show_permission = !selection.harness.is_shell() && !uses_guarded_default(selection);
    let yolo =
        selection.harness.request_permission(selection.permissions) == Some(LaunchPermission::Yolo);
    // The choices cell's words, for its label; drawn below in parts, so YOLO can take the danger ink.
    let mut words = String::new();
    let mut choices = div().flex().min_w_0().truncate();
    if explicit.is_empty() && !show_permission {
        words.push_str(ALL_DEFAULTS);
        choices = choices.child(ALL_DEFAULTS);
    } else if !explicit.is_empty() {
        // The model is host-supplied text (a custom id the user typed), so it is drawn escaped.
        let sentence =
            HostSentence::new(id("explicit")).host(&HostText::from_host(explicit.as_str()));
        words.push_str(&sentence.accessible_name());
        choices = choices.child(sentence);
    }
    if show_permission {
        let lead = if explicit.is_empty() {
            "Permissions: "
        } else {
            " · Permissions: "
        };
        words.push_str(lead);
        words.push_str(permission_value(selection));
        choices = choices.child(lead).child(
            div()
                .when(yolo, |word| word.text_color(theme::color(palette.danger)))
                .child(permission_value(selection)),
        );
    }
    let mono = entry.github_repo.is_none();
    div()
        .flex()
        .items_center()
        .w_full()
        .min_w_0()
        .gap(px(TRACK_GAP))
        .child(
            div()
                .id(id("harness"))
                .role(Role::Label)
                .aria_label(harness_label(selection.harness))
                .w(px(HARNESS_TRACK))
                .flex_none()
                .truncate()
                .text_color(theme::color(palette.fg_0))
                .font_weight(FontWeight::BOLD)
                .child(harness_label(selection.harness)),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(FOLDER_MIN))
                .truncate()
                .when(mono, |cell| cell.font_family(crate::assets::TERMINAL_FONT))
                .child(HostSentence::new(id("folder")).host(destination)),
        )
        .child(
            div()
                .w(px(HOST_TRACK))
                .min_w_0()
                .truncate()
                .child(HostSentence::new(id("host")).host(placement)),
        )
        .child(
            div()
                .id(id("choices"))
                .role(Role::Label)
                .aria_label(SharedString::from(words))
                .w(px(SELECTION_TRACK))
                .min_w_0()
                .text_size(metrics::TEXT_SM)
                .child(choices),
        )
        .child(
            div()
                .id(id("hint"))
                .w(px(HINT_TRACK))
                .flex_none()
                .text_size(metrics::TEXT_SM)
                // Named only while it shows: a hidden hint read out on every row would tell a screen reader user about
                // a key on rows it does not apply to (the old hint was hidden from assistive technology too).
                .when(focused, |hint| {
                    hint.role(Role::Label)
                        .aria_label(SharedString::from(format!("⏎ {verb}")))
                })
                .when(!focused, |hint| hint.invisible())
                .child(format!("⏎ {verb}")),
        )
        .into_any_element()
}
