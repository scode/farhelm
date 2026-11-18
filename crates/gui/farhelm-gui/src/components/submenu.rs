//! A focused item can open another menu without losing its parent menu.
//!
//! The layer manager owns the parent and child lifetime and Escape handling.
//! The trigger owns the anchor and the focus handle Left and Escape return to.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, Role, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};

use super::{MenuItem, home_end_keys};
use crate::focus::{self, Mark};
use crate::layers::{self, Anchor, LayerId, Popover};

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
            Rc::new(Cell::new(None))
        })
        .read(cx)
        .clone();
    let expanded = open
        .get()
        .is_some_and(|layer| layers::is_open(layer, window, cx));
    let returns_to = trigger.clone();
    let drawn = anchor.track();
    div()
        .relative()
        .child(
            item.submenu_trigger(expanded)
                .focus(trigger)
                .on_click(move |_, window, cx| {
                    if open
                        .get()
                        .is_some_and(|layer| layers::is_open(layer, window, cx))
                    {
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
                    // macOS pointer activation can leave focus on the preceding
                    // item. Set the trigger before opening so the layer manager
                    // sees the parent menu and returns focus to this item.
                    focus::highlight(&returns_to, window, cx);
                    let mark = Mark::take(cx);
                    let panel = cx.new(|_| Submenu {
                        items,
                        start: start.clone(),
                        end,
                        open: open.clone(),
                        scroll: ScrollHandle::new(),
                    });
                    open.set(layers::open_popover(
                        Popover::menu(panel, cx.focus_handle(), anchor.clone(), start)
                            .returns_to(focus::ReturnTo::Control(returns_to.clone())),
                        &mark,
                        window,
                        cx,
                    ));
                    window.refresh();
                }),
        )
        .child(drawn)
        .into_any_element()
}

/// Focus handles belong to this opening. As with ordinary menus, the
/// focused entry is the sole highlight and the layer owns Up and Down.
struct Submenu {
    items: Vec<(MenuItem, FocusHandle)>,
    start: FocusHandle,
    end: FocusHandle,
    open: Rc<Cell<Option<LayerId>>>,
    /// Keeps the focused choice visible when a data-driven list is taller
    /// than the window.
    scroll: ScrollHandle,
}

impl Render for Submenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.open.clone();
        let panel = layers::panel(cx)
            .id("submenu")
            .role(Role::Menu)
            .min_w(px(180.))
            .max_h(crate::notices_bell::window_cap(window))
            .overflow_y_scroll()
            .on_key_down(home_end_keys(self.start.clone(), self.end.clone()))
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "left" && !event.keystroke.modifiers.modified() {
                    cx.stop_propagation();
                    if let Some(layer) = open.get() {
                        layers::close(layer, window, cx);
                    }
                }
            })
            .children(self.items.iter().map(|(item, _)| item.clone()));
        focus::scroll_region(panel, &self.scroll)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use gpui::{Modifiers, MouseButton, TestAppContext};

    use super::*;
    use crate::assets::Icon;
    use crate::components::HoverHelp;
    use crate::test_support::{
        anchor_element, bounds, center, check, focused_id, handle, keys, open,
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

    /// Right and Enter move focus into the child. Left and Escape return
    /// to the trigger, keeping the parent open. A click beside the parent's
    /// bounds still reaches the child. Closing the parent takes its child
    /// with it even while that child holds focus.
    #[gpui::test]
    fn a_submenu_returns_focus_and_follows_its_parent(cx: &mut TestAppContext) {
        let captured = Rc::new(RefCell::new(None));
        let out = captured.clone();
        let cx = open(cx, move |window, cx| {
            anchor_element("opener", &out, window, cx).into_any_element()
        });
        let anchor = captured.borrow().clone().expect("the page anchor drew");
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
                    Popover::menu(panel, cx.focus_handle(), anchor, trigger),
                    &Mark::take(cx),
                    window,
                    cx,
                )
            })
            .expect("the parent menu opened");
        cx.run_until_parked();
        check(cx);
        assert_eq!(focused_id(cx).as_deref(), Some("trigger"));
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
}
