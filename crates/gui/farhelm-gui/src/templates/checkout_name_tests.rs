//! Tests of what the launcher's Name field says about a launch template's base name in a managed checkout
//! ([`super::checkout_name`]): when the "From template · numbered if taken" pill and the "(<base> is taken)" note show,
//! read from the draft, and that the launcher draws them in its Name row.

use farhelm_proto::launcher::TemplateNameKind;
use gpui::{TestAppContext, VisualTestContext};

use super::checkout_name::template_base;
use super::dialog_tests::painted;
use super::save_tests::launcher_on;
use crate::launcher::draft::{LaunchDraft, Tab};
use crate::test_support::{self, settle};

/// New on host 1 with a command that launches, a managed checkout of `acme/bar` chosen, and `review` as the session
/// name, set by a launch template with `kind`, as template application sets it.
fn checkout_draft(kind: TemplateNameKind) -> LaunchDraft {
    let mut draft = LaunchDraft::new(Some(1), "~".into(), None);
    draft.choose_tab(Tab::Command);
    draft.edit_command("true".into());
    draft.choose_command_yolo(false);
    draft.choose_repository(farhelm_proto::parse_github_repo("acme/bar").unwrap());
    draft.edit_name("review".into());
    draft.set_template_name_kind(kind);
    draft
}

/// [`checkout_draft`] with a base name the helm already answered as taken once, so the name moved on to `review-2`.
fn advanced_draft() -> LaunchDraft {
    let mut draft = checkout_draft(TemplateNameKind::Base);
    let title = draft.checkout_title();
    assert!(
        draft.advance_checkout_name(&title),
        "the premise: a base name moves on"
    );
    draft
}

/// Whether anything is drawn under the debug selector `selector`.
fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// SPEC.md, "Launch templates": the pill marks a template's base name in a managed checkout, which Farhelm numbers when
/// taken; a fixed name keeps its spelling, and "Typing in Name or accepting `name:` makes it the user's exact name,
/// removes the pill and disables numbering". A pill on a name that is not numbered would promise numbering that never
/// happens. Specified: a base name in a checkout has a template base, not yet taken, and taken once the numbering moved;
/// a fixed name, a typed name and a folder destination have none.
#[test]
fn only_a_template_base_in_a_checkout_has_the_pill() {
    assert_eq!(
        template_base(&checkout_draft(TemplateNameKind::Base)),
        Some(("review".to_owned(), false))
    );
    assert_eq!(
        template_base(&advanced_draft()),
        Some(("review".to_owned(), true))
    );
    assert_eq!(
        template_base(&checkout_draft(TemplateNameKind::Fixed)),
        None
    );
    let mut typed = checkout_draft(TemplateNameKind::Base);
    typed.edit_name("review".into());
    assert_eq!(template_base(&typed), None, "typing makes the name exact");
    let mut folder = checkout_draft(TemplateNameKind::Base);
    folder.edit_folder("/work".into());
    assert_eq!(template_base(&folder), None, "a folder numbers nothing");
}

/// The pill is drawn beside the Name label where the old launcher drew it, and goes the moment the user types a name
/// of their own. Specified: the launcher on a template's base name in a checkout draws the pill and no taken note; after
/// typing in the Name field the pill is gone.
#[gpui::test]
fn the_name_row_shows_the_pill_until_the_user_types(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, checkout_draft(TemplateNameKind::Base), &[]);
    assert!(drawn(cx, "launcher-name-policy"));
    assert!(painted(cx).contains("From template · numbered if taken"));
    assert!(!drawn(cx, "launcher-name-taken"));
    test_support::click(cx, "launcher-name");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("input-state"),
        "the premise: the click put focus in a text field"
    );
    cx.simulate_input("x");
    settle(cx);
    assert!(!drawn(cx, "launcher-name-policy"));
}

/// An advanced name explains why it differs from the template's base. The mode pill stays in the Name label area,
/// above the input, with the reason below the input. This keeps the pill and note from covering the name when the
/// destination card wraps its fields.
#[gpui::test]
fn a_taken_base_says_so_under_the_field(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, advanced_draft(), &[]);
    assert!(drawn(cx, "launcher-name-policy") && drawn(cx, "launcher-name-taken"));
    let text = painted(cx);
    assert!(text.contains("review-2"), "{text}");
    assert_eq!(
        test_support::label(cx, "launcher-name-taken").as_deref(),
        Some("(review is taken)")
    );
    let pill = test_support::bounds(cx, "launcher-name-policy");
    let field = test_support::bounds(cx, "launcher-name");
    let taken = test_support::bounds(cx, "launcher-name-taken");
    assert!(
        pill.left() >= field.left()
            && pill.right() <= field.right()
            && pill.bottom() <= field.top()
            && field.bottom() <= taken.top(),
        "the pill {pill:?}, field {field:?}, and reason {taken:?} fit in the Name column"
    );
}
