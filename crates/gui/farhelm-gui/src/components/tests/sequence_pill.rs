//! Tests of the sequence pill: a display-only element the layer manager draws above everything, so it must let every
//! press through to what it covers.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, IntoElement, Modifiers, ParentElement as _, Render, Styled as _,
    TestAppContext, Window, div, px,
};

use crate::components::{Button, HoverHelp, Role, SequenceChip, SequencePill};
use crate::host_text::HostText;
use crate::layers;
use crate::test_support::{bounds, check, open, redraw};

/// The sequence pill's view, for the test below: a pill with a chip, a word and candidates.
struct SequencePillScene;

impl Render for SequencePillScene {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        SequencePill::new("Launch sequence: ", "Enter launch")
            .chip(SequenceChip::Name(HostText::from_host("my-codex")))
            .chip(SequenceChip::Words("Unbound key 3".into()))
            .word(HostText::from_host("web"))
            .candidates([
                HostText::from_host("web"),
                HostText::from_host("webbuilder"),
            ])
    }
}

/// SPEC.md: "Neither the pill nor the list takes focus or pointer input". The layer manager draws the pill above
/// everything, so a pill that asked for the pointer would eat clicks meant for the terminal or a control under it, and
/// it can only check that by trusting what the component draws (the NOTE on `layers::Pill`). Specified: with the pill,
/// its chips, word and candidates shown centered over a button, a click where the pill covers the button runs the
/// button.
#[gpui::test]
fn a_click_on_the_sequence_pill_reaches_what_it_covers(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new(0));
    let out: Rc<RefCell<Option<layers::Anchor>>> = Rc::new(RefCell::new(None));
    let (counted, kept) = (clicks.clone(), out.clone());
    let cx = open(cx, move |window, cx| {
        let anchor = layers::Anchor::for_element("covered-anchor", window, cx);
        kept.replace(Some(anchor.clone()));
        let counted = counted.clone();
        div()
            .relative()
            .w(px(600.))
            .h(px(120.))
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Button::new(
                            "covered",
                            "Covered",
                            HoverHelp::tooltip("Under the pill"),
                            Role::Button,
                        )
                        .on_click(move |_, _, _| counted.set(counted.get() + 1)),
                    ),
            )
            .child(anchor.track())
            .into_any_element()
    });
    let anchor = out.borrow().clone().expect("the area was drawn");
    cx.update(|window, cx| {
        let pill = layers::Pill {
            view: cx.new(|_| SequencePillScene).into(),
            anchor,
            placement: layers::PillPlacement::Centered,
        };
        layers::show_pill(pill, window, cx)
    })
    .expect("the pill shows");
    redraw(cx);
    let (pill, button) = (bounds(cx, "sequence-capsule"), bounds(cx, "covered"));
    assert!(
        pill.intersects(&button),
        "the premise: the pill covers the button"
    );
    let at = pill.intersect(&button).center();
    cx.simulate_click(at, Modifiers::none());
    check(cx);
    assert_eq!(
        clicks.get(),
        1,
        "the click went through the pill to the button"
    );
}
