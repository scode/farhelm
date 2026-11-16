//! Tests of the template list, Cmd+J: the quick switcher in its templates-only mode, in the real main view over the
//! helm client's fake, ported from the old app's `template-list-shortcut.spec.ts` at reference commit `24bc0580`.
//!
//! The fleet is the switcher tests' sessions a, b and c, with a open, the helm's own host, and three templates in this
//! order: `redeploy` and `deploy`, which both launch on their own (commands `redeploy-cmd` and `deploy-cmd`), so that a
//! query of `deploy` finds an earlier row containing the exact name, and `partial`, which only sets a folder and so
//! needs more choices. ⚡ and number key 1 are on `deploy`, number key 8 on `partial`. The switcher's own helpers
//! (`super::tests`) read the open list and its rows.
//!
//! What a pick then does inside the dispatcher (the catalog wait, the launch, the banner's expiry) is the templates
//! area's, tested in `templates::dispatch_tests`; these stop at the create sent or New opened.

use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateDestination, TemplateFields};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, Preferences,
};
use gpui::{TestAppContext, VisualTestContext};

use super::tests::{
    Main, creates, drawn_templates, field_focused, set_query, switcher, switcher_drawn, type_query,
};
use super::{TEMPLATE_LIST_ROW_HELP, TEMPLATE_ROW_ID};
use crate::actions::{self, QUICK_LAUNCH_CHORD, SEQUENCE_PREFIX_CHORD, TEMPLATE_LIST_CHORD};
use crate::launcher::view_tests::fixture_hosts;
use crate::session_list::host_rail::tests::{hover_help, node, texts_within};
use crate::session_list::tests::{launcher_drawn, listing, painted, preferences_state, session};
use crate::templates::{QUICK_LAUNCH_SHORTCUT_NAME, number_key_name};
use crate::test_support::{
    self, any_open, click, frame, keys, label, open_main, settle, terminal_focused,
};
use crate::{layers, local_state};

/// The chord on this platform (SPEC.md, "Desktop window chrome": Cmd+J on macOS, Ctrl+Shift+J on Linux).
const CHORD: &str = TEMPLATE_LIST_CHORD;

/// The switcher's session read, which the template list must never send.
const READ: &str = "/api/sessions?sort=activity";

// ---- The fleet -----------------------------------------------------------------------------------------------------

/// A command launch of `command`, answered not YOLO, which launches on its own.
fn runs(command: &str) -> TemplateFields {
    TemplateFields {
        kind: Some(LauncherKind::Command),
        command: Some(command.into()),
        yolo: Some(false),
        ..TemplateFields::default()
    }
}

/// The templates collection of `templates`, in that order, as the helm pushes it.
fn collection(templates: &[LaunchTemplate]) -> CollectionState {
    let names: Vec<&str> = templates
        .iter()
        .map(|template| template.name.as_str())
        .collect();
    CollectionState {
        collection: Collection::Templates,
        meta: test_support::meta(&names),
        entries: templates
            .iter()
            .map(|template| KeyedEntry {
                key: template.name.clone(),
                value: serde_json::to_value(template).unwrap(),
            })
            .collect(),
    }
}

/// The module docs' templates: `redeploy`, `deploy` and `partial`.
fn templates() -> Vec<LaunchTemplate> {
    vec![
        LaunchTemplate {
            name: "redeploy".into(),
            fields: runs("redeploy-cmd"),
        },
        LaunchTemplate {
            name: "deploy".into(),
            fields: runs("deploy-cmd"),
        },
        LaunchTemplate {
            name: "partial".into(),
            fields: TemplateFields {
                destination: Some(TemplateDestination::Folder("/srv/partial".into())),
                ..TemplateFields::default()
            },
        },
    ]
}

/// The module docs' fleet: sessions a, b and c with a the last visited, the helm's own host, the templates, ⚡ and
/// number key 1 on `deploy`, number key 8 on `partial`, from a helm that has shown the Welcome card.
fn fleet() -> Vec<CollectionState> {
    vec![
        listing(
            &["a", "b", "c"],
            &[session("a"), session("b"), session("c")],
        ),
        preferences_state(Preferences {
            visit_history: vec!["a".into()],
            quick_launch_template: Some("deploy".into()),
            template_shortcuts: [("1", "deploy"), ("8", "partial")]
                .into_iter()
                .map(|(key, name)| (key.to_owned(), name.to_owned()))
                .collect(),
            welcome_seen: Some(true),
            ..Preferences::default()
        }),
        fixture_hosts(&[0]),
        collection(&templates()),
    ]
}

/// Push `templates` as the helm pushes a change to the collection: the order its meta gives, `removed` taken out.
fn push_templates(main: &mut Main<'_>, templates: &[LaunchTemplate], removed: &[&str]) {
    let state = collection(templates);
    main.helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: removed.iter().map(|name| (*name).to_owned()).collect(),
    });
    settle(main.cx);
}

impl Main<'_> {
    /// Press the template list's chord, which must open the list without sending the switcher's session read.
    fn open_list(&mut self) {
        keys(self.cx, CHORD);
        assert!(switcher_drawn(self.cx), "the premise: the list opened");
        assert_eq!(
            self.session_reads(),
            0,
            "the template list reads no sessions"
        );
    }

    /// How many session reads the window sent since the last call, every other command answered at once.
    fn session_reads(&mut self) -> usize {
        let mut reads = 0;
        for command in self.helm.commands() {
            if command.path == READ {
                reads += 1;
            } else {
                self.helm.reply(command.id, 200, None);
            }
        }
        reads
    }
}

/// Replace the open list's query with `text` as an edit, which the test's keystrokes cannot type (capitals): the field's
/// text set, then the edit the field reports for typing ([`super::QuickSwitcher::edited`]), since setting the text
/// reports none.
fn retype(cx: &mut VisualTestContext, text: &str) {
    set_query(cx, text);
    let list = switcher(cx);
    cx.update(|_, cx| list.update(cx, |list, cx| list.edited(cx)));
    settle(cx);
}

/// The key chips drawn in the template row named `name`, in order: of the texts in the row, the ones spelled as this
/// platform spells the ⚡ chord or a number key's chord (the name and summary never are, in this fleet).
fn chips(cx: &mut VisualTestContext, name: &str) -> Vec<String> {
    let chord_prefix = number_key_name("");
    texts_within(&frame(cx), &format!("{TEMPLATE_ROW_ID}{name}"))
        .into_iter()
        .map(|text| text.text.to_string())
        .filter(|text| text == QUICK_LAUNCH_SHORTCUT_NAME || text.starts_with(&chord_prefix))
        .collect()
}

/// The command of the one create the window sent since the last call.
fn launched(main: &mut Main<'_>) -> String {
    let sent = creates(main);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    let body = create.body.clone().unwrap_or_default();
    body["command"]["command"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

// ---- Opening -------------------------------------------------------------------------------------------------------

/// SPEC.md: "Cmd+J on macOS, Ctrl+Shift+J on Linux, opens a template-only list when no dialog is open. It follows the
/// quick switcher's focus, Escape, loading and modal rules", and "Rows show the template name, summary and its ⚡ or
/// number shortcut when assigned". A list that mixed in sessions or the New row would let Enter do something other
/// than start a template. Specified: from a's live, focused terminal the chord opens the dialog named "Template
/// launcher" with focus in its field (named "Search templates", showing "Start a template") and sends no session read;
/// it lists redeploy, deploy and partial under "Templates" in the helm's order, redeploy selected, each helped "Start a
/// session from this template", with no session, New or Quick launch row; deploy's row has the ⚡ chord and number key
/// 1's chord as chips, partial's number key 8's, redeploy's none; partial's summary "Destination" shows, and the hint
/// "Enter starts the highlighted template. A partial template opens New to complete it.".
#[gpui::test]
fn the_chord_opens_the_template_list_from_a_terminal(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    main.terminal_live();
    main.session_reads();

    main.open_list();
    assert!(field_focused(main.cx), "focus starts in the search field");
    let nodes = frame(main.cx);
    assert_eq!(
        node(&nodes, "quick-switcher").and_then(|card| card.label.as_deref()),
        Some("Template launcher")
    );
    assert!(
        nodes
            .iter()
            .any(|node| node.label.as_deref() == Some("Search templates")),
        "the field is named for templates"
    );
    assert_eq!(
        drawn_templates(main.cx),
        (
            vec!["redeploy".into(), "deploy".into(), "partial".into()],
            Some("redeploy".into())
        )
    );
    let shown: Vec<String> = texts_within(&nodes, "quick-switcher")
        .into_iter()
        .map(|text| text.text.to_string())
        .collect();
    for words in [
        "Start a template",
        "Templates",
        "Destination",
        "Enter starts the highlighted template. A partial template opens New to complete it.",
    ] {
        assert!(
            shown.iter().any(|line| line == words),
            "{words:?} in {shown:?}"
        );
    }
    assert!(
        !nodes.iter().any(|node| {
            node.element_id.as_deref().is_some_and(|id| {
                id.starts_with("quick-switcher-row-")
                    || id == super::NEW_ROW_ID
                    || id == super::QUICK_ROW_ID
            })
        }),
        "no session, New or Quick launch row"
    );
    assert_eq!(
        chips(main.cx, "deploy"),
        [QUICK_LAUNCH_SHORTCUT_NAME.to_owned(), number_key_name("1")]
    );
    assert_eq!(chips(main.cx, "partial"), [number_key_name("8")]);
    assert_eq!(chips(main.cx, "redeploy"), Vec::<String>::new());
    assert_eq!(
        hover_help(main.cx, "quick-switcher-template-deploy").as_deref(),
        Some(TEMPLATE_LIST_ROW_HELP)
    );
}

/// SPEC.md: the chord "does nothing while any modal is open, including New, Clone, Replace with and hidden quick-launch
/// preparation, or before preferences finish loading". The old spec's second case: a list opened over a quick launch
/// being prepared out of sight would take the keyboard from the launch about to start, or let the user start a second
/// one. A dialog is refused by the switcher's shared gate (`a_partial_template_opens_new_with_its_choices_applied`
/// checks it over New); a launcher held out of sight is the template list's own refusal. Specified: with the ⚡ chord's
/// launch of deploy waiting on its catalog read the chord opens no list, and the launch then goes ahead with one create.
#[gpui::test]
fn the_chord_does_nothing_during_a_hidden_preparation(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    main.session_reads();
    keys(main.cx, QUICK_LAUNCH_CHORD);
    // The helm answers in order, so what came before the catalog read is answered now and the rest once it is.
    let sent = main.helm.commands();
    let catalog = sent
        .iter()
        .position(|command| command.path == "/api/launch-catalog")
        .expect("the premise: the quick launch reads the catalog");
    for command in &sent[..catalog] {
        main.helm.reply(command.id, 200, None);
    }
    keys(main.cx, CHORD);
    assert!(!switcher_drawn(main.cx), "no list over the preparation");
    main.helm
        .reply(sent[catalog].id, 200, Some(serde_json::json!([])));
    for command in &sent[catalog + 1..] {
        main.helm.reply(command.id, 200, None);
    }
    settle(main.cx);
    assert_eq!(launched(&mut main), "deploy-cmd", "the launch went ahead");
}

// ---- Choosing and starting -----------------------------------------------------------------------------------------

/// The old spec's first case: "The row shown as selected must be the one Enter starts, even when another saved name is
/// listed first and both contain the typed query" (SPEC.md: "A name exactly equal to the trimmed query
/// (case-sensitively) is highlighted over an earlier substring match, and Enter starts it; arrowing to another row
/// changes that choice"). Typing a whole name is how a user starts a template without looking, so the exact name must
/// win, and only the user's arrows may take that away. It also starts from an armed sequence, as the old case did: the
/// sequence pill is not a modal, and the chord ends the sequence and still opens the list. Specified: with the prefix
/// and `2` typed, the chord opens the list and the sequence is no longer armed; `deploy` lists redeploy then deploy with
/// deploy selected; Up selects redeploy and Down deploy again; after Up, retyping the last letter selects deploy again;
/// `Deploy`, which no name equals, selects redeploy; `deploy` again selects deploy; Enter closes the list and sends one
/// create of deploy's command with no dialog open and focus back in a's terminal.
#[gpui::test]
fn an_exact_name_is_started_over_an_earlier_match_until_the_arrows_move(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    main.terminal_live();
    main.session_reads();
    keys(main.cx, &format!("{SEQUENCE_PREFIX_CHORD} 2"));
    assert!(
        main.cx.update(|_, cx| actions::key_capture_armed(cx)),
        "the premise: a sequence is armed"
    );
    main.open_list();
    assert!(
        !main.cx.update(|_, cx| actions::key_capture_armed(cx)),
        "the chord ended the sequence"
    );

    type_query(main.cx, "deploy");
    assert_eq!(
        drawn_templates(main.cx),
        (
            vec!["redeploy".into(), "deploy".into()],
            Some("deploy".into())
        )
    );
    keys(main.cx, "up");
    assert_eq!(drawn_templates(main.cx).1.as_deref(), Some("redeploy"));
    keys(main.cx, "down");
    assert_eq!(drawn_templates(main.cx).1.as_deref(), Some("deploy"));
    keys(main.cx, "up backspace y");
    assert_eq!(
        drawn_templates(main.cx).1.as_deref(),
        Some("deploy"),
        "an edit gives the exact name back its preference"
    );
    retype(main.cx, "Deploy");
    assert_eq!(
        drawn_templates(main.cx).1.as_deref(),
        Some("redeploy"),
        "the exact name is matched case-sensitively"
    );
    retype(main.cx, "deploy");
    assert_eq!(drawn_templates(main.cx).1.as_deref(), Some("deploy"));

    keys(main.cx, "enter");
    assert!(!switcher_drawn(main.cx), "the list closed");
    assert_eq!(launched(&mut main), "deploy-cmd");
    assert!(!any_open(main.cx), "the launch was unseen");
    assert!(terminal_focused(main.cx), "focus stayed in a's terminal");
}

/// The old spec's third case, its first half: "An empty result must leave Enter inert; Escape still returns to the
/// terminal", and SPEC.md's switcher rule that a press outside closes it the same way. Starting whatever happened to be
/// first would launch a template the user did not ask for. Specified: from a's focused terminal, `zzz` says "No
/// templates match" with no row selected, and Enter leaves the list open and sends no create; Escape closes it with
/// focus back in the terminal; opened again, a press on b's sidebar row, outside the card, closes it with focus back
/// in the terminal and a still the open session.
#[gpui::test]
fn with_no_match_enter_does_nothing_and_escape_or_a_press_outside_closes(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    main.terminal_live();
    main.session_reads();
    main.open_list();
    type_query(main.cx, "zzz");
    assert_eq!(drawn_templates(main.cx), (Vec::new(), None));
    assert_eq!(
        label(main.cx, "quick-switcher-templates-empty").as_deref(),
        Some("No templates match")
    );
    keys(main.cx, "enter");
    assert!(switcher_drawn(main.cx), "Enter left the list open");
    assert!(creates(&mut main).is_empty(), "and launched nothing");
    keys(main.cx, "escape");
    assert!(!any_open(main.cx));
    assert!(terminal_focused(main.cx), "Escape gave focus back");

    main.open_list();
    let card = test_support::bounds(main.cx, "quick-switcher");
    let row = test_support::bounds(main.cx, "session-row-b");
    let outside = gpui::point(row.left() + gpui::px(8.), row.center().y);
    assert!(
        row.contains(&outside) && !card.contains(&outside),
        "the premise: {outside:?} is on b's row and outside the card {card:?}"
    );
    main.cx.simulate_click(outside, gpui::Modifiers::none());
    settle(main.cx);
    assert!(!any_open(main.cx), "the press closed the list");
    assert!(terminal_focused(main.cx), "and gave focus back");
    assert_eq!(
        main.cx.update(|_, cx| local_state::selected(cx)).as_deref(),
        Some("a")
    );
}

/// The old spec's third case, its second half, and SPEC.md: "A partial template opens New with its choices applied",
/// with the needs-more-choices banner, which for a list pick does not recommend the sequence prefix (SPEC.md,
/// "Template number shortcuts": "The same banner accompanies an incomplete Cmd+J or quick-switcher pick, without
/// recommending the sequence prefix"). Launching it would only be refused. Specified: `partial` and Enter close the
/// list, send no create, and open New with focus inside it, the template's folder and the banner "partial needs more
/// choices before it can launch; complete it below."; the chord then opens no list over New.
#[gpui::test]
fn a_partial_template_opens_new_with_its_choices_applied(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    main.session_reads();
    main.open_list();
    type_query(main.cx, "partial");
    keys(main.cx, "enter");
    assert!(!switcher_drawn(main.cx), "the list closed");
    assert!(creates(&mut main).is_empty(), "nothing launched");
    assert!(launcher_drawn(main.cx), "New opened");
    assert!(
        main.cx
            .update(|window, cx| layers::focus_layer_open(window, cx)),
        "focus is inside New"
    );
    assert!(
        painted(&frame(main.cx))
            .iter()
            .any(|line| line.contains("/srv/partial"))
    );
    assert_eq!(
        label(main.cx, "launcher-banner").as_deref(),
        Some("partial needs more choices before it can launch; complete it below.")
    );
    keys(main.cx, CHORD);
    assert!(!switcher_drawn(main.cx), "no list over New");
}

/// SPEC.md: "clicking a row does the same" as Enter, and "All or a host avatar returns to Sessions, as does an accepted
/// quick-switcher pick or New handoff"; the old list's pick went through the list's New, which left the Hosts panel. A
/// launch started from the panel would otherwise leave its new session out of sight. Specified: with the Hosts panel
/// shown, a click on deploy's row (not the selected one) closes the list and sends one create of deploy's command, and
/// the list's column shows in place of the panel.
#[gpui::test]
fn a_click_starts_the_clicked_template_and_leaves_the_hosts_panel(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    main.session_reads();
    click(main.cx, "hosts-panel-toggle");
    assert!(
        node(&frame(main.cx), "hosts-panel").is_some(),
        "the premise: the Hosts panel shows"
    );
    main.open_list();
    click(main.cx, "quick-switcher-template-deploy");
    assert!(!switcher_drawn(main.cx), "the list closed");
    assert_eq!(launched(&mut main), "deploy-cmd");
    let nodes = frame(main.cx);
    assert!(node(&nodes, "hosts-panel").is_none(), "the panel went");
    assert!(
        node(&nodes, "session-list").is_some(),
        "Sessions shows again"
    );
}

/// The old spec's fourth case: "Enter must not give authority to a template captured before another window removed
/// it" (SPEC.md: "A pick uses the template currently holding that name, including a template edited or recreated since
/// the list opened; a name missing from the current catalog shows a preparation error in New"). The rows here follow
/// the helm's pushes while the list is open and a pick launches unseen, so a choice kept by row, or worked out again on
/// each frame, would start a template the user never chose. Specified, deploy's command pushed as `edited-cmd`
/// throughout:
///
/// - with deploy chosen by Down, a template pushed ahead of it leaves deploy chosen;
/// - with `deploy` typed (redeploy also listed), a push deleting deploy leaves redeploy drawn and no row selected, and
///   Down then selects redeploy;
/// - with `deploy` typed while deploy is deleted (redeploy selected), a push bringing deploy back selects it, and a
///   second push deleting it again leaves no row selected; Enter closes the list, sends no create and opens New saying
///   "Not launched. Template deploy is no longer available";
/// - with `deploy` typed while deploy is deleted, a push bringing it back selects it, and Enter launches `edited-cmd`.
#[gpui::test]
fn a_pick_uses_the_template_as_the_store_holds_it_now(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    main.session_reads();
    main.open_list();
    keys(main.cx, "down");
    assert_eq!(
        drawn_templates(main.cx).1.as_deref(),
        Some("deploy"),
        "the premise"
    );
    let mut edited = templates();
    edited[1].fields = runs("edited-cmd");
    let mut with_alpha = vec![LaunchTemplate {
        name: "alpha".into(),
        fields: runs("alpha-cmd"),
    }];
    with_alpha.extend(edited);
    push_templates(&mut main, &with_alpha, &[]);
    assert_eq!(
        drawn_templates(main.cx),
        (
            vec![
                "alpha".into(),
                "redeploy".into(),
                "deploy".into(),
                "partial".into()
            ],
            Some("deploy".into())
        ),
        "a row pushed ahead did not move the choice"
    );
    keys(main.cx, "escape");

    let mut without_deploy = with_alpha.clone();
    without_deploy.retain(|template| template.name != "deploy");
    main.open_list();
    type_query(main.cx, "deploy");
    assert_eq!(
        drawn_templates(main.cx).1.as_deref(),
        Some("deploy"),
        "the premise"
    );
    push_templates(&mut main, &without_deploy, &["deploy"]);
    assert_eq!(
        drawn_templates(main.cx),
        (vec!["redeploy".into()], None),
        "deploy's row went, and the choice did not move to redeploy"
    );
    keys(main.cx, "down");
    assert_eq!(
        drawn_templates(main.cx).1.as_deref(),
        Some("redeploy"),
        "Down from no row"
    );
    keys(main.cx, "escape");

    main.open_list();
    type_query(main.cx, "deploy");
    assert_eq!(
        drawn_templates(main.cx).1.as_deref(),
        Some("redeploy"),
        "the premise"
    );
    push_templates(&mut main, &with_alpha, &[]);
    assert_eq!(
        drawn_templates(main.cx).1.as_deref(),
        Some("deploy"),
        "the pushed exact name"
    );
    push_templates(&mut main, &without_deploy, &["deploy"]);
    assert_eq!(
        drawn_templates(main.cx),
        (vec!["redeploy".into()], None),
        "deploy stayed chosen"
    );
    keys(main.cx, "enter");
    assert!(!switcher_drawn(main.cx), "the list closed");
    assert!(creates(&mut main).is_empty(), "nothing launched");
    assert_eq!(
        label(main.cx, "launcher-note").as_deref(),
        Some("Not launched. Template deploy is no longer available")
    );
    keys(main.cx, "escape");
    assert!(!any_open(main.cx), "the premise: New closed");

    main.open_list();
    type_query(main.cx, "deploy");
    assert_eq!(
        drawn_templates(main.cx).1.as_deref(),
        Some("redeploy"),
        "the premise"
    );
    push_templates(&mut main, &with_alpha, &[]);
    assert_eq!(drawn_templates(main.cx).1.as_deref(), Some("deploy"));
    keys(main.cx, "enter");
    assert_eq!(
        launched(&mut main),
        "edited-cmd",
        "the edit is what launched"
    );
}

/// The ⚡ and number-key chips are the template list's alone: the old switcher's `tl:` section drew a template's name and
/// summary only, and its rows open New rather than starting anything. Specified: with ⚡ and number keys bound, Cmd+K
/// and `tl:` draw deploy's and partial's rows with no chip, and deploy's row keeps the section's own hover help.
#[gpui::test]
fn the_switchers_tl_rows_draw_no_key_chips(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    main.session_reads();
    keys(main.cx, crate::actions::QUICK_SWITCHER_CHORD);
    assert!(switcher_drawn(main.cx), "the premise: the switcher opened");
    set_query(main.cx, "tl:");
    assert_eq!(drawn_templates(main.cx).0.len(), 3, "the premise: the rows");
    assert_eq!(chips(main.cx, "deploy"), Vec::<String>::new());
    assert_eq!(chips(main.cx, "partial"), Vec::<String>::new());
    assert_eq!(
        hover_help(main.cx, "quick-switcher-template-deploy").as_deref(),
        Some(super::TEMPLATE_ROW_HELP)
    );
}
