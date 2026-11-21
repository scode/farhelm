//! Tests of the template rows and the quick switcher's template entry points: the `tl:` label (pure), and, in the real
//! main view over the helm client's fake, the rows a query gets, a pick from the switcher's templates section and the
//! Quick launch row. The switcher does not draw these rows yet, so the tests call the entry points as its handlers
//! will, with no dialog open (the switcher closes before it acts).
//!
//! The window and its templates are the dispatcher tests' (`super::dispatch_tests`), in the helm's order `go`,
//! `partial`, `nowhere`, `offline`, `model-only`, `wrong-model`: `go` launches on its own, `partial` only sets a folder.

use std::collections::BTreeMap;
use std::time::Duration;

use farhelm_proto::launcher::{LaunchTemplate, TemplateDestination, TemplateFields};
use farhelm_proto::ui_channel::{Availability, KeyedEntry, Preferences};
use gpui::{TestAppContext, VisualTestContext};

use super::dispatch_tests::{
    banner, creates, marked, note, push, templates, text, unreadable, unseen, window,
    window_without_preferences,
};
use super::rows::{self, template_query};
use super::summary;
use crate::launcher::view::BANNER_SHOWS_FOR;
use crate::session_list::tests::{painted, preferences_state};
use crate::test_support;
use crate::test_support::{any_open, frame, settle};

/// SPEC.md: the switcher's section takes over "With a leading `tl:` (case-insensitive)", and "`tl:` alone lists every
/// template". The label is read as the old launcher's search read it, so a query that only looks similar must not
/// turn into a template search. Specified: `tl:` asks for everything, `  TL: build ` for `build`, `Tl:a:b` for `a:b`
/// (only the first colon ends the label); `tl : x`, `x tl:y` and `build` are not template queries.
#[test]
fn a_leading_tl_label_makes_a_template_query() {
    assert_eq!(template_query("tl:"), Some(""));
    assert_eq!(template_query("  TL: build "), Some("build"));
    assert_eq!(template_query("Tl:a:b"), Some("a:b"));
    for other in ["tl : x", "x tl:y", "build", ""] {
        assert_eq!(template_query(other), None, "{other:?}");
    }
}

/// The names [`rows::rows`] gives for `filter`, in order.
fn names(filter: &str, cx: &mut VisualTestContext) -> Vec<String> {
    let rows = cx.update(|_, cx| rows::rows(filter, cx)).expect("rows");
    rows.into_iter().map(|row| row.name).collect()
}

/// SPEC.md: rows show "the template name, summary and its ⚡ or number shortcut", matched "using the session launcher's
/// own template search"; the old search kept the helm's order and matched a lowercased substring. The switcher, the
/// launcher's `tl:` and Cmd+J share these rows, so a wrong match or order would show in all three. Specified: an empty
/// filter lists all six in the helm's order; `O` (any case) finds the five names with an o; ` GO ` finds `go` alone;
/// `partial`'s row carries its summary and its number key 3, and `go`'s its ⚡ and no key; once `Nightly-Build` is
/// added, `build` finds it (the name is lowercased too).
#[gpui::test]
fn rows_match_a_lowercased_substring_in_order_with_summaries_and_shortcuts(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = window(cx, Some("go"));
    push(
        &mut helm,
        cx,
        preferences_state(Preferences {
            quick_launch_template: Some("go".into()),
            template_shortcuts: BTreeMap::from([("3".to_owned(), "partial".to_owned())]),
            welcome_seen: Some(true),
            ..Preferences::default()
        }),
    );
    let all = [
        "go",
        "partial",
        "nowhere",
        "offline",
        "model-only",
        "wrong-model",
    ];
    assert_eq!(names("", cx), all);
    assert_eq!(
        names("O", cx),
        ["go", "nowhere", "offline", "model-only", "wrong-model"]
    );
    assert_eq!(names(" GO ", cx), ["go"]);
    let mut more = templates(Availability::Available);
    let nightly = LaunchTemplate {
        name: "Nightly-Build".into(),
        fields: TemplateFields::default(),
    };
    more.meta.order.push(nightly.name.clone());
    more.entries.push(KeyedEntry {
        key: nightly.name.clone(),
        value: serde_json::to_value(&nightly).unwrap(),
    });
    push(&mut helm, cx, more);
    assert_eq!(names("build", cx), ["Nightly-Build"]);

    let rows = cx.update(|_, cx| rows::rows("", cx)).expect("rows");
    let partial = TemplateFields {
        destination: Some(TemplateDestination::Folder("/srv/partial".into())),
        ..TemplateFields::default()
    };
    assert_eq!(rows[1].summary, summary::parts(&partial));
    assert_eq!((rows[1].marked, rows[1].key), (false, Some("3")));
    assert_eq!((rows[0].marked, rows[0].key), (true, None));
}

/// SPEC.md has a failed template read shown where `tl:` shows refusals, in the old words. Rows from a collection the
/// helm cannot read would be stale without saying so. Specified: with the templates unavailable ("busy"), the rows are
/// "Couldn't load templates: busy".
#[gpui::test]
fn unavailable_templates_give_the_reason_instead_of_rows(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(
        &mut helm,
        cx,
        templates(Availability::Unavailable {
            error: "busy".into(),
        }),
    );
    let reason = cx
        .update(|_, cx| rows::rows("", cx))
        .expect_err("no rows while unavailable");
    assert_eq!(
        reason.accessible_name().as_ref(),
        "Couldn't load templates: busy"
    );
}

/// SPEC.md: "Picking one opens New with its edits applied, without using the query as the session name ... Neither
/// name nor template pick launches a session: Launch remains the user's action", and an incomplete pick gets the
/// needs-more-choices banner. A pick that launched would start a session the user only meant to look at. Specified: a
/// pick of `go` opens New with its command and nothing to say, sends no create; once closed, a pick of `partial` opens
/// New with its folder and the banner, which is still there just before the banner's time is up after the template
/// applied and gone just after it, New staying open, as every quick-launch banner goes.
#[gpui::test]
fn a_switcher_pick_opens_new_with_the_template_and_never_launches(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    pick(cx, "go");
    assert!(creates(&mut helm, cx).is_empty(), "go: nothing launched");
    assert!(any_open(cx), "go: New opened");
    assert!(
        painted(&frame(cx)).iter().any(|line| line == "true"),
        "go: its command field holds `true`"
    );
    assert_eq!((banner(cx), note(cx)), (None, None), "go: saying nothing");
    assert!(!unseen(cx), "go: no launch is out");
    crate::test_support::keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Escape closed it");

    pick(cx, "partial");
    assert!(
        creates(&mut helm, cx).is_empty(),
        "partial: nothing launched"
    );
    assert!(
        text(cx).contains("/srv/partial"),
        "partial: with its folder"
    );
    assert_eq!(
        banner(cx).as_deref(),
        Some("partial needs more choices before it can launch; complete it below.")
    );
    test_support::advance(cx, BANNER_SHOWS_FOR - Duration::from_millis(100));
    assert!(
        banner(cx).is_some(),
        "partial: the banner is there just before its time"
    );
    test_support::advance(cx, Duration::from_millis(200));
    assert_eq!(banner(cx), None, "partial: and gone just after it");
    assert!(any_open(cx), "partial: New stayed open");
}

/// A pick never meant to launch, so its refusals must not say "Not launched.": they are the old switcher pick's words.
/// Telling the user a launch failed when they only asked to see a template would send them looking for a session that
/// was never going to exist. Specified, each opening New and sending no create: a pick of `gone`, a name no template
/// has, says "No template is named \"gone\"; it may have been deleted"; a pick of `nowhere`, whose host no install
/// has, gives the application's own reason, naming `nowhere`, with no "Not launched."; with the templates unavailable
/// ("busy"), a pick says "Couldn't load templates: busy; search tl: and retry".
#[gpui::test]
fn a_switcher_picks_refusals_are_the_old_switchers_words(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    pick(cx, "gone");
    assert_eq!(
        note(cx).as_deref(),
        Some("No template is named \"gone\"; it may have been deleted")
    );
    crate::test_support::keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Escape closed it");

    pick(cx, "nowhere");
    assert!(
        creates(&mut helm, cx).is_empty(),
        "nowhere: nothing launched"
    );
    let shown = note(cx).expect("nowhere: New said why");
    assert!(
        shown.contains("nowhere") && !shown.starts_with("Not launched."),
        "{shown:?}"
    );
    crate::test_support::keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Escape closed it");

    push(
        &mut helm,
        cx,
        templates(Availability::Unavailable {
            error: "busy".into(),
        }),
    );
    pick(cx, "go");
    assert_eq!(
        note(cx).as_deref(),
        Some("Couldn't load templates: busy; search tl: and retry")
    );
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
}

/// A refused pick still leaves the user in an ordinary New: its refusal is a note, not a lock, so the user can go on
/// with the launch by hand (the old quick-switcher "opening a template reports the launcher's catalog read failure",
/// whose last step typed a name). A New that took no typing after a refusal would strand the user. Specified: with the
/// templates unavailable ("busy"), a pick of `go` opens New saying "Couldn't load templates: busy; search tl: and
/// retry", and a click into its Name field and typing `by hand` put `by hand` in the draft's name.
#[gpui::test]
fn new_stays_editable_after_a_picks_refusal(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(
        &mut helm,
        cx,
        templates(Availability::Unavailable {
            error: "busy".into(),
        }),
    );
    pick(cx, "go");
    assert_eq!(
        note(cx).as_deref(),
        Some("Couldn't load templates: busy; search tl: and retry"),
        "the premise: the pick was refused"
    );
    crate::test_support::click(cx, "launcher-name");
    cx.simulate_input("by hand");
    settle(cx);
    assert_eq!(
        crate::launcher::view_tests::shown_draft(cx).name.value(),
        "by hand"
    );
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
}

/// SPEC.md: with no query, "a separate final 'Quick launch: <template>' row uses the direct quick-launch path, or opens
/// Templates to mark one". Its label is what tells the user which template the row starts. Specified: with nothing
/// marked the label is "Quick launch: Mark a template" and the action opens Templates with the ⚡ hint, sending
/// nothing; with `go` marked the label is "Quick launch: go" and the action sends one create with no layer open.
#[gpui::test]
fn the_quick_launch_row_names_the_mark_and_launches_or_opens_templates(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    let label = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            rows::quick_launch_row_label(cx)
                .accessible_name()
                .to_string()
        })
    };
    assert_eq!(label(cx), "Quick launch: Mark a template");
    cx.update(rows::quick_launch_from_switcher);
    settle(cx);
    assert!(
        creates(&mut helm, cx).is_empty(),
        "nothing marked: nothing launched"
    );
    assert!(
        text(cx).contains("Mark a template with ⚡"),
        "Templates with its hint"
    );
    crate::test_support::keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Escape closed it");

    push(&mut helm, cx, marked(Some("go")));
    assert_eq!(label(cx), "Quick launch: go");
    cx.update(rows::quick_launch_from_switcher);
    settle(cx);
    assert_eq!(creates(&mut helm, cx).len(), 1, "go launched");
    assert!(!any_open(cx), "unseen");
    assert!(unseen(cx));
}

/// The label's text while the bindings cannot be read is what the old row showed, its own copy of the preferences:
/// the last mark it held, or "Mark a template" before they arrived. The row's action is the ⚡ chord's, which refuses
/// then rather than launching the last template the store knew, or opening Templates to mark one that may be marked.
/// Specified: over a helm that pushed no preferences, the label is "Quick launch: Mark a template" and the action opens
/// New with "Not launched. Couldn't prepare quick launch: the shortcut bindings have not arrived yet".
#[gpui::test]
fn the_quick_launch_row_before_the_preferences_arrive_says_mark_a_template_and_refuses(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = window_without_preferences(cx);
    assert_eq!(label(cx), "Quick launch: Mark a template");
    cx.update(rows::quick_launch_from_switcher);
    settle(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert_eq!(
        note(cx).as_deref(),
        Some(
            "Not launched. Couldn't prepare quick launch: the shortcut bindings have not arrived yet"
        )
    );
}

/// The Quick launch row's label, as the switcher will draw it.
fn label(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| {
        rows::quick_launch_row_label(cx)
            .accessible_name()
            .to_string()
    })
}

/// The mirror of the case above, once the helm has pushed a mark and then lost its preferences: the old row went on
/// showing the mark its copy held. Specified: with ⚡ on `go` and then the preferences unavailable ("database is
/// locked"), the label is still "Quick launch: go", and the action sends no create and opens New with "Not launched.
/// Couldn't prepare quick launch: database is locked".
#[gpui::test]
fn the_quick_launch_row_keeps_the_last_mark_and_refuses_while_the_bindings_are_unreadable(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = window(cx, Some("go"));
    push(&mut helm, cx, unreadable("database is locked"));
    assert_eq!(label(cx), "Quick launch: go");
    cx.update(rows::quick_launch_from_switcher);
    settle(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert_eq!(
        note(cx).as_deref(),
        Some("Not launched. Couldn't prepare quick launch: database is locked")
    );
}

/// Pick the template named `name` in the switcher's templates section, as its handler will once the switcher has
/// closed, and settle.
fn pick(cx: &mut VisualTestContext, name: &str) {
    cx.update(|window, cx| rows::pick_from_switcher(name, window, cx));
    settle(cx);
}

/// The old spec's "a template pick waits for launcher inputs before applying the template": a template whose effect
/// depends on the model catalog (a model-only one picks the agent type that offers its model) applied before the
/// catalog arrived would be refused or applied wrong. The old switcher opened New at once and applied the template once
/// its inputs were in, and so does this. Specified: a pick of `model-only` opens New at once on its prefill, without the
/// model or a banner, while the catalog read is out; once it answers, New shows the model `gpt-x` with Codex as its
/// agent type and the needs-more-choices banner, and sends no create.
#[gpui::test]
fn a_pick_applies_its_template_once_the_catalog_is_read(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    pick(cx, "model-only");
    assert!(any_open(cx), "New opened at once");
    assert!(
        crate::launcher::view_tests::shown_draft(cx)
            .agent
            .model
            .is_none(),
        "the template waits for the catalog read"
    );
    assert_eq!(banner(cx), None, "and so does its banner");
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert_eq!(
        crate::launcher::view_tests::shown_draft(cx)
            .agent
            .model
            .as_deref(),
        Some("gpt-x")
    );
    assert!(
        test_support::label(cx, "launcher-launch")
            .is_some_and(|name| name.starts_with("Launch Codex · ")),
        "the launch action names Codex as its agent type"
    );
    assert_eq!(
        banner(cx).as_deref(),
        Some("model-only needs more choices before it can launch; complete it below.")
    );
}
