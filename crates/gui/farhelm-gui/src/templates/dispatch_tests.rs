//! Tests of the quick-launch dispatcher: resolving references and choosing the incomplete banner (pure), and, in the
//! real main view over the helm client's fake, what the ⚡ chord, the number keys and a sequence's Enter do with what
//! they name.
//!
//! The window tests start with no sessions, so New's prefill is the helm's own host (host1, connected, install
//! `identity1`) in `~`, and with these templates: `go` launches on its own (a command, YOLO answered no), `partial`
//! only sets a folder, `nowhere` names a host no install has, `offline` would launch `offline-cmd` on host2, which is
//! still connecting, `model-only` sets only the model `gpt-x`, which the catalog lists for Codex alone, and `wrong-model`
//! asks Claude for that Codex model. Which ones hold ⚡ and the number keys is each test's choice. A create the window
//! sends is read off the fake helm's commands; every other command is answered at once, the catalog read with
//! [`catalog`]. What the launcher says is read from its banner's and refusal alert's accessible text, which is the whole
//! sentence.

use std::collections::BTreeMap;
use std::time::Duration;

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateDestination, TemplateFields};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, HostView,
    KeyedEntry, Preferences,
};
use farhelm_proto::{LaunchHarness, LaunchTarget, Place};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::SEQUENCE_PREFIX_NAME;
use super::dispatch::{Origin, Reference, incomplete_banner, resolve};
use super::save_tests::panel_name_focused;
use super::sequence::{self, Binding, Refusal};
use super::shortcuts::Bindings;
use super::summary::Part;
use crate::actions::{QUICK_LAUNCH_CHORD, QUICK_SWITCHER_CHORD, SEQUENCE_PREFIX_CHORD};
use crate::launcher::environment_launch_tests::{self, answer_except, answer_with};
use crate::launcher::environment_tests::{account, built_json, ready_key};
use crate::launcher::launch_tests::{creates_answering, creates_answering_with, is_status};
use crate::launcher::request::container_reason;
use crate::launcher::view::BANNER_SHOWS_FOR;
use crate::launcher::view_tests::fixture_hosts;
use crate::launcher::yolo_tests::refuse_for_yolo;
use crate::launcher::{self, launch};
use crate::main_view::MainView;
use crate::notices::{self, Notice};
use crate::session_list::host_rail::tests::node;
use crate::session_list::tests::{listing, painted, preferences_state, session};
use crate::test_support::{self, any_open, focused_id, frame, keys, label, meta, settle, started};
use crate::{focus, local_state};

// ---- Resolving and the banner ----------------------------------------------------------------------------------------

/// A template named `name` with `fields`.
fn template(name: &str, fields: TemplateFields) -> LaunchTemplate {
    LaunchTemplate {
        name: name.into(),
        fields,
    }
}

/// Names of what [`resolve`] found, or why it found nothing.
fn names(
    references: &[Reference],
    bindings: &Bindings,
    templates: &[LaunchTemplate],
) -> Result<Vec<String>, Refusal> {
    resolve(references, bindings, templates)
        .map(|found| found.into_iter().map(|template| template.name).collect())
}

/// Which refusal wins decides whether the user lands in Templates or in New, and SPEC.md ties "no usable mark" to
/// Templates: a mark that names a deleted template must not launch nothing silently nor stand in for another one.
/// Specified: ⚡, a key and an exact name resolve in order, repeats kept; a ⚡ or key bound to nothing, or to a name no
/// template has, is refused as that unbound reference, with its digit, the first in order winning, named alone only
/// when it was the whole batch; a missing exact name wins over any of them, as the old dispatcher's missing-name check
/// did; names match exactly, never by case.
#[test]
fn references_resolve_in_order_and_unusable_ones_choose_the_fallback() {
    let templates = [
        template("one", TemplateFields::default()),
        template("two", TemplateFields::default()),
    ];
    let bindings = Bindings {
        marked: Some("two".into()),
        keys: BTreeMap::from([
            ("1".to_owned(), "one".to_owned()),
            ("2".to_owned(), "gone".to_owned()),
        ]),
    };
    let mark = Reference::Mark;
    let key = |digit: &str| Reference::Key(digit.into());
    let name = |name: &str| Reference::Name(name.into());
    assert_eq!(
        names(
            &[key("1"), mark.clone(), name("one")],
            &bindings,
            &templates
        ),
        Ok(vec!["one".into(), "two".into(), "one".into()])
    );
    assert_eq!(
        names(&[key("2"), mark.clone()], &bindings, &templates),
        Err(Refusal::Unbound(Binding {
            reference: sequence::Reference::Key("2".into()),
            alone: false,
        })),
        "a key bound to a deleted template is unusable"
    );
    let unmarked = Bindings {
        marked: None,
        ..bindings.clone()
    };
    assert_eq!(
        names(&[mark.clone(), key("9")], &unmarked, &templates),
        Err(Refusal::Unbound(Binding {
            reference: sequence::Reference::Mark,
            alone: false,
        })),
        "the first unusable binding is the one reported"
    );
    assert_eq!(
        names(&[key("9"), name("gone")], &bindings, &templates),
        Err(Refusal::Missing("gone".into())),
        "a missing name wins over an unusable binding"
    );
    assert_eq!(
        names(&[name("ONE")], &bindings, &templates),
        Err(Refusal::Missing("ONE".into())),
        "an exact name matches by case"
    );
    assert_eq!(
        names(&[key("9")], &bindings, &templates),
        Err(Refusal::Unbound(Binding {
            reference: sequence::Reference::Key("9".into()),
            alone: true,
        })),
        "a batch of one unbound key is named alone"
    );
}

/// The banner explains the gesture the user made, and the wrong advice would send them in circles: telling a sequence
/// to use the prefix it already used hides that the combined setup is what lacks a choice. Specified, in the old app's
/// words: a number key names its template and the prefix as this platform spells it; ⚡, Cmd+J and the switcher name
/// the template as needing more choices; a sequence speaks of the whole setup. Template names are host parts.
#[test]
fn the_incomplete_banner_follows_the_origin() {
    let templates = [template("model-only", TemplateFields::default())];
    let host = Part::Host("model-only".into());
    assert_eq!(
        incomplete_banner(Origin::NumberKey, &templates),
        vec![
            host.clone(),
            Part::Words(format!(
                " cannot launch on its own. Use {} to combine template keys, then Enter to launch, or complete this \
                 setup below.",
                super::SEQUENCE_PREFIX_NAME
            )),
        ]
    );
    for origin in [Origin::Mark, Origin::TemplateList, Origin::Switcher] {
        assert_eq!(
            incomplete_banner(origin, &templates),
            vec![
                host.clone(),
                Part::Words(" needs more choices before it can launch; complete it below.".into()),
            ],
            "{origin:?}"
        );
    }
    assert_eq!(
        incomplete_banner(
            Origin::Sequence,
            &[templates[0].clone(), templates[0].clone()]
        ),
        vec![Part::Words(
            "This sequence doesn't make a complete setup; complete it below.".into()
        )]
    );
}

// ---- The window ------------------------------------------------------------------------------------------------------

/// The model catalog the helm answers with: `gpt-x`, a Codex model and no other agent type's.
fn catalog() -> serde_json::Value {
    json!([{ "id": "gpt-x", "harness": "codex", "efforts": [] }])
}

/// A command launch of `true` answered not YOLO, which launches on its own.
fn runs_true() -> TemplateFields {
    TemplateFields {
        kind: Some(LauncherKind::Command),
        command: Some("true".into()),
        yolo: Some(false),
        ..TemplateFields::default()
    }
}

/// The templates collection holding the module docs' templates, available or not.
pub(super) fn templates(availability: Availability) -> CollectionState {
    let all = [
        template("go", runs_true()),
        template(
            "partial",
            TemplateFields {
                destination: Some(TemplateDestination::Folder("/srv/partial".into())),
                ..TemplateFields::default()
            },
        ),
        template(
            "nowhere",
            TemplateFields {
                host: Some("no-such-install".into()),
                ..runs_true()
            },
        ),
        template(
            "offline",
            TemplateFields {
                host: Some("identity2".into()),
                command: Some("offline-cmd".into()),
                ..runs_true()
            },
        ),
        template(
            "model-only",
            TemplateFields {
                model: Some(Some("gpt-x".into())),
                ..TemplateFields::default()
            },
        ),
        template(
            "wrong-model",
            TemplateFields {
                agent: Some(LaunchHarness::Claude),
                model: Some(Some("gpt-x".into())),
                ..TemplateFields::default()
            },
        ),
    ];
    let keys: Vec<&str> = all.iter().map(|template| template.name.as_str()).collect();
    CollectionState {
        collection: Collection::Templates,
        meta: CollectionMeta {
            availability,
            ..meta(&keys)
        },
        entries: all
            .iter()
            .map(|template| KeyedEntry {
                key: template.name.clone(),
                value: serde_json::to_value(template).unwrap(),
            })
            .collect(),
    }
}

/// The preferences collection with ⚡ on `marked`, if anything, and no number keys, from a helm that has shown the
/// Welcome card.
pub(super) fn marked(marked: Option<&str>) -> CollectionState {
    bound(marked, &[])
}

/// The preferences collection with ⚡ on `marked`, if anything, and number keys `keys` (each a digit and the template
/// name it holds), from a helm that has shown the Welcome card.
fn bound(marked: Option<&str>, keys: &[(&str, &str)]) -> CollectionState {
    preferences_state(Preferences {
        quick_launch_template: marked.map(str::to_owned),
        template_shortcuts: keys
            .iter()
            .map(|(key, name)| ((*key).to_owned(), (*name).to_owned()))
            .collect(),
        // A helm that has shown the Welcome card, which would otherwise open over the main view (`crate::welcome`).
        welcome_seen: Some(true),
        ..Preferences::default()
    })
}

/// The real main view over the fake helm: no sessions, [`hosts`], the templates and ⚡ on `mark`.
pub(super) fn window<'a>(
    cx: &'a mut TestAppContext,
    mark: Option<&str>,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![
        listing(&[], &[]),
        // host1, the helm's own (connected, `identity1`), and host2 (ssh, connecting, `identity2`).
        fixture_hosts(&[0, 1]),
        marked(mark),
        templates(Availability::Available),
    ]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    settle(window);
    (helm, window)
}

/// `state` pushed as the helm pushes a change.
pub(super) fn push(helm: &mut FakeHelm, cx: &mut VisualTestContext, state: CollectionState) {
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
}

/// The creates the window sent since the last call, every other command answered at once, the catalog read with
/// [`catalog`], round after round (the launcher's own helper).
pub(super) fn creates(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    creates_answering(helm, cx, &catalog())
}

/// Press the ⚡ chord once.
pub(super) fn chord(cx: &mut VisualTestContext) {
    keys(cx, QUICK_LAUNCH_CHORD);
}

/// Press number key `digit`'s chord once, as this platform spells it (Cmd+digit, Ctrl+Shift+digit). The keystroke
/// carries no native identity, so the physical-key table matches it by its logical key, as it does every injected key
/// (`crate::actions`).
pub(super) fn number_key(cx: &mut VisualTestContext, digit: u8) {
    let chord = if cfg!(target_os = "macos") {
        format!("cmd-{digit}")
    } else {
        format!("ctrl-shift-{digit}")
    };
    keys(cx, &chord);
}

/// Every line of text the frame painted, joined with single spaces.
pub(super) fn text(cx: &mut VisualTestContext) -> String {
    painted(&frame(cx)).join(" ")
}

/// The launcher's banner, by its accessible text, if one is drawn.
pub(super) fn banner(cx: &mut VisualTestContext) -> Option<String> {
    label(cx, "launcher-banner")
}

/// The launcher's refusal alert (a press's reason, the helm's words), by its accessible text, if one is drawn.
fn refusal(cx: &mut VisualTestContext) -> Option<String> {
    label(cx, "launcher-refusal")
}

/// The launcher's refusal note (a quick launch's refusal on New, a shortcut's in the open launcher), drawn as its own
/// alert above the refusal alert, by its accessible text, if one is drawn.
pub(super) fn note(cx: &mut VisualTestContext) -> Option<String> {
    label(cx, "launcher-note")
}

/// Whether the launch whose create is out is still a quick launch New never showed.
pub(super) fn unseen(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| launch::pending_unseen_quick_launch(cx))
}

/// Open New, as its button would, and answer its catalog read, checking that it opened with nothing to say.
fn open_new(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    assert!(
        cx.update(launcher::entry::open_new),
        "the premise: New opened"
    );
    assert!(creates(helm, cx).is_empty(), "opening New sends no create");
    assert_eq!(
        (banner(cx), note(cx), refusal(cx)),
        (None, None, None),
        "the premise: New says nothing"
    );
}

// ---- The page ----

/// The point of quick launch is a session without the launcher: SPEC.md, "starts it directly in the desktop app when no
/// dialog is open, without showing the launcher on success", through "exactly New's defaults and ordinary launch path".
/// The "never shown" mark must hold until the launch settles, since naming the session turns on it. Specified: with
/// `go` marked, the chord sends one create of New's prefill (host1, `~`) with the template's command, opens no layer
/// and leaves the launch marked as a quick launch never shown; a press while the create is out sends nothing more, and
/// the sequence prefix arms nothing then (the old app refused it while a launch was pending); a create still out two
/// seconds later shows the launcher's progress pill, as any hidden launch does (SPEC.md: "Once the request has waited
/// about two seconds, a small pill ... shows"), since a quick launch is nothing but a launch whose launcher was never
/// drawn; once the helm lists the new session and answers, the pill goes, and since the presses made while the create
/// was out count as moving on (SPEC.md, "Creation": a launch opens its session only if the user "has pressed no key or
/// pointer button anywhere in the page since their last answer to it"), nothing is selected and the "Started" report
/// names the session. A quick launch the user waited for opening its session is
/// `a_taken_base_name_is_numbered_and_launched_out_of_sight`.
#[gpui::test]
fn the_chord_launches_the_marked_template_unseen(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    chord(cx);
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one press sent one create, not {}", sent.len());
    };
    let body = create.body.clone().unwrap_or_default();
    assert_eq!(body["host"], json!(1), "{body}");
    assert_eq!(body["cwd"], json!("~"), "{body}");
    assert_eq!(
        body["command"]["command"],
        json!("true"),
        "the template's command"
    );
    assert!(!any_open(cx), "the launcher was never shown");
    assert!(
        unseen(cx),
        "the launch is marked as a quick launch New never showed"
    );

    chord(cx);
    assert!(
        creates(&mut helm, cx).is_empty(),
        "a press while the create is out sent nothing"
    );
    assert!(unseen(cx), "and left the mark alone");
    keys(cx, SEQUENCE_PREFIX_CHORD);
    assert!(
        !cx.update(|_, cx| super::armed::is_armed(cx)),
        "the prefix arms no sequence while the create is out"
    );
    test_support::advance(cx, Duration::from_secs(2));
    let progress = label(cx, "launch-progress").expect("the progress pill shows");
    assert!(progress.starts_with("Launching on "), "{progress:?}");

    push(&mut helm, cx, listing(&["n"], &[session("n")]));
    helm.reply(create.id, 200, Some(session("n").entry().value));
    settle(cx);
    assert_eq!(
        label(cx, "launch-progress"),
        None,
        "the reply took the pill down"
    );
    assert_eq!(
        cx.update(|_, cx| local_state::selected(cx)),
        None,
        "the user moved on, so nothing opened"
    );
    assert_eq!(
        label(cx, "launch-started-report").as_deref(),
        Some("Started “n”")
    );
}

/// SPEC.md: "A launch that revealed New gets no naming dialog, even if a later submission succeeds". The mark that
/// naming reads must go the moment the launcher is shown. Specified: `go` launched by the chord and refused by the helm
/// comes back shown with the helm's words; a press of Launch sends a second create, which is no longer a quick launch
/// New never showed.
#[gpui::test]
fn a_revealed_quick_launch_is_no_longer_unseen(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    chord(cx);
    let sent = creates(&mut helm, cx);
    assert!(
        unseen(cx),
        "the premise: the first create is an unseen quick launch"
    );
    helm.reply(sent[0].id, 409, Some(json!("the folder does not exist")));
    settle(cx);
    assert_eq!(
        refusal(cx).as_deref(),
        Some("Not launched. the folder does not exist")
    );
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        creates(&mut helm, cx).len(),
        1,
        "Launch sent the draft again"
    );
    assert!(
        !unseen(cx),
        "a launch from the shown launcher is not unseen"
    );
}

/// SPEC.md: "An edited marked template that no longer supplies what its active launch kind needs opens New with it
/// applied and a prominent banner saying it needs more choices", and the banner is the launcher's to clear once the user
/// starts editing. Launching it would only be refused. Specified: with `partial` marked, the chord sends no create and
/// opens New with the template's folder and the banner "partial needs more choices before it can launch; complete it
/// below."; typing into a field (the folder; focus starts in the search box, whose query is not an edit) takes the
/// banner away.
#[gpui::test]
fn an_incomplete_mark_opens_new_with_its_choices_and_banner(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("partial"));
    chord(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert!(
        text(cx).contains("/srv/partial"),
        "New, with the template applied"
    );
    assert_eq!(
        banner(cx).as_deref(),
        Some("partial needs more choices before it can launch; complete it below.")
    );
    test_support::click(cx, "launcher-folder-row");
    keys(cx, "x");
    assert_eq!(banner(cx), None, "an edit took the banner away");
}

/// SPEC.md, "Creation": "With no harness selected yet, a typed known model fills in its owning harness", and a template
/// applies as the same edits by hand do. That needs the catalog, so a quick launch reads it before applying. Specified:
/// with `model-only` marked, the chord opens New with the model applied, its agent type Codex picked by the catalog (the
/// footer names it), and the needs-more-choices banner, not a refusal.
#[gpui::test]
fn a_model_only_template_picks_its_agent_type_from_the_catalog(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("model-only"));
    chord(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert_eq!(note(cx), None, "the model applied");
    assert_eq!(
        banner(cx).as_deref(),
        Some("model-only needs more choices before it can launch; complete it below.")
    );
    let shown = text(cx);
    assert!(
        shown.contains("gpt-x"),
        "the model field shows it: {shown:?}"
    );
    assert!(
        label(cx, "launcher-launch").is_some_and(|name| name.starts_with("Launch Codex · ")),
        "the launch action names Codex as the agent type"
    );
}

/// SPEC.md, "Launch templates": "a model the chosen agent type does not offer, is refused with a message naming the
/// field, and nothing from it is applied". Without the catalog the model would launch as a typed id for that agent.
/// Specified: with `wrong-model` marked (Claude with a Codex-only model), the chord sends no create and opens New with
/// "Not launched." and the refusal naming the model.
#[gpui::test]
fn a_model_the_agent_type_does_not_offer_is_refused(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("wrong-model"));
    chord(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    let shown = note(cx).expect("New opened with a refusal");
    assert!(
        shown.starts_with("Not launched. ")
            && shown.contains("\"wrong-model\" was not applied: its model"),
        "{shown:?}"
    );
}

/// SPEC.md: a template that does not apply "is refused with a message naming the field, and nothing from it is
/// applied", and a quick launch's refusal reveals New with it. Specified: with `nowhere` marked, the chord sends no
/// create and opens New on its prefill (the folder untouched) with the refusal alert, "Not launched." first, naming
/// `nowhere` and its host.
#[gpui::test]
fn a_refused_mark_opens_new_unchanged_with_the_refusal(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("nowhere"));
    chord(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    let shown = note(cx).expect("New opened with a refusal");
    assert!(
        shown.starts_with("Not launched. ") && shown.contains("nowhere") && shown.contains("host"),
        "{shown:?}"
    );
    assert!(
        !text(cx).contains("/srv"),
        "the prefill's folder, untouched"
    );
}

/// The launcher, not the shortcut, judges whether a draft can launch: a template naming a host that is not connected
/// must neither send a create nor launch somewhere else (the old "quick launch reveals an offline host without
/// dispatching a create"), and the launcher it shows must show the draft it would launch, its fields included, not the
/// prefill it was made from. Specified: with `offline` marked, the chord sends no create and shows the launcher with
/// "Not launched." and the host reason beside Launch, its command field holding the template's `offline-cmd` (which the
/// footer never shows, so only the field can paint it).
#[gpui::test]
fn a_draft_the_launcher_refuses_is_shown_with_its_reason(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("offline"));
    chord(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was sent");
    assert_eq!(
        refusal(cx).as_deref(),
        Some("Not launched. Choose a connected host before launching")
    );
    assert!(
        text(cx).contains("offline-cmd"),
        "the command field shows the draft's command"
    );
}

/// Quick launch shares the launcher's suspended-host refusal. A shortcut must reveal the named host's Resume advice
/// without silently resuming it or sending a create, with the ordinary "Not launched." prefix kept.
#[gpui::test]
fn a_suspended_quick_launch_asks_to_resume_its_named_host(cx: &mut TestAppContext) {
    let (mut helm, connection) = started(cx);
    let mut hosts = fixture_hosts(&[0, 11]);
    hosts.entries[1].value["name"] = json!("שלום\u{202e}box");
    let host: HostView = serde_json::from_value(hosts.entries[1].value.clone()).unwrap();
    assert_eq!(host.identity.as_deref(), Some("identity12"));
    assert!(matches!(
        host.state,
        farhelm_proto::ui_channel::HostStateView::Suspended { .. }
    ));
    let snapshot = vec![
        listing(&[], &[]),
        hosts,
        marked(Some("paused")),
        templates_with(template(
            "paused",
            TemplateFields {
                host: Some("identity12".into()),
                destination: Some(TemplateDestination::Folder("~".into())),
                ..runs_true()
            },
        )),
    ];
    helm.connect(snapshot.clone());
    cx.run_until_parked();
    test_support::check_store(&connection, &snapshot, cx);
    let (_, cx) = test_support::open_view(cx, MainView::new);
    settle(cx);
    chord(cx);
    let sent = answer_except(&mut helm, cx, |command| {
        command.path == "/api/sessions" || command.path.ends_with("/resume")
    });
    assert!(sent.is_empty(), "nothing was launched or resumed: {sent:?}");
    assert_eq!(
        refusal(cx).as_deref(),
        Some("Not launched. שלום<U+202E>box is suspended. Resume it in Hosts, then launch.")
    );
    assert!(
        crate::session_list::host_rail::tests::texts_within(&frame(cx), "launcher-refusal-words")
            .iter()
            .any(|text| crate::host_text::tests::paints_isolated(&text.text, "שלום<U+202E>box")),
        "the host name cannot reorder the refusal's advice"
    );
    assert_eq!(
        crate::launcher::view_tests::launch_help(cx).as_deref(),
        Some("שלום<U+202E>box is suspended. Resume it in Hosts, then launch.")
    );
}

/// A quick launch being prepared holds the launch slot, and SPEC.md keeps the launcher busy then as during a launch:
/// New must not open over it, another shortcut or the sequence prefix must not start a second one (the old app refused
/// the prefix while a launch was pending, too), and a result the user moved on from must not open over what they went
/// on to (SPEC.md, "Keyboard focus"). Replace with is refused the same way: the GPUI page is not blocked during the
/// preparation (the parity file's deviation "Hidden quick-launch preparation does not block the page"), so its entry
/// must refuse rather than put Replace with's draft in the launcher the preparation holds. Specified: with session `a`
/// listed, `partial` marked and the chord pressed, before the catalog read answers, New and Replace with of `a` refuse
/// to open, a second chord opens nothing, and the prefix arms no sequence; after a key press, the read's answer opens
/// nothing, sends no create, and the "launch not started" notice offers the launcher.
#[gpui::test]
fn presses_during_a_preparation_wait_and_its_result_respects_the_moved_on_rule(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = window(cx, Some("partial"));
    push(&mut helm, cx, listing(&["a"], &[session("a")]));
    chord(cx);
    assert!(!any_open(cx), "the premise: the preparation draws nothing");
    assert!(
        !cx.update(launcher::entry::open_new),
        "New waits for the preparation"
    );
    assert!(
        !cx.update(|window, cx| launcher::entry::open_replace_with("a", window, cx)),
        "Replace with waits for the preparation"
    );
    chord(cx);
    assert!(!any_open(cx), "a second chord opened nothing");
    keys(cx, SEQUENCE_PREFIX_CHORD);
    assert!(
        !cx.update(|_, cx| super::armed::is_armed(cx)),
        "the prefix armed no sequence"
    );
    keys(cx, "x");
    assert!(
        creates(&mut helm, cx).is_empty(),
        "answering the catalog read sent no create"
    );
    assert!(
        !any_open(cx),
        "nothing opened over what the user went on to"
    );
    assert!(
        cx.update(|_, cx| notices::posted(cx).contains(&Notice::LaunchNotStarted)),
        "the notice offers the launcher"
    );
}

/// SPEC.md: "With no usable mark it opens Templates with an explanation". Specified: with nothing marked, and again
/// once ⚡ moves to a name no template has, the chord opens Templates with the ⚡ hint and sends no create.
#[gpui::test]
fn a_missing_or_unusable_mark_opens_templates_with_its_hint(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    for mark in [None, Some("deleted")] {
        push(&mut helm, cx, marked(mark));
        assert!(!any_open(cx), "{mark:?}: the premise: nothing is open");
        chord(cx);
        assert!(
            creates(&mut helm, cx).is_empty(),
            "{mark:?}: nothing launched"
        );
        assert!(
            text(cx).contains("Mark a template with ⚡"),
            "{mark:?}: Templates with its hint"
        );
        keys(cx, "escape");
    }
}

/// SPEC.md: "A missing mark or failed read shows its refusal"; the old app opened New with "Couldn't prepare quick
/// launch" when its template read failed. Saying the mark is missing, or opening Templates, would be false while ⚡ is
/// set. Specified: with `go` marked and the helm reporting its templates unavailable ("busy"), the chord sends no create
/// and opens New with "Not launched. Couldn't prepare quick launch: busy".
#[gpui::test]
fn unavailable_templates_refuse_a_press_on_the_page(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    let busy = Availability::Unavailable {
        error: "busy".into(),
    };
    push(&mut helm, cx, templates(busy));
    chord(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert_eq!(
        note(cx).as_deref(),
        Some("Not launched. Couldn't prepare quick launch: busy")
    );
}

/// SPEC.md: "A hidden quick-launch launcher still consumes the chord without applying it". A refused quick launch the
/// user moved on from waits behind its notice; a press then must neither start another launch nor drop that one.
/// Specified: `go` launched by the chord, a key pressed, then the helm's refusal: nothing opens and the notice offers
/// the launcher; another chord sends nothing and opens nothing, and the notice stays.
#[gpui::test]
fn a_press_waits_while_a_refused_launch_waits_behind_its_notice(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    chord(cx);
    let sent = creates(&mut helm, cx);
    keys(cx, "x");
    helm.reply(sent[0].id, 409, Some(json!("host1 is not connected")));
    settle(cx);
    let notice = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| notices::posted(cx).contains(&Notice::LaunchNotStarted))
    };
    assert!(
        !any_open(cx) && notice(cx),
        "the premise: the refusal waits behind its notice"
    );
    chord(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert!(
        !any_open(cx) && notice(cx),
        "nothing opened, and the notice still offers the launcher"
    );
}

/// SPEC.md: "Other visible dialogs keep the keyboard", for the ⚡ chord and the number keys alike ("Other visible modals
/// retain the keyboard"). A chord that launched or applied from under another dialog would act behind the user's back.
/// Specified: with ⚡ and number key 1 on `go` and Templates open, the chord and the number key send nothing and leave
/// the window as it was.
#[gpui::test]
fn the_chord_and_number_keys_do_nothing_under_another_dialog(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    push(&mut helm, cx, bound(Some("go"), &[("1", "go")]));
    cx.update(|window, cx| {
        let mark = focus::Mark::take(cx);
        super::open_with_hint(super::Hint::NumberKey, &mark, window, cx)
    })
    .expect("Templates opened");
    settle(cx);
    let before = text(cx);
    chord(cx);
    number_key(cx, 1);
    assert!(creates(&mut helm, cx).is_empty(), "nothing launched");
    assert_eq!(text(cx), before, "nothing changed");
}

/// A sequence launches without taking focus from the terminal. The next Enter reaches that terminal and makes
/// the pending launch leave it selected when the reply arrives ("Creation" in SPEC.md).
#[gpui::test]
fn a_sequences_enter_launches_unseen(cx: &mut TestAppContext) {
    let snapshot = vec![
        listing(&["a"], &[session("a")]),
        fixture_hosts(&[0, 1]),
        crate::session_list::tests::preferences(&["a"], Some("a")),
        templates(Availability::Available),
    ];
    let mut main = test_support::open_main(cx, snapshot);
    main.terminal_live();
    keys(main.cx, &format!("{SEQUENCE_PREFIX_CHORD} g o enter"));
    let [create]: [FakeCommand; 1] = creates(&mut main.helm, main.cx)
        .try_into()
        .expect("one create");
    assert!(unseen(main.cx) && !any_open(main.cx), "launched unseen");
    assert!(test_support::terminal_focused(main.cx));
    assert_eq!(main.program_input(), 0, "the sequence stayed in Farhelm");
    keys(main.cx, "enter");
    assert_eq!(main.program_input(), 1);
    launcher::launch_tests::created(&mut main.helm, main.cx, create.id, "b");
    let selected = main.cx.update(|_, cx| local_state::selected(cx));
    assert_eq!(selected.as_deref(), Some("a"));
    assert!(test_support::terminal_focused(main.cx) && !any_open(main.cx));
    let report = label(main.cx, "launch-started-report");
    assert_eq!(report.as_deref(), Some("Started “b”"));
}

/// Outside the launcher, an incomplete sequence opens New "with a prominent banner saying 'This sequence doesn't make
/// a complete setup; complete it below'", which "follows the number-key banner's expiry", and a refused one "opens New
/// with that error" (SPEC.md); neither may launch. Specified: the prefix, `partial` and Enter opens New with the
/// template's folder and the sequence banner, which is gone six seconds later, New still open; once that
/// New is closed, `zzz` and Enter opens New with "Not launched." and "No template matches"; neither sends a create.
#[gpui::test]
fn a_sequence_that_cannot_launch_opens_new(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} p a r t i a l enter"));
    assert!(
        creates(&mut helm, cx).is_empty(),
        "partial: nothing launched"
    );
    assert!(
        text(cx).contains("/srv/partial"),
        "partial: New with the template applied"
    );
    assert_eq!(
        banner(cx).as_deref(),
        Some("This sequence doesn't make a complete setup; complete it below.")
    );
    test_support::advance(cx, BANNER_SHOWS_FOR);
    assert_eq!(banner(cx), None, "the banner went on its own");
    assert!(text(cx).contains("/srv/partial"), "and New stayed");
    keys(cx, "escape");
    assert!(!any_open(cx), "the premise: New closed");

    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} z z z enter"));
    assert!(creates(&mut helm, cx).is_empty(), "zzz: nothing launched");
    let shown = note(cx).expect("zzz: New opened with a refusal");
    assert!(
        shown.starts_with("Not launched. No template matches"),
        "{shown:?}"
    );
}

/// Whether the sidebar shows its Hosts panel in place of the session list.
fn hosts_panel(cx: &mut VisualTestContext) -> bool {
    node(&frame(cx), "hosts-panel").is_some()
}

/// Show the sidebar's Hosts panel, as its rail button does.
fn show_hosts_panel(cx: &mut VisualTestContext) {
    test_support::click(cx, "hosts-panel-toggle");
    assert!(hosts_panel(cx), "the premise: the Hosts panel shows");
}

/// SPEC.md: "All or a host avatar returns to Sessions, as does an accepted quick-switcher pick or New handoff". In the
/// old app every quick launch from the page went through the list's New, which hid the panel, so a session the user
/// started from the panel showed up in the list. The dispatcher hides it where a page quick launch starts preparing and
/// where a refusal opens New; the ⚡ chord's launch and Templates with a hint are pinned through the switcher's Quick
/// launch row (`session_list::quick_switcher::tests`). Specified, each from the Hosts panel: number key 2 on `partial`
/// opens New and the panel goes; a sequence of `zzz` opens New with its refusal and the panel goes.
#[gpui::test]
fn a_quick_launch_from_the_page_leaves_the_hosts_panel(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, bound(None, &[("2", "partial")]));
    show_hosts_panel(cx);
    number_key(cx, 2);
    assert!(
        creates(&mut helm, cx).is_empty(),
        "the premise: nothing launched"
    );
    assert!(
        text(cx).contains("/srv/partial"),
        "the premise: New with partial"
    );
    assert!(!hosts_panel(cx), "number key: the panel went");
    keys(cx, "escape");

    show_hosts_panel(cx);
    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} z z z enter"));
    assert!(
        creates(&mut helm, cx).is_empty(),
        "the premise: nothing launched"
    );
    assert!(
        note(cx).is_some(),
        "the premise: New with the sequence's refusal"
    );
    assert!(!hosts_panel(cx), "sequence: the panel went");
}

// ---- The open launcher ----

/// SPEC.md: "While the session launcher is the only visible dialog, the same chord applies its template to that draft
/// and leaves Launch to the user. Consecutive presses apply in order; a refusal changes nothing from that press and does
/// not discard a later press." Launching from an open launcher would discard what the user was preparing. Specified:
/// with New open, the chord with `partial` marked puts its folder in the draft and sends nothing; with ⚡ moved to
/// `nowhere`, the next press shows its refusal and leaves the folder; with ⚡ on `go`, the next applies the command
/// (the Command tab showing `true`), takes the refusal away, and still sends no create, the launcher staying open.
#[gpui::test]
fn presses_apply_to_the_open_launcher_in_order(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("partial"));
    open_new(&mut helm, cx);
    chord(cx);
    assert!(text(cx).contains("/srv/partial"), "partial applied");
    assert_eq!((banner(cx), note(cx)), (None, None), "and said nothing");

    push(&mut helm, cx, marked(Some("nowhere")));
    chord(cx);
    let shown = note(cx).expect("nowhere refused");
    assert!(
        shown.contains("nowhere") && !shown.starts_with("Not launched."),
        "{shown:?}"
    );
    assert!(
        text(cx).contains("/srv/partial"),
        "the refused press changed nothing"
    );

    push(&mut helm, cx, marked(Some("go")));
    chord(cx);
    assert_eq!(note(cx), None, "the applied press took the refusal away");
    assert!(text(cx).contains("true"), "go's command applied");
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert!(any_open(cx), "Launch is left to the user");
}

/// The template rules need the catalog, so a press that lands while the open launcher's catalog read is out must wait
/// for it rather than apply against nothing or be lost (the old app held shortcut batches while its catalog loaded).
/// Specified: with `partial` marked and New just opened, its catalog read unanswered, the chord changes nothing yet;
/// once the read answers, the folder is applied.
#[gpui::test]
fn a_press_waits_for_the_open_launchers_catalog(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("partial"));
    assert!(
        cx.update(launcher::entry::open_new),
        "the premise: New opened"
    );
    settle(cx);
    chord(cx);
    assert!(
        !text(cx).contains("/srv/partial"),
        "nothing applied before the catalog"
    );
    assert!(
        creates(&mut helm, cx).is_empty(),
        "answering the read sent no create"
    );
    assert!(
        text(cx).contains("/srv/partial"),
        "the press applied once the catalog answered"
    );
}

/// SPEC.md: in the launcher, "A missing mark or failed read shows its refusal where `tl:` does, without opening
/// Templates". Opening Templates over the launcher would bury the draft. Specified: with New open and nothing marked,
/// the chord shows "No template is marked ⚡ for quick launch" in the launcher and opens no Templates; with the
/// templates unavailable ("busy"), it shows "Couldn't load templates: busy".
#[gpui::test]
fn a_missing_mark_or_failed_read_refuses_in_the_launcher(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    open_new(&mut helm, cx);
    chord(cx);
    assert_eq!(
        note(cx).as_deref(),
        Some("No template is marked ⚡ for quick launch")
    );
    assert!(
        !text(cx).contains("Mark a template with ⚡"),
        "Templates did not open"
    );

    push(&mut helm, cx, marked(Some("go")));
    push(
        &mut helm,
        cx,
        templates(Availability::Unavailable {
            error: "busy".into(),
        }),
    );
    chord(cx);
    assert_eq!(note(cx).as_deref(), Some("Couldn't load templates: busy"));
}

/// A shortcut's refusal answers what the user did last, so it must show while the launcher has something else in its
/// alert (a press's reason, the helm's words, which can say a launch may still start), and must hide none of it; hidden,
/// the press would seem to do nothing, and hiding the alert could hide a warning. Specified: with New open and nothing
/// marked, Launch pressed on the incomplete draft shows its "Not launched." reason; the chord then shows "No template is
/// marked ⚡ for quick launch" as the note, above the reason, which still shows.
#[gpui::test]
fn a_shortcut_refusal_shows_over_a_press_reason(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    open_new(&mut helm, cx);
    test_support::click(cx, "launcher-launch");
    let reason = refusal(cx).expect("the press shows its reason");
    assert!(
        reason.starts_with("Not launched."),
        "the premise: {reason:?}"
    );
    chord(cx);
    assert_eq!(
        note(cx).as_deref(),
        Some("No template is marked ⚡ for quick launch")
    );
    assert_eq!(refusal(cx), Some(reason), "the press's reason still shows");
    let (top, under) = (
        test_support::bounds(cx, "launcher-note"),
        test_support::bounds(cx, "launcher-refusal"),
    );
    assert!(
        top.bottom() <= under.top(),
        "the note is above the alert: {top:?}, {under:?}"
    );
}

/// SPEC.md: "In the session launcher, Enter instead applies the queued templates to the current draft without
/// launching; each completed sequence is one refusal unit, and the pill says 'apply' instead of 'launch'", and an
/// unbound key shows a refusal without changing the draft. With New open and focus
/// moved to the host button, the prefix arms the "Apply sequence:" pill;
/// `partial` and Enter put its folder in the draft and send nothing; `zzz` and Enter show "No template matches"; the
/// unbound key 9 and Enter show "No template is bound to key 9" and open no Templates.
#[gpui::test]
fn a_sequence_in_the_launcher_applies_or_refuses_there(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    open_new(&mut helm, cx);
    // Focus starts in the search box; Tab walks on to the host button, after any destination choice drawn before it
    // (the Where row's and the Runs on switch's segments).
    for _ in 0..8 {
        if focused_id(cx).as_deref() == Some("launcher-host") {
            break;
        }
        keys(cx, "tab");
    }
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("launcher-host"),
        "the premise: focus is on the host button, not a text field"
    );
    keys(cx, SEQUENCE_PREFIX_CHORD);
    assert!(text(cx).contains("Apply sequence:"), "the launcher's pill");
    keys(cx, "p a r t i a l enter");
    assert!(text(cx).contains("/srv/partial"), "partial applied");
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");

    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} z z z enter"));
    let shown = note(cx).expect("zzz refused");
    assert!(shown.starts_with("No template matches"), "{shown:?}");

    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} 9 enter"));
    assert_eq!(note(cx).as_deref(), Some("No template is bound to key 9"));
    assert!(
        !text(cx).contains("Give a template a number key"),
        "Templates did not open"
    );
    assert!(
        text(cx).contains("/srv/partial"),
        "the refusals changed nothing"
    );
}

/// The mirror of the case above: a refusal is about the press it answered, and a later press of Launch must say its own
/// reason rather than leave an older refusal standing (the refusal note is cleared on submit). Specified: with New open
/// and nothing marked, the chord shows "No template is marked ⚡ for quick launch" as the note; Launch pressed on the
/// incomplete draft then shows its own "Not launched." reason in the alert, and the note is gone.
#[gpui::test]
fn a_press_after_a_shortcut_refusal_shows_its_own_reason(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    open_new(&mut helm, cx);
    chord(cx);
    assert_eq!(
        note(cx).as_deref(),
        Some("No template is marked ⚡ for quick launch"),
        "the premise: the shortcut refused"
    );
    test_support::click(cx, "launcher-launch");
    let reason = refusal(cx).expect("the press shows its reason");
    assert!(reason.starts_with("Not launched."), "{reason:?}");
    assert_eq!(note(cx), None, "the press took the note away");
}

/// SPEC.md, "Losing the connection to the helm": drafts do not survive a reset, and a hidden quick-launch preparation
/// starts over too. A preparation that outlived the reset would keep New refused, or open a launcher over the new
/// session. Specified: with `partial` marked, the chord starts a preparation; the channel breaks before the catalog read
/// answers and a new UI session connects; nothing opens, and New opens again.
#[gpui::test]
fn a_reset_drops_a_preparation(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("partial"));
    chord(cx);
    assert!(
        !cx.update(launcher::entry::open_new),
        "the premise: the preparation holds New"
    );
    helm.break_channel();
    helm.connect(vec![
        listing(&[], &[]),
        fixture_hosts(&[0, 1]),
        marked(Some("partial")),
        templates(Availability::Available),
    ]);
    settle(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert!(!any_open(cx), "nothing of the preparation came back");
    assert!(cx.update(launcher::entry::open_new), "New opens again");
}

/// Template batches waiting for the catalog speak of the draft as it was; once Launch has sent that draft, applying
/// them to the hidden launcher would edit a draft whose create is out, and a refusal would bring it back changed.
/// Specified: on a complete New draft (a command, `~`) whose catalog read is unanswered, the chord with `partial`
/// marked waits; Launch sends the create with `~`; when the helm refuses it, the launcher comes back without
/// `partial`'s folder.
#[gpui::test]
fn launch_drops_the_batches_waiting_for_the_catalog(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("partial"));
    let mut draft = launcher::draft::LaunchDraft::new(Some(1), "~".into(), None);
    draft.choose_tab(launcher::draft::Tab::Command);
    draft.edit_command("true".into());
    draft.choose_command_yolo(false);
    cx.update(|window, cx| {
        let mark = focus::Mark::take(cx);
        launcher::view::open(draft, &mark, window, cx)
    })
    .expect("the premise: New opened");
    settle(cx);
    chord(cx);
    assert!(
        !text(cx).contains("/srv/partial"),
        "the premise: the press waits for the catalog"
    );
    test_support::click(cx, "launcher-launch");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("Launch sent one create, not {}", sent.len());
    };
    assert_eq!(create.body.clone().unwrap_or_default()["cwd"], json!("~"));
    helm.reply(create.id, 409, Some(json!("the folder does not exist")));
    settle(cx);
    assert!(
        any_open(cx),
        "the premise: the refusal brought the launcher back"
    );
    assert!(
        !text(cx).contains("/srv/partial"),
        "the waiting press was dropped, not applied"
    );
}

// ---- Number keys ----

/// SPEC.md, "Template number shortcuts": a key "starts its template through the same direct launch path as quick
/// launch", so a bound key gives a session without the launcher, through the real handler. That a held key acts once
/// is the actions module's guard, tested there
/// (`actions_tests::a_held_number_key_acts_once_and_a_held_text_size_chord_repeats`). Specified: with number key 1 on
/// `go`, the chord sends one create, of `go`'s command, opens no layer and leaves the launch marked as a quick launch
/// New never showed.
#[gpui::test]
fn a_number_key_launches_its_template_unseen(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, bound(None, &[("1", "go")]));
    number_key(cx, 1);
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one press sent one create, not {}", sent.len());
    };
    assert_eq!(
        create.body.clone().unwrap_or_default()["command"]["command"],
        json!("true"),
        "the template's command"
    );
    assert!(!any_open(cx), "the launcher was never shown");
    assert!(unseen(cx), "a quick launch New never showed");
}

/// The old app's "container template quick launch retains its saved place" (`podman-environments.spec.ts` at the
/// reference commit): a number key bound to a container template launches it out of sight into a new container at the
/// stored place, through the environment route only, since an ordinary create would start the session on the Host
/// row's host instead. Specified: with the recipe catalog listing `web` and number key 1 on `container-go` (a command
/// of `true`, not YOLO, in a container of `web` on host1), the key sends exactly one `POST /api/environment-sessions`
/// and no ordinary create, with that recipe and place, while no layer opens; the helm's refusal brings the launcher back
/// with the stored place still chosen and shown in its Recipe and Container host rows. Here host1's check has passed; the
/// old test's wait for the place answer is the launcher's, tested in `environment_launch_tests`.
#[gpui::test]
fn a_container_template_on_a_number_key_launches_at_its_place(cx: &mut TestAppContext) {
    let stored = LaunchTarget::Environment {
        recipe: "web".into(),
        place: Place::PodmanHost(1),
    };
    let hosts = fixture_hosts(&[0, 1]);
    let host1: HostView = serde_json::from_value(hosts.entries[0].value.clone()).unwrap();
    assert_eq!(
        (host1.id, container_reason(&host1)),
        (1, None),
        "the premise: host1's Podman check passed"
    );
    // [`window`]'s snapshot with the recipe catalog added: a collection the snapshot did not install cannot be pushed
    // later.
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![
        listing(&[], &[]),
        hosts,
        bound(None, &[("1", "container-go")]),
        templates_with(template(
            "container-go",
            TemplateFields {
                target: Some(stored.clone()),
                ..runs_true()
            },
        )),
        environment_launch_tests::catalog(),
    ]);
    cx.run_until_parked();
    let (_, cx) = test_support::open_view(cx, MainView::new);
    settle(cx);
    number_key(cx, 1);
    let sent = answer_except(&mut helm, cx, |command| {
        command.path == "/api/environment-sessions" || command.path == "/api/sessions"
    });
    let [create] = &sent[..] else {
        panic!("one press sent one create, not {sent:?}");
    };
    assert_eq!(
        create.path, "/api/environment-sessions",
        "not an ordinary create"
    );
    let body = create.body.clone().unwrap_or_default();
    assert_eq!(body["recipe"], json!("web"), "{body}");
    assert_eq!(
        body["place"],
        json!({ "kind": "podman_host", "host": 1 }),
        "{body}"
    );
    assert_eq!(
        body["request"]["command"]["command"],
        json!("true"),
        "the template's command"
    );
    assert!(!any_open(cx), "the launcher was never shown");

    helm.reply(create.id, 409, Some(json!("container template refused")));
    settle(cx);
    assert!(any_open(cx), "the refusal brought the launcher back");
    assert_eq!(
        crate::launcher::view_tests::shown_draft(cx).target,
        Some(stored),
        "with the stored place still chosen"
    );
    assert!(
        label(cx, "launcher-recipe").is_some() && label(cx, "launcher-container-host").is_some(),
        "and shows it in its Recipe and Container host rows"
    );
    assert_eq!(
        label(cx, "launcher-host").as_deref(),
        Some("Host: New container"),
        "the picker names the retained container placement"
    );
}

/// The old app's "a recipe template quick launch asks before one recipe request" (`tensorlake-sandbox-sessions.spec.ts`
/// at the reference commit): a quick launch that would create a billed sandbox without the launcher on screen reveals
/// the launcher with "Create a sandbox for this session?" instead of creating, and its answer sends the one create,
/// confirmed. Until the launcher asked, the key's create went out unconfirmed and the helm refused it. Specified: with
/// the catalog listing `ready` (built) and number key 1 on `sandbox-go` (a command of `true` in a new sandbox of
/// `ready`), and an account with an API key, Farhelm's SSH key and the setting that skips the question off, the key
/// sends no create and brings the launcher back with the question naming `ready`, focus on Cancel; Create and launch
/// sends exactly one `POST /api/environment-sessions` and no ordinary create, for `ready` on the Tensorlake account,
/// confirmed, without the naming a quick launch New never showed gets; the helm's refusal brings the launcher back with
/// "Not launched." and its words.
#[gpui::test]
fn a_sandbox_template_on_a_number_key_comes_back_to_ask_before_creating(cx: &mut TestAppContext) {
    let stored = LaunchTarget::Environment {
        recipe: "ready".into(),
        place: Place::TensorlakeAccount,
    };
    let usable = account(
        true,
        ready_key("tl sbx ssh keys add --name fixture key.pub"),
    );
    let catalog = CollectionState {
        collection: Collection::Recipes,
        meta: meta(&["ready"]),
        entries: vec![KeyedEntry {
            key: "ready".into(),
            value: built_json("ready"),
        }],
    };
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![
        listing(&[], &[]),
        fixture_hosts(&[0, 1]),
        bound(None, &[("1", "sandbox-go")]),
        templates_with(template(
            "sandbox-go",
            TemplateFields {
                target: Some(stored),
                ..runs_true()
            },
        )),
        catalog,
    ]);
    cx.run_until_parked();
    let (_, cx) = test_support::open_view(cx, MainView::new);
    settle(cx);
    let keep = |command: &FakeCommand| {
        command.path == "/api/environment-sessions" || command.path == "/api/sessions"
    };
    number_key(cx, 1);
    let sent = answer_with(&mut helm, cx, Some(&usable), keep);
    assert_eq!(sent, [], "nothing is created before the answer");
    assert!(any_open(cx), "the launcher came back to ask");
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Create a sandbox for this session?")
    );
    assert_eq!(
        label(cx, "sandbox-question-recipe").as_deref(),
        Some("Recipe ready")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));

    test_support::click(cx, "prompt-confirm");
    let sent = answer_with(&mut helm, cx, Some(&usable), keep);
    let [create] = &sent[..] else {
        panic!("the answer sent one create, not {sent:?}");
    };
    assert_eq!(
        create.path, "/api/environment-sessions",
        "not an ordinary create"
    );
    let body = create.body.clone().unwrap_or_default();
    assert_eq!(body["recipe"], json!("ready"), "{body}");
    assert_eq!(
        body["place"],
        json!({ "kind": "tensorlake_account" }),
        "{body}"
    );
    assert_eq!(body["confirmed"], json!(true), "{body}");
    assert!(!any_open(cx), "the launcher hid while its create is out");
    assert!(!unseen(cx), "the launch revealed New, so no naming follows");

    helm.reply(
        create.id,
        409,
        Some(json!("template recipe fixture refused")),
    );
    settle(cx);
    assert!(any_open(cx), "the refusal brought the launcher back");
    assert_eq!(
        label(cx, "launcher-refusal").as_deref(),
        Some("Not launched. template recipe fixture refused")
    );
}

/// The templates collection of [`templates`], available, with `yolo-go` added last: a command launch of `true` asserted
/// YOLO, which launches on its own and which a host that asks first refuses until the user answers.
fn with_yolo_go() -> CollectionState {
    templates_with(template(
        "yolo-go",
        TemplateFields {
            yolo: Some(true),
            ..runs_true()
        },
    ))
}

/// The templates collection of [`templates`], available, with `extra` added last.
fn templates_with(extra: LaunchTemplate) -> CollectionState {
    let mut state = templates(Availability::Available);
    state.entries.push(KeyedEntry {
        key: extra.name.clone(),
        value: serde_json::to_value(&extra).unwrap(),
    });
    let keys: Vec<String> = state
        .entries
        .iter()
        .map(|entry| entry.key.clone())
        .collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    state.meta = meta(&keys);
    state
}

/// Check that the launcher shows the ordinary YOLO question over `yolo-go`'s draft: focus on Cancel, the asserted
/// command's reason, the one-off answer, and no helm sentence.
fn the_yolo_question_is_up(cx: &mut VisualTestContext) {
    assert!(any_open(cx), "the launcher is shown");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    assert_eq!(
        label(cx, "yolo-reason").as_deref(),
        Some("This command was asserted to bypass its safety checks.")
    );
    assert_eq!(
        label(cx, "prompt-confirm").as_deref(),
        Some("Start YOLO session anyway")
    );
    assert_eq!(
        label(cx, "launcher-refusal"),
        None,
        "the helm's own sentence is not shown"
    );
    assert_eq!(
        crate::launcher::view_tests::shown_draft(cx).command.command,
        "true",
        "over the template's draft"
    );
}

/// The ordinary YOLO question, revealed by a quick launch the helm refused for YOLO, through each of its answers: press
/// `press` with `yolo-go` behind it and refuse its create as a host that asks first does; Cancel then keeps the draft
/// and sends nothing; Launch asks again; "Start YOLO session anyway" retries once with consent, and that retry is no
/// longer a quick launch New never showed.
fn a_refused_yolo_quick_launch_asks(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    press: impl FnOnce(&mut VisualTestContext),
) {
    press(cx);
    let sent = creates(helm, cx);
    let [create] = &sent[..] else {
        panic!("one press sent one create, not {}", sent.len());
    };
    assert!(!any_open(cx), "the premise: the launch went out unseen");
    assert!(
        unseen(cx),
        "the premise: the create is an unseen quick launch"
    );
    assert_ne!(
        create.body.clone().unwrap_or_default()["confirm_yolo"],
        json!(true),
        "the premise: the first create carries no consent"
    );
    refuse_for_yolo(helm, cx, create.id);
    the_yolo_question_is_up(cx);

    test_support::click(cx, "prompt-cancel");
    assert!(creates(helm, cx).is_empty(), "Cancel sends nothing");
    assert!(any_open(cx), "and keeps the launcher");
    assert_eq!(
        crate::launcher::view_tests::shown_draft(cx).command.command,
        "true",
        "with the template's draft"
    );
    test_support::click(cx, "launcher-launch");
    let sent = creates(helm, cx);
    let [again] = &sent[..] else {
        panic!("Launch sent one create, not {}", sent.len());
    };
    assert_ne!(
        again.body.clone().unwrap_or_default()["confirm_yolo"],
        json!(true),
        "Launch after Cancel asks again rather than carrying consent"
    );
    refuse_for_yolo(helm, cx, again.id);
    the_yolo_question_is_up(cx);

    test_support::click(cx, "prompt-confirm");
    let sent = creates(helm, cx);
    let [retry] = &sent[..] else {
        panic!("the answer sent one create, not {}", sent.len());
    };
    assert_eq!(
        retry.body.clone().unwrap_or_default()["confirm_yolo"],
        json!(true),
        "the retry carries the consent"
    );
    assert_eq!(
        retry.body.clone().unwrap_or_default()["command"]["command"],
        json!("true"),
        "of the same draft"
    );
    assert!(
        !unseen(cx),
        "a revealed launch is no longer unseen while its retry is out"
    );
}

/// SPEC.md: a quick launch goes through "exactly New's defaults and ordinary launch path", and a refusal "reveals New with
/// the draft"; the ordinary path's YOLO refusal is answered by the YOLO question, which "retries with the override only
/// when the user confirms". A hidden launch that swallowed the question, or retried on its own, would either never start
/// or start YOLO without consent. Specified: with ⚡ on `yolo-go`, the chord sends one create without consent; the
/// helm's YOLO refusal reveals the launcher with the question (focus on Cancel, the asserted command's reason, no helm
/// sentence) over the template's draft; Cancel keeps that draft and sends nothing, and Launch asks again; "Start YOLO
/// session anyway" sends one create of the same command with `confirm_yolo`, no longer an unseen quick launch.
#[gpui::test]
fn the_chords_yolo_refusal_reveals_the_ordinary_question_over_its_draft(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, with_yolo_go());
    push(&mut helm, cx, marked(Some("yolo-go")));
    a_refused_yolo_quick_launch_asks(&mut helm, cx, chord);
}

/// The number key's twin of the case above: a number key launches through the same path as the ⚡ chord, so its YOLO
/// refusal must reach the same question (the old spec ran both). Specified: with key 1 on `yolo-go`, the key's create,
/// refused for YOLO, reveals the same question over the template's draft, with the same Cancel and the same answer.
#[gpui::test]
fn a_number_keys_yolo_refusal_reveals_the_ordinary_question_over_its_draft(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, with_yolo_go());
    push(&mut helm, cx, bound(None, &[("1", "yolo-go")]));
    a_refused_yolo_quick_launch_asks(&mut helm, cx, |cx| number_key(cx, 1));
}

/// The template the pill cases launch: [`runs_true`] whose session name is its own name, `named-go`, so that the pill
/// has a template-supplied name to show.
const NAMED_GO: &str = "named-go";

/// A held quick launch of [`NAMED_GO`] through `press`, with `bindings` holding the key, followed through its progress
/// pill, a quick switcher opened and closed meanwhile, and the helm's refusal. The press sends one create titled
/// `named-go` and opens nothing; nothing shows at 1.9 seconds, and at 2.1 the launcher's own pill is a status named
/// "Launching “named-go” on host1…" painting "2 s" (`launcher::pill`). The switcher then opens over the hidden launch
/// and Escape closes it, with the pill up throughout. The refusal takes the pill down and, since those key presses
/// count as moving on (SPEC.md, "Keyboard focus", "Who may move focus"; the old test, run before that rule, saw the
/// launcher come back), opens nothing and leaves the notice; its Open launcher brings back the launcher with "Not
/// launched. hidden launch refused".
fn a_held_named_quick_launch_shows_its_pill(
    cx: &mut TestAppContext,
    bindings: CollectionState,
    press: impl FnOnce(&mut VisualTestContext),
) {
    let (mut helm, cx) = window(cx, None);
    let named_go = TemplateFields {
        name: Some(NAMED_GO.into()),
        ..runs_true()
    };
    push(&mut helm, cx, templates_with(template(NAMED_GO, named_go)));
    push(&mut helm, cx, bindings);
    let pill = |cx: &mut VisualTestContext| label(cx, "launch-progress");
    press(cx);
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one press sent one create, not {}", sent.len());
    };
    assert_eq!(
        create.body.clone().unwrap_or_default()["title"],
        json!(NAMED_GO),
        "the premise: the template names the session"
    );
    assert!(!any_open(cx), "the premise: the launch went out unseen");

    test_support::advance(cx, Duration::from_millis(1_900));
    assert_eq!(pill(cx), None, "nothing before two seconds");
    test_support::advance(cx, Duration::from_millis(200));
    let words = Some("Launching “named-go” on host1…".to_owned());
    assert_eq!(pill(cx), words, "the pill names the session and its host");
    assert!(is_status(cx, "launch-progress"), "announced as a status");
    assert!(
        painted(&frame(cx)).iter().any(|line| line == "2 s"),
        "it counts from the press"
    );

    // SPEC.md, "Desktop window chrome", on the quick switcher: it "works during a pending launch, whose hidden launcher
    // is no longer modal". The switcher's one read (`/api/sessions?sort=activity`) stays unanswered until the create's
    // reply: the fake helm answers in command order, as the helm does, and the create went first. The switcher draws
    // while its read loads, which is all this step needs.
    keys(cx, QUICK_SWITCHER_CHORD);
    let reads = helm.commands();
    let [read] = &reads[..] else {
        panic!("opening the switcher sent one read, not {reads:?}");
    };
    assert!(
        cx.debug_bounds("quick-switcher").is_some(),
        "the switcher opened over the hidden launch"
    );
    assert_eq!(pill(cx), words, "with the pill still up");
    keys(cx, "escape");
    assert!(
        cx.debug_bounds("quick-switcher").is_none(),
        "Escape closed it"
    );
    assert_eq!(pill(cx), words, "and the pill is still up");

    helm.reply(create.id, 409, Some(json!("hidden launch refused")));
    // The closed switcher's read, answered now that it is first in line; nothing is left to read it.
    helm.reply(
        read.id,
        200,
        Some(json!({ "sessions": [], "total": 0, "truncated": false })),
    );
    settle(cx);
    assert_eq!(pill(cx), None, "the refusal took the pill down");
    assert!(
        !any_open(cx),
        "the user moved on, so the launcher did not come back"
    );
    assert!(
        cx.update(|_, cx| notices::posted(cx).contains(&Notice::LaunchNotStarted)),
        "the notice offers the launcher"
    );
    test_support::click(cx, "notice-open-launcher");
    assert!(any_open(cx), "Open launcher brought it back");
    assert_eq!(
        refusal(cx).as_deref(),
        Some("Not launched. hidden launch refused")
    );
}

/// The old app's "quick launch shows a named pending pill" (`launch-progress.spec.ts` at the reference commit). A quick
/// launch is a launch whose launcher was never drawn, so the progress pill is the user's only sign that it is still
/// out (SPEC.md, "Creation": the pill shows "the captured host's name ... including the session name when supplied"),
/// and the session name here comes from the template, not from anything the user typed. Specified: with ⚡ on
/// `named-go`, the walk of [`a_held_named_quick_launch_shows_its_pill`]. Its switcher step stays, as in the old test,
/// because the switcher opening over a real pending launch is tested nowhere else (the switcher's own test of that uses
/// a host probe as the stand-in).
#[gpui::test]
fn the_chords_held_launch_names_its_session_and_host_in_the_pill(cx: &mut TestAppContext) {
    a_held_named_quick_launch_shows_its_pill(cx, marked(Some(NAMED_GO)), chord);
}

/// The number key's twin of the case above (the old spec's "template shortcut shows a named pending pill"): a number
/// key launches through the same path as ⚡, so a held one must show the same named pill and end the same way.
/// Specified: with key 1 on `named-go`, the walk of [`a_held_named_quick_launch_shows_its_pill`].
#[gpui::test]
fn a_number_keys_held_launch_names_its_session_and_host_in_the_pill(cx: &mut TestAppContext) {
    let bindings = bound(None, &[("1", NAMED_GO)]);
    a_held_named_quick_launch_shows_its_pill(cx, bindings, |cx| number_key(cx, 1));
}

/// The creates the window sent, unanswered, and the launch-history reads it made, every other command answered (the
/// catalog with [`catalog`], each history read with one whose only launch went into `/srv/last-work` on host1), round
/// after round until a round sends nothing but creates.
fn creates_and_history_reads(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
) -> (Vec<FakeCommand>, usize) {
    let history = json!({
        "launches": [{
            "host": 1, "github_repo": null, "canonical_cwd": "/srv/last-work", "cwd": "/srv/last-work",
            "selection": { "harness": "codex" }, "created_at": 1, "creation_seq": null,
        }],
        "folders": [],
        "checkout_config_revision": 1,
    });
    creates_answering_with(helm, cx, &catalog(), &history)
}

/// SPEC.md, "Working directory": with no session open on the host, New starts in "the directory of the newest recent
/// setup on that installation that launched into a plain folder", and a quick launch uses "exactly New's defaults".
/// The launcher waits out of sight for the host's history for that (`launcher::folder_default_tests`); through a
/// template's press it must still wait when the template leaves the folder alone, and need not when the template names
/// one: `apply` sets a template's folder with `edit_folder`, which marks it touched, so the history cannot change it,
/// and waiting would only hold the launch on a read it does not need. Specified: with no session open, ⚡ on `go` (no
/// folder) reads host1's history once and sends one create in `/srv/last-work`, the folder it names.
#[gpui::test]
fn a_quick_launch_without_a_folder_starts_in_the_hosts_last_folder(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    chord(cx);
    let (sent, reads) = creates_and_history_reads(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    assert_eq!(reads, 1, "the quick launch read the host's history");
    assert_eq!(
        create.body.clone().unwrap_or_default()["cwd"],
        json!("/srv/last-work")
    );
    assert!(!any_open(cx), "the launcher was never shown");
}

/// The other half of the case above. Specified: with no session open, ⚡ on `go-here`, `go` with its own folder
/// `/srv/here`, sends one create in `/srv/here` and reads no history.
#[gpui::test]
fn a_quick_launch_of_a_template_with_a_folder_reads_no_history(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    assert_eq!(
        cx.update(|_, cx| local_state::selected(cx)),
        None,
        "the premise: no session is open, so New's folder default is the host's"
    );
    assert!(!any_open(cx), "the premise: nothing is open");
    let go_here = TemplateFields {
        destination: Some(TemplateDestination::Folder("/srv/here".into())),
        ..runs_true()
    };
    push(&mut helm, cx, templates_with(template("go-here", go_here)));
    push(&mut helm, cx, marked(Some("go-here")));
    chord(cx);
    let (sent, reads) = creates_and_history_reads(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    assert_eq!(reads, 0, "the template's folder needs no history");
    assert_eq!(
        create.body.clone().unwrap_or_default()["cwd"],
        json!("/srv/here")
    );
}

/// SPEC.md: a template's wrapper prefix is one of the launch choices it carries, and a quick launch uses "exactly New's
/// defaults and ordinary launch path", so the create must carry the template's words (the old "quick launch submits the
/// wrapper prefix saved in its agent template"; the pieces are `apply_tests::a_wrapper_prefix_field_has_three_states`
/// and the launcher's request tests). Specified: ⚡ on a Codex template with the wrapper prefix `env A=1` sends one
/// create whose launch carries `["env", "A=1"]`, unseen.
#[gpui::test]
fn the_chord_launches_a_templates_wrapper_prefix(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    let wrapped = TemplateFields {
        kind: Some(LauncherKind::Agent),
        agent: Some(LaunchHarness::Codex),
        wrapper_prefix: Some(Some(vec!["env".into(), "A=1".into()])),
        ..TemplateFields::default()
    };
    push(&mut helm, cx, templates_with(template("wrapped", wrapped)));
    push(&mut helm, cx, marked(Some("wrapped")));
    chord(cx);
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    assert_eq!(
        create.body.clone().unwrap_or_default()["launch"]["wrapper_prefix"],
        json!(["env", "A=1"])
    );
    assert!(unseen(cx), "a quick launch New never showed");
}

/// SPEC.md, "Quick-launch sequences": a sequence's templates stack, later ones winning, and Enter "directly launches"
/// when together they make a complete setup, though one alone does not (the old "launch sequence stacks a number
/// template and bolt without showing New"; the pieces are `sequence_tests` and `a_sequences_enter_launches_unseen`).
/// Specified: with key 1 on `partial` (a folder only) and ⚡ on `go`, the prefix, 1, the ⚡ chord and Enter send one
/// create of `true` in `/srv/partial`, unseen.
#[gpui::test]
fn a_sequence_of_a_partial_key_and_the_mark_launches_the_stacked_setup(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, bound(Some("go"), &[("1", "partial")]));
    keys(
        cx,
        &format!("{SEQUENCE_PREFIX_CHORD} 1 {QUICK_LAUNCH_CHORD} enter"),
    );
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    let body = create.body.clone().unwrap_or_default();
    assert_eq!(body["cwd"], json!("/srv/partial"), "partial's folder");
    assert_eq!(body["command"]["command"], json!("true"), "go's command");
    assert!(!any_open(cx) && unseen(cx), "launched without showing New");
}

/// The old "a template shortcut preserves editor focus when clearing a quick-launch YOLO question": a press that
/// applies a template to the draft the question was about takes the question away (it would answer for another launch),
/// and must not also move the user's focus or drop their selection, since applying by shortcut "leaves focus and search
/// text alone" (SPEC.md). Specified: the chord's launch of `yolo-named` (YOLO, with the session name `keep this name`)
/// refused for YOLO; with the question up, focus put in the Name field and its whole text selected, and the question
/// still up, key 1 (`partial`) applies the folder, the question goes, and the Name field keeps focus, its text and the
/// selection over all of it (typing then replaces the whole name).
#[gpui::test]
fn a_shortcut_clearing_the_yolo_question_keeps_focus_and_selection(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    let named = TemplateFields {
        yolo: Some(true),
        name: Some("keep this name".into()),
        ..runs_true()
    };
    push(&mut helm, cx, templates_with(template("yolo-named", named)));
    push(
        &mut helm,
        cx,
        bound(Some("yolo-named"), &[("1", "partial")]),
    );
    chord(cx);
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    refuse_for_yolo(&mut helm, cx, create.id);
    assert!(
        label(cx, "yolo-reason").is_some(),
        "the premise: the question is up"
    );
    test_support::click(cx, "launcher-name");
    cx.update(|window, cx| {
        window.dispatch_action(crate::components::EditCommand::SelectAll.action(), cx)
    });
    settle(cx);
    assert_eq!(
        crate::launcher::view_tests::shown_name_focus_and_selection(cx),
        (true, true),
        "the premise: the name is focused and selected"
    );
    assert!(
        label(cx, "yolo-reason").is_some(),
        "the premise: the click and the selection left the question up"
    );
    number_key(cx, 1);
    assert_eq!(
        crate::launcher::view_tests::shown_draft(cx).folder.value(),
        "/srv/partial",
        "the template applied"
    );
    assert_eq!(label(cx, "yolo-reason"), None, "the question went");
    assert_eq!(
        crate::launcher::view_tests::shown_name_focus_and_selection(cx),
        (true, true),
        "focus and a selection stayed in the Name field"
    );
    assert_eq!(
        crate::launcher::view_tests::shown_draft(cx).name.value(),
        "keep this name",
        "the name is unchanged"
    );
    // Typing replaces exactly the selection, so a whole name replaced by one letter shows the selection still covered
    // all of it.
    cx.simulate_input("x");
    settle(cx);
    assert_eq!(
        crate::launcher::view_tests::shown_draft(cx).name.value(),
        "x",
        "the selection still covered the whole name"
    );
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
}

/// SPEC.md: "Unbound keys open Templates with a binding hint", and a key bound to a name no template has is as
/// unusable (the old dispatcher fell back to the hint for both). A key that did nothing would leave the user guessing
/// how to give it a template. Specified: with 1 on `deleted` and 0 unbound, each key sends no create and opens
/// Templates with the number-key hint.
#[gpui::test]
fn an_unbound_number_key_opens_templates_with_its_hint(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, bound(None, &[("1", "deleted")]));
    for digit in [0, 1] {
        assert!(!any_open(cx), "{digit}: the premise: nothing is open");
        number_key(cx, digit);
        assert!(
            creates(&mut helm, cx).is_empty(),
            "{digit}: nothing launched"
        );
        assert!(
            text(cx).contains("Give a template a number key"),
            "{digit}: Templates with its hint"
        );
        keys(cx, "escape");
    }
}

/// SPEC.md: "Any template can hold a number key, including one that cannot launch alone", and "Pressing such a key alone
/// opens New with the template applied and a prominent alert banner at the top explaining that it cannot launch on its
/// own and how to combine it with the sequence prefix. The banner expires about six seconds after application
/// finishes". Counting from the press instead would let a slow catalog read spend the time the banner is there to be
/// read in. Specified: with 1 on `partial` and the catalog read left unanswered for longer than the banner lasts,
/// nothing is drawn; once it answers, New shows the folder and the banner naming `partial` and the prefix, sends no
/// create, keeps the banner until just before six seconds have passed, and drops it at six, New staying open.
#[gpui::test]
fn a_partial_number_key_opens_new_with_the_prefix_banner_for_six_seconds(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, bound(None, &[("1", "partial")]));
    number_key(cx, 1);
    test_support::advance(cx, BANNER_SHOWS_FOR + Duration::from_secs(1));
    assert!(!any_open(cx), "the premise: the catalog read is still out");
    // This answers the catalog read, the templates apply, and the banner's six seconds start now.
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert!(
        text(cx).contains("/srv/partial"),
        "New, with the template applied"
    );
    let expected = format!(
        "partial cannot launch on its own. Use {SEQUENCE_PREFIX_NAME} to combine template keys, then Enter to \
         launch, or complete this setup below."
    );
    assert_eq!(banner(cx), Some(expected));
    test_support::advance(cx, BANNER_SHOWS_FOR - Duration::from_millis(100));
    assert!(
        banner(cx).is_some(),
        "the banner is still there just before six seconds"
    );
    test_support::advance(cx, Duration::from_millis(100));
    assert_eq!(banner(cx), None, "and gone at six");
    assert!(
        text(cx).contains("/srv/partial"),
        "New stayed, with its draft"
    );
}

/// The rest of the old "model-only number key opens New with a temporary composition explanation". SPEC.md has the
/// banner go "when the user starts editing": reading it is not an edit, so a click on it must leave it up, while a
/// choice made with a button is an edit as much as typing is. Specified: with 1 on `model-only`, key 1 opens New with
/// the number key's banner; a click on the banner leaves it, and a click on the Command tab takes it away, New
/// staying open.
#[gpui::test]
fn a_click_on_the_banner_keeps_it_and_a_tab_choice_takes_it_away(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, bound(None, &[("1", "model-only")]));
    number_key(cx, 1);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert!(
        banner(cx).is_some_and(|shown| shown.starts_with("model-only cannot launch on its own")),
        "the premise: New with the number key's banner"
    );
    test_support::click(cx, "launcher-banner");
    assert!(banner(cx).is_some(), "reading the banner is not an edit");
    assert_eq!(
        crate::launcher::view_tests::shown_draft(cx).tab,
        crate::launcher::draft::Tab::Agent,
        "the premise: the Command tab is a change"
    );
    test_support::click(cx, "launcher-tabs-Command");
    assert_eq!(banner(cx), None, "a button choice is an edit");
    assert!(any_open(cx), "New stays");
}

/// SPEC.md: a number key "applies it to an open session launcher draft under the same focus, ordering and refusal rules
/// as the ⚡ chord", and a partial template there needs no banner, since the user is already completing the draft;
/// launching from there would discard what they were preparing. Specified: with New open, 1 on `partial` and 2 on
/// `nowhere`, 1 puts the folder in the draft and says nothing; 2 shows its refusal and leaves the folder; 3, unbound,
/// refuses there by its key without opening Templates; nothing is sent and New stays open.
#[gpui::test]
fn number_keys_apply_to_the_open_launcher(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(
        &mut helm,
        cx,
        bound(None, &[("1", "partial"), ("2", "nowhere")]),
    );
    open_new(&mut helm, cx);
    number_key(cx, 1);
    assert!(text(cx).contains("/srv/partial"), "partial applied");
    assert_eq!((banner(cx), note(cx)), (None, None), "and said nothing");

    number_key(cx, 2);
    let shown = note(cx).expect("nowhere refused");
    assert!(shown.contains("nowhere"), "{shown:?}");
    assert!(
        text(cx).contains("/srv/partial"),
        "the refused press changed nothing"
    );

    number_key(cx, 3);
    assert_eq!(note(cx).as_deref(), Some("No template is bound to key 3"));
    assert!(
        !text(cx).contains("Give a template a number key"),
        "Templates did not open"
    );
    assert!(creates(&mut helm, cx).is_empty(), "nothing was launched");
    assert!(any_open(cx), "Launch is left to the user");
}

// ---- Save as template's panel ----------------------------------------------------------------------------------------

/// SPEC.md, "Quick launch": "while saving a template, the chord is consumed without changing the draft", and the old
/// launcher swallowed the sequence prefix while its Save as template panel was open. A shortcut that edited the draft
/// underneath the panel would launch later something the user did not see while saving.
/// Specified: with New open and Save as template's panel open, focus in its name field, the ⚡ chord and number key 1
/// (both on `partial`) leave the draft without `partial`'s folder and send nothing; with the templates then unavailable,
/// the chord says nothing either; the sequence prefix's action arms no sequence, from the panel's name field and, after
/// a click into New's folder field that leaves the panel open, from there.
#[gpui::test]
fn template_shortcuts_change_nothing_while_save_as_template_is_open(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("partial"));
    push(&mut helm, cx, bound(Some("partial"), &[("1", "partial")]));
    open_new(&mut helm, cx);
    test_support::click(cx, "launcher-save-template");
    assert!(panel_open(cx), "the premise: the panel is open");
    assert!(
        panel_name_focused(cx),
        "the premise: focus starts in the panel's name field"
    );
    chord(cx);
    number_key(cx, 1);
    assert!(creates(&mut helm, cx).is_empty(), "nothing was sent");
    assert!(!text(cx).contains("/srv/partial"), "the draft is unchanged");
    push(
        &mut helm,
        cx,
        templates(Availability::Unavailable {
            error: "busy".into(),
        }),
    );
    chord(cx);
    assert_eq!(note(cx), None, "an unavailable read is not said either");
    push(&mut helm, cx, templates(Availability::Available));

    let armed = |cx: &mut VisualTestContext| cx.update(|_, cx| super::armed::is_armed(cx));
    let begin = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| window.dispatch_action(Box::new(crate::actions::BeginSequence), cx));
        settle(cx);
    };
    begin(cx);
    assert!(
        !armed(cx),
        "from the panel's name field the prefix arms nothing"
    );
    test_support::click(cx, "launcher-folder-row");
    assert!(
        crate::launcher::view_tests::focused(cx, crate::launcher::view_tests::Field::Folder),
        "the premise: the click landed in the folder field"
    );
    assert!(panel_open(cx), "the premise: the panel is still open");
    keys(cx, SEQUENCE_PREFIX_CHORD);
    begin(cx);
    assert!(!armed(cx), "nor from New's folder field");
}

/// Whether Save as template's panel is drawn in the open launcher.
fn panel_open(cx: &mut VisualTestContext) -> bool {
    text(cx).contains("Choose which parts of this setup the template keeps")
}

/// A press made while the launcher's catalog read is out waits for it; if Save as template's panel opens meanwhile,
/// the waiting press must not apply when the read answers, or it would edit the draft under the panel after all.
/// Specified: New opened with its catalog read unanswered, the ⚡ chord on `partial`, then the panel opened; once the
/// read answers, the draft has no `partial` folder and nothing was sent.
#[gpui::test]
fn a_press_waiting_for_the_catalog_is_dropped_when_save_as_template_opens(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("partial"));
    assert!(
        cx.update(launcher::entry::open_new),
        "the premise: New opened"
    );
    settle(cx);
    chord(cx);
    assert!(
        !text(cx).contains("/srv/partial"),
        "the premise: the press waits for the catalog"
    );
    test_support::click(cx, "launcher-save-template");
    assert!(panel_open(cx), "the premise: the panel is open");
    assert!(
        creates(&mut helm, cx).is_empty(),
        "answering the catalog read sent nothing"
    );
    assert!(
        !text(cx).contains("/srv/partial"),
        "and the waiting press did not apply"
    );
}

// ---- Bindings that cannot be read ------------------------------------------------------------------------------------

/// The preferences collection as the helm pushes it when it cannot read them: unavailable, with `error` as its words,
/// and no entries, so the store keeps the last preferences it had (stale, not cleared).
pub(super) fn unreadable(error: &str) -> CollectionState {
    CollectionState {
        collection: Collection::Preferences,
        meta: CollectionMeta {
            availability: Availability::Unavailable {
                error: error.into(),
            },
            ..meta(&[farhelm_proto::ui_channel::PREFERENCES_KEY])
        },
        entries: Vec::new(),
    }
}

/// A template shortcut press, as a test makes it: the ⚡ chord or one number key.
type Press = fn(&mut VisualTestContext);

/// The real main view over a fake helm that has pushed no preferences at all: the sessions, hosts and templates of
/// [`window`], but no ⚡ mark or number keys, known or unknown. No preferences entry also keeps the Welcome card shut.
pub(super) fn window_without_preferences(
    cx: &mut TestAppContext,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![
        listing(&[], &[]),
        fixture_hosts(&[0, 1]),
        templates(Availability::Available),
    ]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    settle(window);
    (helm, window)
}

/// The old app read the preferences afresh for every press and refused when the read failed ("Couldn't prepare quick
/// launch:" with its words). Acting on the bindings the store last held instead launches whatever they named before
/// the helm lost them, and acting on none opens Templates to mark a template that may already be marked. Specified:
/// with ⚡ and key 1 on `go` and then the preferences unavailable ("database is locked"), the chord and key 1 each
/// send no create, open no Templates, and open New with "Not launched. Couldn't prepare quick launch: database is
/// locked", on the helm's own host. The old "a quick preparation error survives New's first host binding" checked that
/// binding New's defaults after the error did not clear it. New's host is bound as it opens, but its folder default and
/// the installation it is bound to settle later, from the host's launch history: so, specified too, New shows the error
/// before that default settles, and still shows it once a history read has settled it.
#[gpui::test]
fn unreadable_bindings_refuse_the_chord_and_number_keys_on_the_page(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, bound(Some("go"), &[("1", "go")]));
    push(&mut helm, cx, unreadable("database is locked"));
    let presses: [(&str, Press); 2] = [("chord", chord), ("key 1", |cx| number_key(cx, 1))];
    for (what, press) in presses {
        assert!(!any_open(cx), "{what}: the premise: nothing is open");
        press(cx);
        let error = Some("Not launched. Couldn't prepare quick launch: database is locked");
        assert_eq!(note(cx).as_deref(), error, "{what}");
        let shown = crate::launcher::view_tests::shown_draft(cx);
        assert_eq!(shown.host, Some(1), "{what}: on host1");
        assert!(
            shown.folder_install.is_none(),
            "{what}: the premise: New's default has not settled yet"
        );
        let (sent, history_reads) = creates_and_history_reads(&mut helm, cx);
        assert!(sent.is_empty(), "{what}: nothing launched");
        assert!(history_reads >= 1, "{what}: the premise: a history read");
        assert!(
            crate::launcher::view_tests::shown_draft(cx)
                .folder_install
                .is_some(),
            "{what}: the premise: New's default settled"
        );
        assert_eq!(note(cx).as_deref(), error, "{what}: the error survived it");
        assert!(
            !text(cx).contains("Mark a template with ⚡"),
            "{what}: Templates did not open"
        );
        keys(cx, "escape");
    }
}

/// Before the preferences have arrived the store holds no bindings, and an empty mark is no evidence that nothing is
/// marked: opening Templates with the ⚡ hint would tell the user to mark a template that may hold ⚡ already.
/// Specified: over a helm that pushed no preferences, the chord sends no create and opens New with "Not launched.
/// Couldn't prepare quick launch: the shortcut bindings have not arrived yet", not Templates.
#[gpui::test]
fn bindings_that_never_arrived_refuse_the_chord_on_the_page(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_without_preferences(cx);
    chord(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing launched");
    assert_eq!(
        note(cx).as_deref(),
        Some(
            "Not launched. Couldn't prepare quick launch: the shortcut bindings have not arrived yet"
        )
    );
    assert!(
        !text(cx).contains("Mark a template with ⚡"),
        "Templates did not open"
    );
}

/// In the open launcher a press with unknown bindings must change nothing, like any refused press there, and say why
/// where the launcher says a shortcut's refusal; the old launcher said "Couldn't load templates:" with the failed
/// read's words, since it read the preferences and the templates as one step. Specified: with ⚡ and key 1 on
/// `partial` and then the preferences unavailable ("database is locked"), the chord and key 1, each in a New of its
/// own that says nothing when it opens, show "Couldn't load templates: database is locked", leave the draft without
/// `partial`'s folder, and send nothing.
#[gpui::test]
fn unreadable_bindings_refuse_a_press_in_the_launcher(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, bound(Some("partial"), &[("1", "partial")]));
    push(&mut helm, cx, unreadable("database is locked"));
    let presses: [(&str, Press); 2] = [("chord", chord), ("key 1", |cx| number_key(cx, 1))];
    for (what, press) in presses {
        // A fresh New for each press, so the note read below is this press's and not one an earlier press left.
        open_new(&mut helm, cx);
        press(cx);
        assert_eq!(
            note(cx).as_deref(),
            Some("Couldn't load templates: database is locked"),
            "{what}"
        );
        assert!(
            !text(cx).contains("/srv/partial"),
            "{what}: the draft is unchanged"
        );
        assert!(
            creates(&mut helm, cx).is_empty(),
            "{what}: nothing was launched"
        );
        assert!(any_open(cx), "{what}: the launcher stays open");
        keys(cx, "escape");
        assert!(!any_open(cx), "{what}: the premise: Escape closed New");
    }
}

/// A sequence copies the bindings when its prefix is accepted, so a copy taken while they could not be read cannot say
/// what its number keys and ⚡ name; resolving through it would call a bound key unbound. A sequence of words names its
/// templates itself and reads no binding, so refusing it too would refuse more than the failure explains. Specified:
/// with key 1 on `go` and then the preferences unavailable ("database is locked"), the prefix, 1 and Enter send no
/// create and open New with "Not launched. Couldn't prepare quick launch: database is locked"; once that New is closed,
/// the prefix, `go` and Enter send one create.
#[gpui::test]
fn a_sequence_on_unreadable_bindings_refuses_its_keys_but_not_its_words(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, bound(None, &[("1", "go")]));
    push(&mut helm, cx, unreadable("database is locked"));
    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} 1 enter"));
    assert!(creates(&mut helm, cx).is_empty(), "key 1: nothing launched");
    assert_eq!(
        note(cx).as_deref(),
        Some("Not launched. Couldn't prepare quick launch: database is locked")
    );
    keys(cx, "escape");
    assert!(!any_open(cx), "the premise: New closed");

    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} g o enter"));
    assert_eq!(creates(&mut helm, cx).len(), 1, "`go` launched by its word");
}

// ---- Quick launch into a managed checkout ----------------------------------------------------------------------------

/// The helm's route for a managed checkout's preview.
const PREVIEW: &str = "/api/github-checkout-preview";
/// The supervisor's words refusing a checkout name that is taken, as the helm passes them on.
const TAKEN: &str = "a directory with that checkout name already exists; choose another title";
/// The helm's words refusing a create whose preview another create took meanwhile.
const LOST: &str =
    "the checkout preview is no longer current; obtain a new preview before resubmitting";

/// The helm's ready preview of a checkout of `acme/bar` in `/work/<basename>`, from host1's installation.
fn ready(basename: &str) -> serde_json::Value {
    json!({
        "canonical_root": "/work", "basename": basename, "cwd": format!("/work/{basename}"),
        "config_revision": 3, "host": "1", "incarnation": 4, "installation_identity": "identity1",
    })
}

/// Add `repo-go` to the helm's templates: the `go` command into a managed checkout of `acme/bar`, with the session name
/// `base` as a template base name.
fn push_repo_template(helm: &mut FakeHelm, cx: &mut VisualTestContext, base: &str) {
    let mut all = templates(Availability::Available);
    let repo_go = template(
        "repo-go",
        TemplateFields {
            destination: Some(TemplateDestination::Github("acme/bar".into())),
            name: Some(base.to_owned()),
            ..runs_true()
        },
    );
    all.meta.order.push(repo_go.name.clone());
    all.entries.push(KeyedEntry {
        key: repo_go.name.clone(),
        value: serde_json::to_value(&repo_go).unwrap(),
    });
    push(helm, cx, all);
}

/// What one round of a checkout quick launch's traffic does with each preview, by the name it asks for: answer it
/// ready, answer it taken, or hold it.
type PreviewAnswer = fn(&str, usize) -> Option<(u16, serde_json::Value)>;

/// Run the window's traffic for `rounds` rounds: each preview answered as `answer` says for its title and how many
/// previews of that title came before, each create collected unanswered, everything else answered plainly (the catalog
/// with [`catalog`]) while no preview is held. `between` runs after each round with the titles and creates so far, and
/// ends the run by returning `true`. Returns the preview titles in order and the creates.
fn run_checkout_traffic(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    rounds: usize,
    answer: PreviewAnswer,
    mut between: impl FnMut(&mut FakeHelm, &mut VisualTestContext, &[String], &[FakeCommand]) -> bool,
) -> (Vec<String>, Vec<FakeCommand>) {
    let mut titles: Vec<String> = Vec::new();
    let mut created = Vec::new();
    let mut holding = false;
    for _ in 0..rounds {
        cx.run_until_parked();
        for command in helm.commands() {
            if command.path == PREVIEW {
                let title = command.body.as_ref().unwrap()["title"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                let before = titles.iter().filter(|seen| **seen == title).count();
                titles.push(title.clone());
                match answer(&title, before) {
                    Some((status, body)) if !holding => helm.reply(command.id, status, Some(body)),
                    _ => holding = true,
                }
            } else if command.path == "/api/sessions" {
                created.push(command);
            } else if !holding {
                let body = command.path.ends_with("launch-catalog").then(catalog);
                helm.reply(command.id, 200, body);
            }
        }
        settle(cx);
        if between(helm, cx, &titles, &created) {
            break;
        }
    }
    (titles, created)
}

/// ⚡ on `repo-go` named by the base `review`, with the preview of `review` answered taken and `review-2` ready: the
/// previews it asked for and its one create, nothing shown at any point. `press` runs once, after the round that asked
/// for the preview of `review-2`.
fn numbered_quick_launch(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    mut press: impl FnMut(&mut VisualTestContext),
) -> FakeCommand {
    push_repo_template(helm, cx, "review");
    chord(cx);
    let mut pressed = false;
    let (titles, created) = run_checkout_traffic(
        helm,
        cx,
        8,
        |title, _| match title {
            "review" => Some((409, json!(TAKEN))),
            "review-2" => Some((200, ready("bar-review-2"))),
            _ => None,
        },
        |_, cx, titles, created| {
            assert!(!any_open(cx), "nothing is shown while it numbers and waits");
            if !pressed && titles.iter().any(|title| title == "review-2") {
                pressed = true;
                press(cx);
            }
            !created.is_empty()
        },
    );
    assert_eq!(titles, ["review", "review-2"]);
    let [create] = &created[..] else {
        panic!("one create, not {}", created.len());
    };
    let body = create.body.as_ref().unwrap();
    assert_eq!(body["title"], json!("review-2"), "{body}");
    assert_eq!(body["cwd"], json!("/work/bar-review-2"), "{body}");
    create.clone()
}

/// SPEC.md, "Managed checkouts": a template base name "uses its first free candidate", and a shortcut "launches without
/// showing the path first"; the old app's case "quick launch numbers a template base name when its checkout is taken",
/// which went on to name the session. A hidden quick launch must follow its own base name's numbering rather than open
/// New. Specified: the preview of `review` answered taken, the launch goes on to preview `review-2` and sends one create
/// titled `review-2` into its folder, nothing shown; its success opens the session with the naming editor over it, as
/// any quick launch New never showed.
#[gpui::test]
fn a_taken_base_name_is_numbered_and_launched_out_of_sight(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("repo-go"));
    let create = numbered_quick_launch(&mut helm, cx, |_| {});
    push(
        &mut helm,
        cx,
        listing(&["n"], &[session("n").title("review-2")]),
    );
    helm.reply(
        create.id,
        200,
        Some(session("n").title("review-2").entry().value),
    );
    settle(cx);
    assert_eq!(
        cx.update(|_, cx| local_state::selected(cx)).as_deref(),
        Some("n")
    );
    assert!(
        any_open(cx),
        "the naming editor opened over the new session"
    );
}

/// SPEC.md, "Keyboard focus": a result the user moved on from "moves no focus, opens no dialog". Numbering keeps the ⚡
/// press as the launch's moment, so a key pressed after the next candidate's preview went out still counts. Specified:
/// a key pressed once the preview of `review-2` was asked for; the launch still starts `review-2`, and its success says
/// "Started" in the launcher's report instead of opening the session or the naming editor.
#[gpui::test]
fn a_numbered_quick_launch_the_user_moved_on_from_starts_quietly(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("repo-go"));
    let create = numbered_quick_launch(&mut helm, cx, |cx| keys(cx, "x"));
    push(
        &mut helm,
        cx,
        listing(&["n"], &[session("n").title("review-2")]),
    );
    helm.reply(
        create.id,
        200,
        Some(session("n").title("review-2").entry().value),
    );
    settle(cx);
    assert!(
        label(cx, "launch-started-report").is_some(),
        "the success took the moved-on path"
    );
    assert!(!any_open(cx), "and opened nothing");
}

/// The old app's case "quick launch waits for a managed preview and reveals its path race", for a template base name: a
/// create that loses its previewed folder to another one must not quietly try another folder (SPEC.md: "A lost-race
/// base-name launch obtains a new preview and may advance, but requires another Launch click"). Specified: ⚡ on
/// `repo-go` named `review`, its preview of `review` ready and its create refused as never accepted; the refreshed
/// preview of `review` is now taken, and `review-2` ready; the launcher is shown with the helm's words and "nothing was
/// accepted" beside `review-2`'s folder, and no second create goes out.
#[gpui::test]
fn a_base_name_quick_launch_that_loses_its_folder_is_shown(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("repo-go"));
    push_repo_template(&mut helm, cx, "review");
    chord(cx);
    let mut refused = false;
    let (titles, created) = run_checkout_traffic(
        &mut helm,
        cx,
        10,
        |title, before| match (title, before) {
            ("review", 0) => Some((200, ready("bar-review"))),
            ("review", _) => Some((409, json!(TAKEN))),
            ("review-2", _) => Some((200, ready("bar-review-2"))),
            _ => None,
        },
        |helm, _, _, created| {
            if let Some(create) = created.first()
                && !refused
            {
                refused = true;
                helm.reply_marked(
                    create.id,
                    409,
                    vec![farhelm_proto::ui_channel::HelmMarker::CreateUnaccepted],
                    Some(json!(LOST)),
                );
            }
            false
        },
    );
    assert!(refused, "the premise: the create went out and was refused");
    assert_eq!(titles, ["review", "review", "review-2"]);
    assert_eq!(created.len(), 1, "no second create");
    assert!(any_open(cx), "the quick launch is shown");
    let shown = text(cx);
    assert!(
        shown.contains(LOST) && shown.contains("nothing was accepted"),
        "{shown}"
    );
    assert!(shown.contains("/work/bar-review-2"), "{shown}");
}
