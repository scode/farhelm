//! Tests of the plumbing the launcher's floating suggestion lists share (`view::field_list`): Up and Down over the
//! rows, without a window, then forgetting a list the layer manager closed on its own, in the real launcher over the
//! helm client's fake (the folder field's list stands in for all four).
//!
//! Each list's own rules (its rows, Enter, when typing drops the highlight) are tested with that list, in
//! `folder_list_tests`, `search_view_tests`, `view_tests` (the model field) and `checkout_tests`.

use gpui::TestAppContext;

use super::draft::LaunchDraft;
use super::view::{Launcher, step_for_tests as step};
use super::view_tests::{fixture_hosts, launcher_with_hosts};
use crate::components::Vertical;
use crate::layers;
use crate::test_support::{self, keys, settle};

/// Down and Up wrap at both ends, and an arrow with nothing highlighted starts at the near end: Down at the first row,
/// Up at the last. Why: every list's keyboard walks its rows this way (the old comboboxes' rule), and a wrong wrap would
/// strand the highlight past the end, where Enter takes nothing or the wrong row.
#[test]
fn arrows_wrap_both_ways_and_start_from_no_highlight() {
    assert_eq!(step(None, Vertical::Down, 3), Some(0));
    assert_eq!(step(None, Vertical::Up, 3), Some(2));
    assert_eq!(step(Some(0), Vertical::Down, 3), Some(1));
    assert_eq!(
        step(Some(2), Vertical::Down, 3),
        Some(0),
        "Down wraps to the first row"
    );
    assert_eq!(
        step(Some(0), Vertical::Up, 3),
        Some(2),
        "Up wraps to the last row"
    );
    assert_eq!(
        step(Some(0), Vertical::Down, 1),
        Some(0),
        "one row wraps onto itself"
    );
}

/// With no rows there is nowhere to move, and a list's highlight stays as it was rather than pointing at a row that
/// does not exist. Why: a repository list with no suggestions yet, or a model list filtered to nothing, still takes
/// arrows, and Enter must not then take a made-up row.
#[test]
fn an_empty_list_moves_nothing() {
    assert_eq!(step(None, Vertical::Down, 0), None);
    assert_eq!(step(Some(1), Vertical::Up, 0), None);
}

/// A list the layer manager closed on its own (as on Escape or a press outside, which tell the field nothing) is
/// forgotten the next time the launcher follows it, with its highlight, and only once. Why: the field otherwise keeps
/// a highlight for rows that are no longer shown, and a later Enter would take a row the user never saw highlighted.
#[gpui::test]
fn a_list_the_layer_manager_closed_is_forgotten_with_its_highlight(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[0]),
    );
    test_support::click(cx, "launcher-folder-row");
    keys(cx, "down");
    test_support::frame(cx);
    assert!(
        cx.debug_bounds("launcher-folder-list").is_some(),
        "the premise: focusing the folder field opened its list"
    );
    cx.update(|window, cx| {
        let launcher = layers::last_opened::<Launcher>(cx).expect("a launcher is open");
        // Closed directly, as the layer manager does on its own, with no frame drawn before the field follows it.
        let layer = launcher.update(cx, |launcher, _| {
            launcher.folder_field_list_for_tests().layer()
        });
        layers::close(layer.expect("the premise: the list is open"), window, cx);
        launcher.update(cx, |launcher, cx| {
            let list = launcher.folder_field_list_for_tests();
            assert_eq!(
                list.highlighted,
                Some(0),
                "the premise: the field still holds the highlight the arrow set"
            );
            assert!(
                !list.is_open(window, cx),
                "the premise: the layer is closed"
            );
            assert!(list.follow(window, cx), "following forgets the closed list");
            assert_eq!(list.highlighted, None, "and its highlight");
            assert!(
                !list.follow(window, cx),
                "a second follow has nothing to forget"
            );
            assert!(
                !list.arrow(Vertical::Down, 0),
                "with no rows an arrow moves nothing"
            );
            assert_eq!(list.highlighted, None, "and leaves the highlight as it was");
            assert!(
                list.arrow(Vertical::Down, 3) && list.highlighted == Some(0),
                "the next arrow starts from no highlight again"
            );
        });
    });
    settle(cx);
}
