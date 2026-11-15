//! Tests of hover help, the layer's shared rule in `components/mod.rs` (`HoverHelp` and `with_hover_help`): when a
//! control's tooltip shows and goes, whose help wins when one control sits inside another, a press putting it away,
//! and help that names a host value or comes in paragraphs. The tooltip itself is the layer manager's; how paragraph
//! help is laid out is pinned in `layers_tests.rs`.

use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    InteractiveElement as _, IntoElement, Modifiers, MouseButton, ParentElement as _, Styled as _,
    TestAppContext, VisualTestContext, Window, div, point, px,
};

use crate::components::{Button, HoverHelp, Role};
use crate::host_text::tests::{painted_words, paints_isolated};
use crate::host_text::{HostSentence, HostText};
use crate::layers;
use crate::test_support::{advance, bounds, center, check, frame, open, redraw};

use super::{tooltip, wait_hover_delay};

/// Hovering a control shows its help through the layer manager once the pointer has rested for the old app's hover
/// delay, and leaving it takes the help down. This is the old app's "every control has hover help" rule, now carried
/// by the constructor instead of a page-walking test; the delay is what keeps a pointer crossing the window from
/// flashing a tooltip on every control it passes.
#[gpui::test]
fn hovering_shows_the_controls_help_after_the_hover_delay_and_leaving_hides_it(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| {
        Button::new(
            "save",
            "Save",
            HoverHelp::tooltip("Save the title"),
            Role::Button,
        )
        .into_any_element()
    });

    let save = center(cx, "save");
    cx.simulate_mouse_move(save, None, Modifiers::none());
    check(cx);
    assert_eq!(tooltip(cx), None, "nothing shows before the hover delay");
    advance(
        cx,
        layers::HOVER_DELAY - std::time::Duration::from_millis(1),
    );
    assert_eq!(
        tooltip(cx),
        None,
        "still nothing just before the delay ends"
    );
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx), Some("Save the title".into()));

    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    check(cx);
    assert_eq!(tooltip(cx), None, "leaving to empty space hid it");
}

/// Moving straight from one control onto its neighbor shows the neighbor's help at once, whichever hover event GPUI
/// delivers first: a user scanning a row of controls does not wait the hover delay at each (the old app's rule). If the
/// first control's leave could take down any tooltip, the neighbor's help would vanish the moment it appeared, and it
/// would also do so for two controls that merely share an id in different parts of the window.
#[gpui::test]
fn one_controls_leave_never_hides_anothers_tooltip(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        div()
            .flex()
            .child(Button::new(
                "same",
                "Save",
                HoverHelp::tooltip("Save the title"),
                Role::Button,
            ))
            // A second "same" in another part of the window: an id is unique only among siblings, so it may repeat
            // under a different parent. (Under the same element path GPUI would treat the two as one element.)
            .child(
                div()
                    .id("panel")
                    .debug_selector(|| "panel".into())
                    .child(Button::new(
                        "same",
                        "Rename",
                        HoverHelp::tooltip("Give it a new title"),
                        Role::Button,
                    )),
            )
            .into_any_element()
    });
    // Two controls share the selector "same", so locate them through the panel that holds the second: the first sits
    // immediately left of it, with no gap.
    let panel = bounds(cx, "panel");
    let first = point(panel.left() - px(4.), panel.center().y);
    let second = point(panel.left() + px(4.), panel.center().y);
    cx.simulate_mouse_move(first, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx),
        Some("Save the title".into()),
        "the premise: the first control's help shows"
    );

    cx.simulate_mouse_move(second, None, Modifiers::none());
    check(cx);
    assert_eq!(
        tooltip(cx),
        Some("Give it a new title".into()),
        "the neighbor's help shows at once, and the first control's leave did not take it down"
    );
}

/// A tooltip closes when its control is gone, though no hover-leave ever fires for a control that simply stops being
/// drawn (a dialog closing on Enter, a deleted row). SPEC_impl.md: an overlay closes when its subject is gone.
#[gpui::test]
fn a_tooltip_closes_when_its_control_is_no_longer_drawn(cx: &mut TestAppContext) {
    let shown = Rc::new(Cell::new(true));
    let drawn = shown.clone();
    let cx = open(cx, move |_, _| {
        div()
            .size(px(200.))
            .when(drawn.get(), |area| {
                area.child(Button::new(
                    "gone",
                    "Delete",
                    HoverHelp::tooltip("Delete it"),
                    Role::Button,
                ))
            })
            .into_any_element()
    });
    let at = center(cx, "gone");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx),
        Some("Delete it".into()),
        "the premise: its help shows"
    );

    // The harness draws from a plain cell that notifies nothing, so redraw it the way a view's own change would.
    shown.set(false);
    redraw(cx);
    assert!(
        cx.debug_bounds("gone").is_none(),
        "the premise: the control is no longer drawn"
    );
    assert_eq!(tooltip(cx), None, "its tooltip went with it");
}

// ---- Nested hover help ----------------------------------------------------------------------------------------------

/// The row's help in the nested hover help tests.
const ROW_HELP: &str = "Open this session";
/// The row's help once its session has changed (a session row's help follows its status).
const ROW_HELP_LATER: &str = "Stopped; open this session";
/// The help of the mark inside that row.
const MARK_HELP: &str = "Last activity 12:00";

/// A session row's shape, for the nested hover help tests: a row control with help of its own, holding a mark with
/// its own after a title, as the session list's age and live dot sit inside their row. The row is drawn while `row`
/// holds and the mark while `mark` does, and the row's help is whatever `row_help` holds when it is drawn. The row
/// stands 40 pixels in from the window's corner, so (1, 1) is over neither, and its 12-pixel left padding is over the
/// row alone.
fn nested_help(
    row: Rc<Cell<bool>>,
    mark: Rc<Cell<bool>>,
    row_help: Rc<Cell<&'static str>>,
) -> impl Fn(&mut Window, &mut gpui::App) -> gpui::AnyElement + 'static {
    move |_, _| {
        let line = div()
            .flex()
            .gap(px(8.))
            .child("Title")
            .when(mark.get(), |line| {
                line.child(crate::components::StatusMark::new(
                    "age",
                    "Age",
                    HoverHelp::tooltip(MARK_HELP),
                    Role::Status,
                    "2m",
                ))
            });
        div()
            .pt(px(40.))
            .pl(px(40.))
            .w(px(340.))
            .when(row.get(), |area| {
                area.child(
                    crate::components::ListRow::new(
                        "row",
                        "Session",
                        HoverHelp::tooltip(row_help.get()),
                        Role::Button,
                    )
                    .line(line),
                )
            })
            .into_any_element()
    }
}

/// The row's help as a cell [`nested_help`] reads, starting at [`ROW_HELP`].
fn row_help() -> Rc<Cell<&'static str>> {
    Rc::new(Cell::new(ROW_HELP))
}

/// Open [`nested_help`] with the row and its mark drawn, the row's help read from `help`, and the pointer resting over
/// neither.
fn open_nested<'a>(
    cx: &'a mut TestAppContext,
    help: Rc<Cell<&'static str>>,
) -> &'a mut VisualTestContext {
    let cx = open(
        cx,
        nested_help(Rc::new(Cell::new(true)), Rc::new(Cell::new(true)), help),
    );
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    check(cx);
    cx
}

/// The mark's center, after checking the fixture's premise that the mark lies inside its row, without which the tests
/// would no longer be about nesting.
fn on_mark_in_row(cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    let on_mark = center(cx, "age");
    assert!(
        bounds(cx, "row").contains(&on_mark),
        "the premise: the mark lies inside the row"
    );
    on_mark
}

/// A point over the row alone: in its left padding, level with the mark.
fn on_row_only(cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    let (row, mark) = (bounds(cx, "row"), center(cx, "age"));
    assert!(
        mark.x > row.left() + px(12.),
        "the premise: the mark is clear of the row's padding"
    );
    point(row.left() + px(4.), mark.y)
}

/// A pointer that lands on a mark inside a row in one move (a fast mouse, the bridge's hover) makes the row and the
/// mark hovered on the same frame, and GPUI tells the row last. The mark's help is what the user pointed at: with the
/// last news winning, the row's help covered every mark's (the session row's live dot, age and stale cause).
/// Specified: from outside both, one move onto the mark shows the mark's help after the hover delay, and one move from
/// the mark back out of both takes it down.
#[gpui::test]
fn a_pointer_landing_on_a_mark_inside_a_row_shows_the_marks_help(cx: &mut TestAppContext) {
    let cx = open_nested(cx, row_help());
    assert_eq!(tooltip(cx), None, "the premise: no help shows");

    let on_mark = on_mark_in_row(cx);
    cx.simulate_mouse_move(on_mark, None, Modifiers::none());
    check(cx);
    assert_eq!(tooltip(cx), None, "nothing shows before the hover delay");
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some(MARK_HELP),
        "the mark's help, not the row's"
    );

    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    check(cx);
    assert_eq!(tooltip(cx), None, "leaving both hid it");
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx), None, "and nothing came back");
}

/// Moving between a mark and the row around it shows the help of whichever is under the pointer. The way back is the
/// case GPUI does not announce: the row was hovered all along, so only the mark hears that the pointer left it, and the
/// row's help never came back. The row's help on the way back is what it says then, not what it said when the pointer
/// first came onto it: a session row's help follows its status, which can change while the pointer rests on a mark.
/// Specified: resting on the row alone shows the row's help; moving onto the mark shows the mark's at once (a tooltip
/// is already showing); moving back onto the row alone shows the row's at once; and after the row's help changes
/// while the pointer is on the mark, moving back onto the row shows the new help.
#[gpui::test]
fn moving_between_a_mark_and_its_row_shows_the_help_under_the_pointer(cx: &mut TestAppContext) {
    let help = row_help();
    let cx = open_nested(cx, help.clone());
    let on_row = on_row_only(cx);
    cx.simulate_mouse_move(on_row, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some(ROW_HELP),
        "the premise: the row's help shows"
    );

    let on_mark = on_mark_in_row(cx);
    cx.simulate_mouse_move(on_mark, None, Modifiers::none());
    check(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some(MARK_HELP),
        "row to mark: the mark's help, at once"
    );

    cx.simulate_mouse_move(on_row, None, Modifiers::none());
    check(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some(ROW_HELP),
        "mark to row: the row's help, at once"
    );

    cx.simulate_mouse_move(on_mark, None, Modifiers::none());
    check(cx);
    help.set(ROW_HELP_LATER);
    redraw(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some(MARK_HELP),
        "the premise: the mark's help shows while the row's changes"
    );
    cx.simulate_mouse_move(on_row, None, Modifiers::none());
    check(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some(ROW_HELP_LATER),
        "mark to row: the row's help as it is now"
    );
}

/// A pointer move, a press and a release can reach the app in one update, before the move's hover news has settled:
/// the test bridge sends a click that way. The press must still put away the help of the control it lands on, the
/// innermost one, not the one around it; an in-app drive found a radio option's help showing right after a bridge
/// click on it. Specified: a move from outside onto a mark inside a row, a press and a release sent in one update leave
/// no help showing after the hover delay.
#[gpui::test]
fn a_press_in_the_same_update_as_the_move_onto_a_mark_puts_the_marks_help_away(
    cx: &mut TestAppContext,
) {
    let cx = open_nested(cx, row_help());
    let on_mark = on_mark_in_row(cx);
    cx.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                position: on_mark,
                pressed_button: None,
                modifiers: Modifiers::none(),
            }),
            cx,
        );
        window.dispatch_event(
            gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                button: MouseButton::Left,
                position: on_mark,
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                button: MouseButton::Left,
                position: on_mark,
                modifiers: Modifiers::none(),
                click_count: 1,
            }),
            cx,
        );
    });
    check(cx);
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx), None, "the press put the mark's help away");
}

/// A frame that brings a row and its mark under a still pointer (a scroll, a row appearing) tells GPUI's hover
/// listeners in drawing order, row first: the reverse of a pointer move's order, so a rule built for one alone gets the
/// other wrong. Specified: with the pointer resting where the mark will be, drawing the row shows the mark's help after
/// the hover delay.
#[gpui::test]
fn a_mark_drawn_under_a_still_pointer_shows_its_help_over_its_rows(cx: &mut TestAppContext) {
    let row = Rc::new(Cell::new(true));
    let cx = open(
        cx,
        nested_help(row.clone(), Rc::new(Cell::new(true)), row_help()),
    );
    let on_mark = center(cx, "age");
    row.set(false);
    redraw(cx);
    cx.simulate_mouse_move(on_mark, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx),
        None,
        "the premise: nothing is under the pointer"
    );

    row.set(true);
    redraw(cx);
    assert!(
        bounds(cx, "age").contains(&on_mark),
        "the premise: the mark is drawn under the pointer"
    );
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx).as_deref(), Some(MARK_HELP));
}

/// A mark that stops being drawn while the pointer rests on it (a session's live dot going when it stops) takes its
/// help with it, and no hover event says the pointer is now over the row alone. The same frame changes the row's help
/// (the row's words follow the session's status), and with the pointer still no move brings the new words, so the
/// row's help that comes back must be the one it was drawn with. Specified: the mark's help goes when the mark does,
/// and the row's new help shows after the hover delay.
#[gpui::test]
fn a_mark_no_longer_drawn_under_the_pointer_gives_its_row_its_help_back(cx: &mut TestAppContext) {
    let mark = Rc::new(Cell::new(true));
    let help = row_help();
    let cx = open(
        cx,
        nested_help(Rc::new(Cell::new(true)), mark.clone(), help.clone()),
    );
    let on_mark = center(cx, "age");
    cx.simulate_mouse_move(on_mark, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some(MARK_HELP),
        "the premise: the mark's help shows"
    );

    mark.set(false);
    help.set(ROW_HELP_LATER);
    redraw(cx);
    assert!(
        cx.debug_bounds("age").is_none() && bounds(cx, "row").contains(&on_mark),
        "the premise: the mark is gone and the pointer is over the row"
    );
    assert_eq!(tooltip(cx), None, "the mark's help went with it");
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx).as_deref(), Some(ROW_HELP_LATER));
}

/// Two buttons side by side with help of their own, for the tests of a pointer moving inside one control.
fn two_buttons() -> impl Fn(&mut Window, &mut gpui::App) -> gpui::AnyElement + 'static {
    |_, _| {
        div()
            .flex()
            .gap(px(8.))
            .child(Button::new(
                "save",
                "Save",
                HoverHelp::tooltip("Save the title"),
                Role::Button,
            ))
            .child(Button::new(
                "rename",
                "Rename",
                HoverHelp::tooltip("Give it a new title"),
                Role::Button,
            ))
            .into_any_element()
    }
}

/// Points inside the control drawn under `selector`, a few pixels either side of its center, for a pointer moving
/// within it.
fn inside(cx: &mut VisualTestContext, selector: &'static str) -> [gpui::Point<gpui::Pixels>; 3] {
    let at = bounds(cx, selector);
    let center = at.center();
    let points = [
        center,
        point(center.x + px(3.), center.y),
        point(center.x - px(3.), center.y),
    ];
    assert!(
        points.iter().all(|point| at.contains(point)),
        "the premise: every point is inside {selector}"
    );
    points
}

/// Every pointer move over a control with help reaches the layer manager now, and what keeps a move inside the same
/// control from acting is that its help is still the one the pointer calls for. Without that, each move over a showing
/// tooltip would show it again and redraw the window, the cost the cached list's tests guard against at another level.
/// Specified: moving inside a control while its help shows keeps that help and draws no frame.
#[gpui::test]
fn moving_inside_a_control_whose_help_shows_redraws_nothing(cx: &mut TestAppContext) {
    let cx = open(cx, two_buttons());
    let [first, _, third] = inside(cx, "save");
    cx.simulate_mouse_move(first, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some("Save the title"),
        "the premise: its help shows"
    );

    cx.update(|_, cx| crate::render_count::reset(cx));
    cx.simulate_mouse_move(third, None, Modifiers::none());
    check(cx);
    assert_eq!(tooltip(cx).as_deref(), Some("Save the title"));
    assert_eq!(
        cx.update(|_, cx| crate::render_count::count("scene", cx)),
        0,
        "a move inside the control drew no frame"
    );
}

/// A press puts a control's help away, as the old app's `tooltip.js` did on any `pointerdown`, and it stays away while
/// the pointer rests there: a Settings section button's tooltip otherwise came back over the section it had just
/// opened. Specified: with a control's help showing, a click takes it down; a move inside the control and the hover
/// delay show nothing; moving onto the other control shows that one's help after the delay.
#[gpui::test]
fn a_press_puts_a_controls_help_away_while_the_pointer_rests_on_it(cx: &mut TestAppContext) {
    let cx = open(cx, two_buttons());
    let [first, second, _] = inside(cx, "save");
    cx.simulate_mouse_move(first, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some("Save the title"),
        "the premise: its help shows"
    );

    cx.simulate_click(first, Modifiers::none());
    check(cx);
    assert_eq!(tooltip(cx), None, "the press put it away");
    cx.simulate_mouse_move(second, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx),
        None,
        "resting on the control brought nothing back"
    );

    let on_rename = center(cx, "rename");
    cx.simulate_mouse_move(on_rename, None, Modifiers::none());
    check(cx);
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx).as_deref(), Some("Give it a new title"));
}

/// The value the host-sentence help tests below name: a direction override, markup-like text and a newline, which
/// would reverse, pose as formatting or forge a second line if they reached the tooltip raw.
const SPOOFED_HOST: &str = "a\u{202E}<b>\n";

/// [`SPOOFED_HOST`] as host text shows it: hidden characters escaped, the markup-like text left as the letters it is.
const SPOOFED_HOST_SHOWN: &str = "a<U+202E><b><U+000A>";

/// Hover help may name a host value (the agent mark's, the permission shield's), and SPEC.md wants every host value the
/// GUI shows outside a terminal escaped, in accessible names too; a tooltip is the one surface the layer manager draws
/// on its own, so without a host form there a view would have to format a raw value into help text. Specified: hovering
/// a control whose help is a host sentence shows that help after the hover delay, drawn in the tooltip as one `Label`
/// node under the sentence's id, named with the host value escaped, and painting the value as a host sentence does,
/// whole and escaped inside its isolate, with no presentation-unsafe character anywhere else in the frame (an isolate
/// is one, but it is the sentence's own and draws as nothing); the tooltip's text, and `HoverHelp::description` (what
/// `with_hover_help` gives the control as its accessible description), are the sentence's accessible text. The
/// description the drawn control carries is not checked: GPUI's driver record has no description field, and the test
/// platform cannot switch on accessibility to read the tree.
#[gpui::test]
fn help_naming_a_host_value_draws_it_escaped_in_the_tooltip(cx: &mut TestAppContext) {
    let help = || {
        HoverHelp::sentence(
            HostSentence::new("agent-help")
                .text("Agent on ")
                .host(&HostText::from_host(SPOOFED_HOST)),
        )
    };
    let expected = format!("Agent on {SPOOFED_HOST_SHOWN}");
    assert_eq!(
        help().description().as_ref(),
        expected,
        "the description the control is given is the sentence's accessible text"
    );
    assert_eq!(
        HoverHelp::tooltip("Save it").description().as_ref(),
        "Save it",
        "and the app's own help describes the control as it always did"
    );

    let cx = open(cx, move |_, _| {
        Button::new("agent", "Agent", help(), Role::Button).into_any_element()
    });
    let at = center(cx, "agent");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx).as_deref(), Some(expected.as_str()));

    let nodes = frame(cx);
    let Some(sentence) = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some("agent-help"))
    else {
        let ids: Vec<_> = nodes.iter().map(|node| &node.element_id).collect();
        panic!("the tooltip drew no host sentence; recorded ids: {ids:?}");
    };
    assert_eq!(nodes[sentence].role, Some(Role::Label));
    assert_eq!(nodes[sentence].label.as_deref(), Some(expected.as_str()));
    // Every node recorded below the sentence, at any depth: its text element.
    let inside = |mut index: usize| loop {
        match nodes[index].parent {
            Some(parent) if parent == sentence => return true,
            Some(parent) => index = parent,
            None => return false,
        }
    };
    let painted: Vec<String> = (0..nodes.len())
        .filter(|&index| inside(index))
        .flat_map(|index| nodes[index].texts.iter().map(|text| text.text.to_string()))
        .collect();
    assert!(
        painted
            .iter()
            .any(|text| paints_isolated(text, SPOOFED_HOST_SHOWN)),
        "the host value was painted whole and escaped, isolated: {painted:?}"
    );
    let unsafe_text: Vec<String> = nodes
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| painted_words(&text.text)))
        .filter(|text| {
            text.chars()
                .any(farhelm_proto::text::is_presentation_unsafe)
        })
        .collect();
    assert!(
        unsafe_text.is_empty(),
        "no raw hidden character was painted: {unsafe_text:?}"
    );
}

/// Paragraph help is the agent mark's and the permission shield's form (SPEC.md, "Session list"), whose whole text is
/// also what a screen reader hears for those marks; a description that kept only the first paragraph would drop the
/// permission's meaning and the stored command, and a tooltip that merged the paragraphs into one node would read as
/// a run-on. How the paragraphs are laid out (wider, unclamped, a blank line apart) is pinned with the real text
/// system in `layers_tests.rs`. Specified: hovering a control whose help is `HoverHelp::paragraphs` shows, after the
/// hover delay, every paragraph in the tooltip as a `Label` node of its own under its sentence's id, named with its
/// host values escaped, in the order given; the tooltip's text and the control's description are every paragraph's
/// accessible text, a blank line between each.
#[gpui::test]
fn help_in_paragraphs_shows_every_paragraph_and_describes_the_control_with_all_of_them(
    cx: &mut TestAppContext,
) {
    let help = || {
        HoverHelp::paragraphs([
            HostSentence::new("agent-help-mode").text("Codex — Guarded · Codex sandboxed auto"),
            HostSentence::new("agent-help-caveat").text("Your own allow rules can loosen it."),
            HostSentence::new("agent-help-command")
                .text("Launched with: ")
                .mono(&HostText::from_host(SPOOFED_HOST)),
        ])
    };
    let names = [
        "Codex — Guarded · Codex sandboxed auto".to_owned(),
        "Your own allow rules can loosen it.".to_owned(),
        format!("Launched with: {SPOOFED_HOST_SHOWN}"),
    ];
    let whole = names.join("\n\n");
    assert_eq!(
        help().description().as_ref(),
        whole,
        "the description the control is given is every paragraph, a blank line apart"
    );

    let cx = open(cx, move |_, _| {
        Button::new("agent", "Agent", help(), Role::Button).into_any_element()
    });
    let at = center(cx, "agent");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx).as_deref(), Some(whole.as_str()));

    let nodes = frame(cx);
    let drawn: Vec<usize> = ["agent-help-mode", "agent-help-caveat", "agent-help-command"]
        .iter()
        .map(|id| {
            match nodes
                .iter()
                .position(|node| node.element_id.as_deref() == Some(*id))
            {
                Some(index) => index,
                None => panic!("the tooltip drew no paragraph {id}"),
            }
        })
        .collect();
    for (index, name) in drawn.iter().zip(&names) {
        assert_eq!(nodes[*index].role, Some(Role::Label));
        assert_eq!(nodes[*index].label.as_deref(), Some(name.as_str()));
    }
    assert!(
        drawn.windows(2).all(|pair| pair[0] < pair[1]),
        "the paragraphs are drawn in the order given: {drawn:?}"
    );
}
