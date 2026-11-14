//! An option row: one result in a list whose keyboard selection lives in a text field beside it, picked by a click or
//! by the field's Enter. The old stylesheet's `.quick-switcher-row`.
//!
//! The old quick switcher kept focus in its search field the whole time it was open: its rows were buttons with
//! `tabindex="-1"`, the arrow keys moved the selected row while the caret stayed in the field, and Tab went from the
//! field straight to Close. This control is that row. It is never a tab stop and a press never focuses it: it tracks
//! no focus handle at all, and it marks its presses as handled, so neither Tab nor a click moves focus off the field,
//! not even through the empty-space rule of the dialog or page around it. It has no keyboard activation of its own
//! (the field's owner handles Enter and the arrows). A click activates it, as a click on the old row did.
//!
//! Its constructor takes the label, hover help and role every control takes. The label is the accessible name, not
//! drawn text: the row draws its own content, host text among it, which only host text may draw. Whether it is the
//! selected row is its owner's to say ([`OptionRow::selected`]); it tells assistive technology through the row's own
//! selected state, since the row is the option.
//!
//! It paints the old row's box: a one-pixel edge, the small radius, 9 by 10 pixels of padding, transparent at rest
//! with the control hover fill under the pointer, and on the selected row the solid accent fill with its accent edge,
//! one step up the accent family under the pointer (`--accent-fill-hover`), so hovering the selection does not hide
//! it. Hover only paints: it never moves the selection, as in the old switcher.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, MouseButton,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::{Handler, HoverHelp};
use crate::theme::{self, metrics};

/// The old row's padding, top and bottom then left and right (`padding: 9px 10px`).
const PAD: (f32, f32) = (9., 10.);

/// One option of a picking list. See the module docs.
#[derive(IntoElement)]
pub struct OptionRow {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    selected: bool,
    children: Vec<AnyElement>,
    on_click: Option<Handler>,
}

impl OptionRow {
    /// An option row. `id` must be stable for the thing the row shows (a session's key, not its position), so its
    /// tooltip follows the row when the list changes; `label` is its accessible name; `role` is what assistive
    /// technology and the test bridge see (usually [`Role::ListBoxOption`]).
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        OptionRow {
            id: id.into(),
            label: label.into(),
            help,
            role,
            selected: false,
            children: Vec::new(),
            on_click: None,
        }
    }

    /// Whether this is the selected row: drawn in the accent fill and reported selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Add `child` to the row's content, which is laid out as a column inside the row's padding.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// What a click on the row does.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for OptionRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let (fill, edge, hover_fill) = if self.selected {
            (
                palette.accent_fill,
                palette.accent_edge,
                palette.accent_fill_hover,
            )
        } else {
            (
                theme::TRANSPARENT,
                theme::TRANSPARENT,
                palette.control_hover,
            )
        };
        let row = div()
            .id(ElementId::Name(self.id.clone()))
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            .aria_selected(self.selected)
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .py(px(PAD.0))
            .px(px(PAD.1))
            .border_1()
            .border_color(theme::color(edge))
            .rounded(metrics::RADIUS_SM)
            .bg(theme::color(fill))
            .cursor_pointer()
            .hover(move |style| style.bg(theme::color(hover_fill)))
            // The press is this control's, though it takes no focus: marked, it never reaches the empty-space rule of
            // the page or dialog around it, which would otherwise move focus off the field (as a disabled button's
            // press is kept from it).
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .children(self.children);
        let row = super::with_hover_help(row, &self.id, &self.help, true, window, cx);
        row.when_some(self.on_click, |row, handler| {
            row.on_click(move |event, window, cx| handler(event, window, cx))
        })
    }
}
