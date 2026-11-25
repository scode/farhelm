//! The launch history the launcher offers again: the folders (the folder list, `folder_list`) and the whole setups (the
//! recent setups strip, `recents`) that launched successfully before on the chosen host's current installation
//! (`GET /api/launch-history?host=<id>`).
//!
//! SPEC.md, "Recent setups" and "Working directory"; SPEC_impl.md, "Composer history". Ported from the old launcher's
//! `request_launch_history` (`list/create_form.rs` at reference commit `24bc0580`). Here's the TLDR:
//!
//! - **Per host connection.** The history is read for the chosen host on the connection the launcher sees it on
//!   ([`HistoryKey`]): when the launcher opens, and again whenever the host or its connection changes. The helm answers
//!   only for a connected host, from what its live connection claims, so a registry row retargeted onto another
//!   installation never offers the replaced one's folders. When the same verified installation suspends during an
//!   opening, its already-read managed-checkout setups can still fill the command pane. The new connection supplies
//!   their exact preview. Folder history, defaults and search still require a current connection.
//! - **Latest wins.** An answer is kept only while the launcher still names the key it was asked about; a key change
//!   drops the old answer at once, so the list never shows a moment of another host's folders, and a late answer for an
//!   earlier key never replaces a newer one. Reads are not held one at a time, as the folder check's are: the helm
//!   answers this one from its own store, never asking the host, so overlapping reads cost nothing worth queuing for.
//! - **Read when asked, not followed.** The old browser app also re-read on every activity-feed notice and on a timer;
//!   the desktop app has no feed; while the launcher is open its history changes only by an agent's or the command
//!   line's launch, which it does not follow, so it is read on the changes above only. A failed read offers no history:
//!   the folder list says it has no recent folders, which is what it says for a host with none.
//! - **New's folder default** (`LaunchDraft::folder_default`): New without the open session's folder starts in `~` and
//!   waits for the first answer for its host, which seeds the folder the newest plain-folder launch started in
//!   ([`remembered_folder`]); a failed read settles it on `~`. It is settled while drawing, and from the answer itself
//!   for a quick launch prepared out of sight, which is never drawn. A re-read (a reconnect) never moves it again.
//! - **The checkout configuration's revision** comes with every answer and goes to the managed-checkout destination
//!   ([`Launcher::observe_checkout_config_revision`]), which asks for a new preview when it rises.
//! - **Launches into new environments** are the helm's own history, not a host's (`GET
//!   /api/environment-launch-history`): helm-wide, kept after the disposable host is gone, and answered with no host
//!   connected. The launcher reads it once, when it is first drawn, and merges it into the host's for the recent
//!   setups alone ([`Launcher::recent_history`]); the folder list, New's folder default and the saved folders read the
//!   host's history only, so an environment's home never becomes a host's folder. A failed read offers none of them.
//!   The old launcher also re-read it on every activity-feed notice, which the desktop app has not (the same deviation
//!   as above).
//! - **A staged run's hosts** (agent builds with a pins file only): the answer's launches and folders are replaced by
//!   the scenario's as it arrives, before anything here reads it ([`crate::agent_startup::staged_launch_history`]), so
//!   every view of the history, the folder default included, sees only the staged one.

use farhelm_helm_client::ApiPath;
use farhelm_proto::session_requests::{LaunchHistoryEntry, LaunchHistoryReply};
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::HostStateView;
use gpui::{App, Context};

use super::super::draft::{FolderDefault, Install};
use super::super::entry::session_folder;
use super::super::recents::{merged, remembered_folder};
use super::Launcher;
use super::recents::RecentSurface;
use crate::{local_state, operations};

/// What one history read is about: the host, and the connection the launcher saw it on.
#[derive(Clone, Debug, PartialEq, Eq)]
struct HistoryKey {
    install: Install,
    incarnation: u64,
    /// The connected identity agreed with the recorded identity when this history was asked for.
    verified: bool,
}

/// A setup change revokes the preview immediately. The matching history answer supplies the new revision, so
/// a preview can follow the change once even when the checkout remained on the same connection.
#[derive(Default, PartialEq, Eq)]
enum RootRefresh {
    #[default]
    Idle,
    Requested,
    Reading,
}

/// The launcher's history read: what it last asked about, and its answer while still current.
#[derive(Default)]
pub(super) struct LaunchHistory {
    /// The key the last read sent is about.
    asked: Option<HistoryKey>,
    /// The last answer, with the key it answers for.
    answered: Option<(HistoryKey, LaunchHistoryReply)>,
    /// The key whose read failed, while it is still the one asked about: New's folder default then settles on `~`.
    failed: Option<HistoryKey>,
    /// A checkout-folder save asks for a new answer on the same connection. Keep the prior answer visible so
    /// the setup field stays mounted and focused until the new configuration is known.
    root_refresh: RootRefresh,
    /// Distinguish two sends for the same connection, including a save during an older pending read.
    generation: u64,
    /// Whether the helm's history of launches into new environments has been asked for: once per launcher.
    environment_asked: bool,
    /// That history's answer, newest first; `None` until it arrives, and after a failed read.
    environment: Option<Vec<LaunchHistoryEntry>>,
}

impl Launcher {
    /// What the history read is about now: the chosen host on its current connection, or `None` when there is no host
    /// or it is not connected (the helm would refuse the read).
    fn history_key(&self, cx: &App) -> Option<HistoryKey> {
        let view = self.chosen_host(cx)?;
        let HostStateView::Connected { identity, .. } = &view.state else {
            return None;
        };
        Some(HistoryKey {
            install: Install::of(view),
            incarnation: view.incarnation,
            verified: identity == &view.identity
                && identity.as_deref().is_some_and(|id| !id.is_empty()),
        })
    }

    /// Follow the chosen host as drawn: read its history when it, or its connection, changed since the last read
    /// (see the module docs). Called while drawing, since a connection changes by a push with no edit of the
    /// launcher's.
    pub(super) fn follow_history(&mut self, cx: &mut Context<Self>) {
        let key = self.history_key(cx);
        // Several disconnected hosts have the same absent read key. Test installation ownership before
        // the early return so switching between them cannot keep a different installation's answer.
        if key.is_none() && !self.retains_checkout_setups(cx) {
            self.history.answered = None;
        }
        let same_host = key == self.history.asked;
        if same_host && (self.history.root_refresh != RootRefresh::Requested || key.is_none()) {
            return;
        }
        self.history.asked = key.clone();
        if self.checkout_root_refresh_pending() {
            // Setup changed shared configuration. A host switch during the read still needs the new revision
            // before a preview, but only the new host's answer can resolve its own setup field.
            self.history.root_refresh = if key.is_some() {
                RootRefresh::Reading
            } else {
                RootRefresh::Requested
            };
        }
        if key.is_some() && !same_host {
            self.history.answered = None;
        }
        self.history.failed = None;
        // The folder list has an index into this history. The command field keeps the earlier installation on its
        // explicit choice so the next Enter can explain why that choice disappeared.
        if !same_host {
            self.drop_folder_highlight();
        }
        if let Some(key) = key {
            self.send_history_read(key, cx);
        }
    }

    /// Invalidate a pending response before the draw that asks again. A read issued before a save cannot
    /// remove the setup field or claim the pending handoff to Repository.
    pub(super) fn refresh_checkout_root(&mut self) {
        self.history.generation += 1;
        self.history.root_refresh = RootRefresh::Requested;
    }

    /// A setup update refreshes the current destination's revision before any preview uses the changed folder.
    pub(super) fn checkout_root_refresh_pending(&self) -> bool {
        self.history.root_refresh != RootRefresh::Idle
    }

    /// A wake continues with a fresh connected preview while the launcher is hidden. It does not draw to
    /// read history again. Retire an unfinished setup read so it cannot either block or replace that preview.
    pub(super) fn retire_checkout_root_refresh(&mut self) {
        if self.checkout_root_refresh_pending() {
            self.history.root_refresh = RootRefresh::Idle;
            self.history.generation += 1;
            self.history.asked = None;
        }
    }

    /// Read `key`'s history.
    fn send_history_read(&mut self, key: HistoryKey, cx: &mut Context<Self>) {
        let host = key.install.host.to_string();
        let Ok(path) =
            ApiPath::api(&["launch-history"]).and_then(|path| path.with_query(&[("host", &host)]))
        else {
            self.history.root_refresh = RootRefresh::Idle;
            return;
        };
        self.history.generation += 1;
        let generation = self.history.generation;
        let launcher = cx.weak_entity();
        if !operations::read::<LaunchHistoryReply>(
            path,
            "GET /api/launch-history",
            cx,
            move |answer, cx| {
                let _ = launcher.update(cx, |launcher, cx| {
                    // An answer for a key the launcher has moved past is dropped here, so a late one never replaces
                    // the answer for the key asked since.
                    if launcher.history.generation != generation
                        || launcher.history.asked.as_ref() != Some(&key)
                        || launcher.history_key(cx).as_ref() != Some(&key)
                    {
                        return;
                    }
                    launcher.history.root_refresh = RootRefresh::Idle;
                    match answer {
                        Ok(reply) => {
                            // A staged run's host shows the scenario's history in place of its real one, whose
                            // folders are the run's scratch directories (`agent_startup`, "Staged host values").
                            let reply = crate::agent_startup::staged_launch_history(
                                reply,
                                launcher.chosen_host(cx),
                                cx,
                            );
                            launcher.observe_checkout_config_revision(
                                reply.checkout_config_revision,
                                cx,
                            );
                            launcher.checkout_root_read(reply.needs_checkout_root);
                            launcher.history.answered = Some((key, reply));
                            // The folder list uses an index. The command field resolves a deliberate pick by identity.
                            launcher.drop_folder_highlight();
                        }
                        Err(_) => launcher.history.failed = Some(key),
                    }
                    // A drawn launcher settles its folder default when it draws; one prepared out of sight is never
                    // drawn, so a quick launch waiting for the default goes on from here.
                    launcher.history_settled_for_quick_launch(cx);
                    cx.notify();
                });
            },
        ) {
            self.history.root_refresh = RootRefresh::Idle;
        }
    }

    /// Read the helm's history of launches into new environments, the first time the launcher is drawn (module docs).
    /// A launcher prepared out of sight for a quick launch is never drawn, and offers no recent setups, so it never
    /// asks.
    pub(super) fn follow_environment_history(&mut self, cx: &mut Context<Self>) {
        if self.history.environment_asked {
            return;
        }
        self.history.environment_asked = true;
        let Ok(path) = ApiPath::api(&["environment-launch-history"]) else {
            return;
        };
        let launcher = cx.weak_entity();
        operations::read::<Vec<LaunchHistoryEntry>>(
            path,
            "GET /api/environment-launch-history",
            cx,
            move |answer, cx| {
                let _ = launcher.update(cx, |launcher, cx| {
                    if let Ok(entries) = answer {
                        launcher.history.environment = Some(entries);
                        cx.notify();
                    }
                });
            },
        );
    }

    /// The history recent setups are drawn from, in the empty pane and the search: the chosen host's while it is current,
    /// with the launches into new environments merged in ([`merged`]); `None` with neither.
    pub(super) fn recent_history(&self, cx: &App) -> Option<LaunchHistoryReply> {
        merged(
            self.launch_history(cx),
            self.history.environment.as_deref().unwrap_or_default(),
        )
    }

    /// A past model with this exact spelling and harness remains available from the helm after a host change.
    /// The search already coalesces the same model from both histories into one row.
    pub(super) fn has_environment_model(
        &self,
        harness: farhelm_proto::LaunchHarness,
        id: &str,
    ) -> bool {
        self.history.environment.as_ref().is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry.selection.harness == harness && entry.selection.model.as_deref() == Some(id)
            })
        })
    }

    /// Carry a verified answer through Suspend so the same installation's repository setup can still be chosen.
    /// Both the registry identity and the connection that answered the original read must be known.
    fn retains_checkout_setups(&self, cx: &App) -> bool {
        let Some(((key, _), host)) = self.history.answered.as_ref().zip(self.chosen_host(cx))
        else {
            return false;
        };
        key.verified
            && key.install == Install::of(host)
            && (self.draft.can_wake(host)
                || matches!(
                    host.state,
                    HostStateView::TensorlakeChanging {
                        direction: LifecycleDirection::Suspending,
                        ..
                    }
                ))
    }

    /// Preserve only repository choices in the command pane while the same installed host can wake.
    /// A retained reply is never a source of working directories or preview authority.
    fn suspended_checkout_setups(&self, cx: &App) -> Option<&LaunchHistoryReply> {
        let host = self.chosen_host(cx)?;
        (self.draft.can_wake(host) && self.retains_checkout_setups(cx))
            .then(|| self.history.answered.as_ref().map(|(_, reply)| reply))
            .flatten()
    }

    /// The empty command pane can take a retained setup during suspension. Search and folder controls continue
    /// to use the connected history through `recent_history`.
    pub(super) fn command_pane_history(&self, cx: &App) -> Option<LaunchHistoryReply> {
        let Some(history) = self.suspended_checkout_setups(cx) else {
            return self.recent_history(cx);
        };
        let mut reply = merged(
            Some(history),
            self.history.environment.as_deref().unwrap_or_default(),
        )?;
        reply
            .launches
            .retain(|entry| entry.target.is_some() || entry.github_repo.is_some());
        reply.folders.clear();
        Some(reply)
    }

    /// Whether `entry` is still among the recent setups' history as it stands, so a row drawn from an earlier one (the
    /// host changed under it) is not taken.
    pub(super) fn recent_offered(
        &self,
        entry: &LaunchHistoryEntry,
        surface: RecentSurface,
        cx: &App,
    ) -> bool {
        match &entry.target {
            Some(_) => self
                .history
                .environment
                .as_ref()
                .is_some_and(|entries| entries.contains(entry)),
            None => {
                self.launch_history(cx)
                    .is_some_and(|history| history.launches.contains(entry))
                    || (matches!(surface, RecentSurface::CommandPane)
                        && entry.github_repo.is_some()
                        && self
                            .suspended_checkout_setups(cx)
                            .is_some_and(|history| history.launches.contains(entry)))
            }
        }
    }

    /// Settle New's pending folder default when the chosen host's default is known (`LaunchDraft::folder_default`): the
    /// selected session's folder when it is on that host's installation, at once; else the folder the host's history
    /// names ([`remembered_folder`]); else `~` when the read failed. Returns whether it settled now, so the caller
    /// shows the seeded folder in its field. Called while drawing, after the history is followed, so it also covers a
    /// host change and a permission Undo that put back a pending draft.
    pub(super) fn settle_folder_default(&mut self, cx: &App) -> bool {
        // The registry's rows by value, so the draft can be changed while they are read; a launcher sees a handful.
        let hosts = self.hosts(cx);
        // A default decided for an installation the host row no longer names is decided again for the new one.
        self.draft.redecide_moved_default(&hosts);
        if self.draft.folder_default != FolderDefault::Pending {
            return false;
        }
        let Some(view) = self.chosen_host(cx) else {
            return false;
        };
        let host = view.id;
        let install = Install::of(view);
        // A hidden wake already accepted what the selection meant at its press. Later navigation can move focus,
        // but cannot change the launch's working directory.
        let selected = (!super::super::launch::waking(cx))
            .then(|| local_state::selected(cx))
            .flatten();
        let collections = self.store.read(cx).collections();
        let folder = if let Some(folder) = session_folder(collections, selected.as_deref(), host) {
            Some(folder)
        } else if let Some(history) = self.launch_history(cx) {
            Some(remembered_folder(history))
        } else if self.history.failed.is_some() && self.history.failed == self.history_key(cx) {
            None
        } else {
            return false;
        };
        self.draft.settle_folder_default(folder, install);
        true
    }

    /// Make `folder`, taken from the chosen host's history, the folder, bound to the installation that history is
    /// about ([`LaunchDraft::take_history_folder`](crate::launcher::draft::LaunchDraft::take_history_folder)). The
    /// history is only ever the current installation's, so that is the chosen host's row as it stands.
    pub(super) fn take_history_folder(&mut self, folder: String, cx: &App) {
        let install = self.chosen_host(cx).map(Install::of);
        match install {
            Some(install) => self.draft.take_history_folder(folder, install),
            None => self.draft.edit_folder(folder),
        }
    }

    /// Whether the host's current connection can supply a history read. An existing-folder wake checks again once
    /// the host connects, if the original press still needs a folder default.
    pub(super) fn history_wanted(&self, cx: &App) -> bool {
        self.history_key(cx).is_some()
    }

    /// The chosen host's history, while the last answer is about it on its current connection.
    pub(super) fn launch_history(&self, cx: &App) -> Option<&LaunchHistoryReply> {
        let key = self.history_key(cx)?;
        self.history
            .answered
            .as_ref()
            .filter(|(answered, _)| answered == &key)
            .map(|(_, reply)| reply)
    }
}
