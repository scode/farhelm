//! The launcher's YOLO question: the inline confirmation it shows when the helm refuses a launch because the host asks
//! before YOLO launches (the refusal's `HelmMarker::YoloConfirmation`).
//!
//! SPEC.md, "Creation", the YOLO paragraph, and "Keyboard focus". Ported from the old app's `yolo_confirm.rs` at
//! reference commit `24bc0580`; the prompt foundation's inline question draws it (its golden `inline-prompt` pictures
//! exactly this question). Here's the TLDR:
//!
//! - **When.** The refused launch comes back as any refusal does (`launcher::launch`): shown again if the user has
//!   not moved on, otherwise offered by the notice. The question is asked when the launcher is next shown, so it opens
//!   in place either way, Cancel holding focus. The helm's own refusal sentence is not shown: it is written for the
//!   command line, and the launcher knows what it sent.
//! - **What it says.** The host, what YOLO means, why this launch is one ([`super::super::yolo::YoloReason`]), and that
//!   nothing has started.
//! - **The answers**, in reading and Tab order: Cancel; "Start guarded session instead" only for an explicit YOLO
//!   choice on an agent type that offers guarded mode (it switches the draft to Guarded and launches without consent or
//!   a host write); the outlined "Start, and don't ask again on this host", which first sets the host to start YOLO
//!   sessions without asking, exactly as the host's settings would, then launches with consent, and if that write fails
//!   starts nothing and keeps the question up with the reason; and the filled "Start YOLO session anyway", which
//!   launches with consent once. The one-off answer is the filled one on purpose: the permanent choice must not be the
//!   reflex target (the old app's reasoning).
//! - **While the host write is out**, the launcher holds still: Escape, Close and Cancel keep it open, and Launch does
//!   nothing, so the only launch is the one the answer sends. An edit of the draft retires the question, and the
//!   write's success then starts nothing: the question it answered is gone.
//!
//! NOTE: Restart with's question (offering guarded only when the result still differs from the stored launch) and the
//! plain Replace question the sidebar places come with those pieces.

use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchPermission, LaunchSelection};
use gpui::{AppContext as _, Context, Window};

use super::Launcher;
use crate::components::{HoverHelp, Tier};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::display_name;
use crate::launcher::draft::Tab;
use crate::launcher::yolo::YoloReason;
use crate::operations::{OpKey, Outcome};
use crate::prompt::{self, Answer, Attempt, Inline, Question};
use crate::session;

/// What the question is about, frozen when the refusal arrives: the host the launch targeted and why it counts as
/// YOLO. The question describes that launch even if the store's view of the host changes while it is up.
#[derive(Clone, Debug)]
pub(in crate::launcher) struct YoloAsk {
    /// The host to stop asking on, for the permanent answer.
    host: i64,
    /// The host's name as the launcher shows it.
    host_name: HostText,
    reason: YoloReason,
    /// Whether the guarded alternative is offered.
    guarded: bool,
}

/// The question's state in the launcher: what it asks about, and the question once asked (`None` until the launcher
/// is next shown, which is when it can take focus in place).
pub(in crate::launcher) struct YoloState {
    ask: YoloAsk,
    inline: Option<Inline<YoloAsk>>,
}

/// One answer button.
fn answer(label: &'static str, help: &'static str, tier: Tier) -> Answer {
    Answer {
        label: label.into(),
        help: HoverHelp::tooltip(help),
        tier,
    }
}

/// The words the question shows when the permanent answer's host write fails (the old `stop_asking`): the helm's
/// reason, then what did not happen, because "nothing started and the host still asks" is what the user decides from.
/// "May still ask": a refusal can also be a lost reply to a write that went through, so the host's state is not known.
fn stop_asking_failed(host_name: &HostText, words: &HostText) -> HostSentence {
    HostSentence::new("yolo-stop-asking-failed")
        .text("Could not stop asking for ")
        .host(host_name)
        .text(": ")
        .host(words)
        .text(". This answer started nothing; ")
        .host(host_name)
        .text(" may still ask before YOLO launches.")
}

impl Launcher {
    /// The helm refused this launcher's launch because its host asks before YOLO launches: freeze what the question
    /// will ask about, from the draft that was sent (it has not changed: the launcher is hidden while its create is
    /// out). The question itself is asked when the launcher is next shown ([`Launcher::ask_pending_yolo`]).
    pub(in crate::launcher) fn yolo_asked(&mut self, cx: &mut Context<Self>) {
        self.pressed = false;
        self.refused = None;
        let Some(host) = self.draft.host else {
            return;
        };
        let selection: Option<LaunchSelection> = match self.draft.tab {
            Tab::Agent => self.draft.agent_selection(),
            Tab::Command => None,
        };
        let reason = YoloReason::of_launch(selection.as_ref());
        let host_name = self
            .hosts(cx)
            .iter()
            .find(|view: &&HostView| view.id == host)
            .map(display_name)
            .unwrap_or_else(|| HostText::from_host("this host"));
        self.yolo = Some(YoloState {
            ask: YoloAsk {
                host,
                host_name,
                reason,
                guarded: reason.offers_guarded(selection.as_ref()),
            },
            inline: None,
        });
        cx.notify();
    }

    /// Ask the pending YOLO question, now that the launcher is shown again for the user's action at `mark`. Asked once:
    /// a question already asked stays as it is.
    pub(in crate::launcher) fn ask_pending_yolo(
        &mut self,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.yolo.as_mut() else {
            return;
        };
        if state.inline.is_some() {
            return;
        }
        let question = yolo_question(&state.ask, cx.weak_entity());
        let confirm = cx.weak_entity();
        state.inline = Some(prompt::ask_inline(
            question,
            mark,
            move |_: YoloAsk, window, cx| {
                let mark = Mark::take(cx);
                let _ = confirm.update(cx, |launcher, cx| launcher.submit(true, mark, window, cx));
            },
            window,
            cx,
        ));
        cx.notify();
    }

    /// The question while it is up, for drawing where the launcher's notices go.
    pub(in crate::launcher) fn yolo_inline(&self) -> Option<&Inline<YoloAsk>> {
        self.yolo.as_ref().and_then(|state| state.inline.as_ref())
    }

    /// Whether the permanent answer's host write is out, during which the launcher refuses to be dismissed.
    pub(in crate::launcher) fn yolo_in_flight(&self, cx: &gpui::App) -> bool {
        self.yolo_inline()
            .is_some_and(|inline| inline.in_flight(cx))
    }
}

/// The question for `ask`, with the alternatives' actions on `launcher`. The one-off answer is its confirmation, whose
/// action [`Launcher::ask_pending_yolo`] supplies.
fn yolo_question(ask: &YoloAsk, launcher: gpui::WeakEntity<Launcher>) -> Question<YoloAsk> {
    let mut question = Question::new(
        "Confirm YOLO launch",
        answer(
            "Start YOLO session anyway",
            "Start the session in YOLO mode, where the agent acts without asking",
            Tier::Danger,
        ),
        ask.clone(),
    )
    .summary(
        HostSentence::new("yolo-consequence")
            .text(
                "YOLO means the agent bypasses its safety checks: no person, classifier, AI reviewer or sandbox stops \
                 its actions on ",
            )
            .host(&ask.host_name)
            .text(" without asking you."),
    )
    // Its own element, so the reason can be found by its id (the question's plain lines carry none).
    .line(HostSentence::new("yolo-reason").text(ask.reason.sentence()))
    .line(
        HostSentence::new("yolo-asks")
            .host(&ask.host_name)
            .text(" asks before every YOLO launch. Nothing has been started."),
    )
    .cancel_answer(answer(
        "Cancel",
        "Cancel: go back without starting anything",
        Tier::Neutral,
    ));
    if ask.guarded {
        let guarded = launcher.clone();
        question = question.alternative(
            answer(
                "Start guarded session instead",
                "Start this session in guarded mode instead, where the agent's own safety checks stay on",
                Tier::Primary,
            ),
            move |_, window, cx| {
                let mark = Mark::take(cx);
                let _ = guarded.update(cx, |launcher, cx| {
                    launcher.draft.choose_permissions(Some(LaunchPermission::Guarded));
                    launcher.submit(false, mark, window, cx);
                });
            },
        );
    }
    question
        .fallible_alternative(
            answer(
                "Start, and don't ask again on this host",
                "Start it, and start YOLO sessions on this host without asking from now on",
                Tier::DangerOutline,
            ),
            move |ask: YoloAsk, attempt: Attempt, window, cx| {
                stop_asking_then_launch(ask, attempt, launcher.clone(), window, cx)
            },
        )
        .starts_on_cancel()
        .loud()
}

/// The permanent answer: set the host to start YOLO sessions without asking (the host settings' own command, on the
/// host's key, as its switch sends it), and only when that succeeds launch with consent. A failed or refused write
/// starts nothing and keeps the question up with the reason.
fn stop_asking_then_launch(
    ask: YoloAsk,
    attempt: Attempt,
    launcher: gpui::WeakEntity<Launcher>,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let (Some(operations), Ok(command)) = (
        session::current_operations(cx),
        crate::hosts::start_yolo_without_asking_command(ask.host),
    ) else {
        let reason = HostText::from_host("the request could not be sent");
        attempt.failed(stop_asking_failed(&ask.host_name, &reason), cx);
        return;
    };
    // The answer is the user's press the create's moved-on check counts from, not the write's reply.
    let mark = Mark::take(cx);
    let handle = window.window_handle();
    let host_name = ask.host_name.clone();
    // The attempt settles from the write's reply; a write the store refuses to start settles it at once instead.
    let pending = std::rc::Rc::new(std::cell::Cell::new(Some(attempt)));
    let launcher_for_redraw = launcher.clone();
    let settling = pending.clone();
    let started = operations.update(cx, |operations, cx| {
        operations.start(
            &OpKey::host(&HostView::key(ask.host)),
            command,
            cx,
            move |settled, cx| {
                let Some(attempt) = settling.take() else {
                    return;
                };
                match &settled.outcome {
                    Outcome::Refused(refusal) => {
                        // The old answer trimmed the helm's closing period, which the sentence supplies itself.
                        let words = HostText::from_host(refusal.words.raw().trim_end_matches('.'));
                        attempt.failed(stop_asking_failed(&host_name, &words), cx);
                        let _ = launcher.update(cx, |_, cx| cx.notify());
                    }
                    Outcome::Done(_) | Outcome::Accepted(_) => {
                        // Only while the question this write answered is still up: an edit of the draft since then
                        // retired it, and launching now would send a draft the user never confirmed.
                        let current = launcher
                            .upgrade()
                            .is_some_and(|launcher| launcher.read(cx).yolo_in_flight(cx));
                        attempt.succeeded(cx);
                        if current {
                            let _ = cx.update_window(handle, |_, window, cx| {
                                let _ = launcher.update(cx, |launcher, cx| {
                                    launcher.submit(true, mark, window, cx)
                                });
                            });
                        }
                    }
                }
            },
        )
    });
    if let Err(not_started) = started
        && let Some(attempt) = pending.take()
    {
        // The store's one wording for a busy key, its closing period dropped for the sentence's own.
        let reason = match not_started {
            crate::operations::NotStarted::Busy(_) => {
                crate::operations::NotStarted::BUSY_REASON.trim_end_matches('.')
            }
            crate::operations::NotStarted::Retired | crate::operations::NotStarted::Path(_) => {
                "the request could not be sent"
            }
        };
        attempt.failed(
            stop_asking_failed(&ask.host_name, &HostText::from_host(reason)),
            cx,
        );
    } else {
        // The launcher draws its controls busy while the write is out; the question refreshes only itself.
        let _ = launcher_for_redraw.update(cx, |_, cx| cx.notify());
    }
}
