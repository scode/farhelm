//! The launcher asks for lasting consent only after a deliberate pick and a successful read of an off switch.
//! An accepted setting write belongs to the helm. Its launch continuation belongs to the draft that was answered.

use std::{cell::Cell, rc::Rc};

use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{App, AppContext as _, Context, WeakEntity, Window};

use super::Launcher;
use crate::components::{HoverHelp, Tier};
use crate::focus::Mark;
use crate::hook_setup::HookAgent;
use crate::host_text::HostSentence;
use crate::launcher::draft::Tab;
use crate::operations::{OpKey, Outcome};
use crate::prompt::{self, Answer, Attempt, Inline, Question};
use crate::session;

/// A read and its resulting question share the pick number. A slow reply cannot answer a later pick of the same agent.
pub(super) struct HookQuestion {
    agent: HookAgent,
    pick: u64,
    inline: Option<Inline<HookAgent>>,
}

impl Launcher {
    fn hook_agent(&self) -> Option<HookAgent> {
        let harness = match self.draft.tab {
            Tab::Agent => self.draft.agent.harness,
            Tab::Command => self.draft.command.agent,
        };
        harness.and_then(HookAgent::for_harness)
    }

    fn hook_hint_dismissed(&self, agent: HookAgent, cx: &App) -> bool {
        self.store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|prefs| agent.dismissal(prefs))
            == Some(true)
    }

    /// Check once per explicit choice. A failed read leaves ordinary launching available and never offers a write.
    pub(super) fn check_hook_question(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hook_question = None;
        let Some(agent) = self
            .hook_agent()
            .filter(|agent| !self.hook_hint_dismissed(*agent, cx))
        else {
            return;
        };
        self.hook_pick += 1;
        let pick = self.hook_pick;
        self.hook_question = Some(HookQuestion {
            agent,
            pick,
            inline: None,
        });
        let mark = Mark::take(cx);
        let handle = window.window_handle();
        let launcher = cx.weak_entity();
        crate::settings::read_hook_setup(agent, cx, move |reply, cx| {
            let _ = cx.update_window(handle, |_, window, cx| {
                let _ = launcher.update(cx, |launcher, cx| {
                    let Some(state) = launcher
                        .hook_question
                        .as_ref()
                        .filter(|state| state.pick == pick)
                    else {
                        return;
                    };
                    if matches!(reply, Ok(false))
                        && !launcher.hook_hint_dismissed(state.agent, cx)
                        && launcher
                            .layer
                            .is_some_and(|layer| crate::layers::is_open(layer, window, cx))
                    {
                        let weak = cx.weak_entity();
                        let inline = ask(agent, pick, weak, &mark, window, cx);
                        if let Some(state) = launcher.hook_question.as_mut() {
                            state.inline = Some(inline);
                        }
                    } else {
                        launcher.hook_question = None;
                    }
                    cx.notify();
                });
            });
        });
        cx.notify();
    }

    /// Keep a question while its agent is being configured. Once its answer starts a write, an edit retires that
    /// answer's launch continuation, even when it leaves the agent unchanged.
    pub(super) fn retire_hook_answer(&mut self, cx: &App) {
        if self
            .hook_question
            .as_ref()
            .is_some_and(|state| Some(state.agent) != self.hook_agent())
            || self.hook_write_in_flight(cx)
        {
            self.hook_question = None;
        }
    }

    pub(super) fn hook_inline(&self, cx: &App) -> Option<&Inline<HookAgent>> {
        self.hook_question
            .as_ref()?
            .inline
            .as_ref()
            .filter(|inline| inline.is_open(cx))
    }

    /// The main Launch button must wait for the same decision as its keyboard shortcut.
    pub(super) fn hook_answer_pending(&self, cx: &App) -> bool {
        self.hook_question.as_ref().is_some_and(|state| {
            state
                .inline
                .as_ref()
                .is_none_or(|inline| inline.is_open(cx))
        })
    }

    pub(super) fn hook_write_in_flight(&self, cx: &App) -> bool {
        self.hook_inline(cx)
            .is_some_and(|inline| inline.in_flight(cx))
    }
}

/// Give hook setup the primary slot while keeping initial focus on Cancel. A setting write holds the question
/// open until the helm accepts it, so a refusal leaves the one-off launch available.
fn ask(
    agent: HookAgent,
    pick: u64,
    launcher: WeakEntity<Launcher>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Inline<HookAgent> {
    let enabling = launcher.clone();
    let dismissing = launcher.clone();
    let question = Question::new(
        format!("Set up Restart for {}?", agent.name()),
        Answer::new(
            "Set up and launch",
            HoverHelp::tooltip("Turn on hook setup for this agent on all hosts and launch"),
            Tier::Primary,
        ),
        agent,
    )
    .summary(HostSentence::new("hook-setup-consent").text(agent.consent()))
    .fallible_alternative(
        Answer::new(
            "Don't ask again",
            HoverHelp::tooltip(
                "Dismiss this prompt and the session hint. You can show them again in Settings.",
            ),
            Tier::Neutral,
        ),
        move |agent, attempt, window, cx| {
            save(agent, attempt, pick, &dismissing, false, window, cx)
        },
    )
    .alternative(
        Answer::new(
            "Launch without",
            HoverHelp::tooltip("Launch now and ask again next time"),
            Tier::Neutral,
        ),
        move |_, window, cx| {
            continue_launch(&launcher, pick, Mark::take(cx), window, cx);
        },
    )
    .starts_on_cancel()
    .wait_until_shown();
    prompt::ask_inline_fallible(
        question,
        mark,
        move |agent, attempt, window, cx| save(agent, attempt, pick, &enabling, true, window, cx),
        window,
        cx,
    )
}

/// Clear the matching question before pressing Launch. A one-off answer runs inside the question's own update,
/// and a successful setting write settles its attempt later. Neither can read the question to see if it closed.
fn continue_launch(
    launcher: &WeakEntity<Launcher>,
    pick: u64,
    mark: Mark,
    window: &mut Window,
    cx: &mut App,
) {
    let _ = launcher.update(cx, |launcher, cx| {
        if launcher
            .hook_question
            .as_ref()
            .is_some_and(|state| state.pick == pick)
            && launcher
                .layer
                .is_some_and(|layer| crate::layers::is_open(layer, window, cx))
        {
            launcher.hook_question = None;
            launcher.press_at(mark, window, cx);
        }
    });
}

/// Use the settings operation key for both writes. A refusal leaves the one-off answer available, and success
/// launches only for the enabling answer and only while the same question still owns the draft.
fn save(
    agent: HookAgent,
    attempt: Attempt,
    pick: u64,
    launcher: &WeakEntity<Launcher>,
    enable: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let command = if enable {
        crate::settings::hook_setup_command(agent, true)
    } else {
        crate::settings::dismiss_hook_setup_command(agent)
    };
    let (Some(operations), Ok(command)) = (session::current_operations(cx), command) else {
        attempt.failed("The setting change could not be sent.", cx);
        return;
    };
    let mark = Mark::take(cx);
    let handle = window.window_handle();
    let pending = Rc::new(Cell::new(Some(attempt)));
    let settling = pending.clone();
    let continuing = launcher.clone();
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::preferences(), command, cx, move |settled, cx| {
            let Some(attempt) = settling.take() else {
                return;
            };
            match &settled.outcome {
                Outcome::Refused(refusal) => attempt.failed(
                    HostSentence::new("hook-setup-save-error")
                        .text("Could not save the setting. ")
                        .host(&refusal.words),
                    cx,
                ),
                Outcome::Done(_) | Outcome::Accepted(_) => {
                    attempt.succeeded(cx);
                    if enable {
                        let _ = cx.update_window(handle, |_, window, cx| {
                            continue_launch(&continuing, pick, mark, window, cx);
                        });
                    }
                }
            }
            let _ = continuing.update(cx, |_, cx| cx.notify());
        })
    });
    if let Err(error) = started
        && let Some(attempt) = pending.take()
    {
        attempt.failed(
            match error {
                crate::operations::NotStarted::Busy(_) => {
                    crate::operations::NotStarted::BUSY_REASON
                }
                _ => "The setting change could not be sent.",
            },
            cx,
        );
    }
    let _ = launcher.update(cx, |_, cx| cx.notify());
}
