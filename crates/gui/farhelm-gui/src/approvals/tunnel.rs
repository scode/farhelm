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
//! The warning opens over whatever dialog is open, as the old app's did: it is about the card's request, not about that
//! dialog. So its opener says it was opened from a card ([`layers::opened_from_approval_card`]), and the dialog's owner
//! closing the dialog (a bulk question whose targets went away) leaves the warning open, focused and above the cards.
//! Focus alone could not say so on macOS, where a press on the card leaves focus in the dialog.
//!
//! A press while a tunnel warning is already up, whoever asked it (this card, another card, Settings' switch, a
//! terminal's Cmd-click), asks nothing, since a second, identical warning would stack over the first, and the card
//! says why in the old app's words ([`Pressed::AnotherTunnelAction`]). The press asks the layer manager whether a layer
//! raised above the approval region is open ([`layers::above_approval_region_open`]), which assumes the tunnel warning
//! is the only layer any opener raises there, as the layer manager's docs say today: a second kind of raised layer
//! would also hold Tunnel and open back while it is up.
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
use crate::layers::{self, Subject};
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
    /// Nothing: the answer bar was paused, an answer to the request or a save of the preferences (an earlier warning's
    /// consent among them) was already on its way while the warning would have been asked, or the user moved on since
    /// the press. The request keeps waiting, and the card says nothing.
    Nothing,
    /// The warning would have been asked while a tunnel warning is already up: the card says so ([`Pressed::reason`])
    /// and the request keeps waiting.
    AnotherTunnelAction,
}

impl Pressed {
    /// What the card's refusal line says, in the app's words, or `None` when there is nothing to say. The old app's
    /// words for a tunnel action pressed while another is still under way (`TunnelController::prepare` in `tunnels.rs`
    /// at reference commit `24bc0580`); the terminal's `BUSY` (`terminal/tunnels.rs`) holds the same words for the
    /// terminal's own tunnel actions, so a rewording changes both.
    pub fn reason(self) -> Option<&'static str> {
        match self {
            Pressed::AnotherTunnelAction => Some("Another tunnel action is still waiting"),
            Pressed::Sent | Pressed::Asking | Pressed::Nothing => None,
        }
    }
}

/// Tunnel and open on `approval`'s card: with the shared tunnel consent recorded, send the answer now; otherwise ask
/// the tunnel warning first (unless the user moved on since `mark`), and on its Allow save the consent and then send
/// the answer, if the request still waits. `refused` hears why the consent could not be saved; the answer itself keeps
/// the card's ordinary outcomes (the helm's refusal on the request's key, a late answer's notice).
///
/// Cancel, Escape or closing the warning send nothing, and the request keeps waiting on its card. The warning closes
/// by itself when the request stops waiting, and only then or by its own answers: it opens over any dialog, and that
/// dialog closing leaves it open (the module docs). A press while a tunnel warning is already up asks nothing and
/// reports [`Pressed::AnotherTunnelAction`].
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
    // An earlier warning's Allow closes the warning before its consent save comes back: until it does, a press would
    // pass every other check and ask a second warning. SPEC_impl.md keeps an accepted warning's claim until its save
    // finishes; the save runs under the preferences' key, so any preferences save holds the press for its short while.
    if saving_preferences(cx) {
        return Ok(Pressed::Nothing);
    }
    // A warning already up, from any opener, is the one this press would ask again (the module docs).
    if layers::above_approval_region_open(window, cx) {
        return Ok(Pressed::AnotherTunnelAction);
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
    let Some(warning) = asked else {
        return Ok(Pressed::Nothing);
    };
    // In the open's own update, before anything can close the dialog under it: on macOS focus is still in that dialog,
    // which would otherwise count as the warning's parent and take it along when its owner closes it.
    layers::opened_from_approval_card(warning, window, cx);
    Ok(Pressed::Asking)
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

/// Whether a save of the preferences is in flight, the tunnel consent's among them.
fn saving_preferences(cx: &App) -> bool {
    session::current_operations(cx).is_some_and(|operations| {
        operations
            .read(cx)
            .in_flight(&OpKey::preferences())
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
