//! Behavior tests for the prompt, on GPUI's simulated platform: that a confirmation acts only on what it showed, where
//! a question starts and how it is answered, cancelled and closed, how its summary draws a host value, the anchored
//! question asked beside its control, the inline question drawn inside its owner's dialog (SPEC.md's YOLO
//! confirmation in the launcher), the answer whose action can fail, which keeps its question up with the reason, and a
//! body the feature draws (the host setup confirmation's plan) with the dialog frame's tile, note and width.
//!
//! Each test opens its window through the shared harness ([`crate::test_support::open`]: GPUI Kit's `Root` around a
//! root view wired as the app's), drives it through simulated keys and clicks, so key bindings resolve as they do in
//! the app, and runs the harness's invariant checks after every step. The rules come from SPEC.md, "Keyboard focus",
//! and SPEC_impl.md, "Questions".

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, Keystroke, Modifiers, MouseButton, ParentElement, Render,
    StatefulInteractiveElement as _, Styled, TestAppContext, VisualTestContext, Window, div, point,
    px,
};

use crate::components::{Button, HoverHelp, Role, Tier};
use crate::focus::{self, Mark, OnPress};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, Anchor, Dialog, LayerId, Subject};
use crate::prompt::{self, Answer, Presentation, Question};
use crate::test_support::{
    Thing, advance, bounds, button, center, check, click, focused, focused_id, frame, handle,
    is_open, keys, open, question, redraw, settle,
};

// ---- The prompt ------------------------------------------------------------------------------------------------------

/// The page a question about a row is asked from: a spot elsewhere to click, then the row's menu button, whose focus
/// handle the test holds and whose anchor an anchored question opens below. `shown` takes the button away, as a
/// deleted row would go.
struct Row {
    anchor: Rc<RefCell<Option<Anchor>>>,
    shown: Rc<Cell<bool>>,
    opener: FocusHandle,
}

impl Row {
    /// Both ways to show a question, the anchored one below this row's menu button.
    fn presentations(&self) -> [(&'static str, Presentation); 2] {
        let Some(anchor) = self.anchor.borrow().clone() else {
            panic!("the premise: the row's menu button was drawn")
        };
        [
            ("centered", Presentation::Centered),
            ("anchored", Presentation::Anchored(anchor)),
        ]
    }
}

/// Open the harness on a [`Row`].
fn open_row(cx: &mut TestAppContext) -> (&mut VisualTestContext, Row) {
    let opener = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let row = Row {
        anchor: Rc::new(RefCell::new(None)),
        shown: Rc::new(Cell::new(true)),
        opener: opener.clone(),
    };
    let (out, shown) = (row.anchor.clone(), row.shown.clone());
    let cx = open(cx, move |window, cx| {
        let mut column = div()
            .flex()
            .flex_col()
            .child(div().debug_selector(|| "elsewhere".into()).size(px(40.)));
        if shown.get() {
            let anchor = Anchor::for_element("row-menu-anchor", window, cx);
            out.replace(Some(anchor.clone()));
            column = column.child(
                div()
                    .relative()
                    .debug_selector(|| "row-menu".into())
                    .w(px(80.))
                    .child(button("opener", &opener))
                    .child(anchor.track()),
            );
        }
        column.into_any_element()
    });
    (cx, row)
}

/// SPEC_impl.md's generic prompt test, for the one prompt kind there is: change the confirmation's subject after it
/// opens, then send Cancel and Confirm with no draw between them, and check that any command sent names the frozen
/// target. Whatever runs acts on what the user was shown. The first answer wins, and Confirm runs at most once. The
/// question carries an extra answer ([`Question::alternative`]), which follows the same rules: it acts on the frozen
/// value, and it is one of the answers the first one wins among.
///
/// Here the frozen value is a plain copy, so the "subject changes" step cannot make the prompt act on something else and
/// cannot fail: the prompt never sees the live subject. That holds by convention, not by the type: `Question<T>` would
/// as happily carry a live handle (an entity, an `Rc<RefCell<_>>`), and a feature's `on_confirm` could capture live
/// state and ignore its argument. The step gains teeth when each real prompt kind runs it with its own subject and
/// command (SPEC_impl.md, "Questions: the prompt"); this test's teeth are the answer ordering (without the
/// first-answer-wins guard, Cancel then Confirm would send).
///
/// It runs for both layer presentations, centered and anchored: the anchored one is a different layer kind with its own
/// answer wiring, and the guarantee must not depend on where the question is drawn. The inline presentation shares
/// the same answer path and has its own no-draw case in
/// [`an_inline_question_answers_once_and_returns_focus_inside_its_dialog`].
#[gpui::test]
fn a_confirmation_acts_only_on_what_it_showed(cx: &mut TestAppContext) {
    let (cx, row) = open_row(cx);
    let thing = cx.update(|_, cx| {
        cx.new(|_| Thing {
            name: "alpha".into(),
        })
    });
    let sent = Rc::new(RefCell::new(Vec::<String>::new()));

    for ((kind, presentation), order) in row.presentations().into_iter().flat_map(|shown| {
        [
            ["cancel", "confirm"],
            ["confirm", "cancel"],
            ["confirm", "confirm"],
            ["alternative", "confirm"],
            ["confirm", "alternative"],
            ["alternative", "alternative"],
            ["cancel", "alternative"],
        ]
        .map(|order| (shown.clone(), order))
    }) {
        thing.update(cx, |thing, _| thing.name = "alpha".into());
        sent.borrow_mut().clear();
        let shown = thing.read_with(cx, |thing, _| thing.name.clone());
        let (record, chosen) = (sent.clone(), sent.clone());
        let asked = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            prompt::ask_for_test(
                presentation,
                question(&shown).alternative(keep_it_answer(), move |frozen: String, _, _| {
                    chosen.borrow_mut().push(format!("kept {frozen}"))
                }),
                Some(Subject::of(&thing)),
                &mark,
                move |frozen, _, _| record.borrow_mut().push(frozen),
                window,
                cx,
            )
        });
        cx.run_until_parked();
        check(cx);
        let asked = asked.expect("the question opened");

        // The subject changes after the question opened.
        thing.update(cx, |thing, _| thing.name = "beta".into());
        // Both answers, with no draw between them.
        cx.update(|window, cx| {
            for answer in order {
                match answer {
                    "cancel" => asked.cancel(window, cx),
                    "alternative" => asked.alternative(0, window, cx),
                    _ => asked.confirm(window, cx),
                }
            }
        });
        cx.run_until_parked();
        check(cx);
        let expected: Vec<String> = match order[0] {
            "confirm" => vec!["alpha".into()],
            "alternative" => vec!["kept alpha".into()],
            _ => Vec::new(),
        };
        assert_eq!(
            *sent.borrow(),
            expected,
            "{kind}, answers {order:?}: only the first ran, on the frozen target"
        );
        assert!(!is_open(cx, asked.layer), "{kind}: the question closed");
    }
}

/// One start-rule case: its name, how the question is adjusted, and whether focus starts on the confirming answer.
type StartCase = (&'static str, fn(Question<String>) -> Question<String>, bool);

/// Where a question starts (SPEC.md, "Opening and closing"): on its confirming button when the user just asked for it,
/// on Cancel when that button is disabled, and when the action is risky or the question arrived later (one builder,
/// [`Question::starts_on_cancel`], covers both). Enter then
/// answers what has focus, so a wrong start is a wrong answer.
#[gpui::test]
fn a_question_starts_on_the_answer_the_rules_pick(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let cases: [StartCase; 3] = [
        ("asked for", |q| q, true),
        ("risky or arrived later", Question::starts_on_cancel, false),
        ("disabled", |q| q.confirm_enabled(false), false),
    ];
    for (case, adjust, starts_on_confirm) in cases {
        let sent = Rc::new(Cell::new(0));
        let record = sent.clone();
        let asked = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            prompt::ask_for_test(
                Presentation::Centered,
                adjust(question("alpha")),
                None,
                &mark,
                move |_, _, _| record.set(record.get() + 1),
                window,
                cx,
            )
        });
        cx.run_until_parked();
        check(cx);
        let asked = asked.expect("the question opened");
        keys(cx, "enter");
        assert_eq!(
            sent.get(),
            usize::from(starts_on_confirm),
            "{case}: Enter answered the start"
        );
        assert!(
            !is_open(cx, asked.layer),
            "{case}: Enter answered and closed it"
        );
    }
}

/// An extra answer for the test questions: "Keep it", a neutral answer that runs its own action.
fn keep_it_answer() -> Answer {
    Answer {
        label: "Keep it".into(),
        help: HoverHelp::tooltip("Keep it, and stop asking"),
        tier: Tier::Neutral,
    }
}

/// One extra-answers case: its name, how the question is adjusted, the keys pressed, and which answer ran.
type ExtraAnswerCase = (
    &'static str,
    fn(Question<String>) -> Question<String>,
    &'static str,
    &'static str,
);

/// The extra answers must not change where a question starts or what Enter does there: SPEC.md's template leave
/// question keeps "initial focus still on Save", so a question that started on an extra answer would discard a draft on
/// Enter. And their place in reading and Tab order is between Cancel and Confirm, in the order added (SPEC.md's YOLO
/// confirmation: "Cancel, the guarded option when offered, the outlined permanent YOLO answer, then the filled one-time
/// YOLO answer"; the leave question's "Keep editing, Discard, or Save").
///
/// Specified, for a question shaped like the YOLO one (two extra answers, "guarded" then "always"): Enter at the start
/// answers Confirm; Shift+Tab from Confirm reaches "always", then "guarded", then Cancel, and Enter on each runs that
/// answer alone (Cancel runs nothing). A question that starts on Cancel still does.
#[gpui::test]
fn extra_answers_sit_in_order_between_cancel_and_confirm_and_never_start(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let cases: [ExtraAnswerCase; 5] = [
        ("at the start", |q| q, "enter", "confirmed"),
        ("one back", |q| q, "shift-tab enter", "always"),
        ("two back", |q| q, "shift-tab shift-tab enter", "guarded"),
        (
            "three back",
            |q| q,
            "shift-tab shift-tab shift-tab enter",
            "",
        ),
        (
            "starting on Cancel",
            Question::starts_on_cancel,
            "enter",
            "",
        ),
    ];
    for (case, adjust, keystrokes, expected) in cases {
        let ran = Rc::new(RefCell::new(String::new()));
        let (confirmed, guarded, always) = (ran.clone(), ran.clone(), ran.clone());
        let asked = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            prompt::ask_for_test(
                Presentation::Centered,
                adjust(question("alpha"))
                    .alternative(keep_it_answer(), move |_, _, _| {
                        guarded.borrow_mut().push_str("guarded")
                    })
                    .alternative(keep_it_answer(), move |_, _, _| {
                        always.borrow_mut().push_str("always")
                    }),
                None,
                &mark,
                move |_, _, _| confirmed.borrow_mut().push_str("confirmed"),
                window,
                cx,
            )
        });
        cx.run_until_parked();
        check(cx);
        let asked = asked.expect("the question opened");
        keys(cx, keystrokes);
        assert_eq!(
            *ran.borrow(),
            expected,
            "{case}: what {keystrokes:?} answered"
        );
        assert!(!is_open(cx, asked.layer), "{case}: the answer closed it");
    }
}

/// The old app's questions said what their Cancel keeps ("Cancel: keep the agent and terminal tabs running" on Stop,
/// Replace and Restart), and the template editor's leave question calls its Cancel "Keep editing": a question words
/// its own Cancel, but Cancel must still be Cancel, or a relabelled button could start acting.
///
/// Specified: a question given its own Cancel answer shows that answer's hover help on its Cancel button after the
/// hover delay, and a click on that button still cancels it, running nothing.
#[gpui::test]
fn a_question_can_word_its_own_cancel(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let ran = Rc::new(Cell::new(0));
    let record = ran.clone();
    let asked = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask_for_test(
            Presentation::Centered,
            question("alpha").cancel_answer(Answer {
                label: "Cancel".into(),
                help: HoverHelp::tooltip("Cancel: keep the agent and terminal tabs running"),
                tier: Tier::Neutral,
            }),
            None,
            &mark,
            move |_, _, _| record.set(record.get() + 1),
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    let asked = asked.expect("the question opened");
    let cancel = center(cx, "prompt-cancel");
    cx.simulate_mouse_move(cancel, None, Modifiers::none());
    advance(cx, layers::HOVER_DELAY);
    assert_eq!(
        cx.update(|window, cx| layers::shown_tooltip(window, cx))
            .map(|shown| shown.text),
        Some("Cancel: keep the agent and terminal tabs running".into()),
        "Cancel shows the question's own help"
    );
    click(cx, "prompt-cancel");
    assert_eq!(
        ran.get(),
        0,
        "the relabelled Cancel cancelled it, running nothing"
    );
    assert!(!is_open(cx, asked.layer), "and closed it");
}

/// Which questions keep focus on the answer in use when their empty space is clicked is each question's own rule
/// ([`Question::keeps_focus_on_empty_click`]): SPEC.md gives it to the Stop, Restart, Replace and Delete questions.
/// Without it a click on the question's text focuses the question itself, and a following Enter presses Confirm; with
/// it, focus stays on Cancel (where a risky question starts) and Enter cancels. The opt-in's plumbing through the
/// question to its dialog is what this guards: lose it and those four questions confirm on a stray click and Enter.
/// Both presentations: the row's questions are the anchored ones.
#[gpui::test]
fn a_question_that_opts_in_keeps_focus_on_an_empty_space_click(cx: &mut TestAppContext) {
    let (cx, row) = open_row(cx);
    for ((kind, presentation), (case, keeps, sent_after_enter)) in
        row.presentations().into_iter().flat_map(|shown| {
            [("opted in", true, 0), ("not opted in", false, 1)].map(|case| (shown.clone(), case))
        })
    {
        let sent = Rc::new(Cell::new(0));
        let record = sent.clone();
        let asked = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            let question = question("alpha").starts_on_cancel();
            let question = if keeps {
                question.keeps_focus_on_empty_click()
            } else {
                question
            };
            prompt::ask_for_test(
                presentation,
                question,
                None,
                &mark,
                move |_, _, _| record.set(record.get() + 1),
                window,
                cx,
            )
        });
        cx.run_until_parked();
        check(cx);
        let asked = asked.expect("the question opened");
        // Empty space: the centered body's padding, or the anchored panel's own padding.
        let empty = if kind == "centered" {
            "prompt-body"
        } else {
            "prompt"
        };
        let body = match cx.debug_bounds(empty) {
            Some(bounds) => bounds,
            None => panic!("{kind}, {case}: the question was not drawn"),
        };
        cx.simulate_click(
            point(body.left() + px(4.), body.top() + px(4.)),
            Modifiers::none(),
        );
        check(cx);
        assert!(
            is_open(cx, asked.layer),
            "{kind}, {case}: the click on its empty space left the question open"
        );
        keys(cx, "enter");
        assert_eq!(
            sent.get(),
            sent_after_enter,
            "{kind}, {case}: Enter after the click answered what kept focus"
        );
    }
}

/// Escape cancels a question and returns focus to where it was; nothing is sent.
#[gpui::test]
fn escape_cancels_a_question_and_returns_focus(cx: &mut TestAppContext) {
    let target = Rc::new(RefCell::new(None));
    let drawn = target.clone();
    let cx = open(cx, move |_, _| {
        div()
            .children(
                drawn
                    .borrow()
                    .clone()
                    .map(|focus| button("row-delete", &focus)),
            )
            .into_any_element()
    });
    let opener = handle(cx);
    target.replace(Some(opener.clone()));
    redraw(cx);
    keys(cx, "tab");
    let sent = Rc::new(Cell::new(false));
    let record = sent.clone();
    let layer = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask(
            question("alpha"),
            None,
            &mark,
            move |_, _, _| record.set(true),
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    keys(cx, "escape");
    assert!(!sent.get(), "Escape sent nothing");
    assert!(!is_open(cx, layer.unwrap()), "Escape closed the question");
    assert!(
        focused(cx, &opener),
        "focus returned to the control that opened it"
    );
}

/// A question about a thing closes when the thing is gone (a session deleted from another window while its delete
/// confirmation was open), and nothing is sent.
#[gpui::test]
fn a_question_closes_when_its_subject_is_gone(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let thing: Entity<Thing> = cx.update(|_, cx| {
        cx.new(|_| Thing {
            name: "alpha".into(),
        })
    });
    let sent = Rc::new(Cell::new(false));
    let record = sent.clone();
    let subject = Subject::of(&thing);
    let layer = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask(
            question("alpha"),
            Some(subject),
            &mark,
            move |_, _, _| record.set(true),
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    drop(thing);
    cx.run_until_parked();
    check(cx);
    assert!(
        !is_open(cx, layer.unwrap()),
        "the question closed with its subject"
    );
    assert!(!sent.get());
}

/// Ask `question` with no subject, draw it with GPUI's driver record on, and hand back that frame's record: every node
/// drawn and the text each painted. The question stays open; the caller answers it.
fn ask_and_record(
    cx: &mut VisualTestContext,
    question: Question<String>,
) -> (prompt::Asked<String>, Vec<gpui::DriverNode>) {
    let asked = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask_for_test(
            Presentation::Centered,
            question,
            None,
            &mark,
            |_, _, _| {},
            window,
            cx,
        )
    });
    let Some(asked) = asked else {
        panic!("the question did not open")
    };
    (asked, frame(cx))
}

/// Why this matters: a prompt's summary is where a confirmation names its target, and the first real one (a session's
/// delete confirmation) names a session whose title a host wrote. When the summary took only a string, the one way to
/// put that title there was its raw value, so an override character in it would reverse the summary on screen and a
/// zero-width one would make two sessions' confirmations look identical, to a screen reader too (SPEC.md: "including
/// in accessible names"). A long title must wrap, not run past the dialog's edge: while a sentence's parts could not
/// shrink below their one-line width, a 108-character title was clipped at the border, so two sessions whose titles
/// share a long start got confirmations that looked the same, the very problem escaping exists to prevent. The
/// plain-string case guards the other direction: every existing question passes app wording as a string, and it must
/// draw as it did before.
///
/// Specification: a question whose summary is a [`HostSentence`] paints the host value in its escaped presentation,
/// and no text the frame paints holds a presentation-unsafe character; inside the dialog the sentence is one `Label`
/// node under its id, named with the value escaped; the dialog's own accessible name is still the question's title.
/// With a summary naming a title too long for one line and a sentence line of app words too long for one line, every
/// text painted inside the dialog lies within the dialog's left and right edges and is not clipped. A question whose
/// summary is a plain string paints that string as one line, exactly as given.
#[gpui::test]
fn a_summary_that_names_a_host_value_is_drawn_and_named_escaped(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    // Inside the dialog drawn under `prompt`, at any depth.
    let inside_dialog = |nodes: &[gpui::DriverNode], mut index: usize| {
        while let Some(parent) = nodes[index].parent {
            if nodes[parent].element_id.as_deref() == Some("prompt") {
                return true;
            }
            index = parent;
        }
        false
    };
    // The accessible name of the dialog drawn under `prompt`, if it was recorded as a dialog.
    let dialog_label = |nodes: &[gpui::DriverNode]| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some("prompt"))
            .filter(|node| node.role == Some(Role::Dialog))
            .and_then(|node| node.label.clone())
    };
    // Every line of text the frame painted, in record order.
    let painted = |nodes: &[gpui::DriverNode]| -> Vec<String> {
        nodes
            .iter()
            .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
            .collect()
    };

    // A host value with a right-to-left override and a zero-width space, in the summary.
    let title = HostText::from_host("invoice\u{202E}fdp\u{200B}.exe");
    let summary = HostSentence::new("prompt-summary")
        .text("Delete ")
        .host(&title)
        .text(" on this machine?");
    let (asked, nodes) = ask_and_record(cx, question("alpha").summary(summary));
    let texts = painted(&nodes);
    assert!(
        texts
            .iter()
            .any(|text| text == "invoice<U+202E>fdp<U+200B>.exe"),
        "the host value is painted escaped; painted: {texts:?}"
    );
    assert!(
        !texts.iter().any(|text| text
            .chars()
            .any(farhelm_proto::text::is_presentation_unsafe)),
        "nothing painted holds a hidden character; painted: {texts:?}"
    );
    let Some(sentence) = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some("prompt-summary"))
    else {
        let ids: Vec<_> = nodes.iter().map(|node| &node.element_id).collect();
        panic!("the summary sentence was not recorded; recorded ids: {ids:?}");
    };
    assert!(
        inside_dialog(&nodes, sentence),
        "the sentence is drawn inside the dialog"
    );
    assert_eq!(nodes[sentence].role, Some(Role::Label));
    assert_eq!(
        nodes[sentence].label.as_deref(),
        Some("Delete invoice<U+202E>fdp<U+200B>.exe on this machine?")
    );
    assert_eq!(dialog_label(&nodes).as_deref(), Some("Delete it?"));
    cx.update(|window, cx| asked.cancel(window, cx));
    cx.run_until_parked();
    check(cx);

    // App wording as a string, as every question before host sentences passed it.
    let (asked, nodes) = ask_and_record(cx, question("api-refactor, on this machine"));
    let texts = painted(&nodes);
    assert!(
        texts
            .iter()
            .any(|text| text == "api-refactor, on this machine"),
        "the string summary is painted as given; painted: {texts:?}"
    );
    assert_eq!(dialog_label(&nodes).as_deref(), Some("Delete it?"));
    cx.update(|window, cx| asked.cancel(window, cx));
    cx.run_until_parked();
    check(cx);

    // A title with no spaces, far wider than the dialog, and a line of app words that is too.
    let long = "a-very-long-session-title-that-a-host-wrote-without-any-spaces-\
                at-all-so-it-cannot-wrap-at-a-word-boundary";
    let words = "The session's terminal and its history are removed, and the working directory stays where it is on \
                 the host.";
    let summary = HostSentence::new("prompt-summary")
        .text("Delete ")
        .host(&HostText::from_host(long))
        .text(" on this machine?");
    let line = HostSentence::new("prompt-line").text(words);
    let (asked, nodes) = ask_and_record(cx, question("alpha").summary(summary).line(line));
    let Some(dialog) = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("prompt"))
    else {
        panic!("the dialog was not recorded")
    };
    let inside: Vec<_> = (0..nodes.len())
        .filter(|&index| inside_dialog(&nodes, index))
        .flat_map(|index| nodes[index].texts.iter())
        .collect();
    for expected in [long, words] {
        assert!(
            inside.iter().any(|text| text.text.as_ref() == expected),
            "the premise: {expected:?} is painted inside the dialog"
        );
    }
    // Text bounds are floats from layout, so allow half a pixel of rounding at each edge.
    let slack = px(0.5);
    for text in &inside {
        assert!(
            text.bounds.left() >= dialog.bounds.left() - slack
                && text.bounds.right() <= dialog.bounds.right() + slack,
            "{:?} is painted within the dialog's edges: text {:?}, dialog {:?}",
            text.text,
            text.bounds,
            dialog.bounds
        );
        assert!(
            text.visible_bounds.size.width >= text.bounds.size.width - slack,
            "{:?} is not clipped: painted {:?}, visible {:?}",
            text.text,
            text.bounds,
            text.visible_bounds
        );
    }
    cx.update(|window, cx| asked.cancel(window, cx));
    cx.run_until_parked();
    check(cx);
}

/// Ask `question("alpha")` below the row's menu button, counting confirmations in `sent`.
fn ask_below(
    cx: &mut VisualTestContext,
    row: &Row,
    subject: Option<Subject>,
    sent: &Rc<Cell<usize>>,
) -> LayerId {
    let Some(anchor) = row.anchor.borrow().clone() else {
        panic!("the premise: the row's menu button was drawn")
    };
    let record = sent.clone();
    let layer = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask_anchored(
            question("alpha"),
            anchor,
            subject,
            &mark,
            move |_, _, _| record.set(record.get() + 1),
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    match layer {
        Some(layer) => layer,
        None => panic!("the anchored question did not open"),
    }
}

/// Why this matters: the row's Delete question is asked beside the row it is about, as the old app's was (the row's
/// menu turned into the question), and it carries a destructive answer. It must keep every rule a centered question
/// keeps, and close in the ways a menu closes, or a question about a row that went away would stay on screen
/// answering for it.
///
/// Specification: the question is drawn just below its anchor, aligned with it. Focus starts on Confirm, Enter confirms
/// once and focus returns to the control that held it. Escape closes it unanswered and returns focus. Tab stays inside
/// it. A press outside closes it unanswered. It closes unanswered when its anchor is no longer drawn, and when its
/// subject is gone.
#[gpui::test]
fn an_anchored_question_sits_below_its_anchor_and_closes_like_a_menu(cx: &mut TestAppContext) {
    let (cx, row) = open_row(cx);
    keys(cx, "tab");
    assert!(
        focused(cx, &row.opener),
        "the premise: the row's menu button has focus"
    );
    let sent = Rc::new(Cell::new(0));

    let layer = ask_below(cx, &row, None, &sent);
    let (drawn, anchor) = (bounds(cx, "prompt"), bounds(cx, "row-menu"));
    assert_eq!(
        (drawn.top(), drawn.left()),
        (anchor.bottom() + px(4.), anchor.left()),
        "the question sits just below its anchor, aligned with it: {drawn:?} under {anchor:?}"
    );
    keys(cx, "enter");
    assert_eq!(sent.get(), 1, "Enter pressed Confirm, where focus started");
    assert!(!is_open(cx, layer));
    assert!(
        focused(cx, &row.opener),
        "focus went back to the row's menu button"
    );

    let layer = ask_below(cx, &row, None, &sent);
    for _ in 0..3 {
        keys(cx, "tab");
        assert!(!focused(cx, &row.opener), "Tab stays inside the question");
    }
    keys(cx, "escape");
    assert!(!is_open(cx, layer), "Escape closed it");
    assert!(focused(cx, &row.opener), "and focus went back");

    let layer = ask_below(cx, &row, None, &sent);
    click(cx, "elsewhere");
    assert!(!is_open(cx, layer), "a press outside closed it");

    let layer = ask_below(cx, &row, None, &sent);
    row.shown.set(false);
    redraw(cx);
    assert!(!is_open(cx, layer), "it closed when its anchor went");
    row.shown.set(true);
    redraw(cx);

    let thing = cx.update(|_, cx| {
        cx.new(|_| Thing {
            name: "alpha".into(),
        })
    });
    let layer = ask_below(cx, &row, Some(Subject::of(&thing)), &sent);
    drop(thing);
    cx.run_until_parked();
    check(cx);
    assert!(!is_open(cx, layer), "it closed when its subject went");
    assert_eq!(
        sent.get(),
        1,
        "nothing but the first Enter confirmed anything"
    );
}

/// The page of [`a_click_on_an_anchored_question_over_a_terminal_stays_with_the_question`]: the row's menu button the
/// question is anchored below, then, right under it, a terminal the question's card covers part of.
///
/// The terminal is a stand-in: an element tracked the way the attached terminal tracks its grid
/// (`focus::track(.., OnPress::Focus)` in `terminal/attached.rs`), which is everything the real one does with a press.
/// The real terminal needs an attachment to a session to draw, and none of that bears on where a press lands.
struct OverTerminal {
    anchor: Rc<RefCell<Option<Anchor>>>,
    opener: FocusHandle,
    terminal: FocusHandle,
    /// Clicks and wheel events that reached the terminal stand-in.
    reached: Rc<Cell<(usize, usize)>>,
}

/// Open the harness on an [`OverTerminal`] page.
fn open_over_terminal(cx: &mut TestAppContext) -> (&mut VisualTestContext, OverTerminal) {
    let (opener, terminal) = cx.update(|cx| {
        (
            cx.focus_handle().tab_stop(true),
            cx.focus_handle().tab_stop(true),
        )
    });
    let page = OverTerminal {
        anchor: Rc::new(RefCell::new(None)),
        opener: opener.clone(),
        terminal: terminal.clone(),
        reached: Rc::new(Cell::new((0, 0))),
    };
    let (out, reached) = (page.anchor.clone(), page.reached.clone());
    let cx = open(cx, move |window, cx| {
        let anchor = Anchor::for_element("row-menu-anchor", window, cx);
        out.replace(Some(anchor.clone()));
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .relative()
                    .debug_selector(|| "row-menu".into())
                    .w(px(80.))
                    .child(button("opener", &opener))
                    .child(anchor.track()),
            )
            .child({
                let (clicked, scrolled) = (reached.clone(), reached.clone());
                focus::track(
                    div()
                        .id("terminal")
                        .debug_selector(|| "terminal".into())
                        .size(px(400.))
                        .on_click(move |_, _, _| {
                            let (clicks, wheels) = clicked.get();
                            clicked.set((clicks + 1, wheels));
                        })
                        .on_scroll(move |_, _, _| {
                            let (clicks, wheels) = scrolled.get();
                            scrolled.set((clicks, wheels + 1));
                        }),
                    &terminal,
                    OnPress::Focus,
                )
            })
            .into_any_element()
    });
    (cx, page)
}

/// Why this matters: a row's Delete question (and the Stop and Replace questions to come) is anchored beside the row, and in the app its card sits
/// over the selected session's terminal. A click on one of its answers was also a press on the terminal under the card,
/// which takes focus on every press, so focus moved to the terminal before the answer ran; the question then closed
/// with focus outside it and left it there, instead of returning it where Enter returns it. A click on the card's empty
/// space reached the terminal the same way, taking focus out of a question still open, so the next keys went to the
/// program instead of the question.
///
/// Specification: with the question's card over a terminal, a click on Confirm confirms once, closes the question and
/// returns focus to the control that held it when the question opened; a click on Cancel closes it unanswered and
/// returns focus the same way; a click on the card's empty space leaves the question open with focus on the answer in
/// use, so Enter then confirms it. In none of these does the terminal take focus, and no click and no wheel turned over
/// the card reaches it.
#[gpui::test]
fn a_click_on_an_anchored_question_over_a_terminal_stays_with_the_question(
    cx: &mut TestAppContext,
) {
    let (cx, page) = open_over_terminal(cx);
    keys(cx, "tab");
    assert!(
        focused(cx, &page.opener),
        "the premise: the row's menu button has focus"
    );
    let Some(anchor) = page.anchor.borrow().clone() else {
        panic!("the premise: the row's menu button was drawn")
    };
    let sent = Rc::new(Cell::new(0));
    let ask = |cx: &mut VisualTestContext| {
        let (anchor, record) = (anchor.clone(), sent.clone());
        let layer = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            prompt::ask_anchored(
                question("alpha").keeps_focus_on_empty_click(),
                anchor,
                None,
                &mark,
                move |_, _, _| record.set(record.get() + 1),
                window,
                cx,
            )
        });
        cx.run_until_parked();
        check(cx);
        match layer {
            Some(layer) => layer,
            None => panic!("the anchored question did not open"),
        }
    };
    let terminal = bounds(cx, "terminal");

    for (answer, confirms) in [("prompt-confirm", 1), ("prompt-cancel", 0)] {
        sent.set(0);
        let layer = ask(cx);
        let at = center(cx, answer);
        assert!(
            terminal.contains(&at),
            "the premise: {answer} is drawn over the terminal: {at:?} in {terminal:?}"
        );
        // The press and the release apart: an answer runs on the release, so the press alone shows whether the
        // terminal under the card saw it, while the question is still open.
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        check(cx);
        assert!(is_open(cx, layer), "the press on {answer} left it open");
        assert!(
            !focused(cx, &page.terminal),
            "the press on {answer} did not focus the terminal under the card"
        );
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        check(cx);
        assert_eq!(sent.get(), confirms, "the click on {answer} answered it");
        assert!(!is_open(cx, layer), "the click on {answer} closed it");
        assert!(
            !focused(cx, &page.terminal),
            "the click on {answer} left the terminal under the card unfocused"
        );
        assert!(
            focused(cx, &page.opener),
            "after {answer}, focus went back to the row's menu button"
        );
    }

    // The card's bottom padding, below the answers: empty space, and over the terminal.
    let layer = ask(cx);
    let card = bounds(cx, "prompt");
    let empty = point(card.left() + px(4.), card.bottom() - px(4.));
    assert!(
        terminal.contains(&empty),
        "the premise: the card's empty space is over the terminal: {empty:?} in {terminal:?}"
    );
    cx.simulate_click(empty, Modifiers::none());
    check(cx);
    assert!(is_open(cx, layer), "the click on empty space left it open");
    assert!(
        !focused(cx, &page.terminal),
        "the click on the card's empty space did not focus the terminal under it"
    );
    assert_eq!(sent.get(), 0, "the click on empty space answered nothing");
    cx.simulate_event(gpui::ScrollEvent {
        position: empty,
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-40.))),
        ..Default::default()
    });
    check(cx);
    assert_eq!(
        page.reached.get(),
        (0, 0),
        "no click and no wheel on the card reached the terminal under it"
    );
    // Focus stayed on Confirm, where this question started, so Enter presses it.
    keys(cx, "enter");
    assert_eq!(sent.get(), 1, "Enter after the click confirmed it");
    assert!(!is_open(cx, layer), "Enter closed it");
    assert!(focused(cx, &page.opener), "and focus went back");
}

/// SPEC.md, "Keyboard focus": "Escape cancels the header's … question and returns focus to the terminal", while every
/// other close (an answer, Cancel included) returns focus to where it was before the question opened, and a row's
/// questions return to the row's menu button on Escape too. A header question whose Escape went back to its button
/// would leave the user's next keys off the terminal they were working in.
///
/// Specified, for a question drawn beside its control and one drawn centered, with a drawn home (standing in for the
/// selected terminal) and the opener focused: the open question holds focus; a question built with
/// `returns_home_on_escape` sends focus home on Escape and back to the opener on a click on Cancel; a question built
/// without it sends focus back to the opener on Escape.
#[gpui::test]
fn escape_on_a_question_can_return_focus_home(cx: &mut TestAppContext) {
    let (opener, home) = cx.update(|cx| {
        (
            cx.focus_handle().tab_stop(true),
            cx.focus_handle().tab_stop(true),
        )
    });
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let (drawn_opener, drawn_home, out) = (opener.clone(), home.clone(), anchor_out.clone());
    let cx = open(cx, move |window, cx| {
        let anchor = Anchor::for_element("header-anchor", window, cx);
        out.replace(Some(anchor.clone()));
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .relative()
                    .w(px(80.))
                    .child(button("opener", &drawn_opener))
                    .child(anchor.track()),
            )
            .child(button("terminal", &drawn_home))
            .into_any_element()
    });
    let terminal = home.clone();
    cx.update(move |window, cx| crate::focus::set_home(Some(&terminal), window, cx));
    let anchored = anchor_out
        .borrow()
        .clone()
        .expect("the page drew its anchor");
    for presentation in [Presentation::Centered, Presentation::Anchored(anchored)] {
        for (built, close, goes_home) in [
            (true, "escape", true),
            (true, "cancel", false),
            (false, "escape", false),
        ] {
            let case = format!("built: {built}, {close}");
            click(cx, "opener");
            assert!(
                focused(cx, &opener),
                "{case}: the premise: the opener has focus"
            );
            let shown = presentation.clone();
            let asked = cx.update(|window, cx| {
                let mark = Mark::take(cx);
                let question = question("alpha");
                let question = if built {
                    question.returns_home_on_escape()
                } else {
                    question
                };
                prompt::ask_for_test(shown, question, None, &mark, |_, _, _| {}, window, cx)
            });
            cx.run_until_parked();
            check(cx);
            let asked = asked.expect("the question opened");
            assert!(
                !focused(cx, &opener) && !focused(cx, &home),
                "{case}: the premise: the open question holds focus"
            );
            if close == "escape" {
                keys(cx, "escape");
            } else {
                click(cx, "prompt-cancel");
            }
            assert!(!is_open(cx, asked.layer), "{case}: it closed");
            let (to, from) = if goes_home {
                (&home, &opener)
            } else {
                (&opener, &home)
            };
            assert!(
                focused(cx, to) && !focused(cx, from),
                "{case}: where focus went"
            );
        }
    }
}

// ---- The inline question ---------------------------------------------------------------------------------------------

/// Where a test keeps the inline question its launcher draws, shared between the test and the launcher's view.
type InlineCell = Rc<RefCell<Option<prompt::Inline<String>>>>;

/// The dialog an inline question is asked in, standing in for the session launcher: its Launch button at the top, a
/// Reset button lower down, then the question, drawn while the test's cell holds it, as the launcher draws its YOLO
/// question in its body. The test holds both buttons' focus handles.
///
/// Reset sits closer to the question than Launch does, so it is what the focus coordinator's recovery would pick if a
/// closing question let focus drop: a test that sees focus back on Launch knows the question returned it.
struct Launcher {
    launch: FocusHandle,
    reset: FocusHandle,
    question: InlineCell,
}

impl Render for Launcher {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "launcher".into())
            .w(px(1100.))
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().w(px(80.)).child(button("launch", &self.launch)))
            .child(div().h(px(40.)))
            .child(div().w(px(80.)).child(button("reset", &self.reset)))
            .children(self.question.borrow().clone())
    }
}

/// An open [`Launcher`]: its layer, its buttons' focus handles (focus starts on Launch) and the question's cell.
struct OpenLauncher {
    layer: LayerId,
    launch: FocusHandle,
    reset: FocusHandle,
    question: InlineCell,
}

/// Open the harness with a [`Launcher`] dialog over an empty page, focus on its Launch button. The dialog has no
/// dismissal hook, so Escape that reaches it closes it: a test can tell whether the question took the key.
fn open_launcher(cx: &mut TestAppContext) -> (&mut VisualTestContext, OpenLauncher) {
    let cx = open(cx, |_, _| div().into_any_element());
    let (launch, reset, container) = (handle(cx), handle(cx), handle(cx));
    let question: InlineCell = Rc::default();
    let (start, drawn, lower) = (launch.clone(), question.clone(), reset.clone());
    let layer = cx.update(move |window, cx| {
        let view = cx.new(|_| Launcher {
            launch: start.clone(),
            reset: lower,
            question: drawn,
        });
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container,
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
                returns_to: focus::ReturnTo::Opener,
                on_dismiss: None,
                placement: layers::DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            &mark,
            window,
            cx,
        )
    });
    settle(cx);
    let Some(layer) = layer else {
        panic!("the launcher did not open")
    };
    assert!(
        focused(cx, &launch),
        "the premise: the launcher's Launch button has focus"
    );
    (
        cx,
        OpenLauncher {
            layer,
            launch,
            reset,
            question,
        },
    )
}

/// What the test question's answers ran, each as "<answer> <frozen value>", in order.
type Ran = Rc<RefCell<Vec<String>>>;

/// The attempts the permanent answer was handed, oldest first, for the test to settle.
type Attempts = Rc<RefCell<Vec<prompt::Attempt>>>;

/// SPEC.md's YOLO confirmation as the launcher asks it, with the old app's words: Cancel, the guarded option, the
/// outlined permanent answer, then the filled one-time answer, which confirms. The permanent answer can fail, as the
/// real one does (it changes the host's setting first): each press records its run in `ran` and its attempt in
/// `attempts` for the test to settle. It starts on Cancel and is drawn loud.
fn yolo(ran: &Ran, attempts: &Attempts) -> Question<String> {
    let (guarded, always, kept) = (ran.clone(), ran.clone(), attempts.clone());
    Question::new(
        "Confirm YOLO launch",
        Answer {
            label: "Start YOLO session anyway".into(),
            help: HoverHelp::tooltip(
                "Start the session in YOLO mode, where the agent acts without asking",
            ),
            tier: Tier::Danger,
        },
        "codex on build-box".to_string(),
    )
    .summary("YOLO means the agent bypasses its safety checks.")
    .line("Nothing has been started.")
    .cancel_answer(Answer {
        label: "Cancel".into(),
        help: HoverHelp::tooltip("Cancel: go back without starting anything"),
        tier: Tier::Neutral,
    })
    .alternative(
        Answer {
            label: "Start guarded session instead".into(),
            help: HoverHelp::tooltip("Start this session in guarded mode instead"),
            tier: Tier::Primary,
        },
        move |frozen, _, _| guarded.borrow_mut().push(format!("guarded {frozen}")),
    )
    .fallible_alternative(
        Answer {
            label: "Start, and don't ask again on this host".into(),
            help: HoverHelp::tooltip(
                "Start it, and start YOLO sessions on this host without asking from now on",
            ),
            tier: Tier::DangerOutline,
        },
        move |frozen, attempt, _, _| {
            always.borrow_mut().push(format!("always {frozen}"));
            kept.borrow_mut().push(attempt);
        },
    )
    .starts_on_cancel()
    .loud()
}

/// Ask `question` inline in the open launcher, with a mark taken now (the user just pressed Launch and the helm's
/// refusal came back), confirming into `ran`, and draw it. The cell is filled inside the same update, so the frame
/// that follows already draws the question its starting answer took focus in.
fn ask_inline_in(
    cx: &mut VisualTestContext,
    launcher: &OpenLauncher,
    question: Question<String>,
    ran: &Ran,
) -> prompt::Inline<String> {
    let (cell, confirmed) = (launcher.question.clone(), ran.clone());
    let inline = cx.update(move |window, cx| {
        let mark = Mark::take(cx);
        let inline = prompt::ask_inline(
            question,
            &mark,
            move |frozen, _, _| confirmed.borrow_mut().push(format!("confirmed {frozen}")),
            window,
            cx,
        );
        cell.replace(Some(inline.clone()));
        inline
    });
    redraw(cx);
    inline
}

/// Settle the oldest unsettled attempt: failed with `reason`, or succeeded with `None`.
fn settle_attempt(cx: &mut VisualTestContext, attempts: &Attempts, reason: Option<HostSentence>) {
    let attempt = attempts.borrow_mut().remove(0);
    cx.update(move |_, cx| match reason {
        Some(reason) => attempt.failed(reason, cx),
        None => attempt.succeeded(cx),
    });
    settle(cx);
}

/// Every line of text painted inside the question in a fresh frame, in any presentation (each draws its root under the
/// element id `prompt`).
fn painted_in_question(cx: &mut VisualTestContext) -> Vec<String> {
    let nodes = frame(cx);
    let inside = |mut index: usize| loop {
        if nodes[index].element_id.as_deref() == Some("prompt") {
            return true;
        }
        match nodes[index].parent {
            Some(parent) => index = parent,
            None => return false,
        }
    };
    (0..nodes.len())
        .filter(|&index| inside(index))
        .flat_map(|index| nodes[index].texts.iter().map(|text| text.text.to_string()))
        .collect()
}

/// The answer ids SPEC.md's YOLO question reads and tabs in: Cancel, the guarded option, the permanent answer, the
/// one-time answer.
const YOLO_ORDER: [&str; 4] = [
    "prompt-cancel",
    "prompt-alternative-0",
    "prompt-alternative-1",
    "prompt-confirm",
];

/// Why this matters: SPEC.md's YOLO question is drawn inline in the launcher, and it is the confirmation that stops a
/// YOLO launch on the wrong machine. It must start on Cancel and tab in SPEC.md's order ("Cancel keeps initial focus",
/// "Tab follows that same order"), or a user tabbing to Cancel lands on a YOLO answer. Each answer must run once, and
/// the question must close inside its dialog: focus goes back to where it was before the question opened, in the
/// launcher, not to the page behind it. Escape on the question means Cancel and must not also throw away the launcher
/// and the draft in it, which is what the layer manager would do with an Escape the question let through.
///
/// Specified: asked inline in a dialog right after the user acted, the question is an alert dialog and takes focus on
/// Cancel, and Tab from Cancel reaches the guarded option, the permanent answer and the one-time answer in turn. Enter
/// on each answer runs it once on the frozen value (Cancel runs nothing; the permanent answer's attempt then succeeds),
/// and Escape on the permanent answer runs nothing. Each closes the question, returns focus to the launcher's Launch
/// button and leaves the launcher open. Cancel then Confirm sent with no draw between them run nothing: the first
/// answer wins inline too. Escape from Launch afterwards closes the launcher.
#[gpui::test]
fn an_inline_question_answers_once_and_returns_focus_inside_its_dialog(cx: &mut TestAppContext) {
    let (cx, launcher) = open_launcher(cx);
    let frozen = "codex on build-box";
    for (tabs, key, expected) in [
        (0, "enter", None),
        (1, "enter", Some("guarded")),
        (2, "enter", Some("always")),
        (3, "enter", Some("confirmed")),
        (2, "escape", None),
    ] {
        let (ran, attempts): (Ran, Attempts) = Default::default();
        let inline = ask_inline_in(cx, &launcher, yolo(&ran, &attempts), &ran);
        if tabs == 0 && key == "enter" {
            let nodes = frame(cx);
            let role = nodes
                .iter()
                .find(|node| node.element_id.as_deref() == Some("prompt"))
                .and_then(|node| node.role);
            assert_eq!(
                role,
                Some(Role::AlertDialog),
                "the question is an alert dialog"
            );
        }
        for _ in 0..tabs {
            keys(cx, "tab");
        }
        let answer = YOLO_ORDER[tabs];
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(answer),
            "focus starts on Cancel and Tab walks the answers in order"
        );
        keys(cx, key);
        if !attempts.borrow().is_empty() {
            settle_attempt(cx, &attempts, None);
        }
        let expected: Vec<String> = expected
            .map(|ran| format!("{ran} {frozen}"))
            .into_iter()
            .collect();
        assert_eq!(*ran.borrow(), expected, "{key} on {answer} ran it once");
        assert!(
            !cx.update(|_, cx| inline.is_open(cx)),
            "{key} on {answer} closed the question"
        );
        assert!(
            focused(cx, &launcher.launch),
            "after {key} on {answer}, focus went back to Launch"
        );
        assert!(is_open(cx, launcher.layer), "the launcher stayed open");
    }
    let (ran, attempts): (Ran, Attempts) = Default::default();
    let inline = ask_inline_in(cx, &launcher, yolo(&ran, &attempts), &ran);
    cx.update(|window, cx| {
        inline.cancel(window, cx);
        inline.confirm(window, cx);
    });
    settle(cx);
    assert!(
        ran.borrow().is_empty(),
        "Cancel won over the Confirm behind it"
    );
    keys(cx, "escape");
    assert!(
        !is_open(cx, launcher.layer),
        "Escape outside the question closed the launcher"
    );
}

/// Why this matters: SPEC.md, "Who may move focus": a question that arrives after the user moved on (a plain Replace's
/// YOLO question, after they typed elsewhere) shows where it belongs and waits; it must not take focus from what they
/// are doing. And an answer that runs while focus is elsewhere leaves focus there: the question returns focus only when
/// it held it, so it never pulls focus away from a control the user moved to.
///
/// Specified: asked inline with a mark the user has moved on from (a key pressed since), the question is drawn and
/// open, and focus stays on the launcher's Launch button. With focus then moved to Reset, a Cancel pressed without
/// taking focus (a macOS click) closes the question and leaves focus on Reset.
#[gpui::test]
fn an_inline_question_that_arrives_after_the_user_moved_on_waits(cx: &mut TestAppContext) {
    let (cx, launcher) = open_launcher(cx);
    let mark = cx.update(|_, cx| Mark::take(cx));
    keys(cx, "x");
    let (ran, attempts): (Ran, Attempts) = Default::default();
    let cell = launcher.question.clone();
    let question = yolo(&ran, &attempts);
    let inline = cx.update(move |window, cx| {
        let inline = prompt::ask_inline(question, &mark, |_, _, _| {}, window, cx);
        cell.replace(Some(inline.clone()));
        inline
    });
    redraw(cx);
    assert!(
        cx.update(|_, cx| inline.is_open(cx)),
        "the question is open"
    );
    assert!(
        cx.debug_bounds("prompt-cancel").is_some(),
        "and drawn in the launcher"
    );
    assert!(
        focused(cx, &launcher.launch),
        "focus stayed where the user had it"
    );
    click(cx, "reset");
    assert!(
        focused(cx, &launcher.reset),
        "the premise: the user moved focus to Reset"
    );
    cx.update(|window, cx| inline.cancel(window, cx));
    settle(cx);
    assert!(
        !cx.update(|_, cx| inline.is_open(cx)),
        "Cancel closed the question"
    );
    assert!(
        focused(cx, &launcher.reset),
        "focus stayed on Reset, where the user had put it"
    );
}

/// Why this matters: a plain Replace's YOLO question arrives on the page, with no dialog open (SPEC.md: it "takes focus
/// only if the user has not moved on since answering Replace"). There the question cannot borrow a dialog's focus
/// rules, so it must take focus through the page's own move and give it back to the control that asked.
///
/// Specified: with focus on a page control and no layer open, a question asked inline right after the user acted takes
/// focus on Cancel; Escape on it runs nothing, closes it and returns focus to that control.
#[gpui::test]
fn an_inline_question_on_the_page_takes_and_returns_focus(cx: &mut TestAppContext) {
    let replace = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let cell: InlineCell = Rc::default();
    let (drawn, shown) = (replace.clone(), cell.clone());
    let cx = open(cx, move |_, _| {
        div()
            .flex()
            .flex_col()
            .child(div().w(px(80.)).child(button("replace", &drawn)))
            .children(shown.borrow().clone())
            .into_any_element()
    });
    keys(cx, "tab");
    assert!(
        focused(cx, &replace),
        "the premise: the header's Replace has focus"
    );
    assert!(
        !cx.update(|window, cx| layers::any_open(window, cx)),
        "the premise: no layer is open"
    );
    let (ran, attempts): (Ran, Attempts) = Default::default();
    let (question, confirmed) = (yolo(&ran, &attempts), ran.clone());
    let inline = cx.update(move |window, cx| {
        let mark = Mark::take(cx);
        let inline = prompt::ask_inline(
            question,
            &mark,
            move |frozen, _, _| confirmed.borrow_mut().push(format!("confirmed {frozen}")),
            window,
            cx,
        );
        cell.replace(Some(inline.clone()));
        inline
    });
    redraw(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-cancel"),
        "the question took focus on Cancel"
    );
    keys(cx, "escape");
    assert!(ran.borrow().is_empty(), "Escape answered nothing");
    assert!(
        !cx.update(|_, cx| inline.is_open(cx)),
        "Escape closed the question"
    );
    assert!(focused(cx, &replace), "focus went back to Replace");
}

// ---- Stays up with the reason ----------------------------------------------------------------------------------------

/// Why this matters: SPEC.md's YOLO question has an answer that first changes the host's setting, and "if changing
/// that setting fails, nothing is started and the confirmation stays up with the reason". A question that closed
/// before the setting was written would leave the user with no answer to pick again and no reason; one that let
/// another answer through while the write was in flight would start a second launch, or cancel one already under way.
///
/// Specified: Enter on the fallible answer runs its action once and leaves the question open, focus still on that
/// answer, and the owner sees an attempt in flight until it settles (so its dismissal hook can keep it open). While it
/// is in flight, every other answer and Escape (which leaves the launcher open too) run nothing. When the attempt
/// fails, the question stays open with the reason painted inside it (a host value in it escaped), focus still on the
/// answer, and a held Enter left over from the first press does not retry. A new Enter retries, running the action
/// again and clearing the reason; when that attempt succeeds, the question closes, focus returns to Launch, and the
/// launcher stays open. The one-time answer never ran.
#[gpui::test]
fn a_failing_answer_keeps_its_question_up_with_the_reason(cx: &mut TestAppContext) {
    let (cx, launcher) = open_launcher(cx);
    let (ran, attempts): (Ran, Attempts) = Default::default();
    let inline = ask_inline_in(cx, &launcher, yolo(&ran, &attempts), &ran);
    let always = "always codex on build-box";
    let open_with_focus_on_it = |cx: &mut VisualTestContext, when: &str| {
        assert!(
            cx.update(|_, cx| inline.is_open(cx)),
            "{when}: the question is open"
        );
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("prompt-alternative-1"),
            "{when}: focus stayed on the answer"
        );
    };

    let in_flight = |cx: &mut VisualTestContext| cx.update(|_, cx| inline.in_flight(cx));
    keys(cx, "tab tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-alternative-1"),
        "the premise: the permanent answer has focus"
    );
    assert!(!in_flight(cx), "the premise: nothing is in flight yet");
    keys(cx, "enter");
    assert_eq!(*ran.borrow(), [always], "the answer ran once");
    assert!(in_flight(cx), "the owner sees the attempt in flight");
    open_with_focus_on_it(cx, "in flight");

    cx.update(|window, cx| {
        inline.cancel(window, cx);
        inline.confirm(window, cx);
        inline.alternative(0, window, cx);
        inline.alternative(1, window, cx);
    });
    settle(cx);
    keys(cx, "escape");
    assert_eq!(*ran.borrow(), [always], "nothing else ran in flight");
    assert_eq!(attempts.borrow().len(), 1, "one attempt is in flight");
    assert!(is_open(cx, launcher.layer), "Escape left the launcher open");
    open_with_focus_on_it(cx, "after the presses in flight");

    let host = HostText::from_host("build-box\u{202E}");
    let reason = HostSentence::new("prompt-reason")
        .text("Could not stop asking for ")
        .host(&host)
        .text(". This answer started nothing.");
    settle_attempt(cx, &attempts, Some(reason));
    assert!(!in_flight(cx), "the failed attempt is no longer in flight");
    open_with_focus_on_it(cx, "failed");
    let painted = painted_in_question(cx);
    assert!(
        painted
            .iter()
            .any(|text| text.contains("build-box<U+202E>")),
        "the reason is painted inside the question, its host value escaped; painted: {painted:?}"
    );
    cx.simulate_event(KeyDownEvent {
        native_key: None,
        keystroke: Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    check(cx);
    assert_eq!(*ran.borrow(), [always], "a held Enter did not retry");

    keys(cx, "enter");
    assert_eq!(*ran.borrow(), [always, always], "Enter retried");
    assert!(in_flight(cx), "the retry is in flight");
    let painted = painted_in_question(cx);
    assert!(
        !painted
            .iter()
            .any(|text| text.contains("Could not stop asking")),
        "the retry cleared the old reason; painted: {painted:?}"
    );
    settle_attempt(cx, &attempts, None);
    assert!(
        !in_flight(cx),
        "the attempt that succeeded is no longer in flight"
    );
    assert!(
        !cx.update(|_, cx| inline.is_open(cx)),
        "success closed the question"
    );
    assert!(focused(cx, &launcher.launch), "focus went back to Launch");
    assert!(is_open(cx, launcher.layer), "the launcher stayed open");
    assert_eq!(
        *ran.borrow(),
        [always, always],
        "the one-time answer never ran"
    );
}

/// Why this matters: a layer closes on a press outside it or when its subject goes, which would take the question away
/// while its answer's work runs on; the work would then finish for a question the user can no longer see. So only an
/// inline question may carry an answer that can fail, and asking one as a layer must fail at once rather than open.
///
/// Specified: a question with a fallible answer, asked centered or anchored, does not open.
#[gpui::test]
fn a_question_with_an_answer_that_can_fail_opens_only_inline(cx: &mut TestAppContext) {
    let (cx, row) = open_row(cx);
    for (kind, presentation) in row.presentations() {
        let (ran, attempts): (Ran, Attempts) = Default::default();
        let question = yolo(&ran, &attempts);
        let asked = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            prompt::ask_for_test(
                presentation,
                question,
                None,
                &mark,
                |_, _, _| {},
                window,
                cx,
            )
        });
        settle(cx);
        assert!(asked.is_none(), "{kind}: it did not open");
    }
}

// ---- A body the feature draws ----------------------------------------------------------------------------------------

/// What a test body's builder was handed, one entry per call.
type Built = Rc<RefCell<Vec<String>>>;

/// A body a feature draws, standing in for the host setup confirmation's plan: a Copy plan button, which is not an
/// answer, counting its presses in `copied`.
struct PlanBody {
    copy: FocusHandle,
    copied: Rc<Cell<usize>>,
}

impl Render for PlanBody {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let copied = self.copied.clone();
        div().w(px(100.)).child(
            Button::new(
                "copy-plan",
                "Copy plan",
                HoverHelp::tooltip("Copy the complete host plan"),
                Role::Button,
            )
            .focus(&self.copy)
            .on_click(move |_, _, _| copied.set(copied.get() + 1)),
        )
    }
}

/// Why this matters: the setup confirmation's body is the helm's plan, and Set up runs the plan the question froze, so
/// the body must be built from the frozen value, once, before the user sees it. Its Copy plan is a control of its own.
/// SPEC.md keeps Tab inside a dialog's controls, so Copy plan must be reachable there, in reading order (the body comes
/// before the answers); and it must stay a control, not an answer: a press on it that closed the question, or ran
/// Confirm through the dialog's Enter rule, would set up a host when the user only copied the plan.
///
/// Specified, for a centered question with a body holding Copy plan: the body's builder runs once, as the question is
/// asked and before anything is drawn, handed the frozen value, and not again over every redraw that follows. Copy plan
/// is drawn below the question's line and above the answers; focus starts on Confirm, as it does without a body; Tab
/// from there reaches Copy plan, then Cancel and Confirm. Enter, Space and a click on Copy plan each press it, and
/// leave the question open with no answer run. A click on Confirm then confirms once, on the frozen value, and closes
/// the question.
///
/// Inside the dialog's trap Tab's order is a cycle, and a cycle looks the same whichever stop it starts from: a body
/// drawn after the answers would give the very same sequence. So where the body sits is pinned by where it is drawn,
/// and Tab by its step from Confirm round to Copy plan and on to Cancel.
#[gpui::test]
fn a_body_is_built_once_and_its_control_sits_between_text_and_answers(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let (built, copied): (Built, Rc<Cell<usize>>) = Default::default();
    let ran: Ran = Default::default();
    let (record, counter, confirmed) = (built.clone(), copied.clone(), ran.clone());
    let question = question("alpha").body(move |frozen: &String, _, cx| {
        record.borrow_mut().push(frozen.clone());
        cx.new(|cx| PlanBody {
            copy: cx.focus_handle().tab_stop(true),
            copied: counter,
        })
    });
    let asked = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        let asked = prompt::ask_for_test(
            Presentation::Centered,
            question,
            None,
            &mark,
            move |frozen, _, _| confirmed.borrow_mut().push(format!("confirmed {frozen}")),
            window,
            cx,
        );
        assert_eq!(
            *built.borrow(),
            ["alpha"],
            "the body was built as the question was asked, from the frozen value"
        );
        asked
    });
    settle(cx);
    let asked = asked.expect("the question opened");
    let line = frame(cx)
        .iter()
        .flat_map(|node| node.texts.iter())
        .find(|text| text.text.as_ref() == "Its history is removed.")
        .map(|text| text.bounds)
        .expect("the question's line was painted");
    let (copy, answers) = (bounds(cx, "copy-plan"), bounds(cx, "prompt-answers"));
    assert!(
        line.bottom() <= copy.top() && copy.bottom() <= answers.top(),
        "Copy plan is drawn between the line and the answers: line {line:?}, Copy plan {copy:?}, answers {answers:?}"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "focus starts on Confirm, as without a body"
    );
    for expected in ["copy-plan", "prompt-cancel", "prompt-confirm", "copy-plan"] {
        keys(cx, "tab");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(expected),
            "Tab reaches the body's control, then the answers"
        );
    }
    keys(cx, "enter");
    keys(cx, "space");
    click(cx, "copy-plan");
    assert_eq!(
        copied.get(),
        3,
        "Enter, Space and a click each pressed Copy plan"
    );
    assert!(is_open(cx, asked.layer), "the question stayed open");
    assert!(ran.borrow().is_empty(), "no answer ran: {:?}", ran.borrow());

    click(cx, "prompt-confirm");
    assert_eq!(*ran.borrow(), ["confirmed alpha"], "Confirm ran once");
    assert!(!is_open(cx, asked.layer), "Confirm closed the question");
    assert_eq!(
        *built.borrow(),
        ["alpha"],
        "the body was built once, however often it was drawn"
    );
}

/// Why this matters: the host confirmations frame their question as the old host dialogs did, at their own width, with
/// the host's mark in a tile and a note saying how to undo "don't ask again". A builder that took them and did not hand
/// them to the dialog frame would drop that note, the one place the user learns how to undo a permanent answer. Where
/// the frame lays them out is the frame's, pinned by the `prompt-body` golden.
///
/// Specified, for a centered question built with a tile, a note and a width of 560 pixels: the dialog is 560 pixels
/// wide, the tile is drawn inside it, and the note is painted.
#[gpui::test]
fn a_questions_tile_note_and_width_reach_its_dialog_frame(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let note = "Undo it with the gear";
    let question = question("alpha")
        .tile(|_, _| div().debug_selector(|| "host-tile".into()).size(px(24.)))
        .note(note)
        .width(px(560.));
    let (asked, nodes) = ask_and_record(cx, question);
    let dialog = bounds(cx, "prompt");
    assert_eq!(
        dialog.size.width,
        px(560.),
        "the dialog takes the question's width"
    );
    assert!(
        dialog.contains(&bounds(cx, "host-tile").center()),
        "the tile is drawn inside the dialog"
    );
    assert!(
        nodes
            .iter()
            .flat_map(|node| node.texts.iter())
            .any(|text| text.text.as_ref() == note),
        "the note is painted"
    );
    cx.update(|window, cx| asked.cancel(window, cx));
    settle(cx);
}
