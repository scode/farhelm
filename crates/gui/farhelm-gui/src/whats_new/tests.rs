//! Drive the real window-start owner with the installed app's capability supplied by a fixture.
//! The fake helm receives the same preference writes as a live helm, and the website is never contacted.

use std::cell::Cell;
use std::future::poll_fn;
use std::rc::Rc;
use std::task::Poll;

use farhelm_helm_client::Method;
use farhelm_proto::ui_channel::Preferences;
use gpui::TestAppContext;
use serde_json::json;

use crate::announcements::tests::{announcement, queue};
use crate::desktop::updates::{self, notes::*};
use crate::focus::Mark;
use crate::session_list::tests::preferences_state;
use crate::test_support::{any_open, click, focused_id, keys, label, one_command, settle};
use crate::welcome::tests::{open_start, reset, seen, shows};

const RUNNING: &str = "0.44.0";

/// A fixture with distinct releases, link and style runs keeps both the static capture and window tests
/// independent of how far the live website has been deployed.
pub(crate) fn highlights() -> Vec<ReleaseHighlights> {
    let piece = |style, text: &str| HighlightPiece {
        style,
        text: text.to_owned(),
    };
    vec![
        ReleaseHighlights {
            version: RUNNING.to_owned(),
            date: "2026-11-20".to_owned(),
            total_changes: 17,
            highlights: vec![HighlightCategory {
                name: "Added".to_owned(),
                entries: vec![Highlight {
                    pieces: vec![
                        piece(HighlightStyle::Bold, "Keyboard shortcuts"),
                        piece(
                            HighlightStyle::Plain,
                            " now appear in menus. Look for them beside actions like ",
                        ),
                        piece(HighlightStyle::Code, "Rename"),
                        piece(HighlightStyle::Plain, " in a session's menu."),
                    ],
                    screenshot_url: Some(
                        "https://farhelm.io/docs/release-notes/#v0.44.0-pr-1".to_owned(),
                    ),
                }],
            }],
        },
        ReleaseHighlights {
            version: "0.43.0".to_owned(),
            date: "2026-11-18".to_owned(),
            total_changes: 3,
            highlights: vec![HighlightCategory {
                name: "Fixes worth highlighting".to_owned(),
                entries: vec![Highlight {
                    pieces: vec![piece(
                        HighlightStyle::Plain,
                        "Terminal tabs keep their scrollback when you switch away and come back.",
                    )],
                    screenshot_url: None,
                }],
            }],
        },
    ]
}

/// Install the host before the window opens so its startup path sees the same capability as an installed app.
fn keep_host(cx: &mut TestAppContext, check: UpgradeNotesCheck) {
    cx.update(|cx| updates::keep(Some(test_host(check)), cx));
}

/// Get an installed-updater fixture with automatic updates off and the supplied highlights capability.
pub(crate) fn test_host(check: UpgradeNotesCheck) -> updates::UpdaterHost {
    let mut host = updates::fake::fake_host(&Rc::default(), &Rc::default(), false);
    host.upgrade_notes = Some(UpgradeNotesHost {
        running: RUNNING.to_owned(),
        check,
    });
    host
}

/// Welcome and Announcements get their own dismissals before highlights are even looked up. The fake updater has
/// automatic updates off. Closing highlights only advances the release baseline.
#[gpui::test]
fn welcome_and_announcements_precede_upgrade_highlights(cx: &mut TestAppContext) {
    let checks = Rc::new(Cell::new(0));
    let counted = checks.clone();
    keep_host(
        cx,
        Box::new(move |previous, show| {
            assert_eq!(previous, None);
            assert!(show);
            counted.set(counted.get() + 1);
            Box::pin(async { Ok(UpgradeNotes::Show(highlights())) })
        }),
    );
    let batch = queue(
        vec![announcement(
            "notice",
            "An announcement",
            "Read this first.",
        )],
        vec![],
    );
    let (mut helm, cx, _, _) = open_start(cx, Some(vec![seen(None), batch]));
    assert!(shows(cx, "welcome"));
    assert_eq!(checks.get(), 0, "Welcome owns the first interaction");
    click(cx, "welcome-got-it");
    let (id, body) = one_command(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(body, json!({"welcome_seen": true}));
    helm.reply(id, 200, None);
    settle(cx);

    assert!(shows(cx, "announcements"));
    assert_eq!(
        checks.get(),
        0,
        "checking also waits for the announcement batch"
    );
    click(cx, "announcements-got-it");
    let (id, body) = one_command(&mut helm, Method::Post, "/api/announcements/shown");
    assert_eq!(body, json!({"ids": ["notice"]}));
    helm.reply(id, 200, None);
    settle(cx);

    assert_eq!(checks.get(), 1);
    assert!(shows(cx, "whats-new"));
    assert_eq!(focused_id(cx).as_deref(), Some("whats-new-got-it"));
    assert_eq!(
        label(cx, "whats-new-release-0.44.0").as_deref(),
        Some("0.44.0, 2026-11-20")
    );
    assert_eq!(
        label(cx, "whats-new-category-0.44.0-0").as_deref(),
        Some("Added")
    );
    assert_eq!(
        label(cx, "whats-new-entry-0.44.0-0-0").as_deref(),
        Some(
            "Keyboard shortcuts now appear in menus. Look for them beside actions like Rename in a session's menu."
        )
    );
    assert_eq!(
        label(cx, "whats-new-footer-note").as_deref(),
        Some("20 changes in total. See the release notes for the full detail.")
    );
    assert!(
        helm.commands().is_empty(),
        "a successful read alone records nothing"
    );
    click(cx, "whats-new-got-it");
    assert!(!any_open(cx));
    let (_, body) = one_command(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(body, json!({"last_whats_new_release": RUNNING}));
}

/// A startup prompt can take focus without user input while the website responds. This test opens a usual dialog
/// directly with no key or pointer event and only then releases the lookup. Highlights must leave it in front.
#[gpui::test]
fn a_late_highlights_answer_leaves_a_newly_focused_prompt_alone(cx: &mut TestAppContext) {
    let answer_ready = Rc::new(Cell::new(false));
    let waiting = Rc::new(Cell::new(None));
    let finished = Rc::new(Cell::new(false));
    let ready = Rc::clone(&answer_ready);
    let poller = Rc::clone(&waiting);
    let completed = Rc::clone(&finished);
    keep_host(
        cx,
        Box::new(move |_, _| {
            let ready = Rc::clone(&ready);
            let poller = Rc::clone(&poller);
            let completed = Rc::clone(&completed);
            Box::pin(poll_fn(move |task| {
                if ready.get() {
                    completed.set(true);
                    Poll::Ready(Ok(UpgradeNotes::Show(highlights())))
                } else {
                    poller.set(Some(task.waker().clone()));
                    Poll::Pending
                }
            }))
        }),
    );
    let (mut helm, cx, opener, _) =
        open_start(cx, Some(vec![seen(Some(true)), queue(vec![], vec![])]));
    assert!(
        !any_open(cx) && !finished.get(),
        "premise: the lookup is still pending"
    );
    let wake = waiting
        .take()
        .expect("the lookup reached the held response");
    cx.update(|window, cx| crate::welcome::open(&opener, &Mark::take(cx), window, cx));
    settle(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("welcome-got-it"));

    answer_ready.set(true);
    wake.wake();
    settle(cx);
    assert!(finished.get(), "the highlights answer ran");
    assert!(!shows(cx, "whats-new"));
    assert_eq!(focused_id(cx).as_deref(), Some("welcome-got-it"));
    assert!(
        helm.commands().is_empty(),
        "no highlights were shown or skipped"
    );
}

/// A failed lookup leaves the baseline intact. On a later connection the same check succeeds. Escape, the header
/// Close and Don't show again each close and record, with only the last also changing the setting.
#[gpui::test]
fn a_failed_check_can_retry_and_all_dismissals_record_the_release(cx: &mut TestAppContext) {
    let checks = Cell::new(0);
    keep_host(
        cx,
        Box::new(move |previous, show| {
            assert_eq!(previous.as_deref(), Some("0.42.0"));
            assert!(show);
            let first = checks.replace(checks.get() + 1) == 0;
            Box::pin(async move {
                if first {
                    anyhow::bail!("website is temporarily unavailable");
                }
                Ok(UpgradeNotes::Show(highlights()))
            })
        }),
    );
    let snapshot = vec![
        preferences_state(Preferences {
            welcome_seen: Some(true),
            last_whats_new_release: Some("0.42.0".to_owned()),
            ..Preferences::default()
        }),
        queue(vec![], vec![]),
    ];
    let (mut helm, cx, _, _) = open_start(cx, Some(snapshot.clone()));
    assert!(!any_open(cx));
    assert!(
        helm.commands().is_empty(),
        "a failed read cannot advance the record"
    );

    for way in ["escape", "whats-new-close", "whats-new-opt-out"] {
        reset(&mut helm, cx, snapshot.clone());
        assert!(shows(cx, "whats-new"));
        assert_eq!(focused_id(cx).as_deref(), Some("whats-new-got-it"));
        if way == "escape" {
            keys(cx, way);
        } else {
            click(cx, way);
        }
        assert!(!any_open(cx));
        let (id, body) = one_command(&mut helm, Method::Put, "/api/preferences");
        let mut expected = json!({"last_whats_new_release": RUNNING});
        if way == "whats-new-opt-out" {
            expected["show_whats_new"] = json!(false);
        }
        assert_eq!(body, expected, "{way}");
        helm.reply(id, 200, None);
        settle(cx);
    }
}
