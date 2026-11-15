//! Tests of the menu item's checked state: a menu's radio item reports whether it is the choice in force and ticks its
//! label, keeping its icon and description.

use gpui::{IntoElement, ParentElement as _, Styled as _, TestAppContext, Toggled, div, px};

use crate::assets::Icon;
use crate::components::{HoverHelp, MenuItem, Role};
use crate::test_support::{frame, open};

use super::painted_under;

/// A menu's radio items must say which one is in force, to assistive technology as checked and on screen as the old
/// " ✓" after the label, while keeping their icon and description, unlike a select's choice, which trades its icon for
/// a tick and reports itself selected. Specified: of three items, one checked, one not and one plain command, the
/// checked one reports `Toggled::True` and paints its label with " ✓", the unchecked one reports `Toggled::False` and
/// paints its label alone, the plain one reports no checked state; all three keep their label as their name and paint
/// their icon and their description.
#[gpui::test]
fn a_checked_menu_item_reports_its_state_and_ticks_its_label(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        let item = |id: &'static str, label: &'static str| {
            MenuItem::new(
                id,
                Icon::Adjust,
                label,
                HoverHelp::tooltip("An order"),
                Role::MenuItemRadio,
            )
            .description("What it does")
        };
        div()
            .flex()
            .flex_col()
            .w(px(240.))
            .child(item("on", "First").checked(true))
            .child(item("off", "Second").checked(false))
            .child(item("plain", "Third"))
            .into_any_element()
    });
    let nodes = frame(cx);
    for (id, label, toggled, painted) in [
        ("on", "First", Some(Toggled::True), "First ✓"),
        ("off", "Second", Some(Toggled::False), "Second"),
        ("plain", "Third", None, "Third"),
    ] {
        let node = nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} was not drawn"));
        assert_eq!(node.label.as_deref(), Some(label), "{id}");
        assert_eq!(node.toggled, toggled, "{id}");
        let (texts, glyphs) = painted_under(&nodes, id);
        assert!(texts.iter().any(|text| text == painted), "{id}: {texts:?}");
        assert!(
            texts.iter().any(|text| text == "What it does"),
            "{id}: {texts:?}"
        );
        assert_eq!(glyphs, 1, "{id} keeps its icon");
    }
}
