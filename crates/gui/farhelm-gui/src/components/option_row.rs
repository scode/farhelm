//! An option row: one result in a list whose keyboard selection lives in a text field beside it, picked by a click or
//! by the field's Enter. The old stylesheet's `.quick-switcher-row`.
//!
//! The old quick switcher kept focus in its search field the whole time it was open: its rows were buttons with
//! `tabindex="-1"`, the arrow keys moved the selected row while the caret stayed in the field, and Tab went from the
//! field straight to Close. This control is that row. It is never a tab stop, so Tab never reaches it, and a press
//! never focuses it: it tracks no focus handle at all, and it marks its presses as handled.
//!
//! Whether a click on it leaves focus in the field depends on the surface it is drawn in. The page's own empty-space
//! rule respects the handled mark, so on the page focus stays put, unless something between the row and the page takes
//! focus on a press (a terminal does). A dialog or anchored question opened with `keeps_focus_on_empty_click` (the
//! quick switcher's dialog) keeps focus too. A dialog whose empty space takes focus ignores the mark: it goes by what
//! focus did during the press, and this row moves none, so a click on the row there moves focus to the dialog. Draw it
//! only where focus stays. It has no keyboard activation of its own (the field's owner handles Enter and the arrows). A
//! click activates it, as a click on the old row did.
//!
//! Its constructor takes the label, hover help and role every control takes. The label is the accessible name, not
//! drawn text: the row draws its own content, host text among it, which only host text may draw.
//!
//! **The selected state.** Whether it is the selected row (the highlighted one, which Enter in the field would pick)
//! is its owner's to say ([`OptionRow::selected`]). The row then draws the fill and the ink on it together, so no owner
//! sets the text color that goes with the fill: the old rows (`.quick-switcher-row`, `.launcher-host-option` and the
//! model list's `.launch-composer-listbox > button`) all set `color: var(--fg-0)` at rest (except the model list's last
//! row, its show-all switch, which rested in `--fg-1`) and `var(--on-fill)` with the selection, and the row's content
//! inherits that ink. Content that draws its own color keeps it, as in the old sheet, whose selected rules for such
//! parts were rules of their own: a host's colored mark, the switcher's quieter columns (`--on-accent-dim`), its match
//! marks. Their owners still pick those for the selected row. The row also tells assistive technology through its own
//! selected state, since the row is the option.
//!
//! It paints the old row's box: a one-pixel edge, the small radius, 9 by 10 pixels of padding, transparent at rest
//! with the control hover fill under the pointer, and on the selected row the solid accent fill with its accent edge,
//! one step up the accent family under the pointer (`--accent-fill-hover`), so hovering the selection does not hide
//! it. Hover only paints: it never moves the selection, as in the old switcher.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::{Handler, HoverHelp};
use crate::theme::{self, metrics};

/// The old row's padding, top and bottom then left and right (`padding: 9px 10px`).
const PAD: (f32, f32) = (9., 10.);

/// Label a run of options without becoming an option. This keeps the heading out of keyboard selection.
pub(crate) fn option_heading(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    palette: &theme::Palette,
) -> AnyElement {
    let id = id.into();
    let label = label.into();
    div()
        .id(id.clone())
        .debug_selector(|| id.to_string())
        .role(Role::Label)
        .aria_label(label.clone())
        .pt(px(4.))
        .px(px(6.))
        .pb(px(1.))
        .text_size(metrics::TEXT_SM)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::color(palette.fg_1))
        .child(label)
        .into_any_element()
}

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
            help: help
                .beside_list(false)
                .ring_radius(metrics::RADIUS_SM - px(1.)),
            role,
            selected: false,
            children: Vec::new(),
            on_click: None,
        }
    }

    /// Use the enclosing popover's edge for launcher suggestions. Inline lists register their own edge.
    pub(crate) fn in_popover(mut self) -> Self {
        self.help = self.help.beside_list(true);
        self
    }

    /// Whether this is the selected row: drawn in the accent fill with the fill's ink as the content's text color, and
    /// reported selected. See the module docs for which content keeps its own color.
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
        let (fill, edge, hover_fill, ink) = if self.selected {
            (
                palette.accent_fill,
                palette.accent_edge,
                palette.accent_fill_hover,
                palette.on_fill,
            )
        } else {
            (
                theme::TRANSPARENT,
                theme::TRANSPARENT,
                palette.control_hover,
                palette.fg_0,
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
            .text_color(theme::color(ink))
            .cursor_pointer()
            .hover(move |style| style.bg(theme::color(hover_fill)))
            // A press of any mouse button is this control's, though it takes no focus. Marked, it leaves focus on the
            // field under the page's empty-space rule, which runs on every button (as a disabled button's press does);
            // a dialog whose empty space takes focus ignores the mark, which is why the module docs limit where the row
            // is drawn.
            .on_any_mouse_down(|_, window, _| window.prevent_default())
            .children(self.children);
        let help = if self.selected {
            self.help.ring_on_fill()
        } else {
            self.help
        };
        let row = super::with_hover_help(row, &self.id, &help, true, None, window, cx);
        row.when_some(self.on_click, |row, handler| {
            row.on_click(move |event, window, cx| handler(event, window, cx))
        })
    }
}
