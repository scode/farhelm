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
use farhelm_proto::LaunchHarness;
use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateDestination, TemplateFields};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, KeyedEntry,
    Preferences,
};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::SEQUENCE_PREFIX_NAME;
use super::dispatch::{Origin, Reference, incomplete_banner, resolve};
use super::sequence::{self, Binding, Refusal};
use super::shortcuts::Bindings;
use super::summary::Part;
use crate::actions::{QUICK_LAUNCH_CHORD, SEQUENCE_PREFIX_CHORD};
use crate::launcher::launch_tests::creates_answering;
use crate::launcher::view::BANNER_SHOWS_FOR;
use crate::launcher::view_tests::fixture_hosts;
use crate::launcher::{self, launch};
use crate::main_view::MainView;
use crate::notices::{self, Notice};
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
fn number_key(cx: &mut VisualTestContext, digit: u8) {
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

/// The point of quick launch is a session without the launcher: SPEC.md, "starts it directly in the desktop app when
/// no dialog is open, without showing the launcher on success", through "exactly New's defaults and ordinary launch
/// path", and success opens the session the user waited for. The "never shown" mark must hold until that success, since
/// naming the session turns on it. Specified: with `go` marked, the chord sends one create of New's prefill (host1,
/// `~`) with the template's command, opens no layer and leaves the launch marked as a quick launch never shown; a press
/// while the create is out sends nothing more, and the sequence prefix arms nothing then (the old app refused it while a
/// launch was pending); a create still out two seconds later shows the launcher's progress pill, as any hidden launch
/// does (SPEC.md: "Once the request has waited about two seconds, a small pill ... shows"), since a quick launch is
/// nothing but a launch whose launcher was never drawn; once the helm lists the new session and answers, the pill goes
/// and the session is selected.
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
        cx.update(|_, cx| local_state::selected(cx)).as_deref(),
        Some("n")
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
        shown.contains("Codex · "),
        "the footer names Codex as the agent type: {shown:?}"
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

/// A quick launch being prepared holds the launch slot, and SPEC.md keeps the launcher busy then as during a launch:
/// New must not open over it, another shortcut or the sequence prefix must not start a second one (the old app refused
/// the prefix while a launch was pending, too), and a result the user moved on from must not open over what they went
/// on to (SPEC.md, "Keyboard focus"). Specified: with `partial` marked and the chord pressed, before the catalog read
/// answers, New refuses to open, a second chord opens nothing, and the prefix arms no sequence; after a key press, the
/// read's answer opens nothing, sends no create, and the "launch not started" notice offers the launcher.
#[gpui::test]
fn presses_during_a_preparation_wait_and_its_result_respects_the_moved_on_rule(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = window(cx, Some("partial"));
    chord(cx);
    assert!(!any_open(cx), "the premise: the preparation draws nothing");
    assert!(
        !cx.update(launcher::entry::open_new),
        "New waits for the preparation"
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

/// A sequence's Enter outside the launcher "directly launches through the same defaults, validation and admission as
/// quick launch" (SPEC.md). Specified: the prefix, `go` and Enter send one create, with no layer opened and the launch
/// marked as one New never showed.
#[gpui::test]
fn a_sequences_enter_launches_unseen(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} g o enter"));
    assert_eq!(creates(&mut helm, cx).len(), 1, "`go` launched");
    assert!(!any_open(cx), "unseen");
    assert!(unseen(cx));
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
/// unbound key "shows a refusal above the search box without changing the draft". Specified: with New open and focus
/// moved to the host button, the prefix arms the "Apply sequence:" pill;
/// `partial` and Enter put its folder in the draft and send nothing; `zzz` and Enter show "No template matches"; the
/// unbound key 9 and Enter show "No template is bound to key 9" and open no Templates.
#[gpui::test]
fn a_sequence_in_the_launcher_applies_or_refuses_there(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    open_new(&mut helm, cx);
    // Focus starts in the search box; Tab walks on to the host button, after any destination choice drawn before it.
    for _ in 0..4 {
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

/// Whether focus is in the open Save as template panel's name field.
fn panel_name_focused(cx: &mut VisualTestContext) -> bool {
    crate::launcher::view_tests::read(cx, |launcher, window, cx| {
        launcher
            .save_panel_for_tests()
            .is_some_and(|panel| panel.read(cx).name_focus(cx).is_focused(window))
    })
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
/// locked".
#[gpui::test]
fn unreadable_bindings_refuse_the_chord_and_number_keys_on_the_page(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, None);
    push(&mut helm, cx, bound(Some("go"), &[("1", "go")]));
    push(&mut helm, cx, unreadable("database is locked"));
    let presses: [(&str, Press); 2] = [("chord", chord), ("key 1", |cx| number_key(cx, 1))];
    for (what, press) in presses {
        assert!(!any_open(cx), "{what}: the premise: nothing is open");
        press(cx);
        assert!(
            creates(&mut helm, cx).is_empty(),
            "{what}: nothing launched"
        );
        assert_eq!(
            note(cx).as_deref(),
            Some("Not launched. Couldn't prepare quick launch: database is locked"),
            "{what}"
        );
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
