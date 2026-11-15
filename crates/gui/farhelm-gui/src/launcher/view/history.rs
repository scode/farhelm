//! The launch history the launcher offers again: the folders, and later the whole setups, that launched successfully
//! before on the chosen host's current installation (`GET /api/launch-history?host=<id>`).
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
//!   the desktop app has no feed, and an open launcher's history changes only by another window's launch, so it is read
//!   on the changes above only. A failed read offers no history: the folder list says it has no recent folders, which
//!   is what it says for a host with none.
//! - **The checkout configuration's revision** comes with every answer and goes to the managed-checkout destination
//!   ([`Launcher::observe_checkout_config_revision`]), which asks for a new preview when it rises.

use farhelm_helm_client::ApiPath;
use farhelm_proto::session_requests::LaunchHistoryReply;
use farhelm_proto::ui_channel::{HostStateView, HostView};
use gpui::{App, Context};

use super::Launcher;
use crate::operations;

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
        // The folder list's rows follow the history, so a highlight on the old rows goes with it.
        self.drop_folder_highlight();
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
                    if launcher.history.asked == Some(key)
                        && let Ok(reply) = answer
                    {
                        launcher
                            .observe_checkout_config_revision(reply.checkout_config_revision, cx);
                        launcher.history.answered = Some((key, reply));
                        // The new recents shift the folder list's rows under any highlight.
                        launcher.drop_folder_highlight();
                        cx.notify();
                    }
                });
            },
        );
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
