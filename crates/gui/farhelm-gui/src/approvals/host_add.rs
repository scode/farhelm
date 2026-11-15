//! An agent's host add: the request a card answers through the ordinary Add host dialog rather than with its own
//! buttons.
//!
//! SPEC.md, "Agent-spawned sessions": an agent's `farhelm host add` "opens the ordinary add host dialog with the
//! requested destination and alias" once the user acts on its card; "opening is not approval". Add answers Allow with
//! the destination then in the dialog, and only an accepted Allow lets the ordinary discovery-first add go on; Cancel
//! answers Deny; and the setup confirmation always shows for an agent's add, whatever "Set up new hosts without asking"
//! says. Ported from the old app's `AgentAddBridge` (`approvals.rs`) and `AddHostForm`'s agent handling (`hosts.rs`) at
//! the reference commit.
//!
//! The dialog itself is settings-and-hosts' ([`crate::hosts::add`]); this module is what that dialog holds for an
//! agent's request: who asked, the answer under the request's own operation key, and the words for what came of it.
//! The answers are work in the operation store under [`OpKey::approval`], the key the card's own answers use, so an
//! answer from the dialog and one from the card exclude each other, and the first pressed wins.
//!
//! A refused answer, or an Allow the helm did not confirm, shows in the dialog's own error line, where the user is
//! looking. A late answer closes the dialog, as the old one closed once its request was gone; after a late Allow the
//! cards' late-answer notice says that nothing was added.

use farhelm_proto::approvals::{ApprovalAction, ApprovalAnswer, PendingApproval};
use gpui::{
    AnyElement, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Styled as _, div, px,
};

use super::answers::{self, Arming};
use super::model::Requester;
use crate::host_text::{self, HostText};
use crate::operations::{NotStarted, OpKey, Outcome, Settled};
use crate::session;
use crate::theme::{self, metrics};

/// What the dialog's error line says when the helm answered an Allow with anything but its bodiless acceptance (the 204
/// it sends only once the waiting add received the destination): the old dialog's words. Nothing is probed then, since
/// only that receipt says the agent's add will go on with what the user approved.
pub const NOT_CONFIRMED: &str =
    "The host add answer did not confirm acceptance; no discovery was started";

/// An agent's host add request, as the Add host dialog holds it from the card's open until the dialog closes.
///
/// It keeps what the dialog must show or use after the request has gone from the store (an accepted Allow removes it,
/// and the dialog goes on with the ordinary add): the request's id, who asked, what the agent asked to add, and the
/// answer bar's pause. Whether it still waits is read from the store each time ([`AgentRequest::waiting`]), never
/// remembered.
#[derive(Clone)]
pub struct AgentRequest {
    /// The helm's id for the request: the answer's route and its operation key.
    pub id: String,
    /// Who asked, for the dialog's labels.
    pub from: Requester,
    /// The destination the agent asked for, which prefills the dialog's field.
    pub destination: String,
    /// The alias the agent asked for (blank for none), which prefills the dialog's field.
    pub alias: String,
    /// The answer bar's pause, shared with the cards: a click on its way when the waiting list changed answers nothing
    /// here either.
    pub arming: Entity<Arming>,
}

/// What came of an answer the dialog sent.
#[derive(Clone, Debug, PartialEq)]
pub enum Answered {
    /// The helm took it: after an Allow, the add handler received the destination and the ordinary add may go on;
    /// after a Deny, the agent was told.
    Delivered,
    /// The request had already stopped waiting, so nothing was added. The dialog closes, as the old one did once its
    /// request was gone, and the cards' late-answer notice says why.
    Gone,
    /// The helm refused, in its words; the request still waits.
    Refused(HostText),
    /// An Allow's reply was a success with a body or a 202, not the add's receipt ([`NOT_CONFIRMED`]). Never a Deny's.
    Unconfirmed,
}

impl AgentRequest {
    /// The request `approval` is about to open the dialog for, or `None` when it is not a host add.
    pub fn of(approval: &PendingApproval, arming: Entity<Arming>) -> Option<Self> {
        let ApprovalAction::HostAdd { destination, alias } = &approval.action else {
            return None;
        };
        Some(AgentRequest {
            id: approval.id.clone(),
            from: Requester::of(&approval.host_name, &approval.session),
            destination: destination.clone(),
            alias: alias.clone().unwrap_or_default(),
            arming,
        })
    }

    /// The operation key both the dialog's and the card's answers to this request run under.
    pub fn key(&self) -> OpKey {
        OpKey::approval(&self.id)
    }

    /// Whether the helm still holds the request: it is in the pushed approvals.
    pub fn waiting(&self, cx: &App) -> bool {
        session::current_store(cx).is_some_and(|store| {
            store
                .read(cx)
                .collections()
                .get::<PendingApproval>(&self.id)
                .is_some()
        })
    }

    /// Whether an answer to the request is in flight, from the dialog or the card.
    pub fn answering(&self, cx: &App) -> bool {
        session::current_operations(cx)
            .is_some_and(|operations| operations.read(cx).in_flight(&self.key()).is_some())
    }

    /// Whether answers are paused now (the answer bar's pause after the waiting list changed).
    pub fn paused(&self, cx: &App) -> bool {
        !self.arming.read(cx).armed()
    }

    /// Answer Allow with `destination`, the one the dialog holds now (the helm adds what the user approved, not what
    /// the agent first asked for), and call `then` with what came of it.
    pub fn allow(
        &self,
        destination: String,
        cx: &mut App,
        then: impl FnOnce(Answered, &mut App) + 'static,
    ) -> Result<answers::Sent, NotStarted> {
        self.answer(ApprovalAnswer::Allow, Some(destination), cx, then)
    }

    /// Answer Deny, and call `then` with what came of it.
    pub fn deny(
        &self,
        cx: &mut App,
        then: impl FnOnce(Answered, &mut App) + 'static,
    ) -> Result<answers::Sent, NotStarted> {
        self.answer(ApprovalAnswer::Deny, None, cx, then)
    }

    /// Send `answer` as the card's answers are sent, under the request's key and through the same pause, but with the
    /// outcome handed to the dialog instead of the cards' late notice.
    fn answer(
        &self,
        answer: ApprovalAnswer,
        host_add: Option<String>,
        cx: &mut App,
        then: impl FnOnce(Answered, &mut App) + 'static,
    ) -> Result<answers::Sent, NotStarted> {
        let allow = answer == ApprovalAnswer::Allow;
        answers::start(
            &self.id,
            answer,
            host_add,
            &self.arming,
            cx,
            move |settled, cx| {
                then(answered(settled, allow), cx);
            },
        )
    }

    /// The dialog's lines naming who asked (the old dialog's "From host", "From session" and "From session ID"), each
    /// value host text in its own element. The full reference is shown here, unlike the card's shortened one: the
    /// dialog has the room, and the old dialog showed it whole.
    pub fn requester(&self, cx: &App) -> AnyElement {
        let unknown = HostText::from_host("Unknown");
        let title = self.from.title.as_ref().unwrap_or(&unknown);
        facts(
            "add-host-agent-requester",
            [
                ("From host", self.from.host.clone()),
                ("From session", title.clone()),
                ("From session ID", HostText::identity(self.from.id.as_str())),
            ],
            cx,
        )
    }
}

/// The dialog's lines spelling out what Add will approve ("Destination to add", "Alias to add"), following the fields
/// as they are edited: the fields hold the raw text, which may hide invisible or direction-changing characters, and these
/// lines show the escaped spelling of exactly what Allow will carry.
pub fn approved_fields(destination: &str, alias: &str, cx: &App) -> AnyElement {
    facts(
        "add-host-agent-approved",
        [
            ("Destination to add", HostText::from_host(destination)),
            ("Alias to add", HostText::from_host(alias)),
        ],
        cx,
    )
}

/// What an answer's reply means for the dialog. An Allow is accepted only by the helm's empty 204, its receipt that
/// the waiting add took the destination (SPEC_impl.md, "Permission prompts for agent actions"); any other success is
/// [`Answered::Unconfirmed`], as the old app refused it. A Deny is taken by any success.
fn answered(settled: &Settled, allow: bool) -> Answered {
    match &settled.outcome {
        Outcome::Done(None) => Answered::Delivered,
        Outcome::Done(Some(_)) | Outcome::Accepted(_) if allow => Answered::Unconfirmed,
        Outcome::Done(Some(_)) | Outcome::Accepted(_) => Answered::Delivered,
        Outcome::Refused(refusal) if refusal.status == answers::GONE => Answered::Gone,
        Outcome::Refused(refusal) => Answered::Refused(refusal.words.clone()),
    }
}

/// Labelled values as the old dialog's description lists drew them: each label on its line in the quiet ink, its value
/// under it, indented as a browser indents a `dd`, in the ordinary ink and wrapping anywhere.
fn facts<const N: usize>(id: &str, rows: [(&'static str, HostText); N], cx: &App) -> AnyElement {
    let palette = theme::palette(cx);
    div()
        .id(SharedString::from(id.to_owned()))
        .flex()
        .flex_col()
        .text_size(metrics::TEXT_SM)
        .children(rows.into_iter().map(|(label, value)| {
            div()
                .flex()
                .flex_col()
                .child(div().text_color(theme::color(palette.fg_1)).child(label))
                .child(
                    div()
                        .ml(px(DD_INDENT))
                        .min_w_0()
                        .text_color(theme::color(palette.fg_0))
                        .child(host_text::element(&value)),
                )
        }))
        .into_any_element()
}

/// A browser's default `dd` indent, which the old dialog's unstyled description lists kept.
const DD_INDENT: f32 = 40.;
