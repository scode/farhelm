//! Choose and restore the recent setups shown in the launcher's command pane and search results.
//!
//! The command pane asks for up to three matches against the current draft. Both lists restore a setup through the
//! draft's ordinary edit methods, including the recorded folder or environment. Taking a row rechecks the current host
//! and history, since a store update can retire the setup between drawing and taking it.

use farhelm_proto::session_requests::LaunchHistoryEntry;
use gpui::{App, Context, IntoElement, ParentElement as _, Styled as _, Window, div, px};

use super::{Launcher, host_name_with_phase, search::SetupSource};
use crate::components::HoverHelp;
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::{DestinationKind, Field};
use crate::launcher::recents::{DraftDestination, RecentFilter, placement, strip};

/// Why a new environment's setup is not taken in Replace with, the old launcher's words for a placement where the mode
/// keeps the source's (decided by the launcher area on 2026-11-16: the row is shown and refused, not left out).
pub(super) const KEEPS_ENVIRONMENT: &str = "Restart and Replace keep the existing environment";

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

/// Only the empty command pane can use a verified checkout setup retained across suspension.
#[derive(Clone, Copy)]
pub(super) enum RecentSurface {
    CommandPane,
    Search,
}

impl Launcher {
    /// The draft's filter for the recent setups: its agent type, model and effort, and its permission only when the
    /// user chose it (`recents`' module docs).
    fn recent_filter(&self) -> RecentFilter {
        let agent = &self.draft.agent;
        RecentFilter {
            harness: agent.harness,
            custom_agent: agent.custom_agent.clone(),
            model: agent.model.clone(),
            effort: agent.effort_value(),
            permissions: agent.permissions.filter(|_| agent.permissions_explicit),
        }
    }

    /// Up to three setups matching the draft, each named by the place where it ran. The command pane can still
    /// show those choices in Restart with, where it draws them disabled.
    pub(super) fn recent_rows(&self, cx: &App) -> Vec<(LaunchHistoryEntry, HostText)> {
        let Some(history) = self.command_pane_history(cx) else {
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

    /// Why `entry` cannot be taken now, if it cannot. Both lists use the same reason when disabling a row and when validating a pick.
    pub(super) fn recent_refusal(
        &self,
        entry: &LaunchHistoryEntry,
        surface: RecentSurface,
        cx: &App,
    ) -> Option<RecentRefusal> {
        if entry.selection.container.is_some() {
            return Some(RecentRefusal::Retired);
        }
        // The row was drawn from the history as it was; a host change since has dropped that history, and with it the
        // row's claim to this host's folders.
        if !self.recent_offered(entry, surface, cx) {
            return Some(RecentRefusal::Stale);
        }
        // Both lists show the reason before taking a held row and still check it again at dispatch.
        if self.draft.holds(Field::AgentType) {
            return Some(RecentRefusal::Held);
        }
        (entry.target.is_some() && self.draft.holds(Field::Host))
            .then_some(RecentRefusal::KeepsEnvironment)
    }

    /// Take `entry`: fill the draft with its setup (see the module docs), or change nothing and say why
    /// ([`Self::recent_refusal`]). Enter on a recent setup in the empty command pane presses Launch only after the setup fills successfully.
    pub(super) fn take_recent(
        &mut self,
        entry: &LaunchHistoryEntry,
        surface: RecentSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), RecentRefusal> {
        if let Some(refusal) = self.recent_refusal(entry, surface, cx) {
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
        self.search_box.source = Some(SetupSource::recent(entry, self.chosen_host(cx)));
        self.notice = None;
        self.sync_fields(window, cx);
        self.changed(cx);
        Ok(())
    }
}

/// Keep a short wrapper at the end of the line even when the earlier choices have to shrink. The accessible name
/// and hover help carry the complete text when this display reaches its cap.
pub(super) fn wrapper_tail(wrapper: HostSentence) -> impl IntoElement {
    div()
        .flex_none()
        .max_w(px(144.))
        .truncate()
        .text_ellipsis_middle()
        .child(wrapper)
}

/// Read why a row is disabled first, with the full setup beneath it. Wrapper details can run past the compact
/// tooltip's line limit even on an enabled row, so give those the same unclamped form.
pub(super) fn recent_hover(
    summary: HostSentence,
    has_wrapper: bool,
    refusal: Option<HostSentence>,
) -> HoverHelp {
    if let Some(reason) = refusal {
        HoverHelp::paragraphs([reason, summary])
    } else if has_wrapper {
        HoverHelp::paragraphs([summary])
    } else {
        HoverHelp::sentence(summary)
    }
}
