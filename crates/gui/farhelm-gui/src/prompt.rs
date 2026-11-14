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
//! The first answer wins: once one answer has run, the others do nothing, and an answer's action runs at most once (a
//! confirmation that could send a second delete after the first succeeded is a bug the prototype shipped). SPEC.md's
//! "One GUI at a time" lets input delivered before a redraw run as delivered; whichever answer runs acts on the frozen
//! value.
//!
//! A question may carry more answers ([`Question::alternative`]), drawn in the order added between Cancel and Confirm:
//! the template editor's "Keep editing, Discard, or Save" (SPEC.md: "in that reading order, with initial focus still on
//! Save"), the old host setup confirmation's "Set up, don't ask again" between Cancel and Set up, and SPEC.md's YOLO
//! confirmation, "Cancel, the guarded option when offered, the outlined permanent YOLO answer, then the filled one-time
//! YOLO answer". They follow the same rules: each acts on the frozen value, they are among the answers the first one
//! wins among, and focus never starts on one. Cancel and Confirm keep their places at the ends, as every question
//! SPEC.md asks has them.
//!
//! NOTE: Every answer closes the question before its action runs. SPEC.md's YOLO question stays up with the reason when
//! its "stop asking" answer fails to change the host's setting; that, and drawing a question inline in its owning
//! surface (the YOLO question, the template editor's leave question), are not built yet.
//!
//! Where focus starts follows SPEC.md, "Opening and closing": on the confirming button for a question the user just
//! asked for, on Cancel when that button is disabled, when the action is risky and cannot be undone, or when the
//! question arrived later. Escape and Cancel close it and return focus to where it was; Enter on the dialog itself
//! presses the enabled confirming answer.
//!
//! A question shows in one of two ways, with every rule above the same in both. [`ask`] centers it over a veil in the
//! dialog frame. [`ask_anchored`] draws it beside the control it is about, as the old app did for the questions a row
//! or the session header asked: the row's Delete, Stop and Replace questions took the place of the row's menu, and the
//! header's sat under their buttons (`list/row.rs` and the `.header-confirm` rule at reference commit `24bc0580`). An
//! anchored question has no title line or veil: its lines in amber (`--warn-bright`), then the answers, on the
//! floating surface menus use. Its title stays its accessible name. A press outside it closes it, which is Cancel, and
//! it closes when its anchor is no longer drawn (the row went away), besides when its subject is gone.
//!
//! NOTE: The old anchored questions kept the quoted title on one line and cut it with an ellipsis. Here lines wrap, as
//! they do in the centered form: two sessions whose titles share a long start would otherwise get questions that look
//! the same, which is the confusion escaping host text exists to prevent. That is a recorded deviation from the old
//! look. An anchored question sits where its anchor asks: below it (or above, without room), or beside it for an
//! anchor made with [`crate::layers::Anchor::beside`], which is where the old row questions sat.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled, WeakEntity,
    Window, div, px,
};

use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::Line;
use crate::layers::{self, Anchor, AnchoredDialog, Dialog, LayerId, Subject};
use crate::theme::{self, metrics};

/// How wide an anchored question may grow before its lines wrap: the old row menu panel's `max-width: 288px`.
const ANCHORED_MAX_WIDTH: f32 = 288.;

/// Where a question shows: centered over a veil, or beside the element it is about.
#[derive(Clone)]
pub(crate) enum Presentation {
    /// In the dialog frame, centered over the window ([`ask`]).
    Centered,
    /// Against `Anchor`'s element, where the anchor asks (below unless it was made [`Anchor::beside`]), following it
    /// ([`ask_anchored`]).
    Anchored(Anchor),
}

/// One of a question's answers, as its button shows it.
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
pub struct Question<T> {
    title: SharedString,
    summary: Option<Line>,
    lines: Vec<Line>,
    confirm: Answer,
    cancel: Answer,
    /// The extra answers between Cancel and Confirm, in reading order ([`Question::alternative`]).
    alternatives: Vec<Answer>,
    /// What each extra answer does, handed to the open prompt when the question opens.
    on_alternatives: Vec<OnConfirm<T>>,
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
            alternatives: Vec::new(),
            on_alternatives: Vec::new(),
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

    /// Cancel's button shown as `answer` instead of the default "Cancel" with generic help: the template editor's leave
    /// question says "Keep editing", and the old Stop, Replace and Restart questions said in Cancel's help what it
    /// keeps ("Cancel: keep the agent and terminal tabs running"). It still does what Cancel does, closing the question
    /// and running nothing, and Escape still means it.
    pub fn cancel_answer(mut self, answer: Answer) -> Self {
        self.cancel = answer;
        self
    }

    /// One more answer, `answer`, drawn between Cancel and Confirm after any added before it: `on_choose` runs at most
    /// once, with the frozen value, if the user picks it, and like Confirm it closes the question first.
    pub fn alternative(
        mut self,
        answer: Answer,
        on_choose: impl FnOnce(T, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.alternatives.push(answer);
        self.on_alternatives.push(Box::new(on_choose));
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

/// An open question's view: the frozen question, its answer buttons' focus handles, and the actions not yet run.
pub struct Prompt<T: 'static> {
    question: Question<T>,
    on_confirm: Option<OnConfirm<T>>,
    /// The extra answers' actions, taken out of the question when it opens, each until it runs.
    on_alternatives: Vec<Option<OnConfirm<T>>>,
    layer: Option<LayerId>,
    /// Drawn beside an anchor rather than in the dialog frame.
    anchored: bool,
    answered: bool,
    confirm_focus: FocusHandle,
    cancel_focus: FocusHandle,
    alternative_focus: Vec<FocusHandle>,
}

/// Which answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Which {
    Confirm,
    Cancel,
    /// The extra answer at this index ([`Question::alternative`]).
    Alternative(usize),
}

impl<T: Clone + 'static> Prompt<T> {
    /// Run `which`, if no answer has run yet: close the question (returning focus to where it was), then, for Confirm
    /// or an extra answer, hand the frozen value to its action.
    fn answer(&mut self, which: Which, window: &mut Window, cx: &mut Context<Self>) {
        if self.answered || (which == Which::Confirm && !self.question.confirm_enabled) {
            return;
        }
        self.answered = true;
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
        let action = match which {
            Which::Confirm => self.on_confirm.take(),
            Which::Alternative(index) => self.on_alternatives.get_mut(index).and_then(Option::take),
            Which::Cancel => None,
        };
        if let Some(action) = action {
            action(self.question.frozen.clone(), window, cx);
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
        // The extra answers sit between Cancel and Confirm, in the order added, in reading order and in Tab's order.
        let alternatives = question
            .alternatives
            .iter()
            .zip(&self.alternative_focus)
            .enumerate()
            .map(|(index, (alternative, focus))| {
                Button::new(
                    SharedString::from(format!("prompt-alternative-{index}")),
                    alternative.label.clone(),
                    alternative.help.clone(),
                    Role::Button,
                )
                .tier(alternative.tier)
                .focus(focus)
                .on_click(answer(Which::Alternative(index), cx))
            });
        let answers: Vec<Button> = std::iter::once(cancel)
            .chain(alternatives)
            .chain(std::iter::once(confirm))
            .collect();
        if self.anchored {
            // `.session-row-menu-panel.menu-prompt` and `.confirm-consequence`: 8 pixels of padding, 6 between
            // lines, 12-pixel amber prose, and the answers right-aligned 8 pixels apart (`.dialog-actions`).
            let lines = question.summary.iter().chain(&question.lines).cloned();
            return layers::surface(cx)
                .id("prompt")
                .debug_selector(|| "prompt".into())
                .role(Role::Dialog)
                .aria_label(question.title.clone())
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(8.))
                .max_w(px(ANCHORED_MAX_WIDTH))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.warn_bright))
                .children(lines.map(|line| div().child(line)))
                .child(
                    div()
                        .debug_selector(|| "prompt-answers".into())
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .gap(px(8.))
                        .children(answers),
                )
                .into_any_element();
        }
        let body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .text_color(theme::color(palette.fg_0))
            .children(question.lines.iter().cloned().map(|line| div().child(line)));
        let mut frame = DialogFrame::new("prompt", question.title.clone(), body, answers);
        if let Some(summary) = &question.summary {
            frame = frame.summary(summary.clone());
        }
        frame.into_any_element()
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

    /// Press the extra answer at `index`, as its button does.
    pub fn alternative(&self, index: usize, window: &mut Window, cx: &mut App) {
        self.prompt.update(cx, |prompt, cx| {
            prompt.answer(Which::Alternative(index), window, cx)
        });
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
    ask_inner(
        question,
        Presentation::Centered,
        subject,
        mark,
        on_confirm,
        window,
        cx,
    )
    .map(|(layer, _)| layer)
}

/// Ask `question` beside the element `anchor` belongs to (a row's menu button, a header button), with the same rules
/// as [`ask`]. Nothing opens when the user has moved on since `mark` or the anchor is no longer drawn. It also closes
/// when the anchor goes, or on a press outside it, without answering.
pub fn ask_anchored<T: Clone + 'static>(
    question: Question<T>,
    anchor: Anchor,
    subject: Option<Subject>,
    mark: &Mark,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    ask_inner(
        question,
        Presentation::Anchored(anchor),
        subject,
        mark,
        on_confirm,
        window,
        cx,
    )
    .map(|(layer, _)| layer)
}

/// [`ask`] or [`ask_anchored`], also handing back the prompt's entity for tests.
fn ask_inner<T: Clone + 'static>(
    mut question: Question<T>,
    presentation: Presentation,
    subject: Option<Subject>,
    mark: &Mark,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<(LayerId, gpui::Entity<Prompt<T>>)> {
    let confirm_focus = cx.focus_handle().tab_stop(true);
    let cancel_focus = cx.focus_handle().tab_stop(true);
    let alternative_focus = question
        .alternatives
        .iter()
        .map(|_| cx.focus_handle().tab_stop(true))
        .collect();
    let on_alternatives = std::mem::take(&mut question.on_alternatives)
        .into_iter()
        .map(Some)
        .collect();
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
        on_alternatives,
        layer: None,
        anchored: matches!(presentation, Presentation::Anchored(_)),
        answered: false,
        confirm_focus,
        cancel_focus,
        alternative_focus,
    });
    let weak = prompt.downgrade();
    let main_action: Option<layers::MainAction> = enabled.then(|| {
        Rc::new(move |window: &mut Window, cx: &mut App| {
            let _ = weak.update(cx, |prompt, cx| prompt.answer(Which::Confirm, window, cx));
        }) as layers::MainAction
    });
    let view = prompt.clone().into();
    let container = cx.focus_handle();
    let layer = match presentation {
        Presentation::Centered => layers::open_dialog(
            Dialog {
                view,
                container,
                start,
                subject,
                main_action,
                keeps_focus_on_empty_click,
                returns_to: ReturnTo::Opener,
                on_dismiss: None,
                placement: layers::DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            mark,
            window,
            cx,
        ),
        Presentation::Anchored(anchor) => layers::open_anchored(
            AnchoredDialog {
                view,
                container,
                start,
                anchor,
                subject,
                main_action,
                keeps_focus_on_empty_click,
            },
            mark,
            window,
            cx,
        ),
    }?;
    prompt.update(cx, |prompt, _| prompt.layer = Some(layer));
    Some((layer, prompt))
}

/// [`ask`] or [`ask_anchored`] for tests, shown as `presentation`: also returns the open question so a test can answer
/// it without drawing in between.
#[cfg(test)]
pub(crate) fn ask_for_test<T: Clone + 'static>(
    presentation: Presentation,
    question: Question<T>,
    subject: Option<Subject>,
    mark: &Mark,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<Asked<T>> {
    ask_inner(
        question,
        presentation,
        subject,
        mark,
        on_confirm,
        window,
        cx,
    )
    .map(|(layer, prompt)| Asked { layer, prompt })
}
