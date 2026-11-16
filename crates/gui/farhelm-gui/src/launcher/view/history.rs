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
//!   installation never offers the replaced one's folders.
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

use farhelm_helm_client::ApiPath;
use farhelm_proto::session_requests::LaunchHistoryReply;
use farhelm_proto::ui_channel::{HostStateView, HostView};
use gpui::{App, Context};

use super::super::draft::{FolderDefault, Install};
use super::super::entry::session_folder;
use super::super::recents::remembered_folder;
use super::Launcher;
use crate::{local_state, operations};

/// What one history read is about: the host, and the connection the launcher saw it on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HistoryKey {
    host: i64,
    incarnation: u64,
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
}

impl Launcher {
    /// What the history read is about now: the chosen host on its current connection, or `None` when there is no host
    /// or it is not connected (the helm would refuse the read).
    fn history_key(&self, cx: &App) -> Option<HistoryKey> {
        let host = self.draft.host?;
        let view = self
            .store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .into_iter()
            .find(|view| view.id == host)?;
        matches!(view.state, HostStateView::Connected { .. }).then_some(HistoryKey {
            host,
            incarnation: view.incarnation,
        })
    }

    /// Follow the chosen host as drawn: read its history when it, or its connection, changed since the last read
    /// (see the module docs). Called while drawing, since a connection changes by a push with no edit of the
    /// launcher's.
    pub(super) fn follow_history(&mut self, cx: &mut Context<Self>) {
        let key = self.history_key(cx);
        if key == self.history.asked {
            return;
        }
        self.history.asked = key;
        self.history.answered = None;
        self.history.failed = None;
        // The folder list's and the search's rows follow the history, so a highlight on the old rows goes with it.
        self.drop_folder_highlight();
        self.reset_search_highlight(cx);
        if let Some(key) = key {
            self.send_history_read(key, cx);
        }
    }

    /// Read `key`'s history.
    fn send_history_read(&mut self, key: HistoryKey, cx: &mut Context<Self>) {
        let host = key.host.to_string();
        let Ok(path) =
            ApiPath::api(&["launch-history"]).and_then(|path| path.with_query(&[("host", &host)]))
        else {
            return;
        };
        let launcher = cx.weak_entity();
        operations::read::<LaunchHistoryReply>(
            path,
            "GET /api/launch-history",
            cx,
            move |answer, cx| {
                let _ = launcher.update(cx, |launcher, cx| {
                    // An answer for a key the launcher has moved past is dropped here, so a late one never replaces
                    // the answer for the key asked since.
                    if launcher.history.asked != Some(key) {
                        return;
                    }
                    match answer {
                        Ok(reply) => {
                            launcher.observe_checkout_config_revision(
                                reply.checkout_config_revision,
                                cx,
                            );
                            launcher.history.answered = Some((key, reply));
                            // The new recents shift the folder list's and the search's rows under any highlight.
                            launcher.drop_folder_highlight();
                            launcher.reset_search_highlight(cx);
                        }
                        Err(_) => launcher.history.failed = Some(key),
                    }
                    // A drawn launcher settles its folder default when it draws; one prepared out of sight is never
                    // drawn, so a quick launch waiting for the default goes on from here.
                    launcher.history_settled_for_quick_launch(cx);
                    cx.notify();
                });
            },
        );
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
        let Some(view) = self
            .draft
            .host
            .and_then(|id| hosts.iter().find(|view| view.id == id))
        else {
            return false;
        };
        let host = view.id;
        let install = Install::of(view);
        let selected = local_state::selected(cx);
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
        let install = self.draft.host.and_then(|id| {
            self.hosts(cx)
                .iter()
                .find(|view| view.id == id)
                .map(Install::of)
        });
        match install {
            Some(install) => self.draft.take_history_folder(folder, install),
            None => self.draft.edit_folder(folder),
        }
    }

    /// Whether a history read for the chosen host is out, or can be asked for: a connected host. A host that is not
    /// connected has no history to wait for (its launch is refused for the host itself).
    pub(super) fn history_wanted(&self, cx: &App) -> bool {
        self.history_key(cx).is_some()
    }

    /// The chosen host's history, while the last answer is about it on its current connection.
    pub(super) fn launch_history(&self, cx: &App) -> Option<&LaunchHistoryReply> {
        let key = self.history_key(cx)?;
        self.history
            .answered
            .as_ref()
            .filter(|(answered, _)| *answered == key)
            .map(|(_, reply)| reply)
    }
}
