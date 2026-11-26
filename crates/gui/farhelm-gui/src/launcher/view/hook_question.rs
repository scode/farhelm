//! The launcher asks for lasting consent only after a deliberate pick and a successful read of an off switch.
//! An accepted setting write belongs to the helm. Its launch continuation belongs to the draft that was answered.

use std::{cell::Cell, rc::Rc};

use farhelm_proto::LaunchTarget;
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{App, AppContext as _, Context, WeakEntity, Window};

use super::{Launcher, SourceConsent};
use crate::components::{HoverHelp, Tier};
use crate::focus::Mark;
use crate::hook_setup::HookAgent;
use crate::host_text::HostSentence;
use crate::launcher::draft::{FolderDefault, Install, Tab};
use crate::launcher::request::Preview;
use crate::operations::{OpKey, Outcome};
use crate::prompt::{self, Answer, Attempt, Inline, Question};
use crate::session;

/// A read and its resulting question share the pick number. A slow reply cannot answer a later pick of the same agent.
pub(super) struct HookQuestion {
    agent: HookAgent,
    tab: Tab,
    pick: u64,
    inline: Option<Inline<HookAnswer>>,
}

/// The setting can save even when the displayed draft cannot launch. The launch half of an answer belongs to the
/// destination and source warning shown together when the user pressed it.
#[derive(Clone, PartialEq)]
pub(super) struct HookAnswer {
    agent: HookAgent,
    shown_launch: Option<ShownHookLaunch>,
}

/// A launch that was available in the frame with the hook question. A resumable host can be available before it has
/// a connection, so a compiled request alone cannot record that a wake was offered.
#[derive(Clone, PartialEq)]
struct ShownHookLaunch {
    source: SourceConsent,
    destination: HookDestination,
}

/// Use the actual placement for a new environment. An existing host's ordinary folder follows its default until the
/// user edits it. A managed checkout uses its previewed directory, or its naming policy when the host needs a wake.
#[derive(Clone, PartialEq)]
enum HookDestination {
    Disposable(LaunchTarget),
    Host {
        installation: Install,
        checkout: bool,
        folder: String,
        folder_default: Option<FolderDefault>,
        can_wake: bool,
    },
}

impl HookQuestion {
    /// Keep the unanswered question in step with the launch drawn beside it. A save in flight keeps the earlier
    /// snapshot even if a read settles in the meantime.
    pub(super) fn show_launch(
        &self,
        launcher: &Launcher,
        source: SourceConsent,
        ready: bool,
        cx: &mut App,
    ) {
        if let Some(inline) = &self.inline {
            let shown_launch =
                ready
                    .then(|| launcher.hook_destination(cx))
                    .flatten()
                    .map(|destination| ShownHookLaunch {
                        source,
                        destination,
                    });
            inline.refresh_value(
                HookAnswer {
                    agent: self.agent,
                    shown_launch,
                },
                cx,
            );
        }
    }
}

/// Each tab retains its own agent choice while another tab is active.
fn offer_slot(tab: Tab) -> usize {
    match tab {
        Tab::Agent => 0,
        Tab::Command => 1,
    }
}

impl Launcher {
    /// Describe an available launch after the caller settles its default folder and checks its refusals. Use the
    /// presented path for a connected checkout. A sleeping checkout instead shows the name it will allocate on wake.
    fn hook_destination(&self, cx: &App) -> Option<HookDestination> {
        Some(if let Some(target) = &self.draft.target {
            HookDestination::Disposable(target.clone())
        } else {
            let host = self.chosen_host(cx)?;
            let can_wake = self.draft.can_wake(host);
            let checkout = self.draft.checkout.is_some();
            let folder = if !checkout {
                self.draft.folder.value().to_owned()
            } else if can_wake {
                self.draft.checkout_title()
            } else if let Preview::Ready(preview) = self.checkout_preview(cx) {
                preview.binding.cwd.clone()
            } else {
                return None;
            };
            HookDestination::Host {
                installation: Install::of(host),
                checkout,
                folder,
                // History can settle the retained default while an explicitly chosen folder stays ready.
                folder_default: self
                    .draft
                    .folder_is_the_default()
                    .then_some(self.draft.folder_default),
                can_wake,
            }
        })
    }

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

    /// Recheck an unanswered offer when a tab brings back the same agent. Old replies stay retired.
    pub(super) fn switch_tab(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.draft.tab;
        self.draft.choose_tab(tab);
        self.changed(cx);
        if self.draft.tab != previous && self.hook_offer_on_return[offer_slot(tab)].is_some() {
            self.check_hook_question(window, cx);
        }
    }

    /// Check once per explicit choice. A failed read leaves ordinary launching available and never offers a write.
    pub(super) fn check_hook_question(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.draft.tab;
        self.hook_offer_on_return[offer_slot(tab)] = None;
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
            tab,
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

    /// Keep a question with its original tab and agent. A tab change can preserve an unanswered offer, including
    /// when search changes tabs. An edit retires an answered question's launch continuation during its write.
    pub(super) fn retire_hook_answer(&mut self, cx: &App) {
        let agent = self.hook_agent();
        let parked = &mut self.hook_offer_on_return[offer_slot(self.draft.tab)];
        // Restoring a different agent from recents replaces the choice that owned the parked offer.
        if *parked != agent {
            *parked = None;
        }
        let Some(question) = self.hook_question.as_ref() else {
            return;
        };
        let writing = self.hook_write_in_flight(cx);
        let same_tab = question.tab == self.draft.tab;
        if same_tab && Some(question.agent) == agent && !writing {
            return;
        }
        if !same_tab && !writing && self.hook_answer_pending(cx) {
            self.hook_offer_on_return[offer_slot(question.tab)] = Some(question.agent);
        }
        self.hook_question = None;
    }

    pub(super) fn hook_inline(&self, cx: &App) -> Option<&Inline<HookAnswer>> {
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
) -> Inline<HookAnswer> {
    let enabling = launcher.clone();
    let dismissing = launcher.clone();
    let question = Question::new(
        format!("Set up Restart for {}?", agent.name()),
        Answer::new(
            "Set up and launch",
            HoverHelp::tooltip("Turn on hook setup for this agent on all hosts and launch"),
            Tier::Primary,
        ),
        HookAnswer {
            agent,
            shown_launch: None,
        },
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
        move |answer, attempt, window, cx| {
            save(answer, attempt, pick, &dismissing, false, window, cx)
        },
    )
    .alternative(
        Answer::new(
            "Launch without",
            HoverHelp::tooltip("Launch now and ask again next time"),
            Tier::Neutral,
        ),
        move |answer, window, cx| {
            continue_launch(
                &launcher,
                pick,
                answer.shown_launch,
                false,
                Mark::take(cx),
                window,
                cx,
            );
        },
    )
    .starts_on_cancel()
    .wait_until_shown();
    prompt::ask_inline_fallible(
        question,
        mark,
        move |answer, attempt, window, cx| save(answer, attempt, pick, &enabling, true, window, cx),
        window,
        cx,
    )
}

/// Spend only the launch shown with this answer. A history read or a new selected session can settle the default
/// before the next frame, so compare after settling it. A one-off answer runs inside the question's own update and
/// a save settles later. In both cases identify the question by its pick without reading the inline prompt itself.
/// Tell the user a confirmed save succeeded even when the launch needs a fresh press.
fn continue_launch(
    launcher: &WeakEntity<Launcher>,
    pick: u64,
    shown_launch: Option<ShownHookLaunch>,
    setting_saved: bool,
    mark: Mark,
    window: &mut Window,
    cx: &mut App,
) {
    let _ = launcher.update(cx, |launcher, cx| {
        if !launcher
            .layer
            .is_some_and(|layer| crate::layers::is_open(layer, window, cx))
        {
            return;
        }
        let same_question = launcher
            .hook_question
            .as_ref()
            .is_some_and(|state| state.pick == pick);
        if !same_question {
            // An edit retires the pending launch, but the successful host setting still belongs to this form.
            if setting_saved {
                launcher.hook_setup_saved = Some(true);
                cx.notify();
            }
            return;
        }
        launcher.hook_question = None;
        if launcher.draft.target.is_none() && launcher.settle_folder_default(cx) {
            launcher.sync_fields(window, cx);
        }
        launcher.refresh_custom_capabilities(cx);
        let ready = launcher.refusal(cx).is_none();
        if let Some(shown) = shown_launch.as_ref()
            && ready
            && launcher.hook_destination(cx).as_ref() == Some(&shown.destination)
        {
            // The helm checks this guard against the source it finds. A newer warning cannot widen the answer.
            launcher.press_at(Some(shown.source), mark, window, cx);
        } else if setting_saved {
            launcher.hook_setup_saved = Some(shown_launch.is_some() || ready);
        }
        cx.notify();
    });
}

/// Use the settings operation key for both writes. A refusal leaves the one-off answer available, and success
/// launches only for the enabling answer and only while the same question still owns the draft.
fn save(
    answer: HookAnswer,
    attempt: Attempt,
    pick: u64,
    launcher: &WeakEntity<Launcher>,
    enable: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let command = if enable {
        crate::settings::hook_setup_command(answer.agent, true)
    } else {
        crate::settings::dismiss_hook_setup_command(answer.agent)
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
                            continue_launch(
                                &continuing,
                                pick,
                                answer.shown_launch,
                                true,
                                mark,
                                window,
                                cx,
                            );
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
