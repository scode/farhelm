//! Drive search through the actual Settings input and layer. The fake helm can leave a read pending on purpose to
//! distinguish a focus move caused by a result pick from one that steals focus when the read eventually answers.

use farhelm_helm_client::Method;
use farhelm_input_mapping::Platform;
use farhelm_proto::LaunchHarness;
use farhelm_proto::ui_channel::Preferences;
use gpui::{TestAppContext, VisualTestContext, px, size};
use serde_json::json;

use super::{Entry, Target, catalog, fold, highlights};
use crate::actions::THIS_PLATFORM;
use crate::layers;
use crate::settings::tests::{
    click_section, drawn, open_rail, open_settings, section_button, sent, show_checkouts, wheel,
};
use crate::settings::{Choice, Section, SettingsDialog, decide_tunnels_for_opened};
use crate::test_support::{
    any_open, click, focused_id, frame, keys, label, press, select_all_text, settle,
};

/// Focus search through the Settings-only binding. A simulated edit could otherwise reach a different control.
fn focus_search(cx: &mut VisualTestContext) {
    keys(
        cx,
        match THIS_PLATFORM {
            Platform::Mac => "cmd-f",
            Platform::Linux => "ctrl-f",
        },
    );
    assert!(search_focused(cx), "the binding focused the Settings field");
}

fn search_focused(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| {
        let dialog = layers::last_opened::<SettingsDialog>(cx).expect("Settings is open");
        dialog
            .read(cx)
            .search
            .input
            .focus_handle(cx)
            .is_focused(window)
    })
}

fn type_query(cx: &mut VisualTestContext, query: &str) {
    assert!(search_focused(cx), "the edit reaches the search field");
    select_all_text(cx);
    cx.simulate_input(query);
    settle(cx);
    assert!(drawn(cx, "settings-search-heading"));
}

fn heading(cx: &mut VisualTestContext) -> Option<String> {
    label(cx, "settings-heading")
}

fn result_ids(cx: &mut VisualTestContext) -> Vec<String> {
    frame(cx)
        .into_iter()
        .filter_map(|node| node.element_id)
        .filter(|id| id.starts_with("settings-result-"))
        .map(|id| id.to_string())
        .collect()
}

fn selected(cx: &mut VisualTestContext) -> Option<String> {
    frame(cx)
        .into_iter()
        .filter(|node| node.selected == Some(true))
        .filter_map(|node| node.element_id)
        .find(|id| id.starts_with("settings-result-"))
        .map(|id| id.to_string())
}

/// A mixed-case query can find separate words in a section name and a setting's label or help. Exact setting order
/// still matters when a section name matches the whole section.
#[test]
fn words_match_across_fields_and_sections_keep_their_order() {
    let catalog = catalog(Platform::Linux);
    let all_hosts: Vec<_> = catalog
        .iter()
        .filter(|entry| entry.matches(&[fold("HoStS")]))
        .filter(|entry| entry.section == Section::Hosts)
        .map(|entry| entry.target)
        .collect();
    assert_eq!(
        all_hosts,
        [
            Target::Choice(Choice::AutoUpdateRemoteHosts),
            Target::Choice(Choice::SkipHostSetupConfirmation),
            Target::Choice(Choice::SkipHostRemoveConfirmation),
        ]
    );
    let words = |query: &str| query.split_whitespace().map(fold).collect::<Vec<_>>();
    let update = catalog
        .iter()
        .find(|entry| entry.target == Target::Choice(Choice::AutoUpdateRemoteHosts))
        .unwrap();
    assert!(update.matches(&words("HOSTS UPgraDE")));
    assert!(!update.matches(&words("HOSTS UPgraDE imaginary")));
    assert!(
        catalog
            .iter()
            .any(|entry| entry.section == Section::App && entry.matches(&words("upgrade")))
    );
}

/// Mixed-length UTF-8 case conversions and overlapping hits must still produce sorted source ranges that GPUI can
/// paint. A match on the leading expansion of dotted I colors the original character as one unit.
#[test]
fn highlights_use_original_character_boundaries() {
    assert_eq!(
        highlights("İ Āanana", &[fold("i"), fold("āa"), fold("ana")]),
        vec![0..2, 3..10]
    );
}

/// Words from standalone help and option labels must be searchable even when the section never loaded.
#[test]
fn authored_help_and_radio_options_lead_to_their_settings() {
    let entries = catalog(Platform::Linux);
    for (query, section, id) in [
        (
            "one step",
            Section::Agent(LaunchHarness::Claude),
            super::agent_trust::CAUTION_ID,
        ),
        (
            "agent trust",
            Section::Agent(LaunchHarness::Claude),
            super::agent_trust::CAUTION_ID,
        ),
        (
            "agent trust",
            Section::Agent(LaunchHarness::Antigravity),
            super::TrustAgent::Antigravity.id(),
        ),
        (
            "agent trust",
            Section::Agent(LaunchHarness::Codex),
            Choice::ShowCodexTrustHint.id(),
        ),
        ("stays quiet", Section::Notifications, super::SOUNDS_ID),
        (
            "grok conversation hooks",
            Section::Agent(LaunchHarness::Grok),
            "settings-grok-hook-setup",
        ),
        (
            "antigravity conversation hooks",
            Section::Agent(LaunchHarness::Antigravity),
            "settings-antigravity-hook-setup",
        ),
        ("dashboard", Section::Credentials, super::tensorlake_key::ID),
        ("dashboard", Section::Tensorlake, super::tensorlake_key::ID),
        ("urgent", Section::Notifications, "ntfy-priority"),
    ] {
        let words: Vec<_> = query.split_whitespace().map(fold).collect();
        assert!(
            entries.iter().any(|entry| {
                entry.section == section && entry.id == id && entry.matches(&words)
            }),
            "{query:?} must find {section:?} {id}"
        );
    }
}

/// Search offers only what this build shows: with no updater, App's settings are not results (SPEC.md lists App only
/// in a desktop app whose updater is active). Enter goes directly to the switch without toggling it, and nothing
/// is read to search.
#[gpui::test]
fn search_skips_hidden_sections_and_enters_the_selected_switch(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(Choice::SkipSessionConfirmations.id())
    );
    assert!(!drawn(cx, section_button(Section::App)));
    focus_search(cx);
    type_query(cx, "UPGRADE");
    let ids = result_ids(cx);
    assert_eq!(
        ids,
        [format!(
            "settings-result-hosts-{}",
            Choice::AutoUpdateRemoteHosts.id()
        )],
        "App's update settings are not offered without an updater",
    );
    assert!(sent(&mut helm).is_empty(), "search sends no reads");
    assert_eq!(selected(cx).as_ref(), Some(&ids[0]));
    keys(cx, "enter");
    assert_eq!(heading(cx).as_deref(), Some("Hosts"));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(Choice::AutoUpdateRemoteHosts.id())
    );
    assert!(
        sent(&mut helm).is_empty(),
        "opening a switch does not toggle it"
    );
}

/// A click that replaces the old section can leave focus on the layer itself. The shortcut has to be on that
/// layer's key path as well as on the paths of its controls.
#[gpui::test]
fn the_search_shortcut_reaches_the_field_from_dialog_fallback_focus(cx: &mut TestAppContext) {
    use crate::focus::{self, Mark};

    let (_, cx, _) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let fallback = cx.update(|window, cx| {
        let container = layers::top_container(window, cx).expect("Settings is open");
        focus::focus_in_layer(&container, &Mark::take(cx), window, cx);
        container
    });
    settle(cx);
    assert!(cx.update(|window, _| fallback.is_focused(window)));
    focus_search(cx);
}

/// The keyboard selection must remain visible through several groups of results while the search field stays put.
/// This catches a result list that simply extends the body or counts group headings as selectable rows.
#[gpui::test]
fn moving_through_many_results_scrolls_the_selection_into_view(cx: &mut TestAppContext) {
    let (_, cx, _) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    focus_search(cx);
    type_query(cx, "a");
    let ids = result_ids(cx);
    let last = ids.last().expect("the query has matches");
    let row = |cx: &mut VisualTestContext| {
        frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(last.as_str()))
            .expect("the last row is laid out")
    };
    let before = row(cx);
    assert_ne!(
        before.bounds, before.visible_bounds,
        "the last row starts below the fold"
    );
    for _ in 1..ids.len() {
        keys(cx, "down");
    }
    let after = row(cx);
    assert_eq!(after.selected, Some(true));
    assert_eq!(
        after.bounds, after.visible_bounds,
        "keyboard selection revealed the last row"
    );
    assert!(search_focused(cx));
}

/// Escape clears the focused search without changing the prior section. Header and footer Close bypass that field
/// handler, so either dismisses a search with text in one press.
#[gpui::test]
fn escape_restores_the_prior_section_and_close_controls_close(cx: &mut TestAppContext) {
    let (_helm, cx, _) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, section_button(Section::Appearance));
    assert_eq!(heading(cx).as_deref(), Some("Appearance"));
    click(cx, "settings-search-field");
    type_query(cx, "wallpaper");
    assert!(result_ids(cx).is_empty());
    keys(cx, "escape");
    assert_eq!(heading(cx).as_deref(), Some("Appearance"));
    assert!(search_focused(cx));
    press(cx, "escape", true);
    assert!(any_open(cx), "one physical Escape only clears the search");
    keys(cx, "escape");
    assert!(!any_open(cx));
    for close in ["settings-close", "settings-footer-close"] {
        open_settings(cx);
        focus_search(cx);
        type_query(cx, "dark mode");
        assert_eq!(result_ids(cx).len(), 1);
        click(cx, close);
        assert!(
            !any_open(cx),
            "{close} dismisses the dialog even during search"
        );
    }
}

/// Clearing a query returns to the same place in the page. Opening a top-of-page result for that same section
/// instead reveals the section content from the top, even though the search field is outside its scroll area.
#[gpui::test]
fn a_top_result_reveals_the_start_of_the_current_scrolled_page(cx: &mut TestAppContext) {
    let (_, cx, _) = open_rail(cx, &Preferences::default());
    cx.simulate_resize(size(px(1000.), px(560.)));
    settle(cx);
    open_settings(cx);
    click(cx, section_button(Section::Notifications));
    let top = |cx: &mut VisualTestContext| {
        frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(super::SOUNDS_ID))
            .expect("the Sounds group is laid out")
            .bounds
            .top()
    };
    let start = top(cx);
    wheel(cx, -5000.);
    let scrolled = top(cx);
    assert!(
        scrolled < start,
        "the Sounds group scrolled off its starting position"
    );

    focus_search(cx);
    type_query(cx, "sounds");
    keys(cx, "escape");
    assert_eq!(top(cx), scrolled);

    type_query(cx, "sounds");
    let sounds = "settings-result-notifications-settings-sounds";
    assert!(result_ids(cx).iter().any(|id| id == sounds));
    click(cx, sounds);
    assert_eq!(top(cx), start);
}

/// Filtering the navigation changes only which section buttons are listed, and a section this build hides is never
/// one of them. Clearing the query restores the old list.
#[gpui::test]
fn settings_search_narrows_sections_and_reports_their_match_counts(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let sections = |cx: &mut VisualTestContext| {
        frame(cx)
            .into_iter()
            .filter(|node| {
                node.element_id
                    .as_ref()
                    .is_some_and(|id| id.starts_with("settings-section-"))
            })
            .filter_map(|node| node.label.map(|label| label.to_string()))
            .collect::<Vec<_>>()
    };
    let initial = sections(cx);
    assert!(initial.iter().any(|name| name == "Hosts"));
    assert!(
        !initial.iter().any(|name| name == "App"),
        "there is no updater"
    );
    focus_search(cx);
    type_query(cx, "upgrade");
    assert_eq!(sections(cx), ["Hosts (1 match)"], "no updater, so no App");
    assert_eq!(result_ids(cx).len(), 1);
    assert!(
        sent(&mut helm).is_empty(),
        "displaying counts does not read their sections"
    );
    type_query(cx, "wallpaper");
    assert!(sections(cx).is_empty());
    assert!(result_ids(cx).is_empty());
    keys(cx, "escape");
    assert_eq!(sections(cx), initial);
    assert_eq!(heading(cx).as_deref(), Some("Sessions"));
}

/// The tunnel switch is drawn only on the Mac, so a Linux build's search must not offer it.
#[gpui::test]
fn the_tunnel_switch_is_searchable_only_where_it_is_drawn(_cx: &mut TestAppContext) {
    let tunnels = |platform| {
        super::catalog(platform)
            .iter()
            .any(|entry| entry.target == super::Target::Tunnels)
    };
    assert!(tunnels(Platform::Mac));
    assert!(!tunnels(Platform::Linux));
}

/// A fresh Enter opens the post-clone command and a held repeat still belongs to search. In an already-loaded
/// growing field, a repeat incorrectly handed to the new focus would insert a newline into the saved draft.
#[gpui::test]
fn repeated_search_enter_leaves_the_post_clone_command_untouched(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_rail(cx, &Preferences::default());
    show_checkouts(&mut helm, cx, json!(null), json!("echo fixture"), true);
    assert_eq!(heading(cx).as_deref(), Some("Checkouts"));
    let field = cx.update(|_, cx| {
        let dialog = layers::last_opened::<SettingsDialog>(cx).expect("Settings is open");
        dialog
            .read(cx)
            .checkout_fields
            .as_ref()
            .unwrap()
            .post_clone
            .clone()
    });
    assert_eq!(cx.update(|_, cx| field.read(cx).text(cx)), "echo fixture");
    click(cx, "settings-search-field");
    type_query(cx, "post-clone command");
    assert_eq!(
        result_ids(cx),
        ["settings-result-checkouts-checkout-post-clone"]
    );
    keys(cx, "enter");
    assert!(cx.update(|window, cx| field.read(cx).focused(window, cx)));
    press(cx, "enter", true);
    assert_eq!(cx.update(|_, cx| field.read(cx).text(cx)), "echo fixture");
}

/// Clicking a setting that is locked until its read answers first shows its section. A timely answer hands focus to
/// the newly available switch. A key the user presses in the meantime cancels that handover.
#[gpui::test]
fn ntfy_search_hands_focus_over_only_if_the_user_has_not_moved_on(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_rail(cx, &Preferences::default());
    for moved_on in [false, true] {
        open_settings(cx);
        focus_search(cx);
        type_query(cx, "send ntfy");
        assert_eq!(
            result_ids(cx),
            ["settings-result-notifications-ntfy-enabled"]
        );
        click(cx, "settings-result-notifications-ntfy-enabled");
        assert_eq!(heading(cx).as_deref(), Some("Notifications"));
        assert!(
            search_focused(cx),
            "while the switch is locked, focus stays on search"
        );
        let reads = sent(&mut helm);
        assert_eq!(reads.len(), 1);
        assert_eq!(
            (reads[0].method, reads[0].path.as_str()),
            (Method::Get, "/api/ntfy-alerts")
        );
        if moved_on {
            keys(cx, "tab");
            assert!(!search_focused(cx), "Tab moved focus away from search");
        }
        let before = focused_id(cx);
        helm.reply(
            reads[0].id,
            200,
            Some(json!(farhelm_proto::ntfy::Settings::default())),
        );
        settle(cx);
        if moved_on {
            assert_eq!(focused_id(cx), before);
        } else {
            assert_eq!(focused_id(cx).as_deref(), Some("ntfy-enabled"));
        }
        click(cx, "settings-footer-close");
        assert!(!any_open(cx));
    }
}

/// Every catalog entry names a control or group in its section. Use a Mac fixture with an updater that offers upgrade
/// highlights and both kinds of file warning so the conditional entries have something drawn. A saved Tensorlake key
/// draws the sandbox list.
#[gpui::test]
fn every_catalog_entry_corresponds_to_something_drawn(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_rail(
        cx,
        &Preferences {
            open_extensions: Some("html, command".into()),
            ..Preferences::default()
        },
    );
    cx.update(|_, cx| {
        use crate::desktop::updates::fake::{Feed, Seen, fake_host};
        use crate::desktop::updates::notes::{UpgradeNotes, UpgradeNotesHost};
        let (feed, seen) = (
            std::rc::Rc::new(Feed::default()),
            std::rc::Rc::new(Seen::default()),
        );
        let mut host = fake_host(&feed, &seen, false);
        // The Show what's new switch is drawn only for an app that can show upgrade highlights.
        host.upgrade_notes = Some(UpgradeNotesHost {
            running: "1.0.0".to_owned(),
            check: Box::new(|_, _| Box::pin(async { Ok(UpgradeNotes::Current) })),
        });
        crate::desktop::updates::keep(Some(host), cx);
    });
    settle(cx);
    open_settings(cx);
    cx.update(|_, cx| decide_tunnels_for_opened(Platform::Mac, cx));
    settle(cx);
    let entries = catalog(Platform::Mac);
    for section in Section::all() {
        click_section(cx, section);
        assert_eq!(
            heading(cx).as_deref(),
            Some(section.label()),
            "opened {section:?}"
        );
        for read in sent(&mut helm) {
            if read.path == "/api/tensorlake" {
                helm.reply(
                    read.id,
                    200,
                    Some(json!({
                        "api_key_set": true, "create_without_asking": false,
                        "helm_tag": "fixture", "ssh_key": { "state": "not_created" },
                    })),
                );
            } else {
                // Static catalog targets are present through a refusal. Answer in wire order so a section visited
                // before Tensorlake cannot leave an earlier read ahead of the account fixture.
                helm.reply(
                    read.id,
                    503,
                    Some(json!("not available in the catalog fixture")),
                );
            }
        }
        settle(cx);
        let observed = frame(cx);
        let drawn: Vec<_> = entries
            .iter()
            .filter(|entry| entry.section == section)
            .map(|Entry { id, .. }| {
                observed
                    .iter()
                    .position(|node| node.element_id.as_deref() == Some(*id))
                    .unwrap_or_else(|| panic!("{section:?} did not draw the catalog target {id}"))
            })
            .collect();
        assert!(
            drawn.is_sorted(),
            "{section:?} results must follow the drawing order"
        );
    }
}
