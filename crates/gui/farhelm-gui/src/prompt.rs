//! The prompt: the one way to ask the user a question.
//!
//! SPEC_impl.md, "Questions: the prompt". The old GUI's largest bug class was an action that ran on state the user was
//! not shown: a confirmation drawn from one row, answered after the list had moved, acting on another. A [`Question`]
//! is built from the state at the moment it opens and is frozen there: its title, its lines (targets, consequences,
//! preconditions) and the value its confirming answer acts on ([`Question::frozen`]) never change while it is open,
//! except through a reword (below), and the answer buttons come pre-wired to that frozen value, so feature code never
//! wires an answer itself. Commands name their targets by stable id inside the frozen value, and the helm refuses a
//! target that is gone; the existing per-route refusals of a target that changed since its prompt stay as they are. No
//! generic precondition protocol is added here.
//!
//! A centered question asked through [`ask_rewordable`] can be reworded while it is open ([`Open::reword`]): SPEC.md's
//! bulk Stop and Delete question rewords itself when a target it lists changes, and closing it to ask the replacement
//! moved focus off the answer the user was on and could take focus out of an approval card beside it. A reword swaps
//! the words and the frozen value together, so what is drawn and what the answers act on still match, and it keeps
//! everything else: the same layer, the same answers with their focus handles, focus where it is, the body's scroll
//! position. What it guards against is the answer the user gave to the old words: an answer other than Cancel that
//! arrives after a reword and before a frame showing the new words has gone on screen does nothing (SPEC.md: "An old
//! answer cannot authorize a replacement question"), since its press was aimed at a question that no longer exists.
//! Shown, not drawn: GPUI draws a dirty window before it handles a key, and shows that frame only at the next frame,
//! so a key queued behind a reword is handled with the new words drawn but not yet on screen.
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
//! Every answer closes the question before its action runs, with one exception: an answer whose action can fail, an
//! extra one ([`Question::fallible_alternative`]) or the confirming one ([`ask_fallible`]). SPEC.md's YOLO question has
//! an extra one: its "stop asking" answer first changes the host's setting, and "if changing that setting fails,
//! nothing is started and the confirmation stays up with the reason". The host remove confirmation's Remove is a
//! confirming one, as the old dialog stayed up through its request and showed the helm's refusal inside it. Such an
//! answer keeps the question open while its action is in flight, with every answer frozen (drawn busy, doing nothing,
//! Escape and any other dismissal included), so the first answer still wins. The action settles through the [`Attempt`]
//! it is handed: success closes the question as any answer does, and failure shows the reason inside the question and
//! arms the answers again, so the user can pick again. Focus stays on the answer the user pressed while the attempt
//! runs, since a busy button keeps it, and the question never closes and reopens. After a failure an inline question
//! leaves focus there. A centered one moves it to Cancel (every question has one), as the old host remove dialog did
//! after a refusal: Enter or Escape then dismisses the question instead of Enter sending the refused action again, and
//! a retry is a press on the answer again. It moves focus only while focus is still on the answer the attempt started
//! from, so a user who tabbed or clicked elsewhere meanwhile keeps their place (SPEC_impl.md, "Questions: the prompt",
//! records this narrow departure from SPEC.md's "Who may move focus"). An extra answer that can fail may sit on an
//! inline question or on a centered one built with [`Question::keeps_focus_on_empty_click`]; a confirming answer that
//! can fail only on the centered one, through [`ask_fallible`] (an inline question's confirming answer runs once).
//! [`Question::fallible_alternative`] says why.
//!
//! A centered question that needs more than lines of text (the host setup confirmation's numbered plan with its Copy
//! plan button, the host remove confirmation's explanation) carries a body the feature draws ([`Question::body`]), and
//! the host dialogs' header tile, footer note and width ([`Question::tile`], [`Question::note`], [`Question::width`]).
//! The body is a view built once, as the question is asked or reworded, from the frozen value; its own controls are
//! not answers.
//! The anchored and inline questions do not take them (below). Approved by the GPUI migration's orchestrator on
//! 2026-11-14.
//!
//! A title is usually the app's own words, but may name a host value, as the host setup confirmation's "Set up host ·
//! <name>" does: then it is a [`HostSentence`](crate::host_text::HostSentence) ([`Question::new`]). The centered and
//! inline questions draw it through the host-text element, and in every presentation the question's accessible name is
//! the sentence's accessible name, with the host value escaped as the screen shows it. Approved by the GPUI migration's
//! orchestrator on 2026-11-14.
//!
//! Where focus starts follows SPEC.md, "Opening and closing": on the confirming button for a question the user just
//! asked for, on Cancel when that button is disabled, when the action is risky and cannot be undone, or when the
//! question arrived later. Escape and Cancel close it and return focus to where it was (Escape sends it home instead
//! for a question built with [`Question::returns_home_on_escape`], and every close sends it to a named control for one
//! built with [`Question::returns_to`]); Enter on the dialog itself presses the enabled confirming answer.
//!
//! A centered question whose text can outgrow the dialog (the bulk question's row per session) can give it a keyboard
//! scroll stop ([`Question::scroll_stop`]), the dialog frame's own, since SPEC.md keeps Tab within a modal's "visible,
//! enabled controls and keyboard scroll regions" and otherwise the keyboard could not bring the lines below the fold
//! into view before answering for them.
//!
//! A question shows in one of three ways, with every rule above the same in all. [`ask`] centers it over a veil in the
//! dialog frame. [`ask_anchored`] draws it beside the control it is about, as the old app did for the questions a row
//! or the session header asked: the row's Delete, Stop and Replace questions took the place of the row's menu, and the
//! header's sat under their buttons (`list/row.rs` and the `.header-confirm` rule at reference commit `24bc0580`). An
//! anchored question has no title line or veil: its lines in amber (`--warn-bright`), then the answers, on the floating
//! surface menus use. Its title is not drawn but stays its accessible name. A press outside it closes it, which is
//! Cancel, and it closes when its anchor is no longer drawn (the row went away), besides when its subject is gone.
//!
//! NOTE: The old anchored questions kept the quoted title on one line and cut it with an ellipsis. Here lines wrap, as
//! they do in the centered form: two sessions whose titles share a long start would otherwise get questions that look
//! the same, which is the confusion escaping host text exists to prevent. That is a recorded deviation from the old
//! look. An anchored question sits where its anchor asks: below it (or above, without room), or beside it for an
//! anchor made with [`crate::layers::Anchor::beside`], which is where the old row questions sat.
//!
//! Wrapping has a cost the old cut did not: a session title can run to tens of thousands of characters, and quoted in
//! full it wrapped into hundreds of lines and pushed Cancel and Confirm off the window, where a pointer user could not
//! answer at all. So an anchored question's text, its summary and lines, stops at about twelve lines
//! ([`ANCHORED_TEXT_MAX_HEIGHT`]), with the answers below it always in view. Only the lines that name a host value give
//! up height to the cap, each scrolling with the wheel inside what is left; the app's own lines are always drawn
//! whole, wherever they sit, since they carry the consequences (the Delete question's managed-checkout line follows the
//! quoted title). This is the old app's own case, terminal.spec's multi-KB unbroken title, whose Delete question "keeps
//! the consequence text intact and clips only the title". The cap is the prompt's, not the layer manager's: the shared
//! clamp keeps a popover's corner inside the window's content area but does not bound its height, so a card taller
//! than the area still ran past its bottom. A fixed cap is enough because the capped card is far shorter than the
//! shortest content area. The keyboard reaches what the cap hides too, as SPEC.md's "Reaching everything by keyboard"
//! asks, without a stop of its own: Page Up, Page Down, Home and End from a focused answer scroll the first host-value
//! line ([`scroll_from_controls`], as a dialog's notices region scrolls around its answers), while the arrows, Enter,
//! Escape and Tab stay the answers'. Decided by the migration's orchestrator on 2026-11-16.
//!
//! [`ask_inline`] hands the question to its owner, which draws it inside its own surface, as the old app drew the
//! launcher's YOLO question in the launcher's body and the template editor's leave question as a band at the editor's
//! foot. It is not a layer: no veil, no header Close, no subject (it goes when its owner stops drawing it), and Tab
//! passes through it as through the rest of the owner's dialog. Where it is drawn is the owner's; how it looks and
//! answers are the prompt's. Focus moves into it when it opens, onto its starting answer, unless the user has moved on
//! since `mark` (then it shows and waits), and an answer pressed while focus is inside it, or clicked, returns focus to
//! what held it when it opened, inside the owner's dialog. A click counts wherever focus is when the answer runs: on
//! macOS a click focuses no button, so its press falls through to the owning dialog's empty-space rule, and in a dialog
//! whose empty space takes focus (Templates, Trash) focus is on the dialog itself by then. Inside a dialog both moves
//! go through the focus coordinator's [`focus::focus_in_layer`], which waits for the frame that first draws the
//! starting answer and scrolls it into view as Tab would, so the owner scrolls nothing itself; on the page they go
//! through [`focus::focus_control`]. Escape while focus is in it means Cancel and is taken there, so the owning dialog
//! stays open; Escape anywhere else in the dialog is the dialog's. Enter presses the focused answer; the question holds
//! no focus of its own, so a press on its empty space does what the owning dialog's empty-space rule says. It draws in
//! one of two looks: plain, the old leave question's text and answers (`.templates-departure`), on whatever band its
//! owner puts it in, or loud ([`Question::loud`]), the old YOLO confirmation's red-edged, red-washed block with its
//! title as a red heading (`.yolo-confirmation`). Decided by the migration's orchestrator on 2026-11-14.
//!
//! A question may carry one link ([`Question::link`]), drawn after its lines and before its answers in every
//! presentation: SPEC.md's sandbox question, asked inline in the launcher, links Tensorlake pricing there. It opens in
//! the browser and is not an answer. Approved by the GPUI migration's orchestrator on 2026-11-16.
//!
//! Some options belong to some presentations only: the body, tile, note, width and scroll stop to the centered
//! question, the loud look to the inline one, and the empty-click rule and the Escape-home and named-control returns
//! to the two layers. A debug build asserts, as the question opens, that none is set on a question asked a way that
//! ignores it; a release build ignores it. Otherwise a question moved from centered to anchored, or a helper that
//! builds one question for both, would lose its body or its tile with no test failing. The answer that can fail is
//! stricter: a question asked a way that may not carry one refuses to open, in every build.
//!
//! NOTE: Two recorded deviations from the old YOLO question's look. The old one drew its last line (the host "asks
//! before every YOLO launch") in the dimmer `--fg-dim`; here every line is drawn alike, since a question's lines carry
//! no emphasis of their own. And a line naming a host mid-sentence can wrap with a word left alone before the host
//! value ("… stops its actions" / "on" / "Local (this machine) without asking you."), where the old one flowed as one
//! paragraph: a [`HostSentence`](crate::host_text::HostSentence) draws each part as a whole block of its own. The fix
//! belongs in `host_text` (one styled text with runs), not here.
//!
//! NOTE: A recorded deviation from the old host remove dialog's look. Its refusal sat below the explanation in the
//! body's ink, and its summary turned into a red "Farhelm refused this action" once there was one. A question's
//! summary is frozen, so the centered question draws a failed attempt's reason below its body in the inline question's
//! amber instead.

use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, AnyWindowHandle, App, AppContext as _, Context, Entity, FocusHandle,
    FontWeight, InteractiveElement as _, IntoElement, ParentElement, Pixels, Render, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, Styled, WeakEntity, WeakFocusHandle, Window,
    div, prelude::FluentBuilder as _, px,
};

use crate::actions::Dismiss;
use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier, scroll_from_controls};
use crate::focus::{self, Mark, ReturnTo};
use crate::host_text::Line;
use crate::layers::{self, Anchor, AnchoredDialog, Dialog, Dismissal, LayerId, Subject};
use crate::theme::{self, metrics};

/// How wide an anchored question may grow before its lines wrap: the old row menu panel's `max-width: 288px`.
const ANCHORED_MAX_WIDTH: f32 = 288.;

/// How tall an anchored question's text (its summary and lines) may grow before its host-value lines scroll, by the
/// wheel and by Page Up, Page Down, Home and End from an answer: about twelve lines of its 12-pixel text at GPUI's
/// default line height (the golden ratio, 19.4 pixels a line). The card around it then stays well inside the shortest
/// content area the app's minimum window leaves ([`crate::bootstrap::MIN_WINDOW_SIZE`]), answers wrapped or not, so
/// the layer manager's clamp can always put the whole card, answers included, inside the window. See the module docs
/// for why the cap exists.
const ANCHORED_TEXT_MAX_HEIGHT: f32 = 233.;

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

/// The link a question carries ([`Question::link`]): app wording and an app URL, never a host's value.
struct Link {
    label: SharedString,
    url: SharedString,
    help: HoverHelp,
}

impl Answer {
    /// An answer button labelled `label`, with `help` as its hover help and `tier` as its look: the one way to build
    /// an answer, since every question builds two or more. The help stays a [`HoverHelp`]
    /// rather than a string, so the label and the help cannot be swapped without a type error, and help in any of its
    /// forms (a host sentence included) still goes in:
    ///
    /// ```ignore
    /// let delete = Answer::new("Delete", HoverHelp::tooltip("Delete it for good"), Tier::Danger);
    /// ```
    pub fn new(label: impl Into<SharedString>, help: HoverHelp, tier: Tier) -> Self {
        Answer {
            label: label.into(),
            help,
            tier,
        }
    }
}

/// A question to ask, frozen at the moment it opens. `T` is what the confirming answer acts on: the targets by stable
/// id and whatever else the action needs, as the user was shown them.
pub struct Question<T> {
    /// The title, and the question's accessible name in every presentation ([`Question::new`]).
    title: Line,
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
    /// The link between the lines and the answers ([`Question::link`]).
    link: Option<Link>,
    /// Builds the feature's body from the frozen value when the question opens ([`Question::body`]).
    body: Option<BuildBody<T>>,
    /// Draws the header's icon tile ([`Question::tile`]).
    tile: Option<DrawTile>,
    /// The footer note beside the answers ([`Question::note`]).
    note: Option<SharedString>,
    /// The dialog frame's width in place of the ordinary one ([`Question::width`]).
    width: Option<Pixels>,
    /// The accessible name of the text's keyboard scroll stop, when it has one ([`Question::scroll_stop`]).
    scroll_stop: Option<SharedString>,
    frozen: T,
}

/// What [`Question::body`] is handed: called once when the question opens, and once more for each reword
/// ([`Open::reword`]), with the frozen value.
type BuildBody<T> = Box<dyn FnOnce(&T, &mut Window, &mut App) -> AnyView>;

/// What [`Question::tile`] is handed: called on every draw, since an element lasts one frame.
type DrawTile = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

impl<T: Clone + 'static> Question<T> {
    /// A question titled `title` whose confirming answer is `confirm` and acts on `frozen`. Cancel is the other answer.
    ///
    /// The title is the question's accessible name wherever it shows, drawn or not (the anchored question draws none).
    /// Usually it is the app's own words, passed as a string. A title that names a host value, as the host setup
    /// confirmation's "Set up host · <name>" does (the old app's, ported; SPEC.md titles such as "Suspend <host>?"
    /// too), is a [`HostSentence`](crate::host_text::HostSentence): it is drawn through the host-text element, and the
    /// accessible name is the sentence's
    /// [`HostSentence::accessible_name`](crate::host_text::HostSentence::accessible_name), so the host value is escaped
    /// both where the user reads it and where a screen reader does. Never format the host's raw value into a string
    /// title: that would reach the screen and the accessible name unescaped.
    pub fn new(title: impl Into<Line>, confirm: Answer, frozen: T) -> Self {
        Question {
            title: title.into(),
            summary: None,
            lines: Vec::new(),
            confirm,
            cancel: Answer::new(
                "Cancel",
                HoverHelp::tooltip("Close this without doing anything"),
                Tier::Neutral,
            ),
            alternatives: Vec::new(),
            on_alternatives: Vec::new(),
            confirm_enabled: true,
            starts_on_cancel: false,
            keeps_focus_on_empty_click: false,
            returns_to: ReturnTo::Opener,
            loud: false,
            link: None,
            body: None,
            tile: None,
            note: None,
            width: None,
            scroll_stop: None,
            frozen,
        }
    }

    /// A one-line summary under the title, usually naming the target. A target a host named (a session's title, a
    /// host's name) goes in as a [`HostSentence`](crate::host_text::HostSentence), never as a string made from its raw
    /// value, as it does in the title ([`Question::new`]).
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
    /// off. Not for an inline question, whose empty space is its owner's ([`ask_inline`]; a debug build asserts it).
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
    /// shows the reason inside the question and arms the answers again (centered, it also moves focus to Cancel while
    /// focus is still on the answer pressed; see the module docs). A retry runs `on_choose` again, with a new attempt,
    /// which is why it is `Fn`.
    ///
    /// While an inline question's attempt is unsettled ([`Inline::in_flight`]), its owner must not go away under it:
    /// the owner's dismissal hook answers [`Dismissal::Stay`](crate::layers::Dismissal::Stay), as an editor whose save
    /// is in flight does. Otherwise Tab out of the question, then Escape or the header Close, would close the launcher,
    /// and the action would run on and start a session the user dismissed.
    ///
    /// Only an inline question ([`ask_inline`]) and a centered one built with
    /// [`Question::keeps_focus_on_empty_click`] may carry one; any other question with one refuses to open. A
    /// confirming answer that can fail exists only on that centered question, through [`ask_fallible`]: an inline
    /// question's confirming answer runs once. An inline question's owner decides its own dismissal and can refuse
    /// it while the action runs. A centered question's veil takes a press without dismissing it, and every dismissal
    /// (Escape, a control routed through [`layers::dismiss`]) asks the dialog's hook, which the prompt answers with
    /// [`Dismissal::Stay`](crate::layers::Dismissal::Stay) while an attempt is in flight. The empty-click rule keeps
    /// focus on the busy answer the user pressed when they click the question's empty space, which otherwise would
    /// move focus to the question itself. An anchored question closes on a press outside it and when its anchor goes,
    /// which nothing can veto. The rule was widened from inline-only by the GPUI migration's orchestrator on
    /// 2026-11-14, for the host remove confirmation.
    ///
    /// A centered question still closes mid-attempt when its subject goes or the connection resets, and the attempt's
    /// settling then does nothing. For the host remove confirmation that close is the success: the registry stops
    /// listing the host.
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
    /// question that looks like the lines around it gets clicked through. Only the inline question takes it: a debug
    /// build asserts it is not set on a question asked another way, and a release build ignores it there.
    pub fn loud(mut self) -> Self {
        self.loud = true;
        self
    }

    /// A link labelled `label`, with `help` as its hover help, that opens `url` in the browser through `cx.open_url`,
    /// the opener the announcements, the Help menu and host setup's pricing link use. SPEC.md's sandbox question,
    /// asked inline in the launcher, is the case it exists for: "linking Tensorlake pricing".
    ///
    /// It is drawn as a link button after the question's lines and before its answers, in every presentation (on the
    /// centered question, above any [`Question::body`]), and sits in Tab's order between the text and the answers. It
    /// is not an answer. A click on it, or Enter or Space while it has focus, opens `url` and leaves the question open
    /// and unanswered, and it never counts towards the first answer winning. Everything else about the question stays
    /// as it would be without it: focus never starts on it, Escape with focus on it is Cancel, the held-Enter guard is
    /// the button's own, and an inline question returns focus from it as it does from an answer. A reword keeps it.
    ///
    /// The label is app wording and `url` an app constant; neither may carry a host's value, since neither goes
    /// through [`crate::host_text`]. A question carries one link, and a debug build asserts this is called once.
    pub fn link(
        mut self,
        label: impl Into<SharedString>,
        url: impl Into<SharedString>,
        help: HoverHelp,
    ) -> Self {
        debug_assert!(self.link.is_none(), "Question::link called twice");
        self.link = Some(Link {
            label: label.into(),
            url: url.into(),
            help,
        });
        self
    }

    /// A body the feature draws, below the lines and above the answers, for a question that needs more than lines of
    /// text: the host setup confirmation's numbered plan with its Copy plan button, say. `build` runs once, when the
    /// question is asked, with the frozen value, and returns the view the prompt then draws for as long as it is open
    /// or until it is reworded ([`Open::reword`], which builds the new question's body from its own frozen value).
    /// It runs even when the question then does not open (the user moved on since the mark), and its view is dropped
    /// unseen, so it should only build the view:
    ///
    /// ```ignore
    /// Question::new("Set up host", set_up, offer.clone())
    ///     .body(|offer: &SetupOffer, _, cx| cx.new(|cx| PlanBody::new(offer, cx)))
    /// ```
    ///
    /// A view built once rather than a function drawn every frame, for two reasons. What it shows is fixed at the
    /// moment the question opens or is reworded, as the lines are, so the plan the user reads is the plan the answers
    /// act on. And
    /// the body's own controls need state that outlives a frame: a Copy button's focus handle, which must stay the
    /// same handle for focus to stay on it. The prompt cannot check the first: a view that reads live state when it
    /// draws (an entity, the store) would show something else, so it holds copies, as the frozen value does.
    ///
    /// The body may hold controls that are not answers (Copy plan). They sit in Tab's order between the question's
    /// text and its answers, since the body is drawn between them; focus still starts where the prompt's rules put it,
    /// never in the body. Such a control is the feature's own: pressing it never closes the question, and it is not
    /// one of the answers the first one wins among.
    ///
    /// A host value in the body goes through [`crate::host_text`]'s elements
    /// ([`HostSentence`](crate::host_text::HostSentence), [`element`](crate::host_text::element)), as everywhere else;
    /// the prompt draws the body as given and cannot check that it did.
    ///
    /// Only the centered question draws a body: a debug build asserts that an anchored or inline question has none, as
    /// it does for the tile, the note and the width. No question asked those ways needs one, and each would need more
    /// than drawing it: the anchored question's amber 12-pixel prose would restyle it, and an inline question returns
    /// focus itself only from its answers, so Escape with focus on a body control would let focus fall elsewhere in the
    /// owner's dialog.
    pub fn body<V: Into<AnyView>>(
        mut self,
        build: impl FnOnce(&T, &mut Window, &mut App) -> V + 'static,
    ) -> Self {
        self.body = Some(Box::new(move |frozen, window, cx| {
            build(frozen, window, cx).into()
        }));
        self
    }

    /// An icon tile left of the title ([`DialogFrame::tile`]): the old host dialogs' host mark. `draw` runs on every
    /// draw, since a drawn element lasts one frame; it should draw what it captured when the question was built, not
    /// read live state. The tile is decoration, and only the centered question has one; a debug build asserts the
    /// others are not given one.
    pub fn tile<E: IntoElement>(
        mut self,
        draw: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.tile = Some(Rc::new(move |window, cx| {
            draw(window, cx).into_any_element()
        }));
        self
    }

    /// A note at the footer's left, beside the answers ([`DialogFrame::note`]): the host confirmations' "Undo "don't
    /// ask again" with the gear at the foot of the navigation rail". App wording only. Only the centered question has
    /// a footer; a debug build asserts the others are not given a note.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// The dialog's width in place of the ordinary one ([`DialogFrame::width`]): the old host setup confirmation was
    /// 560 pixels wide, against the ordinary dialog's 480, so its plan's paths wrap where they did. Only the centered
    /// question takes it; a debug build asserts the others are not given one.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// Give the question's text (its lines, the body and a failed attempt's reason) a keyboard scroll stop named
    /// `label`, for a question whose text can run past the dialog: the bulk Stop and Delete question lists a row per
    /// session ("Sessions affected", the old dialog's name for that region). The answers authorize what the text lists,
    /// so a keyboard user must be able to scroll it into view before answering, and SPEC.md, "Opening and closing",
    /// keeps Tab within a modal's "visible, enabled controls and keyboard scroll regions".
    ///
    /// It is the dialog frame's stop ([`DialogFrame::scroll_stop`]), under the frame's rule: present whenever the text
    /// holds no tab stop, overflowing or not, as the old app's scroll region was, and absent while a control in the
    /// body is the keyboard's way in. It comes first in Tab's order, before the answers, since it is drawn above them;
    /// focus still starts on the answer the rules pick. Without this the text is drawn exactly as before. Only the
    /// centered question has a scrolling body; a debug build asserts the others are not given one.
    pub fn scroll_stop(mut self, label: impl Into<SharedString>) -> Self {
        self.scroll_stop = Some(label.into());
        self
    }

    /// Escape on the question sends focus home (the selected session's terminal) rather than back to the control that
    /// opened it; every other way it closes (an answer, a press outside it) still returns focus to the opener. SPEC.md,
    /// "Keyboard focus", for the session header's Stop, Restart, Replace and Delete questions: "Escape cancels the
    /// header's … question and returns focus to the terminal". A row's questions return to their opener on Escape too.
    /// While there is no home, or another layer is still open, Escape returns focus to the opener all the same. Not for
    /// an inline question, whose Escape keeps focus in its owner's dialog ([`ask_inline`]; a debug build asserts it).
    ///
    /// A question has one return rule: this and [`Question::returns_to`] exclude each other, and a debug build asserts
    /// that only one was called, since whichever came last would otherwise win unseen.
    pub fn returns_home_on_escape(mut self) -> Self {
        debug_assert!(
            self.returns_to == ReturnTo::Opener,
            "Question::returns_home_on_escape on a question that already returns focus elsewhere"
        );
        self.returns_to = ReturnTo::HomeOnEscape;
        self
    }

    /// Every close of the question (Cancel, Escape, any other dismissal, an answer, its subject going) returns focus
    /// to `control` rather than to what held focus when it opened: the layer manager's [`ReturnTo::Control`], with its
    /// fallback when `control` is not drawn as the question closes (then focus goes back to what held it when the
    /// question opened). For a question about a control other than the one that asked it: the bulk Stop and Delete
    /// question returns focus to the marked bar's ⋯, as the old one did, though its Stop or Delete button asked it.
    /// Naming the control costs no focus move before the question opens, where focusing the control first is one more
    /// visible move, and one the focus coordinator refuses once the user has moved on.
    ///
    /// An answer's own action may still move focus after the question has closed (the bulk question's Confirm sends it
    /// to the marked count). Not for an inline question, which returns focus to what held it inside its owner's dialog
    /// ([`ask_inline`]; a debug build asserts it), and not together with [`Question::returns_home_on_escape`].
    pub fn returns_to(mut self, control: FocusHandle) -> Self {
        debug_assert!(
            self.returns_to == ReturnTo::Opener,
            "Question::returns_to on a question that already returns focus elsewhere"
        );
        self.returns_to = ReturnTo::Control(control);
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

    /// Whether this question may carry an answer whose action can fail when shown as `shown`: inline always, centered
    /// only with the empty-click rule, anchored never. [`Question::fallible_alternative`] says why; [`ask_inner`]
    /// refuses to open a layer this answers `false` for.
    fn may_fail_when(&self, shown: Shown) -> bool {
        match shown {
            Shown::Inline => true,
            Shown::Centered => self.keeps_focus_on_empty_click,
            Shown::Anchored => false,
        }
    }

    /// The options set on this question that a question shown as `shown` ignores, by builder name, in the order the
    /// builders are declared: the body, tile, note, width and scroll stop outside the centered question, the loud look
    /// outside the inline one, and inside it the empty-click rule and the focus returns its owner's dialog keeps. This
    /// is the one list of what each presentation ignores, which [`build`] asserts empty in a debug build (see the
    /// module docs). An answer that can fail is not here: a question that may not carry one refuses to open, in every
    /// build ([`Question::may_fail_when`]).
    fn ignored_by(&self, shown: Shown) -> Vec<&'static str> {
        let (centered, inline) = (shown == Shown::Centered, shown == Shown::Inline);
        [
            (
                "Question::keeps_focus_on_empty_click",
                inline && self.keeps_focus_on_empty_click,
            ),
            ("Question::loud", !inline && self.loud),
            ("Question::body", !centered && self.body.is_some()),
            ("Question::tile", !centered && self.tile.is_some()),
            ("Question::note", !centered && self.note.is_some()),
            ("Question::width", !centered && self.width.is_some()),
            (
                "Question::scroll_stop",
                !centered && self.scroll_stop.is_some(),
            ),
            (
                "Question::returns_home_on_escape",
                inline && self.returns_to == ReturnTo::HomeOnEscape,
            ),
            (
                "Question::returns_to",
                inline && matches!(self.returns_to, ReturnTo::Control(_)),
            ),
        ]
        .into_iter()
        .filter_map(|(option, ignored)| ignored.then_some(option))
        .collect()
    }
}

/// What a confirming answer does with the frozen value.
type OnConfirm<T> = Box<dyn FnOnce(T, &mut Window, &mut App)>;

/// What the confirming answer or an extra answer does.
enum Action<T> {
    /// Runs once, after the question closed ([`Question::alternative`], [`ask`]).
    Once(OnConfirm<T>),
    /// Runs with the question still open, and again on each retry, until an attempt succeeds
    /// ([`Question::fallible_alternative`], [`ask_fallible`]).
    Fallible(OnAttempt<T>),
}

/// What an answer whose action can fail does with the frozen value and the attempt it settles.
type OnAttempt<T> = Rc<dyn Fn(T, Attempt, &mut Window, &mut App)>;

/// Settles an [`Attempt`]: `None` for success, the reason for a failure.
type Settle = Box<dyn FnOnce(Option<Line>, &mut App)>;

/// One run of an answer whose action can fail ([`Question::fallible_alternative`], [`ask_fallible`]), which the action
/// settles exactly once, whenever its work is done. Until then the question stays open with every answer frozen.
///
/// Settling takes effect once the update it is called from has returned, so an action may settle at once, from inside
/// its own call. An attempt settled after the question went (its owner stopped drawing it, or its layer closed with
/// its subject) does nothing. An attempt dropped without settling leaves the question frozen until its owner takes it
/// away (a centered question: until its subject goes or the connection resets): an owner whose work can be cancelled
/// settles the attempt as failed when it is. While it is unsettled, an inline question's owner keeps itself open
/// through its dismissal hook ([`Question::fallible_alternative`]); a centered question holds its own dismissals.
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

    /// The action failed: the question stays up showing `reason` and arms its answers again, with focus moved to Cancel
    /// on a centered question while focus is still on the answer pressed (see the module docs). `reason` may quote a
    /// host (the helm's refusal): pass it as a [`HostSentence`](crate::host_text::HostSentence), never as a string made
    /// from the raw value.
    pub fn failed(self, reason: impl Into<Line>, cx: &mut App) {
        (self.settle)(Some(reason.into()), cx);
    }
}

/// An open question's view: the frozen question, its answer buttons' focus handles, and the actions not yet run.
pub struct Prompt<T: 'static> {
    question: Question<T>,
    /// The confirming answer's action, until it runs (a fallible one stays).
    on_confirm: Option<Action<T>>,
    /// The extra answers' actions, taken out of the question when it opens, each until it runs (a fallible one stays).
    on_alternatives: Vec<Option<Action<T>>>,
    layer: Option<LayerId>,
    /// How it is drawn.
    shown: Shown,
    /// The feature's body, built from the frozen value when the question opened ([`Question::body`]).
    body: Option<AnyView>,
    /// An answer has run (or an attempt succeeded): the question is closed, and every answer does nothing.
    answered: bool,
    /// A fallible answer's attempt is in flight: every answer is frozen until it settles.
    in_flight: bool,
    /// Why the last attempt failed, shown inside the question until the next one starts.
    reason: Option<Line>,
    /// What held focus when an inline question opened, where an answer returns it. Layers keep their own.
    opener: Option<WeakFocusHandle>,
    /// The user's input as of the click that ran the last answer (or started the attempt in flight), or `None` when a
    /// key or the test handle ran it. An inline question returns focus after a click it has not moved on from, as
    /// after an answer that holds focus: on macOS the click leaves focus off the answer ([`Prompt::close`]).
    clicked: Option<Mark>,
    /// What held focus when the attempt in flight started (usually the answer the user pressed, for a key or a click
    /// on Linux; a body control, for Enter reaching the main action from it): a refusal moves focus to Cancel only
    /// while focus is still there ([`Prompt::settle`]). Remembered, not owned, as `opener` is. `None` while no
    /// attempt runs, or when nothing held focus then, in which case the refusal leaves focus alone; a centered
    /// question's focus trap keeps focus inside it, so that case does not arise there today.
    focus_at_attempt: Option<WeakFocusHandle>,
    /// How many times the question has been reworded ([`Open::reword`]), and the count as of the last frame shown on
    /// screen. While the two differ, the user has not yet seen the words the answers would act on, and only Cancel
    /// answers.
    wording: u32,
    drawn_wording: u32,
    confirm_focus: FocusHandle,
    cancel_focus: FocusHandle,
    alternative_focus: Vec<FocusHandle>,
    /// The link's, made whether or not there is one, so an inline question can tell focus is on it ([`Prompt::close`]).
    link_focus: FocusHandle,
    /// The anchored question's first host-value line's scroll state, which Page Up, Page Down, Home and End move from
    /// a focused answer. Kept here because the line is rebuilt on every draw.
    text_scroll: ScrollHandle,
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

impl Shown {
    /// The call that asks a question this way, which is how a feature author knows the presentation: for messages.
    fn asked_through(self) -> &'static str {
        match self {
            Shown::Centered => "prompt::ask, prompt::ask_rewordable or prompt::ask_fallible",
            Shown::Anchored => "prompt::ask_anchored",
            Shown::Inline => "prompt::ask_inline",
        }
    }
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
    /// Where `which`'s action is kept: `None` for Cancel, which has none, and for an index past the extra answers.
    fn action_of(&mut self, which: Which) -> Option<&mut Option<Action<T>>> {
        match which {
            Which::Confirm => Some(&mut self.on_confirm),
            Which::Alternative(index) => self.on_alternatives.get_mut(index),
            Which::Cancel => None,
        }
    }

    /// Run `which`, if no answer has run yet and no attempt is in flight: close the question (returning focus to its
    /// opener), then, for Confirm or an extra answer, hand the frozen value to its action. A fallible answer instead
    /// starts an attempt and leaves the question open, frozen, until the attempt settles. Every answer but Cancel does
    /// nothing between a reword and the frame that shows it (see the module docs). `clicked` is the mark of the click
    /// that pressed the answer, `None` for a key.
    fn answer(
        &mut self,
        which: Which,
        clicked: Option<Mark>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let unseen = self.wording != self.drawn_wording;
        if self.answered
            || self.in_flight
            || (which == Which::Confirm && !self.question.confirm_enabled)
            || (which != Which::Cancel && unseen)
        {
            return;
        }
        self.clicked = clicked;
        if let Some(Some(Action::Fallible(action))) = self.action_of(which) {
            let action = action.clone();
            self.in_flight = true;
            self.reason = None;
            self.focus_at_attempt = window.focused(cx).map(|focused| focused.downgrade());
            cx.notify();
            let attempt = Attempt::new(cx.weak_entity(), window.window_handle());
            action(self.question.frozen.clone(), attempt, window, cx);
            return;
        }
        self.answered = true;
        self.close(window, cx);
        let action = match self.action_of(which).and_then(Option::take) {
            Some(Action::Once(action)) => Some(action),
            // A fallible answer started its attempt above; Cancel and an index past the end have no action.
            Some(Action::Fallible(_)) | None => None,
        };
        if let Some(action) = action {
            action(self.question.frozen.clone(), window, cx);
        }
    }

    /// The in-flight attempt settled: closed on success (`reason` is `None`), or open again with the reason shown, and,
    /// centered, focus on Cancel if it is still on the answer the attempt started from.
    fn settle(&mut self, reason: Option<Line>, window: &mut Window, cx: &mut Context<Self>) {
        // Only an attempt settles, and only while it is the one in flight: each settles once, and the next can start
        // only after the last has settled.
        self.in_flight = false;
        let focus_at_attempt = self.focus_at_attempt.take();
        match reason {
            Some(reason) => {
                self.reason = Some(reason);
                // The old host remove dialog sent focus to its Cancel after a refusal (`focus_remove_refusal` at
                // reference commit `24bc0580`), "so the refusal can be dismissed with Enter or Escape" rather than
                // Enter sending the refused action again; the same for either of its answers. Only while focus is still
                // on the answer the attempt started from (SPEC_impl.md, "Questions"): a user who tabbed or clicked
                // elsewhere meanwhile keeps their place, while a press that acted on the question alone (its veil, its
                // empty space, an Escape it held) left focus on the re-armed answer, where Enter would resend. The
                // focus coordinator makes the move and keeps it inside the question's layer. The mark is taken now on
                // purpose: the focus check stands in for the press's mark, and the coordinator still drops the move on
                // any press after this. An inline question leaves focus on the pressed answer.
                let stayed = focus_at_attempt
                    .as_ref()
                    .and_then(WeakFocusHandle::upgrade)
                    .is_some_and(|answer| answer.is_focused(window));
                if self.shown == Shown::Centered && stayed {
                    let cancel = self.cancel_focus.clone();
                    focus::focus_in_layer(&cancel, &Mark::take(cx), window, cx);
                }
            }
            None => {
                self.answered = true;
                self.close(window, cx);
            }
        }
        cx.notify();
    }

    /// Why the last attempt failed, as the inline and centered questions draw it below their text: amber 12-pixel
    /// text, the old launcher's `.yolo-confirmation-error` (the centered question's look is a recorded deviation; see
    /// the module docs).
    fn reason(&self, cx: &App) -> Option<gpui::Div> {
        let palette = theme::palette(cx);
        self.reason.clone().map(|reason| {
            div()
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.warn_bright))
                .child(reason)
        })
    }

    /// The question's link ([`Question::link`]), as every presentation draws it after the lines. It is in a row of its
    /// own so that it sits at the start of its line, as the announcements' link does, rather than stretched across the
    /// question's column with its words centered. Never busy: it is no answer, so an attempt in flight does not
    /// freeze it.
    fn link(&self) -> Option<gpui::Div> {
        self.question.link.as_ref().map(|link| {
            let url = link.url.clone();
            div().flex().child(
                Button::new(
                    "prompt-link",
                    link.label.clone(),
                    link.help.clone(),
                    Role::Link,
                )
                .tier(Tier::Link)
                .focus(&self.link_focus)
                .on_click(move |_, _, cx| cx.open_url(&url)),
            )
        })
    }

    /// Take the question down: close its layer, or, inline, stop drawing it and return focus to what held it when it
    /// opened, if focus is on one of its answers or its link, or a click ran the answer and the user has not moved on
    /// since.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
        if self.shown == Shown::Inline {
            // Focus goes back only from an answer or the link that holds it (Escape on the link is Cancel), so it stays
            // wherever the user took it otherwise. The answer's own press or key is the action, so its mark is taken
            // now.
            let holds = std::iter::once(&self.cancel_focus)
                .chain(&self.alternative_focus)
                .chain([&self.confirm_focus, &self.link_focus])
                .any(|answer| answer.is_focused(window));
            // A click on an answer is an answer from inside the question wherever it left focus. The macOS click rule
            // focuses no button, and the press goes on to the owner's empty-space rule, which in a dialog that takes
            // focus there has put focus on the dialog: reading only `holds`, Keep editing on a Mac left focus on the
            // Templates dialog rather than in the name field. The click's own mark goes to the focus coordinator, which
            // refuses the move once the user has moved on since the click (a press elsewhere while the answer's
            // attempt ran), as it refuses any move for an action the user has left behind.
            let clicked = self.clicked.take();
            let mark = if holds { Some(Mark::take(cx)) } else { clicked };
            if let Some(mark) = mark
                && let Some(opener) = self.opener.as_ref().and_then(WeakFocusHandle::upgrade)
            {
                move_for_question(&opener, &mark, window, cx);
            }
            cx.notify();
        }
    }

    /// Show `question`'s words and act on its frozen value from now on ([`Open::reword`] has the contract): `false`,
    /// changing nothing, while an attempt is in flight. A closed question never gets here ([`Open::reword`]).
    fn reword(
        &mut self,
        mut question: Question<T>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // An attempt in flight was handed the old value; rewording under it would show one thing while that action
        // settles another, and its failure would arm answers the user never saw beside the reason.
        if self.in_flight {
            return false;
        }
        let open = &mut self.question;
        // The layer was opened with these (Enter's main action exists only for an enabled Confirm, and each extra
        // answer has its button and action), so a new question that differs in them would be drawn one way and
        // answered another.
        debug_assert_eq!(
            question.alternatives.len(),
            open.alternatives.len(),
            "a reword must keep the question's extra answers"
        );
        debug_assert_eq!(
            question.confirm_enabled, open.confirm_enabled,
            "a reword must keep whether Confirm is enabled"
        );
        self.body = question
            .body
            .take()
            .map(|build| build(&question.frozen, window, cx));
        open.title = question.title;
        open.summary = question.summary;
        open.lines = question.lines;
        open.confirm.label = question.confirm.label;
        open.confirm.help = question.confirm.help;
        open.frozen = question.frozen;
        self.wording += 1;
        cx.notify();
        true
    }
}

impl<T: Clone + 'static> Render for Prompt<T> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        if self.shown == Shown::Inline && self.answered {
            return div().into_any_element();
        }
        // The words count as seen once the frame drawing them is on screen, not once they render: GPUI draws a dirty
        // window before it handles a key and shows that frame only at the next frame, so a key queued behind a reword
        // would otherwise confirm words never shown. Next-frame callbacks run right before a frame is shown, with no
        // input between, and a pending one asks for a frame, so the guard never outlives the next frame. Each clears
        // only the wording its own frame drew, since a later reword may have come since.
        if self.drawn_wording != self.wording {
            let (drawn, prompt) = (self.wording, cx.weak_entity());
            when_shown(window, move |cx| {
                let _ = prompt.update(cx, |prompt, _| {
                    prompt.drawn_wording = prompt.drawn_wording.max(drawn)
                });
            });
        }
        let busy = self.in_flight;
        let question = &self.question;
        let answer = |which: Which, cx: &mut Context<Self>| {
            let prompt: WeakEntity<Self> = cx.weak_entity();
            move |event: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                // A button hands Enter and Space here too, as GPUI's default click, which is a keyboard one; anything
                // else is the pointer's.
                let clicked = (!event.is_keyboard()).then(|| Mark::take(cx));
                // The prompt may already be gone (answered and closed); then there is nothing to answer.
                let _ = prompt.update(cx, |prompt, cx| prompt.answer(which, clicked, window, cx));
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
            let lines: Vec<Line> = question
                .summary
                .iter()
                .chain(&question.lines)
                .cloned()
                .collect();
            let keyed = lines
                .iter()
                .position(|line| matches!(line, Line::Sentence(_)));
            // Capped at `ANCHORED_TEXT_MAX_HEIGHT`: only lines naming a host value give up height, and scroll.
            let text = div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .max_h(px(ANCHORED_TEXT_MAX_HEIGHT))
                .children(lines.into_iter().enumerate().map(|(index, line)| {
                    match line {
                        Line::App(_) => div().flex_none().child(line).into_any_element(),
                        Line::Sentence(_) => div()
                            .id(("prompt-line", index))
                            .min_h_0()
                            .overflow_y_scroll()
                            .when(keyed == Some(index), |line| {
                                line.track_scroll(&self.text_scroll)
                            })
                            .child(line)
                            .into_any_element(),
                    }
                }));
            let card = layers::surface(cx)
                .id("prompt")
                .debug_selector(|| "prompt".into())
                .role(Role::Dialog)
                .aria_label(question.title.accessible_text())
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(8.))
                .max_w(px(ANCHORED_MAX_WIDTH))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.warn_bright))
                .child(text)
                .children(self.link())
                .child(
                    div()
                        .debug_selector(|| "prompt-answers".into())
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .gap(px(8.))
                        .children(answers),
                );
            return scroll_from_controls(card, &self.text_scroll).into_any_element();
        }
        let body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .text_color(theme::color(palette.fg_0))
            .children(question.lines.iter().cloned().map(|line| div().child(line)))
            .children(self.link())
            .children(self.body.clone())
            .children(self.reason(cx));
        let mut frame = DialogFrame::new("prompt", question.title.clone(), body, answers);
        if let Some(summary) = &question.summary {
            frame = frame.summary(summary.clone());
        }
        if let Some(tile) = &question.tile {
            frame = frame.tile(tile(window, cx));
        }
        if let Some(note) = &question.note {
            frame = frame.note(note.clone());
        }
        if let Some(width) = question.width {
            frame = frame.width(width);
        }
        if let Some(label) = &question.scroll_stop {
            frame = frame.scroll_stop(label.clone());
        }
        frame.into_any_element()
    }
}

/// Run `shown` at the next frame GPUI shows: by then the frame drawn now is on screen, or, when it was drawn early to
/// handle a key, it goes up right after the callback, with no input dispatched in between. A pending callback asks for
/// that frame. The prompt's one use of
/// a next-frame callback, for [`Prompt`]'s old-answer guard (its `render`): it asks for the next frame only while a
/// reword's words have not been shown, once per frame that draws them, and never schedules anything over time.
#[allow(
    clippy::disallowed_methods,
    reason = "one frame per reword, only to learn when its new words are shown; it schedules nothing over time, so \
              the ticker's rule (SPEC_impl.md, \"Animation time: the ticker\") is not bypassed"
)]
fn when_shown(window: &Window, shown: impl FnOnce(&mut App) + 'static) {
    window.on_next_frame(move |_, cx| shown(cx));
}

/// An inline question, as [`ask_inline`] draws it: the title, the summary and the lines, its link if it has one
/// ([`Question::link`]), the reason the last attempt failed (amber, as the old launcher's `.yolo-confirmation-error`
/// was), then the answers right-aligned 8 pixels apart (`.dialog-actions`), in the plain or the loud look (see the
/// module docs). Escape inside it, the link included, is Cancel, and is taken even while an attempt is in flight (then
/// it does nothing), so it never reaches the owning dialog.
fn inline<T: Clone + 'static>(
    prompt: &Prompt<T>,
    answers: Vec<Button>,
    cx: &mut Context<Prompt<T>>,
) -> gpui::AnyElement {
    let palette = theme::palette(cx);
    let question = &prompt.question;
    let loud = question.loud;
    let reason = prompt.reason(cx);
    let element = div()
        .id("prompt")
        .debug_selector(|| "prompt".into())
        // The old blocks' roles: `alertdialog` for the YOLO question, `status` for the leave question.
        .role(if loud {
            Role::AlertDialog
        } else {
            Role::Status
        })
        .aria_label(question.title.accessible_text())
        .on_action(cx.listener(|prompt, _: &Dismiss, window, cx| {
            prompt.answer(Which::Cancel, None, window, cx)
        }))
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
        .children(prompt.link())
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
    /// Press Confirm, as its button does for a key ([`press`]).
    pub fn confirm(&self, window: &mut Window, cx: &mut App) {
        press(&self.prompt, Which::Confirm, window, cx);
    }

    /// Press Cancel, as its button does for a key ([`press`]).
    pub fn cancel(&self, window: &mut Window, cx: &mut App) {
        press(&self.prompt, Which::Cancel, window, cx);
    }

    /// Press the extra answer at `index`, as its button does for a key ([`press`]).
    pub fn alternative(&self, index: usize, window: &mut Window, cx: &mut App) {
        press(&self.prompt, Which::Alternative(index), window, cx);
    }
}

/// Press `which` on `prompt`, as its button does for a key, for tests that answer without drawing in between. It is no
/// click, so an inline question returns focus afterwards only if focus is on one of its answers ([`Prompt::close`]).
#[cfg(test)]
fn press<T: Clone + 'static>(
    prompt: &Entity<Prompt<T>>,
    which: Which,
    window: &mut Window,
    cx: &mut App,
) {
    prompt.update(cx, |prompt, cx| prompt.answer(which, None, window, cx));
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
    /// Whether it is still open: no answer has run (Cancel included), and no fallible answer has succeeded. An owner
    /// reads it to stop drawing the question's place once it closes, to prune finished questions, and to refuse what
    /// SPEC.md refuses while a question is open (an open lifecycle confirmation "refuses another lifecycle action").
    ///
    /// An owner may read it while drawing. Closing notifies the question's own entity, and GPUI redraws every view
    /// drawn above a view that notifies, cached or not, so an owner that draws the question sees it close in its next
    /// frame. A view that reads it without drawing the question (a menu that turns Delete off) sees it close only
    /// because every uncached view redraws with the window, and no view is cached in production (`cache.rs`); a cached
    /// one would have to observe the question's entity. Dropping the question, or no longer drawing it, takes it away
    /// unanswered without closing it, so it stays `true` for any clone the owner still holds.
    pub fn is_open(&self, cx: &App) -> bool {
        !self.prompt.read(cx).answered
    }

    /// Whether an answer whose action can fail has an attempt in flight ([`Question::fallible_alternative`]): while it
    /// does, the owner's dismissal hook answers [`Dismissal::Stay`](crate::layers::Dismissal::Stay), so the owner does
    /// not close under the running action.
    pub fn in_flight(&self, cx: &App) -> bool {
        self.prompt.read(cx).in_flight
    }

    /// Press Confirm, as its button does for a key ([`press`]).
    #[cfg(test)]
    pub(crate) fn confirm(&self, window: &mut Window, cx: &mut App) {
        press(&self.prompt, Which::Confirm, window, cx);
    }

    /// Press Cancel, as its button does for a key ([`press`]).
    #[cfg(test)]
    pub(crate) fn cancel(&self, window: &mut Window, cx: &mut App) {
        press(&self.prompt, Which::Cancel, window, cx);
    }

    /// Press the extra answer at `index`, as its button does for a key ([`press`]).
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
    ask_rewordable(question, subject, mark, on_confirm, window, cx).map(|open| open.layer())
}

/// Ask `question` as a dialog, exactly as [`ask`] does, and hand back the open question so its asker can reword it in
/// place while it is open ([`Open::reword`]). For a question whose subject can change under it in a way SPEC.md says
/// rewords the question rather than closing it: the bulk Stop and Delete question, whose targets' titles, hosts and
/// consequences the listing can change while the user reads it.
pub fn ask_rewordable<T: Clone + 'static>(
    question: Question<T>,
    subject: Option<Subject>,
    mark: &Mark,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<Open<T>> {
    ask_inner(
        question,
        Presentation::Centered,
        subject,
        mark,
        Action::Once(Box::new(on_confirm)),
        window,
        cx,
    )
    .map(|(layer, prompt)| Open {
        layer,
        prompt: prompt.downgrade(),
    })
}

/// A centered question [`ask_rewordable`] opened, which its asker keeps to reword it. Holding it keeps nothing open:
/// the question closes as any other does (an answer, Escape, its subject going), and the handle then rewords nothing.
pub struct Open<T: 'static> {
    layer: LayerId,
    prompt: WeakEntity<Prompt<T>>,
}

impl<T: Clone + 'static> Open<T> {
    /// The question's layer, to ask [`layers::is_open`] or to close it with [`layers::close`].
    pub fn layer(&self) -> LayerId {
        self.layer
    }

    /// Replace the open question's words and the value its answers act on with `question`'s, in place: its title, its
    /// summary and lines, its body (built afresh from `question`'s builder and frozen value, as at asking), and the
    /// Confirm answer's label and help. From the next frame on the question shows them, and every answer acts on
    /// `question`'s frozen value: the confirming action passed when it was asked runs with it, and so do the extra
    /// answers' actions. Returns whether it did.
    ///
    /// Everything else stays as the question opened, and the rest of `question` is not read: the layer, Cancel, the
    /// extra answers and their actions, the link, Confirm's tier, whether Confirm is enabled, the empty-click rule,
    /// where closing returns focus, the tile, note, width and scroll stop, and a failed attempt's reason, which still
    /// says what the last press did. A debug build asserts that `question` has as many extra answers and the same
    /// Confirm enabled, since a question differing there would be drawn one way and answered another. Where focus
    /// starts is not read either: the question has started.
    ///
    /// Focus stays where it is. The answers keep their focus handles and the text's scroll stop its own, so an answer
    /// or the stop that held focus still holds it, and nothing closes, opens, moves or animates. A control inside the
    /// old body is gone with it, so focus on one goes to the closest control left, as for any control that goes away
    /// inside a dialog; no question asked this way has one. An answer other than Cancel pressed after this and before a
    /// frame showing the new words is on screen does nothing (see the module docs). A mouse press that began before
    /// the reword and is released after that frame answers the new words, as the old app's did. A Confirm tooltip
    /// already waiting or showing keeps its old help until it goes, as every tooltip keeps the words it began
    /// with.
    ///
    /// Nothing changes, and `false` comes back, once the question has closed (answered, cancelled, dismissed, its
    /// subject gone, or the connection reset; also from inside its own answer's action, which runs once it closed),
    /// and while one of its answers that can fail has an attempt in flight
    /// ([`Question::fallible_alternative`]): that attempt was handed the old value, and a reword under it would show
    /// one thing while the action settles another. [`layers::is_open`] tells the two apart. A closed question is never
    /// reopened here; an asker that still has something to ask asks it afresh. An answer that can fail must not reword
    /// its own question from inside its action: the question is open and being updated there, so the reword panics
    /// rather than returning `false`. No caller does this.
    pub fn reword(&self, question: Question<T>, window: &mut Window, cx: &mut App) -> bool {
        let Some(prompt) = self.prompt.upgrade() else {
            return false;
        };
        // The entity outlives its layer while its own answer's action runs (that action runs inside the prompt's
        // update, after the close), and updating it again from there would panic.
        if !layers::is_open(self.layer, window, cx) {
            return false;
        }
        prompt.update(cx, |prompt, cx| prompt.reword(question, window, cx))
    }
}

/// Ask `question` as a dialog, as [`ask`] does, with a confirming answer whose action can fail: the host remove
/// confirmation's Remove, which stays up through the helm's answer and shows its refusal. Pressing Confirm leaves the
/// question open with every answer frozen, and `on_confirm` runs with the frozen value and an [`Attempt`] it settles,
/// under the rules [`Question::fallible_alternative`] gives an extra answer, which a question asked this way may also
/// carry. A retry runs `on_confirm` again, with a new attempt.
///
/// `question` must be built with [`Question::keeps_focus_on_empty_click`]; without it nothing opens (its
/// [`Question::fallible_alternative`] says why). Escape and any other dismissal do nothing while an attempt is in
/// flight; `subject` going still closes the question, and the attempt's settling then does nothing.
pub fn ask_fallible<T: Clone + 'static>(
    question: Question<T>,
    subject: Option<Subject>,
    mark: &Mark,
    on_confirm: impl Fn(T, Attempt, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    ask_inner(
        question,
        Presentation::Centered,
        subject,
        mark,
        Action::Fallible(Rc::new(on_confirm)),
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
        Action::Once(Box::new(on_confirm)),
        window,
        cx,
    )
    .map(|(layer, _)| layer)
}

/// Ask `question` inside its owner's surface: the owner draws what this returns where SPEC.md puts the question (the
/// launcher's body for the YOLO question, the template editor's foot for its leave question), from the next frame on.
/// Inside a dialog, focus moves to its starting answer once a frame has drawn it there, and the answer is scrolled into
/// view; on the page it moves at once, so a question the owner does not draw in the next frame loses focus to home. It
/// opens even when the user has moved on since `mark`, since it shows where it belongs; then it does not take focus,
/// and waits to be reached. `on_confirm` runs at most once, with the frozen value, if the user confirms. See the module
/// docs for focus, Escape and the looks.
///
/// [`Question::keeps_focus_on_empty_click`] and [`Question::returns_home_on_escape`] do not apply here, and a debug build
/// asserts neither is set (nor a centered question's body, tile, note or width): an inline question's empty space
/// belongs to its owner's dialog, which has its own rule, and Escape on it keeps focus in that dialog, returning it as
/// any answer does. On the page, with no dialog open (a plain Replace's YOLO question), the same holds against the
/// page: Escape cancels and returns focus to what held it when the question opened, and a press on the question's empty
/// space is a press on the page's empty space, which sends focus home.
pub fn ask_inline<T: Clone + 'static>(
    question: Question<T>,
    mark: &Mark,
    on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Inline<T> {
    let on_confirm = Action::Once(Box::new(on_confirm));
    let (prompt, start) = build(question, Shown::Inline, on_confirm, window, cx);
    let opener = window.focused(cx);
    prompt.update(cx, |prompt, _| {
        prompt.opener = opener.as_ref().map(FocusHandle::downgrade)
    });
    move_for_question(&start, mark, window, cx);
    Inline { prompt }
}

/// Move focus into or out of an inline question for the user's action at `mark`: inside the owner's dialog through
/// [`focus::focus_in_layer`], which waits for a starting answer not drawn yet and scrolls it into view, and on the
/// page, with no layer that holds focus open, through the page's own move. Both do nothing once the user has moved
/// on.
fn move_for_question(target: &FocusHandle, mark: &Mark, window: &mut Window, cx: &mut App) {
    if layers::focus_layer_open(window, cx) {
        focus::focus_in_layer(target, mark, window, cx);
    } else {
        focus::focus_control(target, mark, window, cx);
    }
}

/// [`ask_rewordable`] (and through it [`ask`]), [`ask_fallible`] or [`ask_anchored`], also handing back the prompt's
/// entity (for [`ask_rewordable`]'s handle and for tests). Refuses, opening nothing, a question with an answer that can
/// fail that its presentation may not carry ([`Question::may_fail_when`]).
fn ask_inner<T: Clone + 'static>(
    question: Question<T>,
    presentation: Presentation,
    subject: Option<Subject>,
    mark: &Mark,
    on_confirm: Action<T>,
    window: &mut Window,
    cx: &mut App,
) -> Option<(LayerId, gpui::Entity<Prompt<T>>)> {
    let shown = match presentation {
        Presentation::Centered => Shown::Centered,
        Presentation::Anchored(_) => Shown::Anchored,
    };
    let can_fail = std::iter::once(&on_confirm)
        .chain(&question.on_alternatives)
        .any(|action| matches!(action, Action::Fallible(_)));
    if can_fail && !question.may_fail_when(shown) {
        tracing::error!(
            "a question with an answer that can fail was asked through {}; only an inline question or a centered one \
             built with Question::keeps_focus_on_empty_click may carry one",
            shown.asked_through(),
        );
        return None;
    }
    let enabled = question.confirm_enabled;
    let keeps_focus_on_empty_click = question.keeps_focus_on_empty_click;
    let returns_to = question.returns_to.clone();
    let (prompt, start) = build(question, shown, on_confirm, window, cx);
    let weak = prompt.downgrade();
    let main_action: Option<layers::MainAction> = enabled.then(|| {
        let weak = weak.clone();
        Rc::new(move |window: &mut Window, cx: &mut App| {
            let _ = weak.update(cx, |prompt, cx| {
                prompt.answer(Which::Confirm, None, window, cx)
            });
        }) as layers::MainAction
    });
    // Every dismissal (Escape, a control routed through `layers::dismiss`) waits while an attempt is in flight, as the
    // answers do. Only a question that can fail gets the hook: no other is ever in flight, and its dismissals then
    // read nothing of the prompt.
    let on_dismiss: layers::DismissHook = Rc::new(move |_, cx| {
        if weak
            .upgrade()
            .is_some_and(|prompt| prompt.read(cx).in_flight)
        {
            Dismissal::Stay
        } else {
            Dismissal::Close
        }
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
                on_dismiss: can_fail.then_some(on_dismiss),
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

/// The open prompt for `question`, drawn as `shown`, with its answers' focus handles and its body built from the
/// frozen value, and the handle focus starts on.
fn build<T: Clone + 'static>(
    mut question: Question<T>,
    shown: Shown,
    on_confirm: Action<T>,
    window: &mut Window,
    cx: &mut App,
) -> (Entity<Prompt<T>>, FocusHandle) {
    // Every presentation passes here, so this is the one check: a question moved from one presentation to another, or
    // built by a helper shared between two, fails a debug build instead of silently losing its body or its tile.
    debug_assert!(
        question.ignored_by(shown).is_empty(),
        "a question was asked through {} with {}, which it ignores",
        shown.asked_through(),
        question.ignored_by(shown).join(", "),
    );
    let confirm_focus = cx.focus_handle().tab_stop(true);
    let cancel_focus = cx.focus_handle().tab_stop(true);
    let link_focus = cx.focus_handle().tab_stop(true);
    let alternative_focus = question
        .alternatives
        .iter()
        .map(|_| cx.focus_handle().tab_stop(true))
        .collect();
    let on_alternatives = std::mem::take(&mut question.on_alternatives)
        .into_iter()
        .map(Some)
        .collect();
    // Built here, once, from the value the answers act on: what the body shows is fixed from the moment it opens (or,
    // later, is reworded).
    let body = question
        .body
        .take()
        .map(|build| build(&question.frozen, window, cx));
    let start = if question.starts_focus_on_cancel() {
        cancel_focus.clone()
    } else {
        confirm_focus.clone()
    };
    let prompt = cx.new(|_| Prompt {
        question,
        on_confirm: Some(on_confirm),
        on_alternatives,
        layer: None,
        shown,
        body,
        answered: false,
        in_flight: false,
        reason: None,
        opener: None,
        clicked: None,
        focus_at_attempt: None,
        wording: 0,
        drawn_wording: 0,
        confirm_focus,
        cancel_focus,
        alternative_focus,
        link_focus,
        text_scroll: ScrollHandle::new(),
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
        Action::Once(Box::new(on_confirm)),
        window,
        cx,
    )
    .map(|(layer, prompt)| Asked { layer, prompt })
}
