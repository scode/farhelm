//! Tests of the choice controls' Enter rule, `on_enter_choice` in `components/mod.rs`.
//!
//! The rule itself (apply, then the main action, once; a held Enter taken and inert; a modified Enter passed on) is
//! pinned once, here, on the helper as an area's own tab uses it. Per control only its wiring is pinned, in that
//! control's tests: `radio_group.rs`, `checkbox.rs` and `select.rs`.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, TestAppContext,
    VisualTestContext, div, px,
};

use crate::focus;
use crate::test_support::{click, keys, open};

use super::stroke;

/// Why this matters: the launcher draws its own Agent and Command tabs, and Enter on one chose that tab and then
/// launched (the old launcher's `enter_choice`). Those tabs must follow the same rule as this layer's choices, through
/// [`crate::components::on_enter_choice`], not a copy of it that can drift. This is also the one test of the rule's
/// whole contract, the held and modified Enter included; the per-control tests pin only each control's wiring.
///
/// Specified, on a tab drawn with the helper and the shared click rule, with its own Enter listener after the helper,
/// inside an element listening for Enter: Enter pressed and released chooses the tab once and then runs the action
/// once, reaching neither listener; a click only chooses; a held Enter, pressed and released, does nothing and reaches
/// neither listener; Shift+Enter runs nothing and reaches the tab's own listener and then the element around it.
#[gpui::test]
fn an_areas_own_tab_takes_the_enter_rule_through_the_helper(cx: &mut TestAppContext) {
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::default();
    let focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (drawn, handle) = (log.clone(), focus.clone());
    let cx = open(cx, move |_, _| {
        let entry = |what: &'static str| {
            let log = drawn.clone();
            move || log.borrow_mut().push(what)
        };
        let (choose, launch, own, around, clicked) = (
            entry("choose"),
            entry("launch"),
            entry("own"),
            entry("around"),
            entry("choose"),
        );
        let tab = div()
            .id("agent-tab")
            .debug_selector(|| "agent-tab".to_string())
            .w(px(80.))
            .h(px(30.));
        let tab = focus::track(tab, &handle, focus::OnPress::Control);
        let tab =
            crate::components::on_enter_choice(tab, move |_, _| choose(), move |_, _| launch())
                .on_key_down(move |event: &gpui::KeyDownEvent, _, _| {
                    if event.keystroke.key == "enter" {
                        own();
                    }
                });
        let on_click: crate::components::Handler = Rc::new(move |_, _, _| clicked());
        div()
            .on_key_down(move |event: &gpui::KeyDownEvent, _, _| {
                if event.keystroke.key == "enter" {
                    around();
                }
            })
            .child(crate::components::activate_on_click(tab, Some(on_click)))
            .into_any_element()
    });
    let tab_focused = |cx: &mut VisualTestContext| cx.update(|window, _| focus.is_focused(window));
    keys(cx, "tab");
    assert!(tab_focused(cx), "the premise: the tab has focus");

    stroke(cx, "enter", false);
    assert_eq!(
        *log.borrow(),
        ["choose", "launch"],
        "Enter chose the tab once and then ran the action once, reaching neither listener"
    );
    log.borrow_mut().clear();
    click(cx, "agent-tab");
    assert_eq!(*log.borrow(), ["choose"], "a click only chose");
    log.borrow_mut().clear();
    assert!(
        tab_focused(cx),
        "the premise: the click left focus on the tab"
    );
    stroke(cx, "enter", true);
    assert!(log.borrow().is_empty(), "a held Enter did nothing");
    stroke(cx, "shift-enter", false);
    assert_eq!(
        *log.borrow(),
        ["own", "around"],
        "Shift+Enter ran nothing and went on to the tab's listener, then the element around it"
    );
}
