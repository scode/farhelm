//! A list row's open control: the old stylesheet's `.session-row-open`, a button that fills its row edge to edge and
//! holds the row's lines instead of a text label.
//!
//! The old app drew each session row's body as one native button (real Enter and Space activation, focus, and one
//! accessible name for the row), with the row's lines as its content. This is that button. Its constructor takes the
//! label, hover help and role every control takes; the label is the accessible name, not drawn text, because the row
//! draws its own lines (a host's title among them, which only host text may draw). Its content is laid out as a column
//! of lines with the old 2-pixel gap and padding (6 pixels top and bottom, 12 on the left, none on the right).
//!
//! What it paints itself is only the interaction feedback: the old hover and focus fill (`--bg-2`, or one step up the
//! accent family, `--accent-fill-hover`, on the selected row, so hovering the selected row does not swallow its
//! selection), and the keyboard focus ring, which is drawn inside the edge (the old `outline-offset: -2px`) because the
//! button fills the sidebar's width and an outer ring would be clipped. The selected row's own solid fill belongs to the
//! row around this control, which also holds the row's other controls once they exist.
//!
//! Focus and activation follow [`super::Button`]: a tab stop tracked through the focus coordinator's click rule, and
//! activation on click or through the controls' shared key rule ([`super::activate_on_enter_or_space`]). Which row is
//! selected is the list's to tell assistive technology (on the list item around this control), not this control's.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::{Handler, HoverHelp};
use crate::focus;
use crate::theme::{self, metrics};

/// The open control of one list row. See the module docs.
#[derive(IntoElement)]
pub struct ListRow {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    selected: bool,
    lines: Vec<AnyElement>,
    on_click: Option<Handler>,
}

impl ListRow {
    /// A row control. `id` must be stable for the thing the row shows (a session's key, not its position), so its focus
    /// and tooltip follow the row when the list reorders; `label` is its accessible name; `role` is what assistive
    /// technology and the test bridge see.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        ListRow {
            id: id.into(),
            label: label.into(),
            help,
            role,
            selected: false,
            lines: Vec::new(),
            on_click: None,
        }
    }

    /// Whether this row is the selected one, which it hovers in the accent family rather than with the neutral fill.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Add one line of the row's content, below the lines before it.
    pub fn line(mut self, line: impl IntoElement) -> Self {
        self.lines.push(line.into_any_element());
        self
    }

    /// What activating the row does.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ListRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let focus = focus::control_handle(
            ElementId::Name(format!("{}-focus", self.id).into()),
            window,
            cx,
        );
        let ring = focus::shows_ring(&focus, window);
        let hover_fill = if self.selected {
            palette.accent_fill_hover
        } else {
            palette.bg_2
        };
        let (pad_y, pad_left) = metrics::ROW_PAD;
        let handler = self.on_click.clone();
        let key_handler = handler.clone();

        let mut row = div()
            .id(ElementId::Name(self.id.clone()))
            .relative()
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .pt(pad_y)
            .pb(pad_y)
            .pl(pad_left)
            .cursor_pointer()
            .hover(move |style| style.bg(theme::color(hover_fill)))
            .children(self.lines);
        if ring {
            row = row.bg(theme::color(hover_fill)).child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .border_2()
                    .border_color(theme::color(palette.accent)),
            );
        }
        let row = super::with_hover_help(row, &self.id, &self.help, true, window, cx);
        super::activate_on_enter_or_space(
            focus::track(row, &focus, focus::OnPress::Control),
            key_handler,
        )
        .when_some(handler, |row, handler| {
            row.on_click(move |event, window, cx| handler(event, window, cx))
        })
    }
}
