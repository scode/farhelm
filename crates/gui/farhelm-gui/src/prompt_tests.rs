//! Behavior tests for the prompt, on GPUI's simulated platform: that a confirmation acts only on what it showed, where
//! a question starts and how it is answered, cancelled and closed, how its summary and its title draw a host value,
//! the anchored question asked beside its control (with its text capped, so a huge title cannot push the answers off
//! the window), the inline question drawn inside its owner's dialog (SPEC.md's YOLO confirmation in the launcher), the
//! answer whose action can fail, which keeps its question up with the reason, the debug build's refusal of an option
//! the question's presentation ignores, a body the feature draws (the host setup confirmation's plan) with the dialog
//! frame's tile, note and width, a link that opens without answering (SPEC.md's sandbox question linking Tensorlake
//! pricing), and the bulk question's three needs: rewording an open question in place, returning focus to a named
//! control, and a keyboard scroll stop for long text.
//!
//! Each test opens its window through the shared harness ([`crate::test_support::open`]: GPUI Kit's `Root` around a
//! root view wired as the app's), drives it through simulated keys and clicks, so key bindings resolve as they do in
//! the app, and runs the harness's invariant checks after every step. The rules come from SPEC.md, "Keyboard focus",
//! and SPEC_impl.md, "Questions".

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, Keystroke, Modifiers, MouseButton, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, TestAppContext, VisualTestContext, Window, div, point,
    px, size,
};

use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier};
use crate::focus::{self, Mark, OnPress};
use crate::host_text::tests::{SPOOFING_CORPUS, painted_words, paints_isolated};
use crate::host_text::{HostSentence, HostText, Line};
use crate::layers::{self, Anchor, Dialog, LayerId, Subject};
use crate::prompt::{self, Answer, Presentation, Question};
use crate::test_support::{
    Thing, advance, any_open, bounds, button, center, check, click, focused, focused_id, frame,
    handle, is_open, keys, label, open, question, redraw, settle, show_next_frame,
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
    Answer::new(
        "Keep it",
        HoverHelp::tooltip("Keep it, and stop asking"),
        Tier::Neutral,
    )
}

/// One extra-answers case: its name, how the question is adjusted, the keys pressed, and which answer ran.
type ExtraAnswerCase = (
    &'static str,
    fn(Question<String>) -> Question<String>,
    &'static str,
    &'static str,
);

/// The extra answers must not change where a question starts or what Enter does there: SPEC.md's template leave
/// question "starts with focus on Save", so a question that started on an extra answer would discard a draft on
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
            question("alpha").cancel_answer(Answer::new(
                "Cancel",
                HoverHelp::tooltip("Cancel: keep the agent and terminal tabs running"),
                Tier::Neutral,
            )),
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

/// A question about a thing closes when the thing is gone (a session deleted by an agent or from the command line while
/// its delete confirmation was open), and nothing is sent.
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

/// Ask `question` centered with no subject, draw it with GPUI's driver record on, and hand back that frame's record:
/// every node drawn and the text each painted. The question stays open; the caller answers it.
fn ask_and_record(
    cx: &mut VisualTestContext,
    question: Question<String>,
) -> (prompt::Asked<String>, Vec<gpui::DriverNode>) {
    ask_and_record_as(cx, Presentation::Centered, question)
}

/// [`ask_and_record`], shown as `presentation`.
fn ask_and_record_as(
    cx: &mut VisualTestContext,
    presentation: Presentation,
    question: Question<String>,
) -> (prompt::Asked<String>, Vec<gpui::DriverNode>) {
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
/// Specification: a question whose summary is a [`HostSentence`] paints the host value in its escaped presentation
/// inside its isolate, and no text the frame paints holds a presentation-unsafe character other than a sentence's own
/// isolates; inside the dialog the sentence is one `Label`
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
            .any(|text| paints_isolated(text, "invoice<U+202E>fdp<U+200B>.exe")),
        "the host value is painted escaped, isolated; painted: {texts:?}"
    );
    assert!(
        !texts.iter().any(|text| painted_words(text)
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
            inside
                .iter()
                .any(|text| painted_words(&text.text).contains(expected)),
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

/// A question titled `title`, as the host setup confirmation is ("Set up host · <name>"), frozen at "alpha".
fn titled(title: impl Into<Line>) -> Question<String> {
    let set_up = Answer::new(
        "Set up",
        HoverHelp::tooltip("Set this host up"),
        Tier::Primary,
    );
    Question::new(title, set_up, "alpha".to_string())
        .summary("Farhelm installs its supervisor there.")
}

/// The accessible name of the question drawn under `prompt` in `nodes`, in any presentation.
fn question_name(nodes: &[gpui::DriverNode]) -> Option<String> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("prompt"))
        .and_then(|node| node.label.as_ref().map(ToString::to_string))
}

/// Why this matters: the host setup confirmation is titled "Set up host · <name>" (as in the old app), as are SPEC.md's
/// "Suspend <host>?" and "Create Tensorlake sandbox · <Name>", and the name is one a host or the user typed. The title
/// is also the dialog's accessible name, so a title made from the raw value would put an override or zero-width
/// character on screen and into what a screen reader reads, where two hosts that look alike must still be told apart
/// (SPEC.md: "including in accessible names"). The plain-string case guards every existing question, which passes its
/// title as app words and must draw and be named as before.
///
/// Specification: for every value in the spoofing corpus, a question titled with a [`HostSentence`] "Set up host ·
/// <value>" is named, centered and anchored alike, "Set up host · " followed by the escaped value, which is the
/// sentence's accessible name; nothing the question paints holds a presentation-unsafe character other than a
/// sentence's own isolates; centered, the value is painted in its escaped presentation inside its isolate, and the
/// title is one `Label` node under its id with that same name. A question titled with a plain string is named by that
/// string in both, and paints it as one line when centered.
#[gpui::test]
fn a_title_that_names_a_host_value_is_drawn_and_named_escaped(cx: &mut TestAppContext) {
    let (cx, row) = open_row(cx);
    for (raw, shown) in SPOOFING_CORPUS {
        let title = HostSentence::new("prompt-title")
            .text("Set up host · ")
            .host(&HostText::from_host(*raw));
        let expected = format!("Set up host · {shown}");
        assert_eq!(title.accessible_name().as_ref(), expected, "the premise");
        for (kind, presentation) in row.presentations() {
            let (asked, nodes) = ask_and_record_as(cx, presentation, titled(title.clone()));
            assert_eq!(
                question_name(&nodes),
                Some(expected.clone()),
                "{kind}: the question's name for {shown:?}"
            );
            let texts = painted_in_question(cx);
            assert!(
                !texts.iter().any(|text| painted_words(text)
                    .chars()
                    .any(farhelm_proto::text::is_presentation_unsafe)),
                "{kind}: nothing painted holds a hidden character; painted: {texts:?}"
            );
            if kind == "centered" {
                assert!(
                    texts.iter().any(|text| paints_isolated(text, shown)),
                    "the value is painted escaped, isolated; painted: {texts:?}"
                );
                let sentence = nodes
                    .iter()
                    .find(|node| node.element_id.as_deref() == Some("prompt-title"));
                assert_eq!(
                    sentence
                        .map(|node| (node.role, node.label.as_deref().map(ToString::to_string))),
                    Some((Some(Role::Label), Some(expected.clone()))),
                    "the title is the sentence's one labelled node"
                );
            }
            cx.update(|window, cx| asked.cancel(window, cx));
            settle(cx);
        }
    }
    for (kind, presentation) in row.presentations() {
        let (asked, nodes) = ask_and_record_as(cx, presentation, titled("Set up host"));
        assert_eq!(
            question_name(&nodes).as_deref(),
            Some("Set up host"),
            "{kind}: a string title names it"
        );
        if kind == "centered" {
            let texts = painted_in_question(cx);
            assert!(
                texts.iter().any(|text| text == "Set up host"),
                "painted as given; painted: {texts:?}"
            );
        }
        cx.update(|window, cx| asked.cancel(window, cx));
        settle(cx);
    }
}

/// Why this matters: an inline question draws its title as its heading (the YOLO confirmation's red one) and names
/// itself by it, a second place a title naming a host value must stay escaped.
///
/// Specification: an inline question titled with a [`HostSentence`] naming a value with a right-to-left override is
/// named with the value escaped, which is the sentence's accessible name, and paints the value escaped inside its
/// isolate.
#[gpui::test]
fn an_inline_title_that_names_a_host_value_is_drawn_and_named_escaped(cx: &mut TestAppContext) {
    let (cx, launcher) = open_launcher(cx);
    let title = HostSentence::new("prompt-title")
        .text("Set up host · ")
        .host(&HostText::from_host("invoice\u{202E}fdp.exe"));
    let _inline = ask_inline_in(cx, &launcher, titled(title), &Ran::default());
    assert_eq!(
        question_name(&frame(cx)).as_deref(),
        Some("Set up host · invoice<U+202E>fdp.exe"),
        "the inline question is named with the value escaped"
    );
    let texts = painted_in_question(cx);
    assert!(
        texts
            .iter()
            .any(|text| paints_isolated(text, "invoice<U+202E>fdp.exe")),
        "painted escaped, isolated; painted: {texts:?}"
    );
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

/// The row's Delete question about a running session titled `title` in a managed checkout, in the order
/// `session_list/delete.rs` builds it: the consequence, the quoted title, then the checkout's line.
fn delete_question(title: &str) -> Question<String> {
    let delete = Answer::new(
        "Confirm delete",
        HoverHelp::tooltip("Delete it"),
        Tier::Danger,
    );
    let quoted = HostSentence::new("prompt-title")
        .text("\"")
        .host(&HostText::from_host(title))
        .text("\"");
    Question::new("Delete confirmation", delete, "s1".to_string())
        .line("Still running \u{2014} deleting kills the agent:")
        .line(quoted)
        .line(MANAGED_CHECKOUT)
        .keeps_focus_on_empty_click()
}

/// `session_list::delete::MANAGED_CHECKOUT`'s words, which that module keeps to itself.
const MANAGED_CHECKOUT: &str = "The managed checkout stays while another session uses it. When its last session is \
                                deleted, its folder moves to the trash; no files are deleted.";

/// The text painted in `nodes` whose words (isolates aside) hold `words`, which must be there.
fn painted_text(nodes: &[gpui::DriverNode], words: &str) -> gpui::DriverText {
    let mut texts = nodes.iter().flat_map(|node| node.texts.iter());
    match texts.find(|painted| painted_words(&painted.text).contains(words)) {
        Some(painted) => painted.clone(),
        None => panic!("{words:?} was not painted"),
    }
}

/// Why this matters: an anchored question wraps rather than cuts its lines, and a session title may be tens of
/// thousands of characters. Unbounded, a Delete question quoting one pushed Cancel and Confirm off the window, so a
/// pointer user could not answer it. The old app's case is terminal.spec's multi-KB unbroken title, which "keeps the
/// consequence text intact and clips only the title". What the cap hides must stay in keyboard reach too (SPEC.md,
/// "Reaching everything by keyboard").
///
/// Specification, in a window of the app's minimum size, for the row's Delete question about a session in a managed
/// checkout whose 6,000-character title has no spaces and alone is taller than the window: the card, Cancel and
/// Confirm lie inside the window; the consequence and the checkout's line, before and after the title, are painted
/// whole; the wheel over the title scrolls it while Confirm stays put; with focus on Confirm, Page Down scrolls the
/// title further, focus stays on Confirm and nothing answers; a click on Confirm confirms it once.
#[gpui::test]
fn an_anchored_question_quoting_a_huge_title_keeps_its_answers_in_the_window(
    cx: &mut TestAppContext,
) {
    let (cx, row) = open_row(cx);
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    settle(cx);
    let (title, sent) = ("x".repeat(6_000), Rc::new(Cell::new(0)));
    let (anchor, record) = (row.anchor.borrow().clone(), sent.clone());
    let layer = cx.update(|window, cx| {
        let (question, mark) = (delete_question(&title), Mark::take(cx));
        let Some(anchor) = anchor else {
            panic!("the premise: the row's menu button was drawn")
        };
        let confirm = move |_, _: &mut _, _: &mut _| record.set(record.get() + 1);
        prompt::ask_anchored(question, anchor, None, &mark, confirm, window, cx)
    });
    settle(cx);
    let shown = gpui::Bounds::new(point(px(0.), px(0.)), cx.update(|w, _| w.viewport_size()));
    let nodes = frame(cx);
    let quoted = painted_text(&nodes, &title);
    let premise = quoted.bounds.size.height > shown.size.height;
    assert!(premise, "the premise: the title is taller than the window");
    for drawn in ["prompt", "prompt-cancel", "prompt-confirm"] {
        let at = bounds(cx, drawn);
        assert!(
            at.is_contained_within(&shown),
            "{drawn} lies inside the window: {at:?}"
        );
    }
    for whole in [
        "Still running \u{2014} deleting kills the agent:",
        MANAGED_CHECKOUT,
    ] {
        let painted = painted_text(&nodes, whole);
        assert_eq!(
            painted.visible_bounds, painted.bounds,
            "{whole:?} is painted whole"
        );
    }

    let confirm = bounds(cx, "prompt-confirm");
    cx.simulate_event(gpui::ScrollEvent {
        position: quoted.visible_bounds.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-200.))),
        ..Default::default()
    });
    check(cx);
    let scrolled = painted_text(&frame(cx), &title).bounds;
    assert!(
        scrolled.top() < quoted.bounds.top(),
        "the wheel scrolled the title: {scrolled:?}"
    );
    assert_eq!(bounds(cx, "prompt-confirm"), confirm, "Confirm stayed put");
    let on_confirm =
        |cx: &mut VisualTestContext| focused_id(cx).as_deref() == Some("prompt-confirm");
    assert!(on_confirm(cx), "the premise: focus starts on Confirm");
    keys(cx, "pagedown");
    let paged = painted_text(&frame(cx), &title).bounds;
    assert!(
        paged.top() < scrolled.top(),
        "Page Down scrolled the title: {paged:?}"
    );
    assert!(on_confirm(cx), "focus stayed on Confirm");
    let open = layer.is_some_and(|layer| is_open(cx, layer));
    assert!(open && sent.get() == 0, "Page Down answered nothing");
    click(cx, "prompt-confirm");
    assert_eq!(sent.get(), 1, "the click on Confirm confirmed it once");
}

/// Why this matters: where focus lands when a question closes decides where the user's next keys go. SPEC.md, "Keyboard
/// focus": "Escape cancels the header's … question and returns focus to the terminal", while every other close (an
/// answer, Cancel included) returns focus to where it was before the question opened; a header question whose Escape
/// went back to its button would leave the user's next keys off the terminal they were working in. The bulk Stop and
/// Delete question is asked by the marked bar's Stop or Delete, but SPEC.md and the old app return focus to the bar's ⋯
/// when it closes. Without a way to name that control, the asker focused the ⋯ before asking: one more visible focus
/// move, and one the focus coordinator refuses once the user moved on, which left Cancel returning to whatever held
/// focus before.
///
/// Specified, for a question drawn centered and one drawn beside its control, on a page with the opener, a drawn home
/// (standing in for the selected terminal) and a named control (standing in for the bar's ⋯), opened with focus on the
/// opener: the open question holds focus, so none of the three has it. Then, by how the question was built:
/// - with no return rule, Escape returns focus to the opener;
/// - with `returns_home_on_escape`, Escape sends focus home and a click on Cancel returns it to the opener;
/// - with `returns_to` naming the control, Escape and a click on Cancel each send focus to the named control.
///
/// A named control that is no longer drawn as the question closes falls back to the opener; that rule is the focus
/// coordinator's, pinned in `focus_tests.rs`.
#[gpui::test]
fn a_question_can_return_focus_home_or_to_a_named_control(cx: &mut TestAppContext) {
    /// How a question in this test is told to return focus as it closes.
    #[derive(Clone, Copy, Debug)]
    enum Returns {
        /// No rule: every close returns to the opener.
        Unset,
        /// `returns_home_on_escape`.
        HomeOnEscape,
        /// `returns_to` naming the page's named control.
        Named,
    }

    let (opener, home, named) = cx.update(|cx| {
        (
            cx.focus_handle().tab_stop(true),
            cx.focus_handle().tab_stop(true),
            cx.focus_handle().tab_stop(true),
        )
    });
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let (drawn_opener, drawn_home, drawn_named, out) = (
        opener.clone(),
        home.clone(),
        named.clone(),
        anchor_out.clone(),
    );
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
            .child(button("bar-menu", &drawn_named))
            .into_any_element()
    });
    let terminal = home.clone();
    cx.update(move |window, cx| crate::focus::set_home(Some(&terminal), window, cx));
    let anchored = anchor_out
        .borrow()
        .clone()
        .expect("the page drew its anchor");
    // Which of the page's controls hold focus, by name: exactly one after a close, none while the question is open.
    let holding = |cx: &mut VisualTestContext| -> Vec<&'static str> {
        [("opener", &opener), ("home", &home), ("named", &named)]
            .into_iter()
            .filter(|(_, control)| focused(cx, control))
            .map(|(name, _)| name)
            .collect()
    };
    for presentation in [Presentation::Centered, Presentation::Anchored(anchored)] {
        for (returns, close, lands_on) in [
            (Returns::Unset, "escape", "opener"),
            (Returns::HomeOnEscape, "escape", "home"),
            (Returns::HomeOnEscape, "cancel", "opener"),
            (Returns::Named, "escape", "named"),
            (Returns::Named, "cancel", "named"),
        ] {
            let case = format!("{returns:?}, {close}");
            click(cx, "opener");
            assert_eq!(
                holding(cx),
                ["opener"],
                "{case}: the premise: the opener has focus"
            );
            let (shown, control) = (presentation.clone(), named.clone());
            let asked = cx.update(|window, cx| {
                let mark = Mark::take(cx);
                let question = match returns {
                    Returns::Unset => question("alpha"),
                    Returns::HomeOnEscape => question("alpha").returns_home_on_escape(),
                    Returns::Named => question("alpha").returns_to(control),
                };
                prompt::ask_for_test(shown, question, None, &mark, |_, _, _| {}, window, cx)
            });
            settle(cx);
            let asked = asked.expect("the question opened");
            assert!(
                holding(cx).is_empty(),
                "{case}: the premise: the open question holds focus"
            );
            if close == "escape" {
                keys(cx, "escape");
            } else {
                click(cx, "prompt-cancel");
            }
            assert!(!is_open(cx, asked.layer), "{case}: it closed");
            assert_eq!(holding(cx), [lands_on], "{case}: where focus went");
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
/// The question sits in a place of its own (`question-place`) that the view draws only while
/// [`prompt::Inline::is_open`] says so, read while drawing, as a sidebar row stops drawing a closed question's place.
///
/// Reset sits closer to the question than Launch does, so it is what the focus coordinator's recovery would pick if a
/// closing question let focus drop: a test that sees focus back on Launch knows the question returned it.
struct Launcher {
    launch: FocusHandle,
    reset: FocusHandle,
    question: InlineCell,
}

impl Render for Launcher {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self
            .question
            .borrow()
            .clone()
            .filter(|question| question.is_open(cx));
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
            .children(open.map(|question| {
                div()
                    .debug_selector(|| "question-place".into())
                    .pt(px(12.))
                    .child(question)
            }))
    }
}

/// An open [`Launcher`]: its layer, its own focus handle, its buttons' handles and the question's cell.
struct OpenLauncher {
    layer: LayerId,
    container: FocusHandle,
    launch: FocusHandle,
    reset: FocusHandle,
    question: InlineCell,
}

/// Open the harness with a [`Launcher`] dialog over an empty page, focus on its Launch button. The dialog has no
/// dismissal hook, so Escape that reaches it closes it: a test can tell whether the question took the key. A press on
/// its empty space keeps focus on the control in use, as the real launcher's does.
fn open_launcher(cx: &mut TestAppContext) -> (&mut VisualTestContext, OpenLauncher) {
    open_launcher_with(cx, true)
}

/// [`open_launcher`], with `keeps_focus_on_empty_click` as the dialog's empty-space rule: `false` is the rule of the
/// Templates and Trash dialogs, where a press on the dialog's empty space focuses the dialog itself.
fn open_launcher_with(
    cx: &mut TestAppContext,
    keeps_focus_on_empty_click: bool,
) -> (&mut VisualTestContext, OpenLauncher) {
    let cx = open(cx, |_, _| div().into_any_element());
    let (launch, reset, container) = (handle(cx), handle(cx), handle(cx));
    let question: InlineCell = Rc::default();
    let (start, drawn, lower) = (launch.clone(), question.clone(), reset.clone());
    let dialog = container.clone();
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
                container: dialog,
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click,
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
            container,
            launch,
            reset,
            question,
        },
    )
}

/// What the test question's answers ran, each as "<answer> <frozen value>", in order.
type Ran = Rc<RefCell<Vec<String>>>;

/// The attempts the test question's fallible answers were handed, oldest first, for the test to settle.
type Attempts = Rc<RefCell<Vec<prompt::Attempt>>>;

/// SPEC.md's YOLO confirmation as the launcher asks it, with the old app's words: Cancel, the guarded option, the
/// outlined permanent answer, then the filled one-time answer, which confirms. The permanent answer can fail, as the
/// real one does (it changes the host's setting first): each press records its run in `ran` and its attempt in
/// `attempts` for the test to settle. It starts on Cancel and is drawn loud.
fn yolo(ran: &Ran, attempts: &Attempts) -> Question<String> {
    let (guarded, always, kept) = (ran.clone(), ran.clone(), attempts.clone());
    Question::new(
        "Confirm YOLO launch",
        Answer::new(
            "Start YOLO session anyway",
            HoverHelp::tooltip(
                "Start the session in YOLO mode, where the agent acts without asking",
            ),
            Tier::Danger,
        ),
        "codex on build-box".to_string(),
    )
    .summary("YOLO means the agent bypasses its safety checks.")
    .line("Nothing has been started.")
    .cancel_answer(Answer::new(
        "Cancel",
        HoverHelp::tooltip("Cancel: go back without starting anything"),
        Tier::Neutral,
    ))
    .alternative(
        Answer::new(
            "Start guarded session instead",
            HoverHelp::tooltip("Start this session in guarded mode instead"),
            Tier::Primary,
        ),
        move |frozen, _, _| guarded.borrow_mut().push(format!("guarded {frozen}")),
    )
    .fallible_alternative(
        Answer::new(
            "Start, and don't ask again on this host",
            HoverHelp::tooltip(
                "Start it, and start YOLO sessions on this host without asking from now on",
            ),
            Tier::DangerOutline,
        ),
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

/// Why this matters: an owner draws an inline question inside its own surface, so only the owner can stop drawing the
/// question's place (its spacing, a sidebar row's slot) once the question closes, and only it can refuse what SPEC.md
/// refuses while the question is open ("an open lifecycle confirmation ... refuses another lifecycle action"). It reads
/// [`prompt::Inline::is_open`] for both, while drawing, and nothing notifies the owner itself when the question closes:
/// the question notifies only its own entity, and that must be enough to redraw the owner.
///
/// Specified: while the question is open its owner draws its place. With focus moved to Reset, a Cancel run through the
/// test's handle (neither a key on it nor a click, so it returns no focus) closes the question and leaves focus on
/// Reset, so no focus move redraws the window (a focus return would, and would hide a missing notification); the next
/// frame still has no place, and `is_open` is false.
#[gpui::test]
fn an_owner_reading_is_open_while_drawing_stops_drawing_a_closed_questions_place(
    cx: &mut TestAppContext,
) {
    let (cx, launcher) = open_launcher(cx);
    let mark = cx.update(|_, cx| Mark::take(cx));
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
        cx.debug_bounds("question-place").is_some(),
        "the premise: the owner draws the open question's place"
    );
    click(cx, "reset");
    assert!(
        focused(cx, &launcher.reset),
        "the premise: the user moved focus to Reset"
    );
    cx.update(|window, cx| inline.cancel(window, cx));
    settle(cx);
    assert!(
        focused(cx, &launcher.reset),
        "the premise: closing moved no focus, so only the question's notification redraws"
    );
    assert!(
        cx.debug_bounds("question-place").is_none(),
        "the owner's next frame drew no place for the closed question"
    );
    assert!(
        !cx.update(|_, cx| inline.is_open(cx)),
        "Cancel closed the question"
    );
}

/// Why this matters: SPEC.md, "Who may move focus": a question that arrives after the user moved on (a plain Replace's
/// YOLO question, after they typed elsewhere) shows where it belongs and waits; it must not take focus from what they
/// are doing. And an answer that runs while focus is elsewhere, with no click on it, leaves focus there (an attempt
/// that succeeds after the user moved on): the question returns focus only when it held it or a click ran the answer,
/// so it never pulls focus away from a control the user moved to.
///
/// Specified: asked inline with a mark the user has moved on from (a key pressed since), the question is drawn and
/// open, and focus stays on the launcher's Launch button. With focus then moved to Reset, a Cancel run through the
/// test's handle (neither a key on it nor a click) closes the question and leaves focus on Reset.
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

/// A dialog standing in for the launcher whose inline question lands below its body's fold: Launch at the top of a
/// tall dialog frame body, then the question, drawn while the test's cell holds it.
struct TallLauncher {
    launch: FocusHandle,
    question: InlineCell,
}

impl Render for TallLauncher {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        DialogFrame::new(
            "tall",
            "A tall launcher",
            div()
                .child(div().w(px(80.)).child(button("launch", &self.launch)))
                .child(div().h(px(600.)))
                .children(self.question.borrow().clone()),
            Vec::new(),
        )
    }
}

/// Why this matters: the launcher's body scrolls, and SPEC.md's YOLO question drawn there can land below its fold.
/// Focus moving onto an answer the user cannot see breaks SPEC.md's "Reaching everything by keyboard", and the
/// question's owner used to have to scroll it into view itself. The prompt now moves focus through the coordinator's
/// move inside the layer, which waits for the frame that draws the starting answer and scrolls it into view.
///
/// Specified: asked inline right after the user acted, in a dialog whose body draws the question below its fold in a
/// 400-pixel window, the question takes focus on Cancel and the body scrolls to show Cancel whole.
#[gpui::test]
fn an_inline_question_below_the_fold_is_scrolled_into_view_as_it_takes_focus(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| div().into_any_element());
    cx.simulate_resize(size(px(720.), px(400.)));
    settle(cx);
    let (launch, container) = (handle(cx), handle(cx));
    let question: InlineCell = Rc::default();
    let (start, drawn) = (launch.clone(), question.clone());
    let layer = cx.update(move |window, cx| {
        let view = cx.new(|_| TallLauncher {
            launch: start.clone(),
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
    assert!(
        layer.is_some() && focused(cx, &launch),
        "the premise: the dialog opened with focus on Launch"
    );
    let (launch_at, body_at) = (bounds(cx, "launch"), bounds(cx, "tall-body"));
    assert!(
        launch_at.bottom() + px(600.) > body_at.bottom(),
        "the premise: what follows Launch's 600-pixel spacer, the question, starts below the body's fold"
    );
    let (ran, attempts): (Ran, Attempts) = Default::default();
    let (cell, asked) = (question.clone(), yolo(&ran, &attempts));
    cx.update(move |window, cx| {
        let mark = Mark::take(cx);
        let inline = prompt::ask_inline(asked, &mark, |_, _, _| {}, window, cx);
        cell.replace(Some(inline));
    });
    redraw(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-cancel"),
        "focus moved to the question's starting answer"
    );
    let (answer, body) = (bounds(cx, "prompt-cancel"), bounds(cx, "tall-body"));
    assert!(
        answer.top() >= body.top() && answer.bottom() <= body.bottom(),
        "the body scrolled the whole answer into view: {answer:?} in {body:?}"
    );
    assert!(
        bounds(cx, "launch").top() < body.top(),
        "and the body really scrolled: Launch, at its top, left the view"
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

/// Why this matters: on macOS a click focuses no button, so the press on an inline question's answer falls through to
/// the owning dialog's empty-space rule, and in a dialog that takes focus on an empty-space press (Templates, Trash)
/// focus is on the dialog itself by the time the answer runs. The question used to return focus only from an answer
/// that held it, so a pointer answer there left focus on the dialog: the Templates leave question's Keep editing did
/// not return to the name field, which SPEC.md asks for, and the Trash dialog's own test reached its Cancel by keyboard
/// to get around the same loss. The Linux suite never saw it, since a Linux click focuses the answer first. The dialog
/// whose empty space takes focus is the case the fix is for; the one whose empty space keeps it is a regression guard.
///
/// Specified, under the macOS click rule, in a dialog whose empty-space press focuses the dialog and in one where it
/// keeps focus on the control in use: a pointer click on Cancel, the guarded option or the one-time answer closes the
/// question and returns focus to Launch, which held it when the question opened, as Enter on the same answer does. A
/// click on the answer that can fail leaves the question open until its attempt settles; success then returns focus to
/// Launch too. In the dialog whose empty-space press focuses it, a press on Reset while an attempt is in flight is the
/// user moving on (it focuses the dialog), and success then leaves focus there.
fn a_pointer_answer_returns_focus_under_the_macos_click_rule(
    cx: &mut TestAppContext,
    keeps_focus_on_empty_click: bool,
) {
    let (cx, launcher) = open_launcher_with(cx, keeps_focus_on_empty_click);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    let rule = if keeps_focus_on_empty_click {
        "empty space keeps focus"
    } else {
        "empty space focuses the dialog"
    };
    let ask = |cx: &mut VisualTestContext, ran: &Ran, attempts: &Attempts| {
        let inline = ask_inline_in(cx, &launcher, yolo(ran, attempts), ran);
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("prompt-cancel"),
            "{rule}: the premise: the question took focus on Cancel"
        );
        inline
    };
    for (answer, expected) in [
        ("prompt-cancel", None),
        ("prompt-alternative-0", Some("guarded")),
        ("prompt-alternative-1", Some("always")),
        ("prompt-confirm", Some("confirmed")),
    ] {
        let (ran, attempts): (Ran, Attempts) = Default::default();
        let inline = ask(cx, &ran, &attempts);
        click(cx, answer);
        if !attempts.borrow().is_empty() {
            assert!(
                cx.update(|_, cx| inline.is_open(cx)),
                "{rule}: the question stays up while {answer}'s attempt is in flight"
            );
            settle_attempt(cx, &attempts, None);
        }
        let expected: Vec<String> = expected
            .map(|ran| format!("{ran} codex on build-box"))
            .into_iter()
            .collect();
        assert_eq!(
            *ran.borrow(),
            expected,
            "{rule}: a click on {answer} ran it once"
        );
        assert!(
            !cx.update(|_, cx| inline.is_open(cx)),
            "{rule}: a click on {answer} closed the question"
        );
        assert!(
            focused(cx, &launcher.launch),
            "{rule}: after a click on {answer}, focus went back to Launch; focused: {:?}",
            focused_id(cx)
        );
        assert!(
            is_open(cx, launcher.layer),
            "{rule}: the launcher stayed open"
        );
    }

    // Where empty space keeps focus, a macOS press on Reset leaves focus on the question's Cancel, and success returns
    // it from there by the answer-holds-focus rule, so only the dialog that takes focus shows the press moving on.
    if keeps_focus_on_empty_click {
        return;
    }
    let (ran, attempts): (Ran, Attempts) = Default::default();
    let inline = ask(cx, &ran, &attempts);
    click(cx, "prompt-alternative-1");
    assert_eq!(
        attempts.borrow().len(),
        1,
        "{rule}: the premise: an attempt is in flight"
    );
    click(cx, "reset");
    assert!(
        focused(cx, &launcher.container),
        "{rule}: the premise: the press on Reset focused the dialog"
    );
    settle_attempt(cx, &attempts, None);
    assert!(
        !cx.update(|_, cx| inline.is_open(cx)),
        "{rule}: success closed the question"
    );
    assert!(
        focused(cx, &launcher.container),
        "{rule}: success after the user pressed elsewhere left focus on the dialog"
    );
}

/// [`a_pointer_answer_returns_focus_under_the_macos_click_rule`] in a dialog whose empty-space press focuses the
/// dialog, as the Templates and Trash dialogs' does: the case that left focus on the dialog.
#[gpui::test]
fn a_pointer_answer_returns_focus_on_a_mac_where_empty_space_focuses_the_dialog(
    cx: &mut TestAppContext,
) {
    a_pointer_answer_returns_focus_under_the_macos_click_rule(cx, false);
}

/// [`a_pointer_answer_returns_focus_under_the_macos_click_rule`] in a dialog whose empty-space press keeps focus on the
/// control in use, as the launcher's and Settings' do. A regression guard for those dialogs, not a guard of the fix:
/// there a macOS click leaves focus on the answer in use, so this passed before clicks counted, and it is here so the
/// click path cannot break the dialogs that already worked.
#[gpui::test]
fn a_pointer_answer_returns_focus_on_a_mac_where_empty_space_keeps_focus(cx: &mut TestAppContext) {
    a_pointer_answer_returns_focus_under_the_macos_click_rule(cx, true);
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

/// Why this matters: the YOLO question carries an answer that can fail without the empty-click rule, and it is drawn
/// loud, which only the inline question draws. Asked anchored, a press outside it would take it away while its
/// answer's work runs on, and the work would then finish for a question the user can no longer see; asked centered,
/// a click on its empty space would take focus off the busy answer. Asking it as a layer must fail at once rather than
/// open.
///
/// Specified: the YOLO question, asked centered or anchored, does not open.
/// [`a_question_that_can_fail_opens_as_a_layer_only_centered_with_the_empty_click_rule`] covers the centered questions
/// that may.
#[gpui::test]
fn the_yolo_question_does_not_open_as_a_layer(cx: &mut TestAppContext) {
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

/// Record each run of a fallible answer as "<name> <frozen value>" in `ran`, keeping its attempt in `attempts` for the
/// test to settle.
fn attempting(
    name: &'static str,
    ran: &Ran,
    attempts: &Attempts,
) -> impl Fn(String, prompt::Attempt, &mut Window, &mut gpui::App) + 'static {
    let (ran, attempts) = (ran.clone(), attempts.clone());
    move |frozen, attempt, _, _| {
        ran.borrow_mut().push(format!("{name} {frozen}"));
        attempts.borrow_mut().push(attempt);
    }
}

/// The host remove confirmation's shape, asked centered with a mark taken now: [`question`] "alpha" with the
/// empty-click rule, its confirming answer ("remove") and one extra answer ("always", the permanent one) both able to
/// fail, each recording into `ran` and `attempts` ([`attempting`]). `subject`, if given, closes it when gone. Panics if
/// it did not open.
fn ask_removal(
    cx: &mut VisualTestContext,
    subject: Option<Subject>,
    ran: &Ran,
    attempts: &Attempts,
) -> LayerId {
    let question = question("alpha")
        .keeps_focus_on_empty_click()
        .fallible_alternative(keep_it_answer(), attempting("always", ran, attempts));
    let confirm = attempting("remove", ran, attempts);
    let layer = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask_fallible(question, subject, &mark, confirm, window, cx)
    });
    settle(cx);
    layer.expect("the premise: a centered question with the empty-click rule opened")
}

/// Why this matters: the host remove confirmation is a centered dialog whose confirming answer sends the removal, and
/// it ports the old dialog (`HostRemoveDialog` in `crates/farhelm-ui/src/hosts/settings_dialog.rs` at reference commit
/// `24bc0580`), which stayed up through the helm's answer and showed a refusal inside it. A question that closed on a
/// stray press or an Escape while the removal was in flight would leave a refusal with nowhere to show, and one that
/// let Escape through and then reopened would not be the question the user answered. After a refusal the old dialog
/// sent focus to its Cancel, "so the refusal can be dismissed with Enter or Escape": with focus left on Remove, an
/// Enter meant to dismiss would send the removal again. And one that stayed up after its removal succeeded would ask
/// about a removal already done.
///
/// Specified, once for each way it ends (Enter, Escape, a dismissal through the layer manager (`layers::dismiss`, which
/// any Close control routes through), and a retry that succeeds): Enter on the confirming answer runs its action once
/// and leaves the question open, focus on that answer. While its attempt is in flight, a press on the veil, a press on
/// the question's empty space, Escape and a dismissal run nothing and leave it open, focus still on the answer. When
/// the attempt fails, the question stays open with the reason painted inside it (a host value in it escaped), and focus
/// moves to Cancel. Tab back to the confirming answer and Enter retries, running the action again and clearing the
/// reason. A retry that succeeds closes the question and returns focus to the opener; one that fails again leaves it
/// open with focus on Cancel, and Enter, Escape or the dismissal then closes it, returning focus to the opener. Nothing
/// else runs.
#[gpui::test]
fn a_failing_confirming_answer_keeps_a_centered_question_up_with_the_reason(
    cx: &mut TestAppContext,
) {
    let (cx, row) = open_row(cx);
    let twice = ["remove alpha", "remove alpha"];
    for ending in ["enter", "escape", "dismiss", "success"] {
        let (ran, attempts): (Ran, Attempts) = Default::default();
        click(cx, "opener");
        let layer = ask_removal(cx, None, &ran, &attempts);
        let open_on = |cx: &mut VisualTestContext, when: &str, answer: &str| {
            assert!(is_open(cx, layer), "{ending}, {when}: the question is open");
            assert_eq!(
                focused_id(cx).as_deref(),
                Some(answer),
                "{ending}, {when}: where focus is"
            );
        };
        let reason_painted = |cx: &mut VisualTestContext| {
            painted_in_question(cx)
                .iter()
                .any(|text| text.contains("build-box<U+202E>"))
        };
        open_on(cx, "the premise", "prompt-confirm");
        keys(cx, "enter");
        assert_eq!(
            *ran.borrow(),
            ["remove alpha"],
            "{ending}: the answer ran once"
        );
        open_on(cx, "in flight", "prompt-confirm");

        press_veil(cx);
        open_on(cx, "after a press on the veil", "prompt-confirm");
        press_empty_space(cx);
        open_on(cx, "after a press on its empty space", "prompt-confirm");
        keys(cx, "escape");
        cx.update(|window, cx| layers::dismiss(layer, window, cx));
        settle(cx);
        assert_eq!(
            *ran.borrow(),
            ["remove alpha"],
            "{ending}: nothing else ran in flight"
        );
        assert_eq!(
            attempts.borrow().len(),
            1,
            "{ending}: one attempt is in flight"
        );
        open_on(
            cx,
            "after Escape and a dismissal in flight",
            "prompt-confirm",
        );

        let host = HostText::from_host("build-box\u{202E}");
        let refusal = || {
            HostSentence::new("prompt-reason")
                .text("Farhelm refused to remove ")
                .host(&host)
        };
        settle_attempt(cx, &attempts, Some(refusal()));
        open_on(cx, "failed", "prompt-cancel");
        assert!(
            reason_painted(cx),
            "{ending}: the reason is painted inside the question, its host value escaped; painted: {:?}",
            painted_in_question(cx)
        );
        // Past the extra answer, back to the confirming one.
        keys(cx, "tab tab");
        open_on(cx, "the premise of the retry", "prompt-confirm");
        keys(cx, "enter");
        assert_eq!(*ran.borrow(), twice, "{ending}: Enter on it retried");
        open_on(cx, "retrying", "prompt-confirm");
        assert!(
            !reason_painted(cx),
            "{ending}: the retry cleared the old reason"
        );

        if ending == "success" {
            settle_attempt(cx, &attempts, None);
            assert!(
                !is_open(cx, layer),
                "{ending}: the retry's success closed the question"
            );
        } else {
            settle_attempt(cx, &attempts, Some(refusal()));
            open_on(cx, "failed again", "prompt-cancel");
            match ending {
                "enter" => keys(cx, "enter"),
                "escape" => keys(cx, "escape"),
                _ => {
                    cx.update(|window, cx| layers::dismiss(layer, window, cx));
                    settle(cx);
                }
            }
            assert!(
                !is_open(cx, layer),
                "{ending}: after the refusal it closed the question"
            );
        }
        assert!(
            focused(cx, &row.opener),
            "{ending}: focus went back to the opener"
        );
        assert_eq!(*ran.borrow(), twice, "{ending}: and nothing else ran");
    }
}

/// A press a test makes on the window while an attempt is in flight: a key, a click, a press on the veil.
type Press = fn(&mut VisualTestContext);

/// One run of the refusal-focus table: its name, each press with the control it must leave focused, and where focus
/// must be after the refusal.
type RefusalCase = (&'static str, Vec<(Press, &'static str)>, &'static str);

/// A press on the veil, outside the dialog.
fn press_veil(cx: &mut VisualTestContext) {
    let veil = point(px(2.), px(2.));
    assert!(
        !bounds(cx, "prompt").contains(&veil),
        "the premise: the press lands on the veil, outside the dialog"
    );
    cx.simulate_click(veil, Modifiers::none());
    check(cx);
}

/// A press on the question's empty space: its body's padding.
fn press_empty_space(cx: &mut VisualTestContext) {
    let body = bounds(cx, "prompt-body");
    cx.simulate_click(
        point(body.left() + px(4.), body.top() + px(4.)),
        Modifiers::none(),
    );
    check(cx);
}

/// Ask the host remove confirmation's shape ([`ask_removal`]), start its confirming answer's attempt with Enter, run
/// `presses` in turn, each followed by a check that focus is on the named control, then refuse the attempt; where focus
/// is then. The question is closed again afterwards, so a caller can run the next case on the same window.
fn focus_after_refusal(
    cx: &mut VisualTestContext,
    case: &str,
    presses: &[(Press, &str)],
) -> Option<String> {
    let (ran, attempts): (Ran, Attempts) = Default::default();
    click(cx, "opener");
    let layer = ask_removal(cx, None, &ran, &attempts);
    keys(cx, "enter");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "{case}, the premise: the attempt runs with focus on the pressed answer"
    );
    for (press, then_on) in presses {
        press(cx);
        assert!(
            is_open(cx, layer),
            "{case}, the premise: the question stays up"
        );
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(*then_on),
            "{case}, the premise: where the press left focus"
        );
    }
    let refusal = HostSentence::new("prompt-reason").text("Farhelm refused the removal");
    settle_attempt(cx, &attempts, Some(refusal));
    let landed = focused_id(cx);
    cx.update(|window, cx| layers::close(layer, window, cx));
    settle(cx);
    landed
}

/// Why this matters: SPEC.md, "Who may move focus": a result that arrives after the user moved on must not move focus.
/// A refusal of a centered question's attempt sends focus to Cancel (the old remove dialog's rule, so Enter does not
/// send the refused action again), but a user who tabbed or clicked to another control while the helm took its time
/// must keep their place. A press that acts on the question alone (its veil, its empty space, which keeps focus on the
/// busy answer, an Escape it holds) leaves focus on the answer the refusal re-arms, so the move must still happen then,
/// and must not after the user also moved focus first.
///
/// Specified, under the Linux click rule: on the host remove confirmation's shape, Enter on the confirming answer
/// starts an attempt, focus on it. With nothing pressed, or after a press on the veil, a press on the empty space or
/// Escape (each leaving focus on the confirming answer), the refusal moves focus to Cancel. After Shift-Tab to the
/// extra answer, a click on it, or Shift-Tab followed by a press on the veil or by Escape (each leaving focus on the
/// extra answer), the refusal leaves focus on the extra answer.
#[gpui::test]
fn a_refusal_moves_focus_to_cancel_only_while_focus_stayed_on_the_answer(cx: &mut TestAppContext) {
    let (cx, _row) = open_row(cx);
    let shift_tab: Press = |cx| keys(cx, "shift-tab");
    let escape: Press = |cx| keys(cx, "escape");
    let click_extra: Press = |cx| click(cx, "prompt-alternative-0");
    let (on_confirm, on_extra) = ("prompt-confirm", "prompt-alternative-0");
    let cases: [RefusalCase; 8] = [
        ("nothing pressed", vec![], "prompt-cancel"),
        (
            "a press on the veil",
            vec![(press_veil, on_confirm)],
            "prompt-cancel",
        ),
        (
            "a press on the empty space",
            vec![(press_empty_space, on_confirm)],
            "prompt-cancel",
        ),
        ("Escape", vec![(escape, on_confirm)], "prompt-cancel"),
        ("Shift-Tab", vec![(shift_tab, on_extra)], on_extra),
        (
            "a click on the extra answer",
            vec![(click_extra, on_extra)],
            on_extra,
        ),
        (
            "Shift-Tab, then the veil",
            vec![(shift_tab, on_extra), (press_veil, on_extra)],
            on_extra,
        ),
        (
            "Shift-Tab, then Escape",
            vec![(shift_tab, on_extra), (escape, on_extra)],
            on_extra,
        ),
    ];
    for (case, presses, lands_on) in cases {
        assert_eq!(
            focus_after_refusal(cx, case, &presses).as_deref(),
            Some(lands_on),
            "{case}: where focus is after the refusal"
        );
    }
}

/// Why this matters: under the macOS click rule a click focuses no control, so a click on another answer during the
/// attempt leaves focus on the answer pressed, which the refusal re-arms; moving focus to Cancel is then what keeps
/// Enter from sending the refused action again (SPEC_impl.md, "Questions: the prompt").
///
/// Specified, under the macOS click rule: on the host remove confirmation's shape, Enter on the confirming answer
/// starts an attempt; a click on the extra answer leaves focus on the confirming answer; the refusal moves focus to
/// Cancel.
#[gpui::test]
fn under_the_macos_click_rule_a_click_elsewhere_keeps_the_move_to_cancel(cx: &mut TestAppContext) {
    let (cx, _row) = open_row(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    let click_extra: Press = |cx| click(cx, "prompt-alternative-0");
    assert_eq!(
        focus_after_refusal(
            cx,
            "a click on the extra answer",
            &[(click_extra, "prompt-confirm")]
        )
        .as_deref(),
        Some("prompt-cancel")
    );
}

/// Why this matters: the host remove confirmation closes when the registry stops listing the host, and a successful
/// removal is exactly that, so its subject can go while its own attempt is still in flight. Holding the question open
/// then would leave a question about a host that is gone, frozen until an answer that may never come. An extra answer
/// that can fail (the permanent "Remove, don't ask again") is held the same way as the confirming one, and its refusal
/// sends focus to Cancel as the confirming answer's does: the old dialog's two answers shared one refusal path.
///
/// Specified: on a centered question with the empty-click rule, Enter on the fallible extra answer runs its action once
/// and leaves the question open, focus on that answer. When that attempt fails, the question stays open with the reason
/// painted inside it and focus on Cancel. Tab back to the extra answer and Enter runs it again, focus on it. The
/// subject going then closes it mid-attempt, returning focus to the opener, and the attempt settled afterwards runs
/// nothing and opens nothing.
#[gpui::test]
fn a_centered_extra_answer_refusal_and_its_subject_going_mid_attempt(cx: &mut TestAppContext) {
    let (cx, row) = open_row(cx);
    let thing: Entity<Thing> = cx.update(|_, cx| {
        cx.new(|_| Thing {
            name: "alpha".into(),
        })
    });
    let (ran, attempts): (Ran, Attempts) = Default::default();
    click(cx, "opener");
    let layer = ask_removal(cx, Some(Subject::of(&thing)), &ran, &attempts);
    keys(cx, "shift-tab enter");
    assert_eq!(*ran.borrow(), ["always alpha"], "the extra answer ran once");
    assert!(is_open(cx, layer), "its attempt kept the question open");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-alternative-0"),
        "focus stayed on the extra answer"
    );

    let host = HostText::from_host("build-box\u{202E}");
    let refusal = HostSentence::new("prompt-reason")
        .text("Farhelm refused to remove ")
        .host(&host);
    settle_attempt(cx, &attempts, Some(refusal));
    assert!(is_open(cx, layer), "the refusal left the question open");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-cancel"),
        "the refusal moved focus to Cancel"
    );
    let painted = painted_in_question(cx);
    assert!(
        painted
            .iter()
            .any(|text| text.contains("build-box<U+202E>")),
        "the reason is painted inside the question; painted: {painted:?}"
    );
    keys(cx, "tab enter");
    let twice = ["always alpha", "always alpha"];
    assert_eq!(
        *ran.borrow(),
        twice,
        "Tab back and Enter ran the extra answer again"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-alternative-0"),
        "focus is on the extra answer for its retry"
    );

    drop(thing);
    settle(cx);
    assert!(
        !is_open(cx, layer),
        "the subject going closed it mid-attempt"
    );
    assert!(focused(cx, &row.opener), "focus went back to the opener");
    settle_attempt(cx, &attempts, None);
    assert_eq!(*ran.borrow(), twice, "the late settling ran nothing");
    assert!(!any_open(cx), "and opened nothing");
}

/// Why this matters: the rule for where an answer that can fail is allowed is what keeps such a question from being
/// taken away mid-attempt. An anchored question closes on a press outside it, which nothing can veto, and a centered
/// one without the empty-click rule lets a click on its empty space take focus off the busy answer. Without the check a
/// feature could ask either and lose the refusal it meant to show.
///
/// Specified: a question whose confirming answer can fail, asked centered without the empty-click rule, does not open;
/// a question with the empty-click rule and an extra answer that can fail opens centered but not anchored.
#[gpui::test]
fn a_question_that_can_fail_opens_as_a_layer_only_centered_with_the_empty_click_rule(
    cx: &mut TestAppContext,
) {
    let (cx, row) = open_row(cx);
    let (ran, attempts): (Ran, Attempts) = Default::default();
    let confirm = attempting("remove", &ran, &attempts);
    let asked = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask_fallible(question("alpha"), None, &mark, confirm, window, cx)
    });
    settle(cx);
    assert!(
        asked.is_none(),
        "without the empty-click rule it did not open"
    );
    for ((kind, presentation), opens) in row.presentations().into_iter().zip([true, false]) {
        let question = question("alpha")
            .keeps_focus_on_empty_click()
            .fallible_alternative(keep_it_answer(), attempting("always", &ran, &attempts));
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
        assert_eq!(asked.is_some(), opens, "{kind}: whether it opened");
        if let Some(asked) = asked {
            cx.update(|window, cx| layers::close(asked.layer, window, cx));
            settle(cx);
        }
    }
}

// ---- Options a presentation ignores ----------------------------------------------------------------------------------

/// `question` with every option that only some presentations take: the empty-click rule, the loud look, a body, a tile,
/// a note, a width, a scroll stop and Escape sending focus home. ([`Question::returns_to`] excludes the last, so its
/// own test covers it.) Each test below asks it one way, and the assertion's message names
/// exactly the options that way ignores, in the builders' order, so a test pins both what is refused and what is not.
#[cfg(debug_assertions)]
fn with_every_presentation_option(question: Question<String>) -> Question<String> {
    question
        .keeps_focus_on_empty_click()
        .loud()
        .body(|_: &String, _, cx| {
            cx.new(|cx| PlanBody {
                copy: cx.focus_handle().tab_stop(true),
                copied: Rc::default(),
            })
        })
        .tile(|_, _| div())
        .note("Undo it with the gear")
        .width(px(560.))
        .scroll_stop("Question text")
        .returns_home_on_escape()
}

/// Why this matters: a release build drops an option its presentation ignores, so a question that is moved from one
/// presentation to another, or built by a helper shared between two, loses its look or its rule with nothing on screen
/// to say so. The debug build the tests run in has to fail instead, naming the option and the call.
///
/// Specified: a centered question carrying every presentation-specific option fails a debug build as it is asked,
/// naming the centered question's calls (`prompt::ask`, `prompt::ask_rewordable` or `prompt::ask_fallible`) and the
/// loud look only, since the centered question takes all the others.
#[cfg(debug_assertions)]
#[gpui::test]
#[should_panic(
    expected = "a question was asked through prompt::ask, prompt::ask_rewordable or prompt::ask_fallible with \
                Question::loud, which it ignores"
)]
fn a_centered_question_with_an_option_it_ignores_fails_a_debug_build(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let question = with_every_presentation_option(question("alpha"));
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask(question, None, &mark, |_, _, _| {}, window, cx);
    });
}

/// Why this matters: the anchored question is where a row's or the header's question goes when its control is drawn,
/// and the same question falls back to centered when it is not, so a helper builds one question for both. A body or a
/// tile added there for the centered case would vanish whenever the anchored one shows.
///
/// Specified: an anchored question carrying every presentation-specific option fails a debug build as it is asked,
/// naming `prompt::ask_anchored` with the loud look, the body, the tile, the note, the width and the scroll stop, and
/// not the empty-click and Escape-home rules, which an anchored question keeps.
#[cfg(debug_assertions)]
#[gpui::test]
#[should_panic(
    expected = "a question was asked through prompt::ask_anchored with Question::loud, Question::body, \
                Question::tile, Question::note, Question::width, Question::scroll_stop, which it ignores"
)]
fn an_anchored_question_with_an_option_it_ignores_fails_a_debug_build(cx: &mut TestAppContext) {
    let (cx, row) = open_row(cx);
    let Some(anchor) = row.anchor.borrow().clone() else {
        panic!("the premise: the row's menu button was drawn")
    };
    let question = with_every_presentation_option(question("alpha"));
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask_anchored(question, anchor, None, &mark, |_, _, _| {}, window, cx);
    });
}

/// Why this matters: an inline question leaves its empty space and its Escape to the owner's dialog, so a question
/// built for a layer and asked inline would quietly lose the rule that keeps a stray click from arming Enter, or the
/// return to the terminal SPEC.md asks of the header's questions.
///
/// Specified: an inline question carrying every presentation-specific option fails a debug build as it is asked,
/// naming `prompt::ask_inline` with the empty-click rule, the body, the tile, the note, the width, the scroll stop and
/// the Escape-home rule, and not the loud look, which is the inline question's own.
#[cfg(debug_assertions)]
#[gpui::test]
#[should_panic(
    expected = "a question was asked through prompt::ask_inline with Question::keeps_focus_on_empty_click, \
                Question::body, Question::tile, Question::note, Question::width, Question::scroll_stop, \
                Question::returns_home_on_escape, which it ignores"
)]
fn an_inline_question_with_an_option_it_ignores_fails_a_debug_build(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let question = with_every_presentation_option(question("alpha"));
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        let _ = prompt::ask_inline(question, &mark, |_, _, _| {}, window, cx);
    });
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

// ---- A link ----------------------------------------------------------------------------------------------------------

/// `question` given a link to `url`, as SPEC.md's sandbox question links Tensorlake pricing.
fn linked<T: Clone + 'static>(question: Question<T>, url: &str) -> Question<T> {
    question.link(
        "Pricing",
        url.to_owned(),
        HoverHelp::tooltip("Open the pricing page in your browser"),
    )
}

/// Assert that the link is drawn below the text painted as `line` and above the answers, in a fresh frame.
fn assert_link_between(cx: &mut VisualTestContext, line: &str, case: &str) {
    let line = frame(cx)
        .iter()
        .flat_map(|node| node.texts.iter())
        .find(|text| text.text.as_ref() == line)
        .map(|text| text.bounds)
        .expect("the question's line was painted");
    let (link, answers) = (bounds(cx, "prompt-link"), bounds(cx, "prompt-answers"));
    assert!(
        line.bottom() <= link.top() && link.bottom() <= answers.top(),
        "{case}: the link is drawn between the line and the answers: line {line:?}, link {link:?}, answers {answers:?}"
    );
}

/// Why this matters: the link is the one control on a question that is not an answer and still sits among them. If
/// Enter or a click on it answered the question, or reached the dialog's Enter rule, a user opening the pricing page
/// would create the sandbox they were reading about. And the link must be a tab stop, or a keyboard user could not
/// open it at all.
///
/// Specified, for the centered and the anchored question with a line and a link: the link is drawn below the line and
/// above the answers; focus starts on Confirm, as without a link; Tab from Confirm reaches the link, then Cancel. Enter
/// on the focused link, and in a second question a click on it, opens the link's URL through the app's opener and
/// leaves the question open with no answer run; Cancel then closes it running nothing.
///
/// Inside the layer's trap Tab's order is a cycle, which looks the same whichever stop it starts from, so the walk
/// shows only that the link is a stop in the trap. Where it sits is pinned by where it is drawn, here, and in Tab's
/// order by the inline test, whose Shift+Tab leaves the question for the launcher's Reset.
#[gpui::test]
fn a_link_sits_between_text_and_answers_and_opens_without_answering(cx: &mut TestAppContext) {
    let (cx, row) = open_row(cx);
    for (kind, presentation) in row.presentations() {
        for way in ["enter", "click"] {
            let case = format!("{kind}, {way}");
            let url = format!("https://example.com/pricing/{kind}/{way}");
            let ran: Ran = Default::default();
            let confirmed = ran.clone();
            let shown = presentation.clone();
            let asked = cx.update(|window, cx| {
                let mark = Mark::take(cx);
                prompt::ask_for_test(
                    shown,
                    linked(question("alpha"), &url),
                    None,
                    &mark,
                    move |frozen, _, _| confirmed.borrow_mut().push(format!("confirmed {frozen}")),
                    window,
                    cx,
                )
            });
            settle(cx);
            let asked = asked.expect("the question opened");
            assert_link_between(cx, "Its history is removed.", &case);
            assert_eq!(
                focused_id(cx).as_deref(),
                Some("prompt-confirm"),
                "{case}: focus starts on Confirm"
            );
            for expected in ["prompt-link", "prompt-cancel"] {
                keys(cx, "tab");
                assert_eq!(
                    focused_id(cx).as_deref(),
                    Some(expected),
                    "{case}: Tab reaches the link, then the answers"
                );
            }
            // Back onto the link for the Enter case; the click case clicks it wherever focus is.
            keys(cx, "shift-tab");
            if way == "enter" {
                keys(cx, "enter");
            } else {
                click(cx, "prompt-link");
            }
            assert_eq!(
                cx.opened_url(),
                Some(url),
                "{case}: the link opened its URL"
            );
            assert!(is_open(cx, asked.layer), "{case}: the question stayed open");
            assert!(ran.borrow().is_empty(), "{case}: no answer ran");
            cx.update(|window, cx| asked.cancel(window, cx));
            settle(cx);
            assert!(ran.borrow().is_empty(), "{case}: Cancel ran nothing");
        }
    }
}

/// Why this matters: SPEC.md's sandbox question is asked inline in the launcher, where Tab passes through the question
/// as through the rest of the dialog, so the link's place in Tab's order is between the launcher's controls above, the
/// question's text, and its answers. Escape there is the question's Cancel: if focus on the link were not counted as
/// focus in the question, Cancel would close it and leave focus nowhere in the launcher.
///
/// Specified, for the YOLO question given a link, asked inline in the launcher: the link is drawn below the question's
/// last line and above its answers, with the link role; focus starts on Cancel, as without a link; Shift+Tab from
/// Cancel reaches the link, then the launcher's Reset above the question. Enter on the focused link opens its URL and
/// leaves the question open with no answer run and focus on the link. Escape on the link then cancels it, running
/// nothing, returns focus to Launch, and leaves the launcher open.
#[gpui::test]
fn an_inline_questions_link_opens_without_answering_and_escape_on_it_cancels(
    cx: &mut TestAppContext,
) {
    let (cx, launcher) = open_launcher(cx);
    let url = "https://example.com/pricing/inline";
    let (ran, attempts): (Ran, Attempts) = Default::default();
    let inline = ask_inline_in(cx, &launcher, linked(yolo(&ran, &attempts), url), &ran);
    assert_link_between(cx, "Nothing has been started.", "inline");
    let role = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("prompt-link"))
        .and_then(|node| node.role);
    assert_eq!(role, Some(Role::Link), "the link is named a link");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-cancel"),
        "focus starts on Cancel"
    );
    for (key, expected) in [
        ("shift-tab", "prompt-link"),
        ("shift-tab", "reset"),
        ("tab", "prompt-link"),
    ] {
        keys(cx, key);
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(expected),
            "{key} goes from the answers to the link to the launcher's Reset, and back"
        );
    }
    keys(cx, "enter");
    assert_eq!(
        cx.opened_url().as_deref(),
        Some(url),
        "the link opened its URL"
    );
    assert!(
        cx.update(|_, cx| inline.is_open(cx)),
        "the question stayed open"
    );
    assert!(ran.borrow().is_empty(), "no answer ran");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-link"),
        "focus stayed on the link"
    );
    keys(cx, "escape");
    assert!(ran.borrow().is_empty(), "Escape ran nothing");
    assert!(
        !cx.update(|_, cx| inline.is_open(cx)),
        "Escape on the link cancelled the question"
    );
    assert!(focused(cx, &launcher.launch), "focus went back to Launch");
    assert!(is_open(cx, launcher.layer), "the launcher stayed open");
}

// ---- Rewording an open question --------------------------------------------------------------------------------------

/// The body a rewordable test question draws, standing in for the bulk question's row per session: `count` lines, each
/// `name` followed by its number from 1, drawn under `listed-0` on. Nothing in it takes focus.
struct Listed {
    name: String,
    count: usize,
}

impl Render for Listed {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let name = self.name.clone();
        div().children((0..self.count).map(move |index| {
            div()
                .debug_selector(move || format!("listed-{index}"))
                .child(format!("{name} {}", index + 1))
        }))
    }
}

/// A question shaped like the bulk Delete question, about the sessions `names` and frozen at them: its title, summary,
/// line and Confirm's label and help all count or name them, its body lists them `rows` times ([`Listed`], so a test
/// can make it overflow), and its text has the scroll stop "Sessions affected". Each body build records the names it
/// was handed in `built`.
fn bulk(names: &[&str], rows: usize, built: &Built) -> Question<Vec<String>> {
    let count = names.len();
    let record = built.clone();
    Question::new(
        format!("Delete {count} sessions?"),
        Answer::new(
            format!("Delete {count}"),
            HoverHelp::tooltip(format!("Delete the {count} sessions listed")),
            Tier::Danger,
        ),
        names
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>(),
    )
    .summary(format!("Marked: {}", names.join(", ")))
    .line(format!("{count} sessions are removed."))
    .keeps_focus_on_empty_click()
    .scroll_stop("Sessions affected")
    .body(move |frozen: &Vec<String>, _, cx| {
        let name = frozen.join(",");
        record.borrow_mut().push(name.clone());
        cx.new(|_| Listed { name, count: rows })
    })
}

/// Ask `question` through [`prompt::ask_rewordable`] with a mark taken now, its Confirm recording the frozen names
/// (joined with commas) in `confirmed`, and draw it. Panics if it did not open.
fn ask_rewordable(
    cx: &mut VisualTestContext,
    question: Question<Vec<String>>,
    confirmed: &Ran,
) -> prompt::Open<Vec<String>> {
    let record = confirmed.clone();
    let open = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask_rewordable(
            question,
            None,
            &mark,
            move |frozen: Vec<String>, _, _| record.borrow_mut().push(frozen.join(",")),
            window,
            cx,
        )
    });
    settle(cx);
    open.expect("the premise: the question opened")
}

/// Reword `open` to `question`, then let whatever that queued run and draw.
fn reword(
    cx: &mut VisualTestContext,
    open: &prompt::Open<Vec<String>>,
    question: Question<Vec<String>>,
) -> bool {
    let reworded = cx.update(|window, cx| open.reword(question, window, cx));
    settle(cx);
    reworded
}

/// Why this matters: SPEC.md's bulk Stop and Delete question rewords itself when a target it lists changes, and the
/// first port did it by closing the question and asking the replacement, which put focus back on the starting answer
/// after every listing change (a user who had tabbed to Cancel or into the list was moved) and could pull focus out of
/// an approval card beside it. A reword that left any of its words stale would show the user one set of targets while
/// the answer acts on another, the very bug class the prompt exists to prevent.
///
/// Specified: rewording a question from two sessions to three, with focus on Confirm, on Cancel, or on the text's
/// scroll stop, keeps the same layer open and focus on that same control. With focus on Confirm, the next frame paints
/// the new title, summary, line and body and none of the old title, the body is built once more from the new frozen
/// value, and Confirm is labelled and helped with the new count.
#[gpui::test]
fn a_reword_changes_the_words_in_place_and_keeps_focus_where_it_is(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    for (on, keystrokes) in [
        ("prompt-confirm", ""),
        ("prompt-cancel", "shift-tab"),
        ("prompt-body-stop", "shift-tab shift-tab"),
    ] {
        let (built, confirmed): (Built, Ran) = Default::default();
        let open = ask_rewordable(cx, bulk(&["alpha", "beta"], 1, &built), &confirmed);
        if !keystrokes.is_empty() {
            keys(cx, keystrokes);
        }
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(on),
            "the premise: focus is on {on}"
        );
        assert!(
            reword(cx, &open, bulk(&["alpha", "beta", "gamma"], 1, &built)),
            "{on}: the open question was reworded"
        );
        assert!(
            is_open(cx, open.layer()),
            "{on}: the same question is still open"
        );
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(on),
            "{on}: focus stayed where it was"
        );
        if on != "prompt-confirm" {
            keys(cx, "escape");
            continue;
        }

        let painted = painted_in_question(cx);
        for words in [
            "Delete 3 sessions?",
            "Marked: alpha, beta, gamma",
            "3 sessions are removed.",
            "alpha,beta,gamma 1",
        ] {
            assert!(
                painted.iter().any(|text| text == words),
                "{words:?} is painted; painted: {painted:?}"
            );
        }
        assert!(
            !painted.iter().any(|text| text == "Delete 2 sessions?"),
            "the old title is gone; painted: {painted:?}"
        );
        assert_eq!(
            *built.borrow(),
            ["alpha,beta", "alpha,beta,gamma"],
            "the body was built once more, from the new frozen value"
        );
        assert_eq!(
            label(cx, "prompt-confirm").as_deref(),
            Some("Delete 3"),
            "Confirm carries the new label"
        );
        let confirm = center(cx, "prompt-confirm");
        cx.simulate_mouse_move(confirm, None, Modifiers::none());
        advance(cx, layers::HOVER_DELAY);
        assert_eq!(
            cx.update(|window, cx| layers::shown_tooltip(window, cx))
                .map(|shown| shown.text),
            Some("Delete the 3 sessions listed".into()),
            "Confirm shows the new help"
        );
        keys(cx, "escape");
    }
}

/// Why this matters: the answers of a reworded question must act on what it now shows, or the bulk question would
/// delete the targets of its first wording after the user read the second. And an answer the user gave to the old
/// words must not authorize the new ones (SPEC.md: "An old answer cannot authorize a replacement question"). The key
/// is the case that matters: GPUI draws a dirty window before it handles a key, but shows that frame only at the next
/// frame, so an Enter already queued when a reword lands is handled with the new words drawn and not yet on screen.
/// Focus starts on Confirm, so that Enter is the likeliest answer of all.
///
/// Specified, on a question about two sessions with focus on Confirm: after a reword to one session, an Enter
/// dispatched as input before the next frame is shown runs nothing and leaves the question open; once that frame has
/// been shown, Enter confirms once, on the one session, and closes the question.
#[gpui::test]
fn an_answer_after_a_reword_acts_on_the_new_value_once_it_is_shown(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let (built, confirmed): (Built, Ran) = Default::default();
    let open = ask_rewordable(cx, bulk(&["alpha", "beta"], 1, &built), &confirmed);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "the premise: focus starts on Confirm"
    );
    let reworded = cx.update(|window, cx| open.reword(bulk(&["alpha"], 1, &built), window, cx));
    assert!(reworded, "the premise: the question was reworded");
    // The new wording has drawn but still has to be presented. Raw input preserves that gap.
    cx.simulate_keystrokes("enter");
    crate::test_support::check(cx);
    assert!(
        confirmed.borrow().is_empty(),
        "an Enter before the new words were shown ran nothing: {:?}",
        confirmed.borrow()
    );
    assert!(is_open(cx, open.layer()), "and left the question open");

    show_next_frame(cx);
    keys(cx, "enter");
    assert_eq!(
        *confirmed.borrow(),
        ["alpha"],
        "Enter after the new words were shown confirmed once, on the reworded value"
    );
    assert!(!is_open(cx, open.layer()), "Confirm closed the question");
}

/// Why this matters: the bulk question's asker rewords from a listing change that can arrive after the user closed the
/// question, or after it closed because its subject went. A reword there that drew or reopened anything would put a
/// question in front of a user who had dismissed it (SPEC.md: things that finish later never open over what the user
/// is doing). A reword from inside the question's own answer is the case a closed check must catch: the answer runs
/// inside the prompt's own update, after the close, so the prompt is still alive there and updating it again would
/// panic.
///
/// Specified, for a question closed by Escape, and for one whose Confirm action rewords it: the reword returns `false`,
/// builds no body and opens nothing, and only the Confirm, once, ran.
#[gpui::test]
fn a_reword_after_the_question_closed_does_nothing(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    for close in ["escape", "confirm"] {
        let built: Built = Default::default();
        let ran: Ran = Default::default();
        let handle: Rc<RefCell<Option<prompt::Open<Vec<String>>>>> = Rc::default();
        let (record, inside, rebuilt) = (ran.clone(), handle.clone(), built.clone());
        let opened = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            prompt::ask_rewordable(
                bulk(&["alpha", "beta"], 1, &built),
                None,
                &mark,
                move |frozen: Vec<String>, window, cx| {
                    let reworded = inside
                        .borrow()
                        .as_ref()
                        .map(|open| open.reword(bulk(&["gamma"], 1, &rebuilt), window, cx));
                    record.borrow_mut().push(format!(
                        "confirmed {}, reworded {reworded:?}",
                        frozen.join(",")
                    ));
                },
                window,
                cx,
            )
        });
        settle(cx);
        let layer = opened.as_ref().expect("the premise: it opened").layer();
        handle.replace(opened);
        let reworded = if close == "escape" {
            keys(cx, "escape");
            assert!(!is_open(cx, layer), "escape: the premise: it closed");
            let open = handle
                .borrow_mut()
                .take()
                .expect("the test holds the handle");
            Some(reword(cx, &open, bulk(&["gamma"], 1, &built)))
        } else {
            keys(cx, "enter");
            None
        };
        let expected: Vec<String> = match reworded {
            Some(reworded) => {
                assert!(!reworded, "escape: the reword reported that it did nothing");
                Vec::new()
            }
            None => vec!["confirmed alpha,beta, reworded Some(false)".into()],
        };
        assert_eq!(*ran.borrow(), expected, "{close}: what ran");
        assert_eq!(
            *built.borrow(),
            ["alpha,beta"],
            "{close}: no body was built"
        );
        assert!(!any_open(cx), "{close}: nothing is open");
    }
}

/// Why this matters: an answer whose action can fail is handed the frozen value when it is pressed and keeps the
/// question up until it settles. A reword under it would show one set of targets while the action settles another,
/// and a failure would then arm answers beside a reason about something no longer shown.
///
/// Specified, on a bulk-shaped question with an extra answer that can fail: while that answer's attempt is in flight a
/// reword returns `false`, builds no body and leaves the old words drawn; once the attempt has failed, a reword
/// succeeds and the new words are drawn.
#[gpui::test]
fn a_reword_waits_out_an_attempt_in_flight(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let (built, confirmed, ran): (Built, Ran, Ran) = Default::default();
    let attempts: Attempts = Default::default();
    let (record, kept) = (ran.clone(), attempts.clone());
    let question = bulk(&["alpha", "beta"], 1, &built).fallible_alternative(
        keep_it_answer(),
        move |frozen: Vec<String>, attempt, _, _| {
            record.borrow_mut().push(frozen.join(","));
            kept.borrow_mut().push(attempt);
        },
    );
    let open = ask_rewordable(cx, question, &confirmed);
    keys(cx, "shift-tab enter");
    assert_eq!(
        *ran.borrow(),
        ["alpha,beta"],
        "the premise: the extra answer's attempt is in flight"
    );

    let alternative = |built: &Built| {
        bulk(&["gamma"], 1, built).fallible_alternative(keep_it_answer(), |_, _, _, _| {})
    };
    assert!(
        !reword(cx, &open, alternative(&built)),
        "the reword in flight reported that it did nothing"
    );
    assert!(is_open(cx, open.layer()), "the question is still open");
    assert_eq!(*built.borrow(), ["alpha,beta"], "no body was built");
    let painted = painted_in_question(cx);
    assert!(
        painted.iter().any(|text| text == "Delete 2 sessions?"),
        "the old words are still drawn; painted: {painted:?}"
    );

    settle_attempt(
        cx,
        &attempts,
        Some(HostSentence::new("prompt-reason").text("Busy")),
    );
    assert!(
        reword(cx, &open, alternative(&built)),
        "after the failure, the reword went through"
    );
    let painted = painted_in_question(cx);
    assert!(
        painted.iter().any(|text| text == "Delete 1 sessions?"),
        "the new words are drawn; painted: {painted:?}"
    );
    keys(cx, "escape");
}

// ---- Returning focus to a named control ------------------------------------------------------------------------------

/// Why this matters: an inline question returns focus itself, to what held it inside its owner's dialog, so a named
/// control would be dropped without a word, and a question moved from a dialog to inline would quietly stop returning
/// where its feature asked.
///
/// Specified: an inline question built with `returns_to` fails a debug build as it is asked, naming
/// `prompt::ask_inline` and `Question::returns_to` only.
#[cfg(debug_assertions)]
#[gpui::test]
#[should_panic(
    expected = "a question was asked through prompt::ask_inline with Question::returns_to, which it ignores"
)]
fn an_inline_question_that_names_a_control_to_return_to_fails_a_debug_build(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| div().into_any_element());
    let control = handle(cx);
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        let _ = prompt::ask_inline(
            question("alpha").returns_to(control),
            &mark,
            |_, _, _| {},
            window,
            cx,
        );
    });
}

// ---- The text's scroll stop ------------------------------------------------------------------------------------------

/// With focus on Confirm (asserted as the premise), press Tab and assert that it reached `reached`; then hand back the
/// role and accessible name of the text's scroll stop in a fresh frame, or `None` when no stop is drawn.
fn tab_from_confirm(
    cx: &mut VisualTestContext,
    case: &str,
    reached: &str,
) -> Option<(Role, SharedString)> {
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "{case}: the premise: focus starts on Confirm"
    );
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(reached),
        "{case}: what Tab from Confirm reached"
    );
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("prompt-body-stop"))
        .and_then(|node| node.role.zip(node.label))
}

/// Why this matters: a bulk question lists a row per session, and its answers authorize exactly the rows it lists. With
/// more rows than fit, a keyboard user had no way to scroll the rows below the fold into view before confirming them,
/// since nothing in the list takes focus and GPUI scrolls nothing from the keyboard on its own (SPEC.md: Tab stays
/// within a modal's "visible, enabled controls and keyboard scroll regions"). The stop and its rule are the dialog
/// frame's, pinned in `components/tests/dialog.rs`; this pins that the question hands its label on, and that a
/// question not asking for one keeps its Tab order.
///
/// Specified, in a 400-pixel-high window, focus starting on Confirm each time: a question given the stop whose body
/// overflows has a region named "Sessions affected", which Tab from Confirm reaches first; a question not given the
/// stop has none, and Tab from Confirm reaches Cancel.
#[gpui::test]
fn a_question_given_a_scroll_stop_lets_the_keyboard_reach_its_text(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    cx.simulate_resize(size(px(720.), px(400.)));
    settle(cx);
    let built: Built = Default::default();

    let open = ask_rewordable(cx, bulk(&["alpha", "beta"], 60, &built), &Ran::default());
    assert!(
        bounds(cx, "listed-59").bottom() > bounds(cx, "prompt-body").bottom(),
        "the premise: the long body overflows"
    );
    assert_eq!(
        tab_from_confirm(cx, "given the stop", "prompt-body-stop"),
        Some((Role::Region, "Sessions affected".into())),
        "the stop is a region named by its label"
    );
    keys(cx, "escape");
    assert!(!is_open(cx, open.layer()), "Escape closed it");

    let asked = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask_for_test(
            Presentation::Centered,
            question("alpha"),
            None,
            &mark,
            |_, _, _| {},
            window,
            cx,
        )
    });
    settle(cx);
    let asked = asked.expect("the question opened");
    assert_eq!(
        tab_from_confirm(cx, "not given the stop", "prompt-cancel"),
        None,
        "a question not given the stop has none"
    );
    keys(cx, "escape");
    assert!(!is_open(cx, asked.layer), "Escape closed it");
}
