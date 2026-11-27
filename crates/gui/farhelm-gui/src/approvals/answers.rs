//! Answering an approval card: the command, the answer bar's pause, and a late answer's notice.
//!
//! SPEC.md, "Agent-spawned sessions": "Answer buttons pause for 700ms whenever the waiting list or the expanded request
//! changes, so a click already aimed at the previous request cannot answer the next one", the first answer pressed
//! wins, an answer that arrives after its request stopped waiting says so, and "if turning the setting on fails,
//! nothing is approved and the card stays up with the reason". Ported from the old app's `ApprovalCards` answer
//! handling at the reference commit.
//!
//! An answer is work in the operation store under the request's own key ([`OpKey::approval`]), so the first answer
//! pressed holds the key and a second one is refused until the first settles, a refusal stays on the request's key for
//! the card to show (an Always allow whose setting could not be stored), and a reset drops the wait. The helm pushes a
//! request's removal before it replies to the answer that removed it, so a card that answered never needs hiding by
//! hand: the store already lacks it.
//!
//! A late answer is the helm's 410: the request had already stopped waiting (answered elsewhere, expired, its session
//! deleted or its connection gone), and the helm gives that its own status precisely so a client cannot mistake it for
//! a refusal it must show, as the old app's `AnswerOutcome::Gone` did. Any other refusal stays on the request's key,
//! even when the request goes at about the same time: an Always allow refused for a changed install wrote no setting,
//! and calling it late would wrongly say the setting may have been turned on.
//!
//! The answer route waits on no host, so it needs no bound of its own (SPEC_impl.md, "The UI channel"): it reads the
//! helm's memory and database, briefly takes the host's cache-write lock for an Always allow, and for a host add's
//! Allow waits on the add handler's in-memory receipt, which a dropped handler turns into a prompt 410.

use std::time::Duration;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::approvals::{ApprovalAnswer, ApprovalAnswerRequest, HostAddAnswer};
use gpui::{App, Context, Entity, WeakEntity};

use crate::notices::{self, Notice};
use crate::operations::{Command, NotStarted, OpKey, Outcome, Settled, Work};
use crate::ticker::{self, Deadline};

/// How long the answer buttons stay disarmed after the waiting list or the expanded request changes: long enough to
/// absorb a double click's second click and a click already on its way, short enough not to be noticed when nothing
/// moved (the old app's `ARM_DELAY_MS`).
pub const ARM_DELAY: Duration = Duration::from_millis(700);

/// The answer bar's pause. Disarmed from the moment the region shows a different list of requests or expands a
/// different one, and armed again [`ARM_DELAY`] later; the newest change's deadline replaces any earlier one, so an
/// older pause can never arm a newer card.
///
/// It is the region's own state, not a copy of the store: what it remembers is what the region last showed, which is
/// only ever compared with what it shows now.
#[derive(Default)]
pub struct Arming {
    shown: Option<(Vec<String>, Option<String>)>,
    armed: bool,
    deadline: Option<Deadline>,
}

/// [`Arming::default`] is disarmed with nothing shown yet, so the first requests the region shows start their pause.
impl Arming {
    /// Whether answers may be sent now.
    pub fn armed(&self) -> bool {
        self.armed
    }

    /// An answer bar already armed, for a screenshot scene, which cannot let the pause run out.
    #[cfg(test)]
    pub fn armed_for_scene() -> Self {
        Arming {
            armed: true,
            ..Arming::default()
        }
    }

    /// The region now shows `waiting` (ids, oldest first) with `expanded` expanded. A change disarms the buttons and
    /// starts a new pause, except to an empty list, which only disarms; the same presentation again changes nothing.
    /// Call it from the update that changed what is shown (a store change, a header chosen), never while drawing: a
    /// notify requested while drawing is dropped.
    pub fn observe(
        &mut self,
        waiting: Vec<String>,
        expanded: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let shown = (waiting, expanded);
        if self.shown.as_ref() == Some(&shown) {
            return;
        }
        let empty = shown.0.is_empty();
        self.shown = Some(shown);
        self.armed = false;
        if empty {
            // Nothing is shown, so nothing can be pressed: no pause runs and nothing redraws, so a window with no
            // request waiting sees no frame from the answer bar. The next request to show starts its pause, being a
            // change from this.
            self.deadline = None;
            return;
        }
        let arming: WeakEntity<Arming> = cx.weak_entity();
        self.deadline = Some(ticker::after(ARM_DELAY, cx, move |cx| {
            arming
                .update(cx, |arming, cx| {
                    arming.armed = true;
                    cx.notify();
                })
                .ok();
        }));
        cx.notify();
    }
}

/// The command for `answer` to the request `id`: `POST /api/approvals/{id}` with the answer, and for a host add's
/// Allow the destination and Tailscale choice the add dialog holds then (the helm requires them there and refuses them
/// anywhere else).
pub fn command(
    id: &str,
    answer: ApprovalAnswer,
    host_add: Option<HostAddAnswer>,
) -> Result<Command, PathError> {
    let body = ApprovalAnswerRequest { answer, host_add };
    Ok(Command::new(
        Work::Answer,
        Method::Post,
        ApiPath::api(&["approvals", id])?,
        serde_json::to_value(body).ok(),
        "POST /api/approvals/[request]",
    ))
}

/// Whether a press sent its answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent {
    /// The answer is on its way.
    Sent,
    /// The answer bar was paused when the press arrived, so nothing was sent.
    Paused,
}

/// Send `answer` to the request `id` unless the answer bar is paused, and handle its outcome: the helm's 410 posts the
/// late-answer notice, in its Always-allow form after an Always allow (whose host setting the helm stores before it
/// looks for the request); any other refusal stays on the request's key for the card to show
/// ([`crate::operations::Operations::failure`]). `host_add` is the destination and Tailscale choice a host add's Allow
/// carries from the add dialog.
///
/// Checks the pause when the press arrives, not only when the buttons were drawn: a click already on its way when the
/// list changed must not reach the next request. Refuses with [`NotStarted::Busy`] when an answer to this request is
/// already in flight: the first answer pressed wins.
pub fn send(
    id: &str,
    answer: ApprovalAnswer,
    host_add: Option<HostAddAnswer>,
    arming: &Entity<Arming>,
    cx: &mut App,
) -> Result<Sent, NotStarted> {
    let always = matches!(
        answer,
        ApprovalAnswer::AlwaysAllow | ApprovalAnswer::AlwaysAllowOwnHost
    );
    start(id, answer, host_add, arming, cx, move |settled, cx| {
        settle(always, settled, cx);
    })
}

/// Send `answer` to the request `id` unless the answer bar is paused, under the request's own key, and hand its reply
/// to `on_settled` alone: what [`send`] does for a card, without the late notice. The Add host dialog answers an agent's
/// host add through it and decides itself what each reply shows ([`super::host_add`]). Refuses with
/// [`NotStarted::Busy`] while an answer to the request is in flight, whichever surface sent it.
pub fn start(
    id: &str,
    answer: ApprovalAnswer,
    host_add: Option<HostAddAnswer>,
    arming: &Entity<Arming>,
    cx: &mut App,
    on_settled: impl FnOnce(&Settled, &mut App) + 'static,
) -> Result<Sent, NotStarted> {
    if !arming.read(cx).armed() {
        return Ok(Sent::Paused);
    }
    start_unpaused(id, answer, host_add, cx, on_settled)?;
    Ok(Sent::Sent)
}

/// [`start`] without the answer bar's pause, for an answer whose press already passed it and that went on through a
/// question since: Tunnel and open's tunnel warning, whose Allow answers the card the user pressed, not one that moved
/// under the pointer ([`super::tunnel`]).
pub fn start_unpaused(
    id: &str,
    answer: ApprovalAnswer,
    host_add: Option<HostAddAnswer>,
    cx: &mut App,
    on_settled: impl FnOnce(&Settled, &mut App) + 'static,
) -> Result<(), NotStarted> {
    let command = command(id, answer, host_add).map_err(NotStarted::Path)?;
    let Some(operations) = crate::session::current_operations(cx) else {
        return Err(NotStarted::Retired);
    };
    operations.update(cx, |operations, cx| {
        operations.start(&OpKey::approval(id), command, cx, on_settled)
    })
}

/// What an answer's reply means once it arrives: only the helm's 410 is a late answer. Shared with Tunnel and open,
/// whose answer goes out once the tunnel warning is answered ([`super::tunnel`]).
pub(super) fn settle(always: bool, settled: &Settled, cx: &mut App) {
    let Outcome::Refused(refusal) = &settled.outcome else {
        return;
    };
    if refusal.status != GONE {
        return;
    }
    post_late(always, cx);
}

/// Post the late-answer notice, in its Always-allow form after an Always allow, replacing the other form if it shows.
/// The Add host dialog posts it too when its own Allow was late, since it closes then and the notice is where the user
/// learns that nothing was added ([`crate::hosts::add`]).
pub(crate) fn post_late(always: bool, cx: &mut App) {
    notices::withdraw(Notice::LateAnswer { always: !always }, cx);
    notices::post(Notice::LateAnswer { always }, cx);
}

/// The helm's status for an answer to a request that no longer waits.
pub(super) const GONE: u16 = 410;
