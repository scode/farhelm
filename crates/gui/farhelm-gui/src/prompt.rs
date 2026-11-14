//! The prompt: the one way to ask the user a question.
//!
//! SPEC_impl.md, "Questions: the prompt". The old GUI's largest bug class was an action that ran on state the user was
//! not shown: a confirmation drawn from one row, answered after the list had moved, acting on another. A [`Question`]
//! is built from the state at the moment it opens and is frozen there: its title, its lines (targets, consequences,
//! preconditions) and the value its confirming answer acts on ([`Question::frozen`]) never change while it is open, and
//! the answer buttons come pre-wired to that frozen value, so feature code never wires an answer itself. Commands name
//! their targets by stable id inside the frozen value, and the helm refuses a target that is gone; the existing
//! per-route refusals of a target that changed since its prompt stay as they are. No generic precondition protocol is
//! added here.
//!
//! The first answer wins: once Cancel or Confirm has run, the other does nothing, and Confirm runs its action at most
//! once (a confirmation that could send a second delete after the first succeeded is a bug the prototype shipped).
//! SPEC.md's "One GUI at a time" lets input delivered before a redraw run as delivered; whichever answer runs acts on the
//! frozen value.
//!
//! Where focus starts follows SPEC.md, "Opening and closing": on the confirming button for a question the user just
//! asked for, on Cancel when that button is disabled, when the action is risky and cannot be undone, or when the
//! question arrived later. Escape and Cancel close it and return focus to where it was; Enter on the dialog itself
//! presses the enabled confirming answer.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, FocusHandle, IntoElement, ParentElement, Render, SharedString,
    Styled, WeakEntity, Window, div, px,
};

use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier};
use crate::focus::Mark;
use crate::host_text::Line;
use crate::layers::{self, Dialog, LayerId, Subject};
use crate::theme;

/// One of a question's two answers, as its button shows it.
#[derive(Clone, Debug)]
pub struct Answer {
    /// The button's label and accessible name.
    pub label: SharedString,
    /// The button's hover help.
    pub help: HoverHelp,
    /// The button's tier: [`Tier::Danger`] for a destructive confirmation, [`Tier::Primary`] for an ordinary one.
    pub tier: Tier,
}

/// A question to ask, frozen at the moment it opens. `T` is what the confirming answer acts on: the targets by stable
/// id and whatever else the action needs, as the user was shown them.
#[derive(Clone)]
pub struct Question<T> {
    title: SharedString,
    summary: Option<Line>,
    lines: Vec<Line>,
    confirm: Answer,
    cancel: Answer,
    confirm_enabled: bool,
    starts_on_cancel: bool,
    keeps_focus_on_empty_click: bool,
    frozen: T,
}

impl<T: Clone + 'static> Question<T> {
    /// A question titled `title` whose confirming answer is `confirm` and acts on `frozen`. Cancel is the other answer.
    pub fn new(title: impl Into<SharedString>, confirm: Answer, frozen: T) -> Self {
        Question {
            title: title.into(),
            summary: None,
            lines: Vec::new(),
            confirm,
            cancel: Answer {
                label: "Cancel".into(),
                help: HoverHelp::tooltip("Close this without doing anything"),
                tier: Tier::Neutral,
            },
            confirm_enabled: true,
            starts_on_cancel: false,
            keeps_focus_on_empty_click: false,
            frozen,
        }
    }

    /// A one-line summary under the title, usually naming the target. A target a host named (a session's title, a
    /// host's name) goes in as a [`HostSentence`](crate::host_text::HostSentence), never as a string made from its raw
    /// value; the title stays the app's own words, since it is the question's accessible name.
    pub fn summary(mut self, summary: impl Into<Line>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// One line of the body: a consequence or a precondition, as the user should see it. Like the summary, a line that
    /// names a host value is a [`HostSentence`](crate::host_text::HostSentence); each sentence needs an id of its own.
    pub fn line(mut self, line: impl Into<Line>) -> Self {
        self.lines.push(line.into());
        self
    }

    /// Whether the confirming answer is available. A disabled one is drawn but does nothing, and focus starts on
    /// Cancel instead.
    pub fn confirm_enabled(mut self, enabled: bool) -> Self {
        self.confirm_enabled = enabled;
        self
    }

    /// Focus starts on Cancel, the harmless answer. SPEC.md, "Opening and closing", asks this in two cases: the action is
    /// risky and cannot be undone (the YOLO confirmation, say), or the question arrived after something the user
    /// started earlier finished, rather than in direct answer to what they just did.
    pub fn starts_on_cancel(mut self) -> Self {
        self.starts_on_cancel = true;
        self
    }

    /// A click on the question's empty space (or its text, which counts as empty space since labels are not
    /// selectable, as the GPUI migration's orchestrator decided on 2026-11-13) keeps focus on the answer in use,
    /// instead of moving it to the question itself. SPEC.md, "Opening and closing", gives this rule to the session row
    /// and header Stop, Restart, Replace and Delete questions, which must call this when they are ported: without it a
    /// click on their empty space focuses the question, and a following Enter presses Confirm. Rename, tab close, the
    /// interrupted card's Replace question and the host update confirmation do not have the rule yet; those leave it
    /// off.
    pub fn keeps_focus_on_empty_click(mut self) -> Self {
        self.keeps_focus_on_empty_click = true;
        self
    }

    /// The value the confirming answer acts on.
    pub fn frozen(&self) -> &T {
        &self.frozen
    }

    /// Whether focus starts on Cancel: asked for, or the confirming answer is disabled.
    fn starts_focus_on_cancel(&self) -> bool {
        self.starts_on_cancel || !self.confirm_enabled
    }
}

/// What a confirming answer does with the frozen value.
type OnConfirm<T> = Box<dyn FnOnce(T, &mut Window, &mut App)>;

/// An open question's view: the frozen question, its answer buttons' focus handles, and the action not yet run.
pub struct Prompt<T: 'static> {
    question: Question<T>,
    on_confirm: Option<OnConfirm<T>>,
    layer: Option<LayerId>,
    answered: bool,
    confirm_focus: FocusHandle,
    cancel_focus: FocusHandle,
}

/// Which answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Which {
    Confirm,
    Cancel,
}

impl<T: Clone + 'static> Prompt<T> {
    /// Run `which`, if no answer has run yet: close the question (returning focus to where it was), then, for Confirm,
    /// hand the frozen value to the action.
    fn answer(&mut self, which: Which, window: &mut Window, cx: &mut Context<Self>) {
        if self.answered || (which == Which::Confirm && !self.question.confirm_enabled) {
            return;
        }
        self.answered = true;
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
        if which == Which::Confirm
            && let Some(on_confirm) = self.on_confirm.take()
        {
            on_confirm(self.question.frozen.clone(), window, cx);
        }
    }
}

impl<T: Clone + 'static> Render for Prompt<T> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let question = &self.question;
        let answer = |which: Which, cx: &mut Context<Self>| {
            let prompt: WeakEntity<Self> = cx.weak_entity();
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                // The prompt may already be gone (answered and closed); then there is nothing to answer.
                let _ = prompt.update(cx, |prompt, cx| prompt.answer(which, window, cx));
            }
        };
        let cancel = Button::new(
            "prompt-cancel",
            question.cancel.label.clone(),
            question.cancel.help.clone(),
            Role::Button,
        )
        .tier(question.cancel.tier)
        .focus(&self.cancel_focus)
        .on_click(answer(Which::Cancel, cx));
        let confirm = Button::new(
            "prompt-confirm",
            question.confirm.label.clone(),
            question.confirm.help.clone(),
            Role::Button,
        )
        .tier(question.confirm.tier)
        .disabled(!question.confirm_enabled)
        .focus(&self.confirm_focus)
        .on_click(answer(Which::Confirm, cx));
        let body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .text_color(theme::color(palette.fg_0))
            .children(question.lines.iter().cloned().map(|line| div().child(line)));
        let mut frame = DialogFrame::new(
            "prompt",
            question.title.clone(),
            body,
            vec![cancel, confirm],
        );
        if let Some(summary) = &question.summary {
            frame = frame.summary(summary.clone());
        }
        frame
    }
}

/// An open question, for tests to answer it the way its buttons do.
#[cfg(test)]
pub(crate) struct Asked<T: 'static> {
    pub layer: LayerId,
    prompt: gpui::Entity<Prompt<T>>,
}

#[cfg(test)]
impl<T: Clone + 'static> Asked<T> {
    /// Press Confirm, as its button does.
    pub fn confirm(&self, window: &mut Window, cx: &mut App) {
        self.prompt
            .update(cx, |prompt, cx| prompt.answer(Which::Confirm, window, cx));
    }

    /// Press Cancel, as its button does.
    pub fn cancel(&self, window: &mut Window, cx: &mut App) {
        self.prompt
            .update(cx, |prompt, cx| prompt.answer(Which::Cancel, window, cx));
    }
}

/// Ask `question` as a dialog, unless the user has moved on since `mark` (then nothing opens). `on_confirm` runs at
/// most once, with the frozen value, if the user confirms. `subject`, if given, closes the question when it is gone.
pub fn ask<T: Clone + 'static>(
    question: Question<T>,
    subject: Option<Subject>,
    mark: &Mark,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    ask_inner(question, subject, mark, on_confirm, window, cx).map(|(layer, _)| layer)
}

/// [`ask`], also handing back the prompt's entity for tests.
fn ask_inner<T: Clone + 'static>(
    question: Question<T>,
    subject: Option<Subject>,
    mark: &Mark,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<(LayerId, gpui::Entity<Prompt<T>>)> {
    let confirm_focus = cx.focus_handle().tab_stop(true);
    let cancel_focus = cx.focus_handle().tab_stop(true);
    let start = if question.starts_focus_on_cancel() {
        cancel_focus.clone()
    } else {
        confirm_focus.clone()
    };
    let enabled = question.confirm_enabled;
    let keeps_focus_on_empty_click = question.keeps_focus_on_empty_click;
    let prompt = cx.new(|_| Prompt {
        question,
        on_confirm: Some(Box::new(on_confirm)),
        layer: None,
        answered: false,
        confirm_focus,
        cancel_focus,
    });
    let weak = prompt.downgrade();
    let main_action: Option<layers::MainAction> = enabled.then(|| {
        Rc::new(move |window: &mut Window, cx: &mut App| {
            let _ = weak.update(cx, |prompt, cx| prompt.answer(Which::Confirm, window, cx));
        }) as layers::MainAction
    });
    let layer = layers::open_dialog(
        Dialog {
            view: prompt.clone().into(),
            container: cx.focus_handle(),
            start,
            subject,
            main_action,
            keeps_focus_on_empty_click,
        },
        mark,
        window,
        cx,
    )?;
    prompt.update(cx, |prompt, _| prompt.layer = Some(layer));
    Some((layer, prompt))
}

/// [`ask`] for tests: also returns the open question so a test can answer it without drawing in between.
#[cfg(test)]
pub(crate) fn ask_for_test<T: Clone + 'static>(
    question: Question<T>,
    subject: Option<Subject>,
    mark: &Mark,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<Asked<T>> {
    ask_inner(question, subject, mark, on_confirm, window, cx)
        .map(|(layer, prompt)| Asked { layer, prompt })
}
