//! The plumbing the launcher's floating suggestion lists share: a list that floats under a text field on the layer
//! manager's non-focusing popover, so focus and every key stay in the field, with a highlighted row the field's Up and
//! Down move.
//!
//! Four lists work this way: the model field's (`model_field`), the folder field's (`folder_list`), the composer
//! search's results (`search`) and a managed checkout's repository suggestions (`checkout`). Each keeps what differs:
//! its rows and their words (a [`DrawRows`] function from the launcher's state), what a pick and Enter do, when typing
//! drops or sets the highlight, and its empty states. Their Enter rules differ on purpose, so no shared policy is here.
//!
//! The host picker's list (`host_picker`) opens the same kind of popover from its button rather than a text field, and
//! keeps its own copy of this: the button takes focus on a click, and the list has type-ahead.

use gpui::{
    AnyElement, App, AppContext as _, Context, Div, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollHandle, Stateful, StatefulInteractiveElement as _,
    Styled as _, Subscription, WeakEntity, Window, px,
};

use super::Launcher;
use crate::components::Vertical;
use crate::focus::Mark;
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::theme::metrics;

/// `.launch-composer-listbox` and the composer's other floating lists: at most 260 pixels tall, scrolling past that.
const LIST_MAX_HEIGHT: f32 = 260.;
/// The same lists' `padding: 6px; gap: 2px`.
const LIST_PAD: f32 = 6.;
const LIST_GAP: f32 = 2.;

/// One list's own drawing: from the launcher's state, given the list's surface (already sized, padded and scrolling,
/// under its id), the list's role and name and its rows, with which child is the highlighted row. The weak launcher is
/// for the rows' click handlers.
pub(super) type DrawRows =
    fn(&Launcher, Stateful<Div>, &WeakEntity<Launcher>, &Window, &App) -> Drawn;

/// What a [`DrawRows`] function drew.
pub(super) struct Drawn {
    /// The list's surface, with whatever role and name the list gives it now.
    pub(super) list: Stateful<Div>,
    /// Its children, headings and status lines included: every row must be a direct child, so the scroll handle can
    /// bring any of them into view.
    pub(super) children: Vec<AnyElement>,
    /// The index in `children` of the highlighted row, if one is highlighted and drawn. A heading before it shifts it,
    /// so it is not always the row's own index.
    pub(super) highlighted: Option<usize>,
}

/// Up or Down over `count` rows from the row `from` (none highlighted yet): Down from none is the first row, Up from
/// none the last, and both wrap. `None` when there are no rows to move to.
pub(in crate::launcher) fn step(
    from: Option<usize>,
    arrow: Vertical,
    count: usize,
) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match (arrow, from) {
        (Vertical::Down, None) => 0,
        (Vertical::Down, Some(index)) => (index + 1) % count,
        (Vertical::Up, None) => count - 1,
        (Vertical::Up, Some(index)) => (index + count - 1) % count,
    })
}

/// A floating list's state besides its rows: the open layer, where it opens, and the highlighted row. None of it is
/// shared with another surface, and none survives the launcher.
#[derive(Default)]
pub(in crate::launcher) struct FieldList {
    /// The open list's layer, as last opened by the field. The layer manager closes it itself on Escape or a press
    /// outside, without telling the field, so [`FieldList::is_open`] asks it rather than trusting this.
    layer: Option<LayerId>,
    /// Where the field is drawn, for the list to open under it; refreshed every frame ([`FieldList::track_anchor`]).
    anchor: Option<Anchor>,
    /// The highlighted row, as an index into the list's current rows. Closing the list drops it; when else it changes
    /// is each list's own rule.
    pub(in crate::launcher) highlighted: Option<usize>,
}

impl FieldList {
    /// Whether the list is open: opened by the field and not since closed by the layer manager.
    pub(in crate::launcher) fn is_open(&self, window: &Window, cx: &App) -> bool {
        self.layer
            .is_some_and(|layer| layers::is_open(layer, window, cx))
    }

    /// Whether the field opened the list and has not closed or forgotten it since, whatever the layer manager did with
    /// it meanwhile.
    pub(super) fn opened(&self) -> bool {
        self.layer.is_some()
    }

    /// Up or Down over `count` rows ([`step`]). Returns whether the highlight moved; with no rows it stays as it was.
    pub(in crate::launcher) fn arrow(&mut self, arrow: Vertical, count: usize) -> bool {
        let Some(next) = step(self.highlighted, arrow, count) else {
            return false;
        };
        self.highlighted = Some(next);
        true
    }

    /// Note where the field is drawn under `key` (which must differ from the field's own id), for the list to open
    /// under it as wide as it, and return the element that records it, drawn inside the field's box.
    pub(super) fn track_anchor(
        &mut self,
        key: &'static str,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement + use<> {
        let anchor = Anchor::for_element(key, window, cx).matching_width();
        let tracker = anchor.track();
        self.anchor = Some(anchor);
        tracker
    }

    /// Open the list under the field, as a popover that never takes focus, drawn under `id` by `draw`. Nothing opens
    /// before the field has been drawn once, since there is nowhere to open it yet.
    pub(super) fn open(
        &mut self,
        id: &'static str,
        draw: DrawRows,
        window: &mut Window,
        cx: &mut Context<Launcher>,
    ) {
        let Some(anchor) = self.anchor.clone() else {
            return;
        };
        let launcher = cx.weak_entity();
        let rows = cx.new(|cx| ListRows::new(launcher, id, draw, cx));
        let container = cx.focus_handle();
        let mark = Mark::take(cx);
        self.layer = layers::open_popover(
            Popover::floating(rows, container, anchor),
            &mark,
            window,
            cx,
        );
    }

    /// Close the list if it is open, and drop the highlight.
    pub(super) fn close(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(layer) = self.layer.take()
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
        }
        self.highlighted = None;
    }

    /// Forget a list the layer manager closed on its own (Escape, a press outside), and its highlight, returning
    /// whether there was one to forget, so a caller can put its field back as a close would. Called while the launcher
    /// draws, which relies on the launcher being drawn in every frame its dialog is: the layer manager keeps no cached
    /// copy of a dialog's view, so the first frame after the close sees it.
    pub(in crate::launcher) fn follow(&mut self, window: &Window, cx: &App) -> bool {
        if self.layer.is_none() || self.is_open(window, cx) {
            return false;
        }
        self.layer = None;
        self.highlighted = None;
        true
    }

    /// The open list's layer, for tests that close it the way the layer manager does on its own.
    #[cfg(test)]
    pub(in crate::launcher) fn layer(&self) -> Option<LayerId> {
        self.layer
    }
}

/// The open list's view: the rows its [`DrawRows`] draws from the launcher's state, kept scrolled so the highlighted
/// row is in view.
pub(super) struct ListRows {
    launcher: WeakEntity<Launcher>,
    /// The list's element id, which tests also find it by.
    id: &'static str,
    draw: DrawRows,
    scroll: ScrollHandle,
    /// The child last scrolled into view. The list is scrolled only when the highlight moves, so a wheel scroll is not
    /// pulled back to the highlighted row on the next redraw.
    scrolled_to: Option<usize>,
    /// Redraws the rows when the launcher changes (what is typed, the highlighted row, the draft, a read's answer).
    _redraw: Option<Subscription>,
}

impl ListRows {
    fn new(
        launcher: WeakEntity<Launcher>,
        id: &'static str,
        draw: DrawRows,
        cx: &mut Context<Self>,
    ) -> Self {
        let redraw = launcher
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()));
        ListRows {
            launcher,
            id,
            draw,
            scroll: ScrollHandle::new(),
            scrolled_to: None,
            _redraw: redraw,
        }
    }
}

impl Render for ListRows {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.id;
        let list = layers::surface(cx)
            .id(id)
            // Tests find the list by where it is drawn; this records nothing outside test builds.
            .debug_selector(move || id.into())
            .flex()
            .flex_col()
            .gap(px(LIST_GAP))
            .p(px(LIST_PAD))
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .text_size(metrics::TEXT_MD);
        let Some(launcher) = self.launcher.upgrade() else {
            return list;
        };
        let drawn = (self.draw)(launcher.read(cx), list, &self.launcher, window, cx);
        if drawn.highlighted != self.scrolled_to {
            if let Some(child) = drawn.highlighted {
                self.scroll.scroll_to_item(child);
            }
            self.scrolled_to = drawn.highlighted;
        }
        drawn.list.children(drawn.children)
    }
}
