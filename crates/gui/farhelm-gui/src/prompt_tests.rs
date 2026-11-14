//! Behavior tests for the prompt, on GPUI's simulated platform: that a confirmation acts only on what it showed, where
//! a question starts and how it is answered, cancelled and closed, how its summary draws a host value, and the anchored
//! question asked beside its control.
//!
//! Each test opens its window through the shared harness ([`crate::test_support::open`]: GPUI Kit's `Root` around a
//! root view wired as the app's), drives it through simulated keys and clicks, so key bindings resolve as they do in
//! the app, and runs the harness's invariant checks after every step. The rules come from SPEC.md, "Keyboard focus",
//! and SPEC_impl.md, "Questions".

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AppContext as _, Entity, FocusHandle, InteractiveElement as _, IntoElement, Modifiers,
    ParentElement, Styled, TestAppContext, VisualTestContext, div, point, px,
};

use crate::components::Role;
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::layers::{Anchor, LayerId, Subject};
use crate::prompt::{self, Presentation, Question};
use crate::test_support::{
    Thing, bounds, button, check, click, focused, handle, is_open, keys, open, question, redraw,
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
/// target. Whatever runs acts on what the user was shown. The first answer wins, and Confirm runs at most once.
///
/// Here the frozen value is a plain copy, so the "subject changes" step cannot make the prompt act on something else and
/// cannot fail: the prompt never sees the live subject. That holds by convention, not by the type: `Question<T>` would
/// as happily carry a live handle (an entity, an `Rc<RefCell<_>>`), and a feature's `on_confirm` could capture live
/// state and ignore its argument. The step gains teeth when each real prompt kind runs it with its own subject and
/// command (SPEC_impl.md, "Questions: the prompt"); this test's teeth are the answer ordering (without the
/// first-answer-wins guard, Cancel then Confirm would send).
///
/// It runs for both presentations, centered and anchored: the anchored one is a different layer kind with its own
/// answer wiring, and the guarantee must not depend on where the question is drawn.
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
        ]
        .map(|order| (shown.clone(), order))
    }) {
        thing.update(cx, |thing, _| thing.name = "alpha".into());
        sent.borrow_mut().clear();
        let shown = thing.read_with(cx, |thing, _| thing.name.clone());
        let record = sent.clone();
        let asked = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            prompt::ask_for_test(
                presentation,
                question(&shown),
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
                    _ => asked.confirm(window, cx),
                }
            }
        });
        cx.run_until_parked();
        check(cx);
        let expected: Vec<String> = if order[0] == "confirm" {
            vec!["alpha".into()]
        } else {
            Vec::new()
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
    cx.update(|window, _| window.set_driver_enabled(true));
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
    redraw(cx);
    let nodes = cx.update(|window, _| window.driver_nodes().to_vec());
    (asked, nodes)
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
