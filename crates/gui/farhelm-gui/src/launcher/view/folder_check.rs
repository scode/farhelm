//! The folder check: whether the launcher's folder exists as a directory on its host, asked shortly after the host or
//! the path stops changing, and said under the folder field when it does not; and the same marking for a launch the
//! helm refused because of its folder.
//!
//! SPEC.md, "Creation" ("Folder check"): "in plain-folder mode on a connected host, the launcher checks the folder
//! 150 ms after the host or path stops changing. A missing folder or a path that is not a directory marks the field
//! and says so underneath, naming the host. The warning is advisory and never disables Launch ... Permission errors, timeouts,
//! busy hosts and other inconclusive answers show no warning. An answer for an earlier host, connection or path is
//! discarded"; and "a folder refusal also marks the folder field naming the host". Ported from the old launcher's
//! `FolderProblem`, `FolderRefusal` and `BrowseAuthority` (`list/create_form.rs` at reference commit `24bc0580`).
//! Here's the TLDR:
//!
//! - **Asked by a deadline.** Each frame works out what the folder is now on which connection ([`CheckKey`]); when that
//!   differs from what was last asked about, a ticker deadline 150 ms out replaces the earlier one (dropping it cancels
//!   it), so typing asks once, after the last key. The check is `POST /api/browse-directory` with `membership_only`,
//!   which asks only whether the path is a usable directory.
//! - **Latest wins, one at a time** (`latest`). The answer is kept only while the launcher still names the key it was
//!   asked about; an answer for an earlier host, connection or path changes nothing, and any change forgets the earlier
//!   answer, so going back to a path asks again (the user may have just created the folder). Checks are serial, as the
//!   old launcher's were: a deadline that comes due while a check is out does not start an overlapping one; the next
//!   check goes out when that answer arrives, so a slow host never collects a read per keystroke pause.
//! - **Words, not codes.** The supervisor refuses a folder in fixed words, which the helm passes on (sometimes inside a
//!   sentence of its own); [`FolderProblem::recognize`] finds them. Anything else, a timeout or a busy host included,
//!   is inconclusive and shows nothing. So is "not usable" from the check: the supervisor says that for a permission
//!   error, which SPEC.md lists as inconclusive, as the old check did; a launch refused that way still marks the
//!   field. Matching words is acceptable here, unlike for consent, because the marking only
//!   points at the field and authorizes nothing; if the words ever change, the launcher falls back to showing no mark.
//! - **Advisory.** Nothing here refuses Launch; the supervisor still validates the folder at launch time, and a launch
//!   it refuses for the folder is marked the same way (from the refusal's words) while the draft still names it, which
//!   [`Launcher::changed`](super::Launcher) ensures by dropping the refusal on any edit.
//!
//! Only a folder destination is checked: a managed checkout's folder does not exist until the launch makes it, and the
//! helm's preview names it instead (`checkout`).

use std::time::Duration;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::session_requests::{BrowseDirectoryBody, BrowseDirectoryReply};
use farhelm_proto::ui_channel::HostStateView;
use gpui::{AnyElement, Context, IntoElement, ParentElement as _, Styled as _, div, px};

use super::latest::Latest;
use super::{LABEL_COLUMN, LABEL_GAP, Launcher};
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::display_name;
use crate::operations;
use crate::theme::{self, metrics};
use crate::ticker;

/// How long the folder must stay unchanged before it is checked: SPEC.md's 150 ms.
const CHECK_AFTER: Duration = Duration::from_millis(150);

/// What a host's words say is wrong with a folder, when they say so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FolderProblem {
    Missing,
    NotADirectory,
    Unusable,
}

impl FolderProblem {
    /// Match the first supervisor refusal phrase in `message`. The helm can wrap that sentence, and the supervisor
    /// appends the path after its reason, so a later match can be part of the folder name.
    pub(super) fn recognize(message: &str) -> Option<FolderProblem> {
        [
            ("working directory does not exist: ", FolderProblem::Missing),
            (
                "working directory is not a directory: ",
                FolderProblem::NotADirectory,
            ),
            ("working directory is not usable: ", FolderProblem::Unusable),
        ]
        .into_iter()
        .filter_map(|(words, problem)| message.find(words).map(|offset| (offset, problem)))
        .min_by_key(|(offset, _)| *offset)
        .map(|(_, problem)| problem)
    }

    /// The line's words before the host's name, the old launcher's.
    fn lead(self) -> &'static str {
        match self {
            FolderProblem::Missing => "⚠ This folder does not exist on ",
            FolderProblem::NotADirectory => "⚠ This path is not a folder on ",
            FolderProblem::Unusable => "⚠ This folder cannot be used on ",
        }
    }
}

/// What one check is about: the host, the connection the launcher saw it on, and the folder as typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CheckKey {
    host: i64,
    incarnation: u64,
    cwd: String,
}

/// The launcher's folder check: what it last asked about, the deadline that will ask, and the last conclusive answer:
/// the path a usable folder resolves to, or the problem the host confirmed.
pub(super) type FolderCheck = Latest<CheckKey, Result<String, FolderProblem>>;

impl Launcher {
    /// What the folder check is about now: the chosen host on its current connection and the folder as it stands, or
    /// `None` when there is nothing to check (no host, a host that is not connected, an empty folder).
    fn check_key(&self, cx: &gpui::App) -> Option<CheckKey> {
        // A managed checkout's folder does not exist yet, and the helm previews it instead (`checkout`); nor does a new
        // environment's home, which is fixed.
        if self.draft.checkout.is_some() || self.draft.target.is_some() {
            return None;
        }
        // The folder exactly as the launch would send it, so the check and the launch ask about the same path.
        let cwd = self.draft.folder.value();
        if cwd.trim().is_empty() {
            return None;
        }
        let view = self.chosen_host(cx)?;
        if !matches!(view.state, HostStateView::Connected { .. }) {
            return None;
        }
        Some(CheckKey {
            host: view.id,
            incarnation: view.incarnation,
            cwd: cwd.clone(),
        })
    }

    /// Follow the folder and host as drawn: when what would be checked differs from what was last asked about, ask
    /// again 150 ms from now (see the module docs). Called while drawing, since a host's connection changes by a push
    /// with no edit of the launcher's.
    pub(super) fn follow_folder_check(&mut self, cx: &mut Context<Self>) {
        let key = self.check_key(cx);
        let Some(key) = self.folder_check.follow(key) else {
            return;
        };
        let launcher = cx.weak_entity();
        let deadline = ticker::after(CHECK_AFTER, cx, move |cx| {
            let _ = launcher.update(cx, |launcher, cx| launcher.send_folder_check(key, cx));
        });
        self.folder_check.wait(deadline);
    }

    /// Ask the host about `key`'s folder, if the launcher still names it and no check is out (then the answer sends
    /// it; see the module docs).
    fn send_folder_check(&mut self, key: CheckKey, cx: &mut Context<Self>) {
        let Ok(path) = ApiPath::api(&["browse-directory"]) else {
            return;
        };
        let body = BrowseDirectoryBody {
            host: key.host,
            cwd: key.cwd.clone(),
            expected_incarnation: Some(key.incarnation),
            membership_only: true,
        };
        let Ok(body) = serde_json::to_value(&body) else {
            return;
        };
        if !self.folder_check.begin(&key) {
            return;
        }
        let launcher = cx.weak_entity();
        let sent = operations::send(
            Method::Post,
            path,
            Some(body),
            "POST /api/browse-directory",
            cx,
            move |answer, cx| {
                let found = match answer {
                    Ok(body) => body
                        .and_then(|body| serde_json::from_value::<BrowseDirectoryReply>(body).ok())
                        .map(|listing| Ok(listing.cwd)),
                    // Only the supervisor's own folder words are conclusive, and of those not "not usable", which is
                    // how a permission error reads (see the module docs); anything else says nothing.
                    Err(words) => FolderProblem::recognize(words.raw())
                        .filter(|problem| *problem != FolderProblem::Unusable)
                        .map(Err),
                };
                let _ = launcher.update(cx, |launcher, cx| {
                    let finished = launcher.folder_check.finish(&key, found);
                    if finished.kept {
                        cx.notify();
                    }
                    // A deadline came due meanwhile (and no newer one is armed, which would have dropped it): check the
                    // folder as it is now.
                    if let Some(now) = finished.next {
                        launcher.send_folder_check(now, cx);
                    }
                });
            },
        );
        if !sent {
            self.folder_check.unsent();
        }
    }

    /// The latest conclusive observation for this path on the selected host's current connection.
    pub(super) fn current_folder_check(
        &self,
        cx: &gpui::App,
    ) -> Option<&Result<String, FolderProblem>> {
        self.folder_check.answer_for(&self.check_key(cx)?)
    }

    /// What is wrong with the folder as the launcher now names it, if anything is known to be: the last check's answer
    /// while it is about the current host, connection and folder, else a refused launch's words naming its folder.
    pub(super) fn folder_problem(&self, cx: &gpui::App) -> Option<FolderProblem> {
        let checked = self
            .current_folder_check(cx)
            .and_then(|answer| answer.as_ref().err().copied());
        checked.or_else(|| {
            self.refusal_words(cx)
                .and_then(|words| FolderProblem::recognize(words.raw()))
        })
    }

    /// The line under the folder field naming what is wrong and on which host (`.launch-composer-folder-problem`: 12
    /// pixels in `--warn-bright`, under the field), or `None` while nothing is known to be wrong.
    pub(super) fn folder_problem_line(
        &self,
        palette: &theme::Palette,
        cx: &gpui::App,
    ) -> Option<AnyElement> {
        let problem = self.folder_problem(cx)?;
        let host = self
            .chosen_host(cx)
            .map(display_name)
            .unwrap_or_else(|| HostText::from_host("this host"));
        let words = HostSentence::new("launcher-folder-problem")
            .text(problem.lead())
            .host(&host)
            .text(".");
        Some(
            div()
                .pl(px(LABEL_COLUMN + LABEL_GAP))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.warn_bright))
                .child(words)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::FolderProblem;

    /// The supervisor's folder refusals are recognized wherever the helm puts them, and nothing else is: a timeout or a
    /// busy host must never mark a folder that may well exist. Specified: each of the three phrasings, alone and inside
    /// a helm sentence, paths that contain a different refusal phrase, and two inconclusive answers.
    #[test]
    fn only_the_supervisors_folder_words_name_a_problem() {
        assert_eq!(
            FolderProblem::recognize("working directory does not exist: /nope"),
            Some(FolderProblem::Missing)
        );
        assert_eq!(
            FolderProblem::recognize(
                "create refused: working directory is not a directory: /etc/hosts"
            ),
            Some(FolderProblem::NotADirectory)
        );
        assert_eq!(
            FolderProblem::recognize("working directory is not usable: /root"),
            Some(FolderProblem::Unusable)
        );
        assert_eq!(
            FolderProblem::recognize(
                "create refused: working directory is not a directory: /project/working directory does not exist: notes"
            ),
            Some(FolderProblem::NotADirectory)
        );
        assert_eq!(
            FolderProblem::recognize(
                "working directory is not usable: /project/working directory is not a directory: draft (permission denied)"
            ),
            Some(FolderProblem::Unusable)
        );
        assert_eq!(FolderProblem::recognize("the host timed out"), None);
        assert_eq!(FolderProblem::recognize("host1 is busy"), None);
    }
}
