//! Tunnel and open: a link card's answer that opens a remote session's loopback link on this Mac through a tunnel.
//!
//! SPEC.md, "Agent-spawned sessions": "For a loopback URL from a remote session in the Mac app, the card offers
//! **Tunnel and open** first", with "the same consent warning and lifetime as a terminal Cmd-click"; "It names the
//! remote host and port, and explains that opening directly reaches the Mac instead." Ported from the old app's card
//! path through its `TunnelController` (`prepare_card`, `complete`, `observe_cards`) at the reference commit.
//!
//! The answer is the helm's to carry out: on `tunnel_and_open` the waiting request opens the tunnel for the asking
//! session and the browser at its local port, and the helm refuses the answer while the shared tunnel consent is not
//! recorded. So the press asks the terminal's tunnel warning first when consent is missing
//! ([`tunnels::ask_consent_about`]), and sends the answer only once the warning's Allow saved it, while the request
//! still waits; the warning goes away with the request, as the old app retired a card's warning when its card
//! vanished. Once the helm took the answer, the terminal's views of the asking session read their tunnels again
//! ([`tunnels::opened_elsewhere`]), since the tunnel was opened outside the terminal.
//!
//! Offered only in the Mac app: the platform is a value ([`offer`]), not a `cfg`, so Linux tests cover both card
//! shapes.

use farhelm_input_mapping::Platform;
use farhelm_proto::approvals::{ApprovalAnswer, PendingApproval};
use gpui::{App, Entity, Window};

use super::answers::{self, Arming};
use super::model::TunnelOffer;
use crate::focus::Mark;
use crate::host_text::HostText;
use crate::layers::Subject;
use crate::operations::{NotStarted, OpKey, Outcome};
use crate::session;
use crate::terminal::tunnels;

/// Whether cards offer Tunnel and open on `platform`: in the Mac app only (SPEC.md, "in the Mac app").
pub fn offer(platform: Platform) -> TunnelOffer {
    match platform {
        Platform::Mac => TunnelOffer::Offered,
        Platform::Linux => TunnelOffer::NotOffered,
    }
}

/// What a press of Tunnel and open did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressed {
    /// The answer is on its way.
    Sent,
    /// The tunnel warning is up; the answer goes out on its Allow.
    Asking,
    /// Nothing: the answer bar was paused, an answer to the request was already on its way while the warning would
    /// have been asked, or the user moved on since the press.
    Nothing,
}

/// Tunnel and open on `approval`'s card: with the shared tunnel consent recorded, send the answer now; otherwise ask
/// the tunnel warning first (unless the user moved on since `mark`), and on its Allow save the consent and then send
/// the answer, if the request still waits. `refused` hears why the consent could not be saved; the answer itself keeps
/// the card's ordinary outcomes (the helm's refusal on the request's key, a late answer's notice).
///
/// Cancel, Escape or closing the warning send nothing, and the request keeps waiting on its card. The warning closes
/// by itself when the request stops waiting.
pub fn tunnel_and_open(
    approval: &PendingApproval,
    arming: &Entity<Arming>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
    refused: impl FnOnce(HostText, &mut App) + 'static,
) -> Result<Pressed, NotStarted> {
    if !arming.read(cx).armed() {
        return Ok(Pressed::Nothing);
    }
    let id = approval.id.clone();
    let session = approval.session.id.clone();
    if tunnels::consented(cx) {
        // The operation store refuses a second answer to the request with Busy, which the card can show.
        send(&id, session, cx)?;
        return Ok(Pressed::Sent);
    }
    // No warning over an answer already on its way: it would only end in a refusal of the second answer.
    if answering(&id, cx) {
        return Ok(Pressed::Nothing);
    }
    let subject = request_subject(&id, cx);
    let asked =
        tunnels::ask_consent_about(subject, mark, window, cx, move |saved, cx| match saved {
            // The warning took time: the request may have ended meanwhile (its subject closes the warning, but a save
            // under way when it ended still lands here). The answer bar's pause is not checked again: it guards a press
            // aimed at a card that moved, and this answer comes from the warning the user answered for this card, as the
            // old app's prepared card went on.
            Ok(()) if waiting(&id, cx) => {
                if let Err(not_started) = send(&id, session, cx) {
                    tracing::debug!(?not_started, "Tunnel and open not sent after the warning");
                }
            }
            Ok(()) => {}
            Err(reason) => refused(reason, cx),
        });
    Ok(if asked.is_some() {
        Pressed::Asking
    } else {
        Pressed::Nothing
    })
}

/// Send the answer, past the answer bar's pause (the caller checked it at the press), with the cards' ordinary
/// outcomes, and have the asking session's tunnels read again once the helm took it.
fn send(id: &str, session: String, cx: &mut App) -> Result<(), NotStarted> {
    answers::start_unpaused(
        id,
        ApprovalAnswer::TunnelAndOpen,
        None,
        cx,
        move |settled, cx| {
            answers::settle(false, settled, cx);
            if !matches!(settled.outcome, Outcome::Refused(_)) {
                tunnels::opened_elsewhere(&session, cx);
            }
        },
    )
}

/// The warning's subject: the request `id`, gone once the helm stops holding it. `None` without a store, where the
/// warning cannot open anyway.
fn request_subject(id: &str, cx: &App) -> Option<Subject> {
    let store = session::current_store(cx)?;
    let id = id.to_owned();
    Some(Subject::when(store, move |store, _| {
        store.collections().get::<PendingApproval>(&id).is_none()
    }))
}

/// Whether the request `id` still waits.
fn waiting(id: &str, cx: &App) -> bool {
    session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<PendingApproval>(id)
            .is_some()
    })
}

/// Whether an answer to the request `id` is in flight, from any surface.
fn answering(id: &str, cx: &App) -> bool {
    session::current_operations(cx).is_some_and(|operations| {
        operations
            .read(cx)
            .in_flight(&OpKey::approval(id))
            .is_some()
    })
}
