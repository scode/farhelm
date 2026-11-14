// Modified for Farhelm: changed or added by the patch layers in vendor/patches/gpui/ (vendor/README.md, "GPUI").
//! The bridge layer's driver record (`src/window/driver.rs`), read the way the automation bridge reads it: enable
//! recording, let a frame draw, and look at the nodes it recorded.

use gpui::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Role,
    StatefulInteractiveElement as _, TestAppContext, Window, div,
};

/// Three list items that differ only in what they say about selection: selected, not selected, and nothing at all.
struct ListView;

impl Render for ListView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(
                div()
                    .id("selected")
                    .role(Role::ListItem)
                    .aria_selected(true),
            )
            .child(
                div()
                    .id("unselected")
                    .role(Role::ListItem)
                    .aria_selected(false),
            )
            .child(div().id("unmarked").role(Role::ListItem))
    }
}

/// Why this matters: the driver record is the only place the bridge learns an element's state from, and a list's
/// selected item (the open session's row in the sidebar) says so only through `aria_selected`, which nothing else in
/// the record carries. Without the field, a test driving the app cannot tell which row is selected except by its
/// colors.
///
/// Specified: a list item marked selected records `selected: Some(true)`; one marked not selected records
/// `Some(false)`; one that never calls `aria_selected` records `None`, so "not selected" stays distinct from
/// "selection does not apply", as AccessKit keeps them.
#[gpui::test]
fn a_list_items_selected_state_reaches_the_driver_record(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| ListView);
    cx.update(|window, _| window.set_driver_enabled(true));
    cx.run_until_parked();
    let nodes = cx.update(|window, _| window.driver_nodes().to_vec());
    let recorded = |id: &str| {
        let node = nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id));
        let Some(node) = node else {
            let ids: Vec<_> = nodes.iter().map(|node| &node.element_id).collect();
            panic!("no node {id:?} in the recorded frame: {ids:?}");
        };
        // The premise: the item recorded its accessibility role, so its accessibility state was read at all.
        assert_eq!(node.role, Some(Role::ListItem), "{id}'s role");
        node.selected
    };
    assert_eq!(recorded("selected"), Some(true));
    assert_eq!(recorded("unselected"), Some(false));
    assert_eq!(recorded("unmarked"), None);
}
