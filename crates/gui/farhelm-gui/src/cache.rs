//! The one way to cache a view: compute its inputs from shared state, and redraw only when they differ.
//!
//! GPUI's `.cached()` replays a view's last frame until that view notifies, and a missed notification leaves it showing
//! stale state with nothing to say so (SPEC_impl.md, "The store, and how views redraw"; the prototype shipped five such
//! bugs, all found by review). This wrapper removes the notification from the hands of feature code: it observes the
//! entity the view draws from, recomputes the view's inputs from it on every change, and notifies only when the inputs
//! differ. A view behind it cannot go stale by forgetting to notify, because it never notifies; and it does not redraw
//! when an unrelated part of the source changed, because its inputs did not.
//!
//! The inputs are a plain value the view's render reads; the view never reads the source while drawing. `compute` sees
//! only the source entity, not the app, so it cannot read state the wrapper does not observe: an unobserved read is
//! exactly the stale-view bug this wrapper exists to rule out. Caching follows the cookbook's rules: a cached view needs
//! an explicit size (it is laid out from its style alone), and cached views are kept flat (a cache miss re-renders the
//! whole subtree uncached, so nesting buys nothing).
//!
//! NOTE: One source entity per cached view. A view that draws from two sources (a session row that also shows its
//! host's name, say) is settled when the store lands: either the store offers one entity that joins them, or the
//! wrapper observes each source explicitly. It must never become a second, unobserved read inside `compute`.

use gpui::{
    AnyElement, App, Context, Entity, IntoElement, Render, StyleRefinement, Subscription, Window,
};

use crate::render_count;

/// Computes a view's inputs from the state it draws from, and from nothing else.
type Compute<S, I> = Box<dyn Fn(&S) -> I>;
/// Draws a view from its inputs.
type Draw<I> = Box<dyn Fn(&I, &mut Window, &mut App) -> AnyElement>;

/// A cached view: inputs `I` computed from the source entity `S`, drawn by a function of those inputs alone.
pub struct Cached<S: 'static, I: PartialEq + 'static> {
    /// The name the render counter files this view under.
    name: &'static str,
    source: Entity<S>,
    compute: Compute<S, I>,
    draw: Draw<I>,
    inputs: I,
    _observing: Subscription,
}

impl<S: 'static, I: PartialEq + 'static> Cached<S, I> {
    /// A cached view named `name` over `source`. `compute` picks what the view shows out of the source; `draw` draws
    /// it. Both run inside GPUI's update cycle, so they must not update other entities.
    pub fn new(
        name: &'static str,
        source: Entity<S>,
        compute: impl Fn(&S) -> I + 'static,
        draw: impl Fn(&I, &mut Window, &mut App) -> AnyElement + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        let inputs = compute(source.read(cx));
        let observing = cx.observe(&source, |this, _, cx| this.recompute(cx));
        Cached {
            name,
            source,
            compute: Box::new(compute),
            draw: Box::new(draw),
            inputs,
            _observing: observing,
        }
    }

    /// Recompute the inputs and redraw only if they changed.
    fn recompute(&mut self, cx: &mut Context<Self>) {
        let next = (self.compute)(self.source.read(cx));
        if next != self.inputs {
            self.inputs = next;
            cx.notify();
        }
    }
}

impl<S: 'static, I: PartialEq + 'static> Render for Cached<S, I> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record(self.name, cx);
        (self.draw)(&self.inputs, window, cx)
    }
}

/// `view` as an element its parent can place, replayed from cache until its inputs change. `size` must give the view
/// a definite size: a cached view is laid out from this style alone, never measured from its content.
pub fn element<S: 'static, I: PartialEq + 'static>(
    view: &Entity<Cached<S, I>>,
    size: StyleRefinement,
) -> impl IntoElement + use<S, I> {
    view.clone().cached(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext as _, Context, ParentElement, Styled, TestAppContext, div, px};

    use crate::components::{Button, HoverHelp, Role};
    use crate::{layers, test_support};

    /// A stand-in for a store: rows the list draws, changed only through a method that notifies.
    struct Rows(Vec<String>);
    impl Rows {
        fn rename(&mut self, index: usize, title: &str, cx: &mut Context<Self>) {
            self.0[index] = title.to_owned();
            cx.notify();
        }
    }

    /// A window root holding a list of `n` cached rows over one source, as the session list will be, plus one button
    /// with hover help and the layer manager's overlay, as the main view draws them.
    struct List {
        rows: Vec<Entity<Cached<Rows, String>>>,
        _layers: layers::Attachment,
    }
    impl Render for List {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            render_count::record("list", cx);
            let mut list = div().flex().flex_col();
            list = list.child(Button::new(
                "help",
                "Help",
                HoverHelp::tooltip("What this list is"),
                Role::Button,
            ));
            for row in &self.rows {
                let mut size = StyleRefinement::default();
                size.size.width = Some(px(300.).into());
                size.size.height = Some(px(2.).into());
                list = list.child(element(row, size));
            }
            list.children(layers::overlay(window, cx))
        }
    }

    /// Open a bootstrapped window showing `n` cached rows over `rows`.
    fn open<'a>(
        cx: &'a mut TestAppContext,
        rows: &Entity<Rows>,
        n: usize,
    ) -> &'a mut gpui::VisualTestContext {
        test_support::install(cx);
        let rows = rows.clone();
        let (_list, cx) = cx.add_window_view(move |window, cx| List {
            rows: (0..n)
                .map(|index| {
                    cx.new(|cx| {
                        Cached::new(
                            "row",
                            rows.clone(),
                            move |rows: &Rows| rows.0[index].clone(),
                            |title, _, _| div().child(title.clone()).into_any_element(),
                            cx,
                        )
                    })
                })
                .collect(),
            _layers: layers::attach(window, cx),
        });
        cx.run_until_parked();
        cx
    }

    /// The point of the wrapper (SPEC_impl.md, "The store, and how views redraw"): a change to one row's input redraws
    /// that row and nothing else, and a notification that changes no row's input redraws no row. Measured at 200 rows,
    /// the goal's early check of the wrapper at the session list's scale; the counts are the evidence (a debug build on
    /// the fake platform says nothing about time).
    #[gpui::test]
    fn a_change_redraws_only_the_cached_rows_whose_inputs_changed(cx: &mut TestAppContext) {
        const N: usize = 200;
        let rows = cx.new(|_| Rows((0..N).map(|i| format!("session {i}")).collect()));
        let cx = open(cx, &rows, N);
        assert_eq!(
            cx.update(|_, cx| render_count::count("row", cx)),
            N,
            "first frame draws every row"
        );

        cx.update(|_, cx| render_count::reset(cx));
        rows.update(cx, |rows, cx| rows.rename(7, "renamed", cx));
        cx.run_until_parked();
        assert_eq!(
            cx.update(|_, cx| render_count::count("row", cx)),
            1,
            "only the renamed row redraws"
        );

        cx.update(|_, cx| render_count::reset(cx));
        rows.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert_eq!(
            cx.update(|_, cx| render_count::count("row", cx)),
            0,
            "an unrelated notification redraws no row"
        );
    }

    /// Hover help must not cost the list a redraw. Showing and hiding a tooltip redraws the root that draws the overlay,
    /// and the cached rows replay; a whole-window refresh would re-render all 200 rows each time the pointer crossed a
    /// control, which is what this guards against.
    ///
    /// The tooltip is shown and hidden through the layer manager's own functions from a test update, the calls a
    /// control's hover listener makes. A simulated mouse move is not used here: GPUI's simulated platform can apply the
    /// hover change while a frame is being drawn, where GPUI ignores a refresh, so a regression back to refreshing the
    /// window would pass unseen (checked by putting the refresh back: this version fails, the mouse-driven one did not).
    /// The mouse path itself is covered by the component hover tests.
    #[gpui::test]
    fn showing_and_hiding_a_tooltip_redraws_no_cached_row(cx: &mut TestAppContext) {
        const N: usize = 200;
        let rows = cx.new(|_| Rows((0..N).map(|i| format!("session {i}")).collect()));
        let cx = open(cx, &rows, N);
        let owner = cx.update(|_, cx| cx.new(|_| layers::TooltipOwner));
        cx.update(|_, cx| render_count::reset(cx));

        cx.update(|window, cx| {
            layers::show_tooltip(&owner, "What this list is".into(), window, cx)
        });
        cx.run_until_parked();
        assert!(
            cx.update(|window, cx| layers::shown_tooltip(window, cx))
                .is_some(),
            "the premise: the tooltip is showing"
        );
        assert!(
            cx.update(|_, cx| render_count::count("list", cx)) > 0,
            "the premise: the root redrew to draw the tooltip"
        );
        assert_eq!(
            cx.update(|_, cx| render_count::count("row", cx)),
            0,
            "showing redraws no row"
        );

        cx.update(|window, cx| layers::hide_tooltip(&owner, window, cx));
        cx.run_until_parked();
        assert_eq!(
            cx.update(|window, cx| layers::shown_tooltip(window, cx)),
            None,
            "the premise: it is hidden"
        );
        assert_eq!(
            cx.update(|_, cx| render_count::count("row", cx)),
            0,
            "hiding redraws no row"
        );
    }
}
