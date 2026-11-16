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
//! The sidebar's plain Replace asks the same question through `launcher::entry::ask_yolo_for_replace`, with its own
//! retry and no guarded answer; an [`Owner`] is what differs between the two.
//!
//! Restart with asks the launcher's question too, offering the guarded answer only while a guarded restart would still
//! change something ([`crate::launcher::draft::LaunchDraft::guarded_still_changes`]).

use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchPermission, LaunchSelection, SessionLaunch};
use std::rc::Rc;

use gpui::{AppContext as _, Context, WeakEntity, Window};

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
pub struct YoloAsk {
    /// The host to stop asking on, for the permanent answer.
    host: i64,
    /// The host's name as the launcher shows it.
    host_name: HostText,
    reason: YoloReason,
    /// Whether the guarded alternative is offered.
    guarded: bool,
}

impl YoloAsk {
    /// What a plain Replace's question asks about: the source's host and its stored launch, which Replace relaunches
    /// as it is. Never offers guarded: plain Replace keeps the stored launch (SPEC.md: the guarded answer is "in the
    /// session launcher", for an explicit YOLO choice there).
    pub(in crate::launcher) fn for_stored_launch(
        host: i64,
        host_name: HostText,
        launch: &SessionLaunch,
    ) -> Self {
        let reason = match launch {
            SessionLaunch::Agent { selection, .. } => YoloReason::of_launch(Some(selection)),
            SessionLaunch::Command(_) => YoloReason::of_launch(None),
            // Plain Replace refuses a launch from before launch kinds, so its question never comes; were it asked, the
            // stored command is the agent's own setting as far as Farhelm can tell.
            SessionLaunch::Legacy { .. } => YoloReason::OwnSetting,
        };
        YoloAsk {
            host,
            host_name,
            reason,
            guarded: false,
        }
    }
}

/// The question's state in the launcher: what it asks about, and the question once asked (`None` until the launcher
/// is next shown, which is when it can take focus in place).
pub(in crate::launcher) struct YoloState {
    ask: YoloAsk,
    inline: Option<Inline<YoloAsk>>,
}

/// One answer button.
fn answer(label: &'static str, help: &'static str, tier: Tier) -> Answer {
    Answer::new(label, HoverHelp::tooltip(help), tier)
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
                guarded: reason.offers_guarded(selection.as_ref())
                    && self.draft.guarded_still_changes(),
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
        let guarded = state.ask.guarded.then(|| {
            let launcher = cx.weak_entity();
            Box::new(move |window: &mut Window, cx: &mut gpui::App| {
                let mark = Mark::take(cx);
                let _ = launcher.update(cx, |launcher, cx| {
                    launcher
                        .draft
                        .choose_permissions(Some(LaunchPermission::Guarded));
                    // A managed checkout's preview is stale after the switch; this waits for the new one.
                    let _ = launcher.submit_or_wait_for_preview(mark, window, cx);
                });
            }) as GuardedAction
        });
        let owner = launcher_owner(cx.weak_entity());
        state.inline = Some(ask_question(&state.ask, guarded, owner, mark, window, cx));
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

/// Send a launch again with consent, for the user's answer at the given mark.
type LaunchAgain = Rc<dyn Fn(Mark, &mut Window, &mut gpui::App)>;

/// A plain Replace's retry with consent, the caller's own.
pub(in crate::launcher) type Retry = Rc<dyn Fn(Mark, &mut Window, &mut gpui::App)>;

/// The guarded alternative's action: switch the launch to Guarded and start it without consent.
type GuardedAction = Box<dyn FnOnce(&mut Window, &mut gpui::App)>;

/// What owns a YOLO question and its launch: the launcher, for its own refused launches, or the sidebar's plain
/// Replace, through `launcher::entry::ask_yolo_for_replace`. The question's words and answers are the same for both;
/// only these differ.
#[derive(Clone)]
pub(in crate::launcher) struct Owner {
    /// Send the launch again with consent, for the user's answer at the given mark.
    launch: LaunchAgain,
    /// Whether the question is still current when the permanent answer's host write succeeds: the launcher's is not
    /// once an edit of its draft has retired it.
    current: Rc<dyn Fn(&gpui::App) -> bool>,
    /// Redraw the owner when the host write starts or fails (the launcher draws its controls busy meanwhile).
    redraw: Rc<dyn Fn(&mut gpui::App)>,
}

/// A plain Replace as a question's owner: its launch is the caller's `retry`, which sends the Replace again with
/// consent; its question stays current (there is no draft to edit), and nothing of the launcher redraws.
pub(in crate::launcher) fn replace_owner(retry: Retry) -> Owner {
    Owner {
        launch: Rc::new(move |mark, window, cx| retry(mark, window, cx)),
        current: Rc::new(|_| true),
        redraw: Rc::new(|_| {}),
    }
}

/// The launcher as a question's owner: it launches its own draft with consent, its question is current while it is in
/// flight, and it redraws itself.
fn launcher_owner(launcher: WeakEntity<Launcher>) -> Owner {
    let (launching, checking, redrawing) = (launcher.clone(), launcher.clone(), launcher);
    Owner {
        launch: Rc::new(move |mark, window, cx| {
            let _ = launching.update(cx, |launcher, cx| launcher.submit(true, mark, window, cx));
        }),
        current: Rc::new(move |cx| {
            checking
                .upgrade()
                .is_some_and(|launcher| launcher.read(cx).yolo_in_flight(cx))
        }),
        redraw: Rc::new(move |cx| {
            let _ = redrawing.update(cx, |_, cx| cx.notify());
        }),
    }
}

/// Ask the YOLO question for `ask` inline, for the user's action at `mark`: the one-off answer and the permanent
/// answer launch through `owner`, and `guarded`, when given, is the guarded alternative's action.
pub(in crate::launcher) fn ask_question(
    ask: &YoloAsk,
    guarded: Option<GuardedAction>,
    owner: Owner,
    mark: &Mark,
    window: &mut Window,
    cx: &mut gpui::App,
) -> Inline<YoloAsk> {
    let launch = owner.launch.clone();
    prompt::ask_inline(
        question(ask, guarded, owner),
        mark,
        move |_: YoloAsk, window, cx| {
            let mark = Mark::take(cx);
            launch(mark, window, cx);
        },
        window,
        cx,
    )
}

/// The question for `ask`. Its one-off answer is the confirmation, whose action [`ask_question`] supplies.
fn question(ask: &YoloAsk, guarded: Option<GuardedAction>, owner: Owner) -> Question<YoloAsk> {
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
    if let Some(guarded) = guarded {
        question = question.alternative(
            answer(
                "Start guarded session instead",
                "Start this session in guarded mode instead, where the agent's own safety checks stay on",
                Tier::Primary,
            ),
            move |_, window, cx| guarded(window, cx),
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
                stop_asking_then_launch(ask, attempt, owner.clone(), window, cx)
            },
        )
        .starts_on_cancel()
        .loud()
}

/// The permanent answer: set the host to start YOLO sessions without asking (the host settings' own command, on the
/// host's key, as its switch sends it), and only when that succeeds launch with consent through `owner`, if its
/// question is still current. A failed or refused write starts nothing and keeps the question up with the reason.
fn stop_asking_then_launch(
    ask: YoloAsk,
    attempt: Attempt,
    owner: Owner,
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
    // The answer is the user's press the launch's moved-on check counts from, not the write's reply.
    let mark = Mark::take(cx);
    let handle = window.window_handle();
    let host_name = ask.host_name.clone();
    // The attempt settles from the write's reply; a write the store refuses to start settles it at once instead.
    let pending = Rc::new(std::cell::Cell::new(Some(attempt)));
    let settling = pending.clone();
    let on_reply = owner.clone();
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
                        (on_reply.redraw)(cx);
                    }
                    Outcome::Done(_) | Outcome::Accepted(_) => {
                        // Only while the question this write answered is still current: for the launcher, an edit
                        // of the draft since then retired it, and launching now would send a draft the user never
                        // confirmed.
                        let current = (on_reply.current)(cx);
                        attempt.succeeded(cx);
                        if current {
                            let launch = on_reply.launch.clone();
                            let _ =
                                cx.update_window(handle, |_, window, cx| launch(mark, window, cx));
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
        // The owner may draw itself busy while the write is out; the question refreshes only itself.
        (owner.redraw)(cx);
    }
}
