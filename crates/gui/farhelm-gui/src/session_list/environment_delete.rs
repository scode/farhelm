//! The Delete of a session in a disposable environment (a Tensorlake sandbox or a Podman container Farhelm made for
//! it): which of its two Deletes a click asks, what each question says and sends, and how the helm's answer settles.
//!
//! SPEC.md, "Sandbox sessions" and "Container sessions": only the Farhelm inside the environment knows which sessions it
//! holds, so no Delete decides from this window's list that a session is the last. Ported from the old app at the
//! reference commit `24bc0580`: `sandbox_sessions.rs` (`DeleteScope`, `delete_scope`), `api/sandbox_sessions.rs`
//! (`delete_in_environment` and its reply classes), the row's and the header's environment Delete questions in
//! `list/row.rs` and `session_view.rs`, and `list/view.rs` (`admit_delete`, `unasked_follow_up`, `delete_kept_text`).
//! The words are the provider's ([`crate::environment_ui::ProviderUi`]); nothing here names a provider. Here's the
//! TLDR of the rules:
//!
//! - **Two lanes** ([`Lane`]). The session lane asks the environment's own Farhelm to delete this session and, if no
//!   other session remains, to seal the environment, which the helm then destroys
//!   (`?delete_environment_session=true`); its question states both outcomes and is answered **Delete session**. The
//!   whole lane destroys the environment with everything in it, listed or not (`?delete_sandbox=true`), behind the
//!   warning that says so, answered **Delete session and sandbox** or **Delete session and container**. It is offered
//!   only when the helm cannot ask.
//! - **The lane is chosen at the click, from the hosts read** ([`lane`]): the whole lane only when a successful read
//!   lists the host as not connected (suspended, paused, unreachable, a version this helm cannot talk to). A failed
//!   read, which keeps the hosts it last listed, picks the session lane, so an uncertain read never widens the consent
//!   the user is asked for: the helm decides again when it sends, and answers "unasked" when it cannot ask. Never by
//!   the provider, and never by the suspended state as such. Before the first hosts read a session is not known to be
//!   an environment session at all (`super::environment::Environment::of` needs its host listed), so it gets the
//!   ordinary Delete, which the helm refuses for such a session; the old app also knew one only by its listed host
//!   (its `owned_host`).
//! - **The answer sends the lane it was asked in.** The lane is frozen into the question with everything else it
//!   shows ([`Asked`], inside `super::delete::Frozen`), so an answer given after the host reconnected or dropped still
//!   sends what the question said. A question asked in the session lane and answered after the host disconnected meets
//!   "unasked" (next rule); the reverse, a whole-lane answer becoming a session Delete, cannot happen, and neither can
//!   the whole lane without its own answer.
//! - **Always asked.** Neither question is ever skipped, whatever Skip session confirmations says and even for a
//!   session with nothing alive (SPEC.md: "Delete's confirmations are never skipped"). Each states its lane's
//!   consequence and then the environment's own name (" sandbox: <name>"), then the session's quoted title, and no
//!   managed-checkout line, since an environment's checkouts go with it or are discarded. The row menu's and the
//!   header's Delete help name the lane a click would ask in ([`help`]).
//! - **Sending.** One request on the session's own key, as [`Work::EnvironmentDelete`], so the row and the header show
//!   it as a Delete in flight and the session's other commands wait. It carries no guard: the helm refuses a guarded
//!   environment Delete, since the environment's Farhelm takes the session as the question described it. An answer
//!   given once the environment started being deleted sends nothing, as the old answer was disabled then.
//! - **Settling** ([`settle`]). A success records the Delete as this window's own (`super::delete::deleted_here`) and
//!   puts the result in the Delete notice band: a session-lane reply with a count says the environment stays and how
//!   many other sessions its Farhelm counted, and every other success retires an earlier count for the same host
//!   ([`super::delete_notice`]). A 404 counts as deleted: an earlier Delete, perhaps elsewhere, sealed and destroyed the
//!   environment. The helm's 504 at its 20-second bound is routine, not a failure: the Delete carries on and the pushed
//!   state shows how it ended, so its record is dropped. A session-lane answer carrying the helm's "unasked" marker
//!   (the helm could not confirm the outcome) asks the whole-lane question where the first one was asked,
//!   the row's menu button or the header's Delete, when the user has not moved on since answering (the operation
//!   store's [`Settled::user_moved_on`]), and only the helm's own marker does that, never its words. Any other refusal
//!   stays on the session's key, which the row and the header show as any Delete refusal.
//! - **No trash cue.** An environment Delete never archives a checkout, so it never plays the archive cue, and the
//!   whole lane's reply carries no notice to show (SPEC.md: whole-environment deletion "produces no archive cue or
//!   not-archived notice").
//!
//! The 20-second bound changed three things the old app did, as SPEC.md now says (decided by the GPUI migration's
//! orchestrator on 2026-11-16): a session-lane Delete still unanswered at the bound does not offer the whole lane at
//! once (the next click does, once the host reads not connected); a success after the bound shows no "stays" count;
//! and a whole-lane failure after the bound reaches only the helm's log. While the environment stays connected but its
//! Farhelm does not answer, Hosts' Delete sandbox is the way out.
//!
//! NOTE: An "unasked" answer that arrives after the user moved on opens nothing: the helm's words stay on the row, as a
//! refusal does, and the next click picks its lane from the hosts read again. Nothing remembers the answer, so if the
//! host still reads connected then (a read that lags the helm's own view), that click asks in the session lane once
//! more and meets "unasked" again. The old app's question could wait on a closed menu; a GPUI question cannot open
//! behind the user's back.
//!
//! The bulk Delete of marked sessions (`super::bulk`) sends these same two Deletes, frozen with its own question,
//! through [`start`]: one session-lane Delete per marked session of a connected environment, and one whole-lane Delete
//! per environment the helm cannot ask. It settles each the same way but never asks again: an "unasked" answer leaves
//! the helm's words on the row, as the old bulk Delete did.

use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Collections, Method, PathError};
use farhelm_proto::ui_channel::{HostStateView, SessionRow};
use gpui::{App, SharedString};

use super::delete::{self, Frozen};
use super::delete_notice;
use super::environment::{self, CARRIES_ON, Environment};
use super::status::HostLookup;
use crate::components::{HoverHelp, Tier};
use crate::environment_ui::ProviderUi;
use crate::focus::Mark;
use crate::host_text::HostSentence;
use crate::operations::{Command, HelmMarker, NotStarted, OpKey, Outcome, Settled, Work};
use crate::prompt::Answer;
use crate::session;

/// The status the helm answers a Delete of a session it does not know: counted as deleted (the module docs).
const GONE: u16 = 404;

/// Which of an environment session's two Deletes a question asks for (the module docs' first rule): the old
/// `DeleteScope`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lane {
    /// Ask the environment's own Farhelm to delete this session, and to seal the environment if it was the last.
    Session,
    /// Destroy the whole environment with everything in it, behind the warned question.
    Whole,
}

/// What an environment session's Delete question froze besides the session's key: its environment's host and provider,
/// the lane, and the environment's own name as the question showed it. The host and the provider travel with the
/// answer, so its result is worded for the right resource and tagged with the right host even after the registry
/// changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Asked {
    pub(crate) host: i64,
    pub(crate) ui: ProviderUi,
    pub(crate) lane: Lane,
    /// The environment's name, raw host text; empty when the helm has none, and then the question names no resource.
    pub(crate) name: String,
}

/// How a Delete asked somewhere asks the whole-lane question there again once the helm answers "unasked": handed the
/// frozen question to ask and the mark the prompt opens under, it says whether the question opened. The row's and the
/// header's ask their own way (`super::delete`'s `SessionList::delete`, `session_pane/header.rs`), so each hands its own
/// to the answer it confirms with ([`super::delete::confirming`]).
pub(crate) type AskAgain = Rc<dyn Fn(Frozen, Mark, &mut App) -> bool>;

// ---- Which lane, and what the questions say ---------------------------------------------------------------------

/// The lane a click on a session whose environment is `host` asks in (the module docs' second rule): the whole lane only
/// when a successful hosts read lists the host in a state other than connected. The pending and absent arms are
/// unreachable from production today, since [`asked`] knows an environment session only by its listed host; they pick
/// the session lane anyway, so that a caller that reaches them can never widen the consent.
pub(super) fn lane(collections: &Collections, host: i64) -> Lane {
    match HostLookup::of(collections, host) {
        HostLookup::Known(host) if !matches!(host.state, HostStateView::Connected { .. }) => {
            Lane::Whole
        }
        HostLookup::Known(_) | HostLookup::Pending | HostLookup::Failed | HostLookup::Absent => {
            Lane::Session
        }
    }
}

/// What a Delete of `row` would freeze now, or `None` for a session that is not an environment session (an ordinary
/// host, or a host the registry does not list), read from the current UI session's store.
pub(crate) fn asked(row: &SessionRow, cx: &App) -> Option<Asked> {
    asked_in(row, session::current_store(cx)?.read(cx).collections())
}

/// [`asked`] from `collections`, the store's as the caller already holds them: the bulk Delete freezes each marked
/// session's lane this way, from the hosts read that supplied the drawn bar (`super::bulk`).
pub(super) fn asked_in(row: &SessionRow, collections: &Collections) -> Option<Asked> {
    let environment = Environment::of(row, collections)?;
    Some(Asked {
        host: environment.host.id,
        ui: environment.ui,
        lane: lane(collections, environment.host.id),
        name: environment.name().to_owned(),
    })
}

/// The row menu's and the header's Delete help for a session in `environment`, while a click would ask in the lane
/// [`lane`] picks from `collections` now: what the question will offer.
pub(crate) fn help(environment: Environment, collections: &Collections) -> &'static str {
    match lane(collections, environment.host.id) {
        Lane::Session => environment.ui.session_delete_help(),
        Lane::Whole => environment.ui.whole_delete_help(),
    }
}

impl Asked {
    /// The question's answer: the lane's label and help, in the danger tier.
    pub(super) fn answer(&self) -> Answer {
        let (label, help) = match self.lane {
            Lane::Session => (
                self.ui.session_delete_label(),
                self.ui.session_delete_help(),
            ),
            Lane::Whole => (self.ui.whole_delete_label(), self.ui.whole_delete_help()),
        };
        Answer::new(label, HoverHelp::tooltip(help), Tier::Danger)
    }

    /// The lane's consequence in the provider's words: the session lane's both outcomes, or the whole lane's warning.
    /// The bulk question's row for this session says this alone, as the old bulk row did, with no resource name.
    pub(super) fn lane_consequence(&self) -> &'static str {
        match self.lane {
            Lane::Session => self.ui.session_delete_consequence(),
            Lane::Whole => self.ui.whole_delete_warning(),
        }
    }

    /// The question's first line, for the session filed under `key`: the lane's consequence, then the environment's own
    /// name when the helm gave one, as the old consequence ended with its `resource_label`.
    pub(super) fn consequence(&self, key: &str) -> HostSentence {
        let line = HostSentence::new(SharedString::from(format!(
            "session-delete-consequence-{key}"
        )))
        .text(self.lane_consequence());
        if self.name.is_empty() {
            line
        } else {
            self.ui.resource_label(line, &self.name)
        }
    }
}

// ---- Sending and settling ---------------------------------------------------------------------------------------

/// The command a Delete of `session` in `lane` sends: `DELETE /api/sessions/{id}` with the lane's own flag and no guard
/// (the module docs' "Sending").
pub(crate) fn command(session: &str, lane: Lane) -> Result<Command, PathError> {
    let flag = match lane {
        Lane::Session => "delete_environment_session",
        Lane::Whole => "delete_sandbox",
    };
    Ok(Command::new(
        Work::EnvironmentDelete,
        Method::Delete,
        ApiPath::api(&["sessions", session])?.with_query(&[(flag, "true")])?,
        None,
        format!("DELETE /api/sessions/[session]?{flag}=true"),
    ))
}

/// Send the environment Delete `frozen` holds through the operation store, on its session's key, and settle it
/// ([`settle`]) with `again` as the way to ask the whole lane if the helm answers "unasked". Nothing is sent for a
/// session with a request in flight or whose environment is being deleted now.
pub(super) fn send(frozen: Frozen, again: Option<AskAgain>, cx: &mut App) {
    if frozen.environment.is_none() {
        return;
    }
    if environment::deleting_reason(&frozen.key, cx).is_some() {
        return;
    }
    if let Err(refused) = start(frozen, again, |_, _| {}, cx) {
        tracing::debug!(?refused, "an environment Delete was not sent");
    }
}

/// [`send`] for a caller that has to know what happened, the bulk Delete (`super::bulk`): send `frozen`'s environment
/// Delete in the lane it froze, with no check of the environment's phase (the bulk Delete's admission refuses a session
/// being deleted through `super::menu::refused`), and say why nothing was sent. A `frozen` with no environment Delete
/// sends nothing and reads as [`NotStarted::Retired`], the refusal that shows nothing on a row; no caller passes one.
/// Once it has settled ([`settle`]), `then` runs, told whether the helm answered the 504 that says the Delete carries on
/// past its bound: the bulk Delete sends nothing more into that environment then. A reset drops `then` with the
/// Delete's own waiting.
pub(super) fn start(
    frozen: Frozen,
    again: Option<AskAgain>,
    then: impl FnOnce(bool, &mut App) + 'static,
    cx: &mut App,
) -> Result<(), NotStarted> {
    let Some(asked) = frozen.environment.clone() else {
        tracing::warn!("an environment Delete without its environment was not sent");
        return Err(NotStarted::Retired);
    };
    super::start_request(
        &OpKey::session(&frozen.key),
        command(&frozen.key, asked.lane),
        cx,
        move |settled, cx| {
            let carries_on = settle(frozen, asked, again, settled, cx);
            then(carries_on, cx);
        },
    )
    .inspect_err(|refused| {
        if let NotStarted::Path(_) = refused {
            tracing::warn!("a session key that cannot be a path segment was not deleted");
        }
    })
}

/// `frozen`'s environment Delete, asked as `asked`, settled (the module docs' "Settling"). A refusal this shows some
/// other way (a 404 as a deletion, a 504 as routine, an "unasked" answer as the whole-lane question) is forgotten from
/// the session's key; any other stays there for the row and the header. With no `again`, as the bulk Delete sends it,
/// an "unasked" answer is an ordinary refusal: the helm's words stay on the row and nothing opens. Says whether the
/// answer was the 504 at the helm's bound, the Delete still running.
fn settle(
    frozen: Frozen,
    asked: Asked,
    again: Option<AskAgain>,
    settled: &Settled,
    cx: &mut App,
) -> bool {
    let refusal = match &settled.outcome {
        Outcome::Done(reply) => {
            delete::record_deleted_here(&frozen.key, cx);
            // Only the session lane's reply says anything: the whole lane's carries no count and no checkout notice.
            let (kept, notice) = match asked.lane {
                Lane::Session => {
                    let outcome = delete::decode(reply.as_ref()).unwrap_or_default();
                    (
                        outcome
                            .environment_remaining
                            .map(|remaining| asked.ui.delete_kept(remaining)),
                        outcome.notice,
                    )
                }
                Lane::Whole => (None, None),
            };
            delete_notice::publish_environment(asked.host, kept, notice, cx);
            return false;
        }
        // These routes never accept long work (202); nothing is shown if one ever does.
        Outcome::Accepted(_) => return false,
        Outcome::Refused(refusal) => refusal,
    };
    let carries_on = refusal.status == CARRIES_ON;
    let shown_elsewhere = if refusal.status == GONE {
        delete::record_deleted_here(&frozen.key, cx);
        delete_notice::publish_environment(asked.host, None, None, cx);
        true
    } else if carries_on {
        true
    } else if asked.lane == Lane::Session && refusal.has(HelmMarker::EnvironmentUnasked) {
        let whole = Frozen {
            environment: Some(Asked {
                lane: Lane::Whole,
                ..asked
            }),
            ..frozen.clone()
        };
        // The operation store says whether the user moved on since answering; a question opens only if they have not,
        // and the prompt takes its mark now, in the same completion, so nothing can come between.
        !settled.user_moved_on && again.is_some_and(|again| again(whole, Mark::take(cx), cx))
    } else {
        false
    };
    if shown_elsewhere && let Some(operations) = session::current_operations(cx) {
        operations.update(cx, |operations, cx| {
            operations.forget(&OpKey::session(&frozen.key), cx)
        });
    }
    carries_on
}
