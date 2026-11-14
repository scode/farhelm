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
//! Every answer closes the question before its action runs, with one exception: an extra answer whose action can fail
//! ([`Question::fallible_alternative`]). SPEC.md's YOLO question has one: its "stop asking" answer first changes the
//! host's setting, and "if changing that setting fails, nothing is started and the confirmation stays up with the
//! reason". Such an answer keeps the question open while its action is in flight, with every answer frozen (drawn busy,
//! doing nothing, Escape included), so the first answer still wins. The action settles through the [`Attempt`] it is
//! handed: success closes the question as any answer does, and failure shows the reason inside the question and arms
//! the answers again, so the user can pick again. Focus stays on the answer the user pressed throughout, since a busy
//! button keeps it, and the question never closes and reopens. Only the inline presentation offers it (below).
//!
//! Where focus starts follows SPEC.md, "Opening and closing": on the confirming button for a question the user just
//! asked for, on Cancel when that button is disabled, when the action is risky and cannot be undone, or when the
//! question arrived later. Escape and Cancel close it and return focus to where it was (Escape sends it home instead
//! for a question built with [`Question::returns_home_on_escape`]); Enter on the dialog itself presses the enabled
//! confirming answer.
//!
//! A question shows in one of three ways, with every rule above the same in all. [`ask`] centers it over a veil in the
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
//!
//! [`ask_inline`] hands the question to its owner, which draws it inside its own surface, as the old app drew the
//! launcher's YOLO question in the launcher's body and the template editor's leave question as a band at the editor's
//! foot. It is not a layer: no veil, no header Close, no subject (it goes when its owner stops drawing it), and Tab
//! passes through it as through the rest of the owner's dialog. Where it is drawn, and scrolling it into view, are the
//! owner's; how it looks and answers are the prompt's. Focus moves into it when it opens, onto its starting answer,
//! unless the user has moved on since `mark` (then it shows and waits), and an answer pressed while focus is inside it
//! returns focus to what held it when it opened, inside the owner's dialog; both moves go through the focus
//! coordinator's [`focus::move_within_control`]. Escape while focus is in it means Cancel and is taken there, so the
//! owning dialog stays open; Escape anywhere else in the dialog is the dialog's. Enter presses the focused answer; the
//! question holds no focus of its own, so a press on its empty space does what the owning dialog's empty-space rule
//! says. It draws in one of two looks: plain, the old leave question's text and answers (`.templates-departure`), on
//! whatever band its owner puts it in, or loud ([`Question::loud`]), the old YOLO confirmation's red-edged, red-washed
//! block with its title as a red heading (`.yolo-confirmation`). Decided by the migration's orchestrator on 2026-11-14.
//!
//! NOTE: Two recorded deviations from the old YOLO question's look. The old one drew its last line (the host "asks
//! before every YOLO launch") in the dimmer `--fg-dim`; here every line is drawn alike, since a question's lines carry
//! no emphasis of their own. And a line naming a host mid-sentence can wrap with a word left alone before the host
//! value ("… stops its actions" / "on" / "Local (this machine) without asking you."), where the old one flowed as one
//! paragraph: a [`HostSentence`](crate::host_text::HostSentence) draws each part as a whole block of its own. The fix
//! belongs in `host_text` (one styled text with runs), not here.

use std::rc::Rc;

use gpui::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, WeakEntity, WeakFocusHandle, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::actions::Dismiss;
use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier};
use crate::focus::{self, Mark, ReturnTo};
use crate::host_text::Line;
use crate::layers::{self, Anchor, AnchoredDialog, Dialog, LayerId, Subject};
use crate::theme::{self, metrics};

/// How wide an anchored question may grow before its lines wrap: the old row menu panel's `max-width: 288px`.
const ANCHORED_MAX_WIDTH: f32 = 288.;

/// The plain inline question's line height: the old `.templates-departure p`, 13-pixel text at `line-height: 1.5`.
const INLINE_LINE_HEIGHT: f32 = 19.5;

/// The loud inline question's line height: the old `.yolo-confirmation` paragraphs' `line-height: normal` at the
/// launcher's 16-pixel text, 20 pixels from line to line in the reference pictures. GPUI's default would space them
/// about 26 pixels apart.
const LOUD_LINE_HEIGHT: f32 = 20.;

/// The loud inline question's text size: the old launcher's 16-pixel body text (the web view's default), set here
/// so the question looks the same wherever its owner draws it.
const LOUD_TEXT_SIZE: f32 = 16.;

/// Where a question shown as a layer shows: centered over a veil, or beside the element it is about. An inline
/// question is not a layer, and is asked through [`ask_inline`] instead.
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
    /// The button's tier: [`Tier::Danger`] for a destructive confirmation, [`Tier::Primary`] for an ordinary one,
    /// [`Tier::DangerOutline`] for a permanent answer beside a one-off danger answer.
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
    on_alternatives: Vec<Action<T>>,
    confirm_enabled: bool,
    starts_on_cancel: bool,
    keeps_focus_on_empty_click: bool,
    returns_to: ReturnTo,
    /// Drawn inline in the loud look ([`Question::loud`]).
    loud: bool,
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
            returns_to: ReturnTo::Opener,
            loud: false,
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
        self.on_alternatives.push(Action::Once(Box::new(on_choose)));
        self
    }

    /// One more answer, as [`Question::alternative`], whose action can fail: SPEC.md's YOLO "stop asking" answer, which
    /// changes the host's setting before it starts anything. Picking it does not close the question. `on_choose` runs
    /// with the frozen value and an [`Attempt`], and the question stays open with every answer frozen until the action
    /// settles that attempt: [`Attempt::succeeded`] closes the question as any answer does, and [`Attempt::failed`]
    /// shows the reason inside the question and arms the answers again. A retry runs `on_choose` again, with a new
    /// attempt, which is why it is `Fn`.
    ///
    /// While an attempt is unsettled ([`Inline::in_flight`]), the owner must not go away under it: its dismissal hook
    /// answers [`Dismissal::Stay`](crate::layers::Dismissal::Stay), as an editor whose save is in flight does.
    /// Otherwise Tab out of the question, then Escape or the header Close, would close the launcher, and the action
    /// would run on and start a session the user dismissed.
    ///
    /// Only an inline question ([`ask_inline`]) may carry one: a centered or anchored question with it refuses to open.
    /// An inline question's owner decides its own dismissal and can refuse it while the action runs; a question shown
    /// as a layer can also close on a press outside it or when its subject goes, which nothing can veto.
    pub fn fallible_alternative(
        mut self,
        answer: Answer,
        on_choose: impl Fn(T, Attempt, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.alternatives.push(answer);
        self.on_alternatives
            .push(Action::Fallible(Rc::new(on_choose)));
        self
    }

    /// Drawn inline in the loud look: a red edge and wash, with the title as a red heading and the summary in bold.
    /// SPEC.md asks for a "prominent confirmation" before a YOLO launch, and the old app made it loud on purpose: a
    /// question that looks like the lines around it gets clicked through. The other presentations ignore it.
    pub fn loud(mut self) -> Self {
        self.loud = true;
        self
    }

    /// Escape on the question sends focus home (the selected session's terminal) rather than back to the control that
    /// opened it; every other way it closes (an answer, a press outside it) still returns focus to the opener. SPEC.md,
    /// "Keyboard focus", for the session header's Stop, Restart, Replace and Delete questions: "Escape cancels the
    /// header's … question and returns focus to the terminal". A row's questions return to their opener on Escape too.
    /// While there is no home, or another layer is still open, Escape returns focus to the opener all the same.
    pub fn returns_home_on_escape(mut self) -> Self {
        self.returns_to = ReturnTo::HomeOnEscape;
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

/// What an extra answer does.
enum Action<T> {
    /// Runs once, after the question closed ([`Question::alternative`]).
    Once(OnConfirm<T>),
    /// Runs with the question still open, and again on each retry, until an attempt succeeds
    /// ([`Question::fallible_alternative`]).
    Fallible(OnAttempt<T>),
}

/// What an answer whose action can fail does with the frozen value and the attempt it settles.
type OnAttempt<T> = Rc<dyn Fn(T, Attempt, &mut Window, &mut App)>;

/// Settles an [`Attempt`]: `None` for success, the reason for a failure.
type Settle = Box<dyn FnOnce(Option<Line>, &mut App)>;

/// One run of an answer whose action can fail ([`Question::fallible_alternative`]), which the action settles exactly
/// once, whenever its work is done. Until then the question stays open with every answer frozen.
///
/// Settling takes effect once the update it is called from has returned, so an action may settle at once, from inside
/// its own call. An attempt settled after the question went (its owner stopped drawing it) does nothing. An attempt
/// dropped without settling leaves the question frozen until its owner takes it away: an owner whose work can be
/// cancelled settles the attempt as failed when it is. While it is unsettled, the owner's dismissal hook keeps the
/// owner open ([`Question::fallible_alternative`]).
#[must_use = "an unsettled attempt leaves its question frozen"]
pub struct Attempt {
    settle: Settle,
}

impl Attempt {
    /// The attempt for `prompt`, drawn in `window`.
    fn new<T: Clone + 'static>(prompt: WeakEntity<Prompt<T>>, window: AnyWindowHandle) -> Self {
        Attempt {
            settle: Box::new(move |reason, cx| {
                // Deferred: the action may settle from inside the prompt's own update, which cannot be re-entered.
                cx.defer(move |cx| {
                    let _ = window.update(cx, |_, window, cx| {
                        prompt.update(cx, |prompt, cx| prompt.settle(reason, window, cx))
                    });
                });
            }),
        }
    }

    /// The action did what the answer asked: the question closes as any answer does, returning focus where it was.
    pub fn succeeded(self, cx: &mut App) {
        (self.settle)(None, cx);
    }

    /// The action failed: the question stays up showing `reason` and arms its answers again. `reason` may quote a host
    /// (the helm's refusal): pass it as a [`HostSentence`](crate::host_text::HostSentence), never as a string made
    /// from the raw value.
    pub fn failed(self, reason: impl Into<Line>, cx: &mut App) {
        (self.settle)(Some(reason.into()), cx);
    }
}

/// An open question's view: the frozen question, its answer buttons' focus handles, and the actions not yet run.
pub struct Prompt<T: 'static> {
    question: Question<T>,
    on_confirm: Option<OnConfirm<T>>,
    /// The extra answers' actions, taken out of the question when it opens, each until it runs (a fallible one stays).
    on_alternatives: Vec<Option<Action<T>>>,
    layer: Option<LayerId>,
    /// How it is drawn.
    shown: Shown,
    /// An answer has run (or an attempt succeeded): the question is closed, and every answer does nothing.
    answered: bool,
    /// A fallible answer's attempt is in flight: every answer is frozen until it settles.
    in_flight: bool,
    /// Why the last attempt failed, shown inside the question until the next one starts.
    reason: Option<Line>,
    /// What held focus when an inline question opened, where an answer returns it. Layers keep their own.
    opener: Option<WeakFocusHandle>,
    confirm_focus: FocusHandle,
    cancel_focus: FocusHandle,
    alternative_focus: Vec<FocusHandle>,
}

/// How an open question is drawn, which decides its look and, inline, that it returns focus itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    /// In the dialog frame over a veil ([`ask`]).
    Centered,
    /// On the floating surface beside its anchor ([`ask_anchored`]).
    Anchored,
    /// Inside its owner's surface, which draws it ([`ask_inline`]). Not a layer, so the prompt itself returns focus.
    Inline,
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
    /// Run `which`, if no answer has run yet and no attempt is in flight: close the question (returning focus to its
    /// opener), then, for Confirm or an extra answer, hand the frozen value to its action. A fallible answer instead
    /// starts an attempt and leaves the question open, frozen, until the attempt settles.
    fn answer(&mut self, which: Which, window: &mut Window, cx: &mut Context<Self>) {
        if self.answered
            || self.in_flight
            || (which == Which::Confirm && !self.question.confirm_enabled)
        {
            return;
        }
        if let Which::Alternative(index) = which
            && let Some(Some(Action::Fallible(action))) = self.on_alternatives.get(index)
        {
            let action = action.clone();
            self.in_flight = true;
            self.reason = None;
            cx.notify();
            let attempt = Attempt::new(cx.weak_entity(), window.window_handle());
            action(self.question.frozen.clone(), attempt, window, cx);
            return;
        }
        self.answered = true;
        self.close(window, cx);
        let action = match which {
            Which::Confirm => self.on_confirm.take(),
            Which::Alternative(index) => {
                match self.on_alternatives.get_mut(index).and_then(Option::take) {
                    Some(Action::Once(action)) => Some(action),
                    // A fallible answer started its attempt above; an index past the end has no button.
                    Some(Action::Fallible(_)) | None => None,
                }
            }
            Which::Cancel => None,
        };
        if let Some(action) = action {
            action(self.question.frozen.clone(), window, cx);
        }
    }

    /// The in-flight attempt settled: closed on success (`reason` is `None`), or open again with the reason shown.
    fn settle(&mut self, reason: Option<Line>, window: &mut Window, cx: &mut Context<Self>) {
        // Only an attempt settles, and only while it is the one in flight: each settles once, and the next can start
        // only after the last has settled.
        self.in_flight = false;
        match reason {
            Some(reason) => self.reason = Some(reason),
            None => {
                self.answered = true;
                self.close(window, cx);
            }
        }
        cx.notify();
    }

    /// Take the question down: close its layer, or, inline, stop drawing it and return focus to what held it when it
    /// opened, if focus is on one of its answers.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
        if self.shown == Shown::Inline {
            // The coordinator moves focus only from the answer that holds it, so at most one of these moves happens,
            // and none when the user had already taken focus elsewhere.
            if let Some(opener) = self.opener.as_ref().and_then(WeakFocusHandle::upgrade) {
                for answer in std::iter::once(&self.cancel_focus)
                    .chain(&self.alternative_focus)
                    .chain(std::iter::once(&self.confirm_focus))
                {
                    focus::move_within_control(answer, &opener, window, cx);
                }
            }
            cx.notify();
        }
    }
}

impl<T: Clone + 'static> Render for Prompt<T> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        if self.shown == Shown::Inline && self.answered {
            return div().into_any_element();
        }
        let busy = self.in_flight;
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
        .busy(busy)
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
        .busy(busy)
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
                .busy(busy)
                .focus(focus)
                .on_click(answer(Which::Alternative(index), cx))
            });
        let answers: Vec<Button> = std::iter::once(cancel)
            .chain(alternatives)
            .chain(std::iter::once(confirm))
            .collect();
        if self.shown == Shown::Inline {
            return inline(self, answers, cx);
        }
        if self.shown == Shown::Anchored {
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

/// An inline question, as [`ask_inline`] draws it: the title, the summary and the lines, the reason the last attempt
/// failed (amber, as the old launcher's `.yolo-confirmation-error` was), then the answers right-aligned 8 pixels apart
/// (`.dialog-actions`), in the plain or the loud look (see the module docs). Escape inside it is Cancel, and is taken
/// even while an attempt is in flight (then it does nothing), so it never reaches the owning dialog.
fn inline<T: Clone + 'static>(
    prompt: &Prompt<T>,
    answers: Vec<Button>,
    cx: &mut Context<Prompt<T>>,
) -> gpui::AnyElement {
    let palette = theme::palette(cx);
    let question = &prompt.question;
    let loud = question.loud;
    let reason = prompt.reason.clone().map(|reason| {
        div()
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.warn_bright))
            .child(reason)
    });
    let element = div()
        .id("prompt")
        .debug_selector(|| "prompt".into())
        // The old blocks' roles: `alertdialog` for the YOLO question, `status` for the leave question.
        .role(if loud {
            Role::AlertDialog
        } else {
            Role::Status
        })
        .aria_label(question.title.clone())
        .on_action(
            cx.listener(|prompt, _: &Dismiss, window, cx| prompt.answer(Which::Cancel, window, cx)),
        )
        .flex()
        .flex_col()
        .text_color(theme::color(palette.fg_0))
        .child(div().child(question.title.clone()).when(loud, |heading| {
            heading
                .text_color(theme::color(palette.danger))
                .font_weight(FontWeight::BOLD)
        }))
        .children(question.summary.clone().map(|summary| {
            div()
                .when(loud, |summary| summary.font_weight(FontWeight::SEMIBOLD))
                .child(summary)
        }))
        .children(question.lines.iter().cloned().map(|line| div().child(line)))
        .children(reason)
        .child(
            div()
                .debug_selector(|| "prompt-answers".into())
                .flex()
                .flex_wrap()
                .justify_end()
                .gap(px(8.))
                .children(answers),
        );
    if loud {
        // `.yolo-confirmation`: a 2-pixel red edge on a 14% red wash, 10 by 12 pixels of padding, 6 between lines.
        element
            .gap(px(6.))
            .py(px(10.))
            .px(px(12.))
            .text_size(px(LOUD_TEXT_SIZE))
            .line_height(px(LOUD_LINE_HEIGHT))
            .bg(theme::color(palette.danger_wash))
            .border_2()
            .border_color(theme::color(palette.danger))
            .rounded(metrics::RADIUS_MD)
            .into_any_element()
    } else {
        // `.templates-departure`: 10 pixels between the question and its answers, 13-pixel text at 1.5.
        element
            .gap(px(10.))
            .text_size(metrics::TEXT_MD)
            .line_height(px(INLINE_LINE_HEIGHT))
            .into_any_element()
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
        press(&self.prompt, Which::Confirm, window, cx);
    }

    /// Press Cancel, as its button does.
    pub fn cancel(&self, window: &mut Window, cx: &mut App) {
        press(&self.prompt, Which::Cancel, window, cx);
    }

    /// Press the extra answer at `index`, as its button does.
    pub fn alternative(&self, index: usize, window: &mut Window, cx: &mut App) {
        press(&self.prompt, Which::Alternative(index), window, cx);
    }
}

/// Press `which` on `prompt`, as its button does, for tests that answer without drawing in between.
#[cfg(test)]
fn press<T: Clone + 'static>(
    prompt: &Entity<Prompt<T>>,
    which: Which,
    window: &mut Window,
    cx: &mut App,
) {
    prompt.update(cx, |prompt, cx| prompt.answer(which, window, cx));
}

/// A question its owner draws inside its own surface ([`ask_inline`]). The owner keeps it and draws it as a child where
/// SPEC.md puts it (`.child(question.clone())`) for as long as it wants the question shown; once answered it draws
/// nothing, so an owner may drop it lazily. Dropping it, or no longer drawing it, takes it away unanswered.
pub struct Inline<T: 'static> {
    prompt: Entity<Prompt<T>>,
}

impl<T: 'static> Clone for Inline<T> {
    fn clone(&self) -> Self {
        Inline {
            prompt: self.prompt.clone(),
        }
    }
}

impl<T: Clone + 'static> Inline<T> {
    /// Whether it is still open: no answer has run, and no fallible answer has succeeded. Test-only until an owner
    /// needs it: an owner may simply keep drawing an answered question, which draws nothing.
    #[cfg(test)]
    pub(crate) fn is_open(&self, cx: &App) -> bool {
        !self.prompt.read(cx).answered
    }

    /// Whether an answer whose action can fail has an attempt in flight ([`Question::fallible_alternative`]): while it
    /// does, the owner's dismissal hook answers [`Dismissal::Stay`](crate::layers::Dismissal::Stay), so the owner does
    /// not close under the running action.
    pub fn in_flight(&self, cx: &App) -> bool {
        self.prompt.read(cx).in_flight
    }

    /// Press Confirm, as its button does.
    #[cfg(test)]
    pub(crate) fn confirm(&self, window: &mut Window, cx: &mut App) {
        press(&self.prompt, Which::Confirm, window, cx);
    }

    /// Press Cancel, as its button does.
    #[cfg(test)]
    pub(crate) fn cancel(&self, window: &mut Window, cx: &mut App) {
        press(&self.prompt, Which::Cancel, window, cx);
    }

    /// Press the extra answer at `index`, as its button does.
    #[cfg(test)]
    pub(crate) fn alternative(&self, index: usize, window: &mut Window, cx: &mut App) {
        press(&self.prompt, Which::Alternative(index), window, cx);
    }
}

impl<T: Clone + 'static> IntoElement for Inline<T> {
    type Element = <Entity<Prompt<T>> as IntoElement>::Element;

    fn into_element(self) -> Self::Element {
        self.prompt.into_element()
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

/// Ask `question` inside its owner's surface: the owner draws what this returns where SPEC.md puts the question (the
/// launcher's body for the YOLO question, the template editor's foot for its leave question), and must draw it from the
/// next frame on, or focus, which moves to its starting answer at once, finds nothing there and recovers elsewhere. It
/// opens even when the user has moved on since `mark`, since it shows where it belongs; then it does not take focus,
/// and waits to be reached. `on_confirm` runs at most once, with the frozen value, if the user confirms. See the module
/// docs for focus, Escape and the looks.
///
/// [`Question::keeps_focus_on_empty_click`] and [`Question::returns_home_on_escape`] do not apply here: an inline
/// question's empty space belongs to its owner's dialog, which has its own rule, and Escape on it keeps focus in that
/// dialog, returning it as any answer does. On the page, with no dialog open (a plain Replace's YOLO question), the
/// same holds against the page: Escape cancels and returns focus to what held it when the question opened, and a press
/// on the question's empty space is a press on the page's empty space, which sends focus home.
pub fn ask_inline<T: Clone + 'static>(
    question: Question<T>,
    mark: &Mark,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Inline<T> {
    let (prompt, start) = build(question, Shown::Inline, on_confirm, cx);
    let opener = window.focused(cx);
    prompt.update(cx, |prompt, _| {
        prompt.opener = opener.as_ref().map(FocusHandle::downgrade)
    });
    if !focus::has_moved_on(mark, cx) {
        if layers::focus_layer_open(window, cx) {
            // Inside the owner's dialog: from what holds focus there to the question's starting answer beside it.
            if let Some(opener) = &opener {
                focus::move_within_control(opener, &start, window, cx);
            }
        } else {
            // On the page, with no layer open: the ordinary move after something the user did.
            focus::focus_control(&start, mark, window, cx);
        }
    }
    Inline { prompt }
}

/// [`ask`] or [`ask_anchored`], also handing back the prompt's entity for tests.
fn ask_inner<T: Clone + 'static>(
    question: Question<T>,
    presentation: Presentation,
    subject: Option<Subject>,
    mark: &Mark,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<(LayerId, gpui::Entity<Prompt<T>>)> {
    if question
        .on_alternatives
        .iter()
        .any(|action| matches!(action, Action::Fallible(_)))
    {
        tracing::error!(
            "a question with an answer that can fail was asked as a layer; only an inline question may carry one"
        );
        return None;
    }
    let enabled = question.confirm_enabled;
    let keeps_focus_on_empty_click = question.keeps_focus_on_empty_click;
    let returns_to = question.returns_to;
    let shown = match presentation {
        Presentation::Centered => Shown::Centered,
        Presentation::Anchored(_) => Shown::Anchored,
    };
    let (prompt, start) = build(question, shown, on_confirm, cx);
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
                returns_to,
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
                returns_to,
            },
            mark,
            window,
            cx,
        ),
    }?;
    prompt.update(cx, |prompt, _| prompt.layer = Some(layer));
    Some((layer, prompt))
}

/// The open prompt for `question`, drawn as `shown`, with its answers' focus handles, and the handle focus starts on.
fn build<T: Clone + 'static>(
    mut question: Question<T>,
    shown: Shown,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    cx: &mut App,
) -> (Entity<Prompt<T>>, FocusHandle) {
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
    let prompt = cx.new(|_| Prompt {
        question,
        on_confirm: Some(Box::new(on_confirm)),
        on_alternatives,
        layer: None,
        shown,
        answered: false,
        in_flight: false,
        reason: None,
        opener: None,
        confirm_focus,
        cancel_focus,
        alternative_focus,
    });
    (prompt, start)
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
