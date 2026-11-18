//! A submenu opens beside its trigger, with the parent keeping focus until the user enters the child.
//!
//! The layer manager owns the parent and child lifetime and Escape handling.
//! The trigger owns the anchor and the focus handle Left and Escape return to.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, Role, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};

use super::{Handler, MenuItem, home_end_keys};
use crate::focus::{self, Mark};
use crate::layers::{self, Anchor, LayerId, Popover};

/// Hover previews the choices, a click opens a closed menu, and keys can enter an already open menu.
#[derive(Clone, Copy)]
enum Activation {
    Hover,
    Click,
    Key,
}

/// Anchor a child menu to `item`, with a fresh list of entries on each
/// activation. The owner can use the same trigger for fixed commands and
/// for choices that follow a listing. A disabled item opens nothing.
pub fn submenu(
    id: SharedString,
    item: MenuItem,
    trigger: &FocusHandle,
    entries: impl Fn(&App) -> Vec<MenuItem> + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let anchor =
        Anchor::for_element(SharedString::from(format!("{id}-anchor")), window, cx).submenu();
    let open = window
        .use_keyed_state(SharedString::from(format!("{id}-open")), cx, |_, _| {
            Rc::new(RefCell::new(None))
        })
        .read(cx)
        .clone();
    let expanded = open
        .borrow()
        .as_ref()
        .is_some_and(|(layer, _)| layers::is_open(*layer, window, cx));
    let preview = open.clone();
    let returns_to = trigger.clone();
    let drawn = anchor.track();
    let activate = Rc::new(move |activation, window: &mut Window, cx: &mut App| {
        if let Some((layer, start)) = open.borrow().as_ref()
            && layers::is_open(*layer, window, cx)
        {
            if matches!(activation, Activation::Key) && layers::enter_submenu(*layer, window, cx) {
                focus::enter_layer(start, window, cx);
            }
            return;
        }
        let items: Vec<_> = entries(cx)
            .into_iter()
            .map(|item| {
                let focus = cx.focus_handle().tab_stop(true);
                (item.focus(&focus), focus)
            })
            .collect();
        let (Some((_, first)), Some((_, last))) = (items.first(), items.last()) else {
            return;
        };
        let start = first.clone();
        let end = last.clone();
        // A click can leave focus elsewhere on macOS. The layer manager needs focus in the trigger's menu
        // before the child opens, because it uses that focus to record which layer owns the child.
        focus::highlight(&returns_to, window, cx);
        let mark = Mark::take(cx);
        let panel = cx.new(|_| Submenu {
            items,
            start: start.clone(),
            end,
            open: open.clone(),
            scroll: ScrollHandle::new(),
        });
        let opened = layers::open_submenu(
            Popover::menu(panel, cx.focus_handle(), anchor.clone(), start.clone())
                .returns_to(focus::ReturnTo::Control(returns_to.clone())),
            !matches!(activation, Activation::Hover),
            &mark,
            window,
            cx,
        );
        *open.borrow_mut() = opened.map(|layer| (layer, start));
        window.refresh();
    });
    let on = |activation| -> Handler {
        let activate = activate.clone();
        Rc::new(move |_, window, cx| activate(activation, window, cx))
    };
    let click = on(Activation::Click);
    div()
        .relative()
        .on_key_down(move |event, window, cx| {
            // A preview's keys still come through the trigger's element, outside the child panel.
            if event.keystroke.key == "left"
                && !event.keystroke.modifiers.modified()
                && let Some((layer, _)) = preview.borrow().as_ref()
                && layers::is_open(*layer, window, cx)
            {
                cx.stop_propagation();
                layers::dismiss(*layer, window, cx);
            }
        })
        .child(
            item.submenu_trigger(expanded, on(Activation::Key), on(Activation::Hover))
                .focus(trigger)
                .on_click(move |event, window, cx| click(event, window, cx)),
        )
        .child(drawn)
        .into_any_element()
}

/// Focus handles belong to this opening. When the child holds focus, the layer owns Up and Down.
struct Submenu {
    items: Vec<(MenuItem, FocusHandle)>,
    start: FocusHandle,
    end: FocusHandle,
    open: Rc<RefCell<Option<(LayerId, FocusHandle)>>>,
    /// Keeps the focused choice visible when a data-driven list is taller
    /// than the window.
    scroll: ScrollHandle,
}

impl Render for Submenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.open.clone();
        let layer = self.open.borrow().as_ref().map(|(layer, _)| *layer);
        let start = self.start.clone();
        let enter_by_pointer = move |window: &mut Window, cx: &mut App| {
            if let Some(layer) = layer
                && layers::enter_submenu(layer, window, cx)
            {
                focus::highlight(&start, window, cx);
            }
        };
        let enter_by_press = enter_by_pointer.clone();
        let panel = layers::panel(cx)
            .id("submenu")
            .debug_selector(|| "submenu".to_owned())
            .role(Role::Menu)
            .min_w(px(180.))
            .max_h(crate::notices_bell::window_cap(window))
            .overflow_y_scroll()
            .on_mouse_move(move |event, window, cx| {
                // Enter through the panel's padding as well as through a choice. An item that
                // already handled this move owns the highlight.
                if event.pressed_button.is_none() && !cx.has_active_drag() {
                    enter_by_pointer(window, cx);
                }
            })
            // Assistive activation can send a press without a preceding pointer move.
            .capture_any_mouse_down(move |_, window, cx| enter_by_press(window, cx))
            .on_key_down(home_end_keys(self.start.clone(), self.end.clone()))
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "left" && !event.keystroke.modifiers.modified() {
                    cx.stop_propagation();
                    if let Some((layer, _)) = open.borrow().as_ref() {
                        layers::close(*layer, window, cx);
                    }
                }
            })
            .children(self.items.iter().map(|(item, _)| item.clone()));
        focus::scroll_region(panel, &self.scroll)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};

    use super::*;
    use crate::assets::Icon;
    use crate::components::HoverHelp;
    use crate::focus_tests::held;
    use crate::test_support::{
        anchor_element, bounds, center, check, click, focused_id, handle, is_open, keys, open,
        question, settle,
    };

    /// The test opens an actual parent popover from the page. The child
    /// can then exercise the layer manager's focus and ancestry rules.
    struct ParentMenu {
        trigger: FocusHandle,
        choices: Rc<Cell<usize>>,
    }

    impl Render for ParentMenu {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let choices = self.choices.clone();
            layers::panel(cx).w(px(160.)).child(submenu(
                "trigger".into(),
                item("trigger"),
                &self.trigger,
                move |_| {
                    let choices = choices.clone();
                    vec![
                        item("first"),
                        item("second").on_click(move |_, _, _| choices.set(choices.get() + 1)),
                    ]
                },
                window,
                cx,
            ))
        }
    }

    fn item(id: &'static str) -> MenuItem {
        MenuItem::new(id, Icon::Pin, id, HoverHelp::tooltip(id), Role::MenuItem)
    }

    /// Use a drawn page anchor so the parent's outside-press and ancestry rules are active in each test.
    fn open_parent(
        cx: &mut TestAppContext,
    ) -> (&mut VisualTestContext, LayerId, Rc<Cell<usize>>, Anchor) {
        let captured = Rc::new(RefCell::new(None));
        let out = captured.clone();
        let cx = open(cx, move |window, cx| {
            anchor_element("opener", &out, window, cx).into_any_element()
        });
        let anchor = captured.borrow().clone().expect("the page anchor drew");
        let parent_anchor = anchor.clone();
        let trigger = handle(cx);
        let choices = Rc::new(Cell::new(0));
        let counted = choices.clone();
        let parent = cx
            .update(move |window, cx| {
                let panel = cx.new(|_| ParentMenu {
                    trigger: trigger.clone(),
                    choices: counted,
                });
                layers::open_popover(
                    Popover::menu(panel, cx.focus_handle(), parent_anchor, trigger),
                    &Mark::take(cx),
                    window,
                    cx,
                )
            })
            .expect("the parent menu opened");
        settle(cx);
        assert_eq!(focused_id(cx).as_deref(), Some("trigger"));
        (cx, parent, choices, anchor)
    }

    /// Right and Enter move focus into the child. Left and Escape return to the trigger. A click beside the parent's
    /// bounds still reaches the child, and closing the parent takes the child with it while that child holds focus.
    #[gpui::test]
    fn a_submenu_returns_focus_and_follows_its_parent(cx: &mut TestAppContext) {
        let (cx, parent, choices, _) = open_parent(cx);
        keys(cx, "right");
        assert_eq!(focused_id(cx).as_deref(), Some("first"));
        assert!(bounds(cx, "first").left() > bounds(cx, "trigger").right());
        keys(cx, "down");
        assert_eq!(focused_id(cx).as_deref(), Some("second"));
        keys(cx, "left");
        assert_eq!(focused_id(cx).as_deref(), Some("trigger"));
        keys(cx, "enter");
        assert_eq!(focused_id(cx).as_deref(), Some("first"));
        keys(cx, "escape");
        assert_eq!(focused_id(cx).as_deref(), Some("trigger"));
        keys(cx, "right");
        assert_eq!(focused_id(cx).as_deref(), Some("first"));
        let second = center(cx, "second");
        cx.simulate_mouse_down(second, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        check(cx);
        assert!(cx.update(|window, cx| layers::is_open(parent, window, cx)));
        cx.simulate_mouse_up(second, MouseButton::Left, Modifiers::none());
        check(cx);
        assert_eq!(choices.get(), 1, "the parent did not eat the child's click");
        cx.update(|window, cx| layers::close(parent, window, cx));
        cx.run_until_parked();
        check(cx);
        assert!(cx.debug_bounds("trigger").is_none());
        assert!(cx.debug_bounds("first").is_none());
    }

    /// Hover opens the child with keys still on the trigger. A click on the expanded trigger leaves that state alone.
    /// Escape closes just the child, and clicking the still-hovered but now closed trigger enters it.
    #[gpui::test]
    fn hover_opens_a_submenu_before_its_first_choice_takes_focus(cx: &mut TestAppContext) {
        let (cx, parent, choices, _) = open_parent(cx);
        assert!(cx.debug_bounds("first").is_none());

        let at_trigger = center(cx, "trigger");
        cx.simulate_mouse_move(at_trigger, None, Modifiers::none());
        settle(cx);
        assert!(bounds(cx, "first").left() > bounds(cx, "trigger").right());
        assert_eq!(focused_id(cx).as_deref(), Some("trigger"));
        click(cx, "trigger");
        assert!(cx.debug_bounds("first").is_some());
        assert_eq!(focused_id(cx).as_deref(), Some("trigger"));
        keys(cx, "left");
        assert!(cx.debug_bounds("first").is_none());
        assert_eq!(focused_id(cx).as_deref(), Some("trigger"));

        cx.simulate_mouse_move(at_trigger, None, Modifiers::none());
        settle(cx);
        assert!(cx.debug_bounds("first").is_some());
        keys(cx, "escape");
        held(cx, "escape");
        assert!(cx.debug_bounds("first").is_none());
        assert_eq!(focused_id(cx).as_deref(), Some("trigger"));

        cx.simulate_mouse_move(at_trigger, None, Modifiers::none());
        settle(cx);
        assert!(cx.debug_bounds("first").is_some());
        keys(cx, "right");
        assert_eq!(focused_id(cx).as_deref(), Some("first"));
        click(cx, "trigger");
        assert_eq!(focused_id(cx).as_deref(), Some("first"));
        keys(cx, "escape");
        assert!(cx.debug_bounds("first").is_none());
        assert_eq!(focused_id(cx).as_deref(), Some("trigger"));
        click(cx, "trigger");
        assert_eq!(focused_id(cx).as_deref(), Some("first"));
        assert_eq!(choices.get(), 0);
        keys(cx, "escape");
        keys(cx, "escape");
        assert!(!is_open(cx, parent), "a fresh Escape can close the parent");
    }

    /// An anchored question leaves parts of the menu hit-testable. Hover there must leave the question in charge.
    #[gpui::test]
    fn a_menu_hover_does_not_replace_an_anchored_question(cx: &mut TestAppContext) {
        let (cx, parent, _, anchor) = open_parent(cx);
        let prompt = cx
            .update(move |window, cx| {
                crate::prompt::ask_anchored(
                    question("session"),
                    anchor.beside(),
                    None,
                    &Mark::take(cx),
                    |_, _, _| panic!("hover cannot answer the question"),
                    window,
                    cx,
                )
            })
            .expect("the question opened");
        settle(cx);
        let trigger = center(cx, "trigger");
        assert!(!bounds(cx, "prompt").contains(&trigger));
        assert!(is_open(cx, parent));
        let before = focused_id(cx);
        assert!(
            before
                .as_deref()
                .is_some_and(|id| id.starts_with("prompt-"))
        );
        cx.simulate_mouse_move(trigger, None, Modifiers::none());
        settle(cx);
        assert!(is_open(cx, prompt));
        assert_eq!(focused_id(cx), before);
        assert!(cx.debug_bounds("first").is_none());
    }
}
