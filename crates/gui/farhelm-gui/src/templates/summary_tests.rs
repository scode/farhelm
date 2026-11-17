//! Tests of the Templates list's one-line summaries.
//!
//! The oracle is the old app's `template_summary_parts` and its tests in `list/templates.rs` at reference commit
//! `24bc0580`. The words matter here beyond looks: the quick switcher and the Cmd+J list show the same summary, and
//! the list is how a user tells similar templates apart, so a field that silently drops out of the summary, or a value
//! drawn as plain words rather than host text, is a real regression.

use farhelm_proto::launcher::{LauncherKind, TemplateDestination, TemplateFields};
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchTarget, Place};

use super::summary::{Part, parts, plain};

/// A template that sets nothing says so, rather than drawing an empty line under its name.
#[test]
fn a_template_that_sets_nothing_says_so() {
    assert_eq!(plain(&TemplateFields::default()), "Sets nothing yet");
}

/// Every field a template can set is named in the old app's fixed order, joined by " · ", whatever order the fields
/// were added in. Commands and resume commands are named, never shown.
#[test]
fn fields_are_named_in_the_fixed_order() {
    let fields = TemplateFields {
        name: Some("review".into()),
        destination: Some(TemplateDestination::Folder("/src/app".into())),
        host: Some("install-one".into()),
        resume_command: Some(Some("tool --resume {conversation} {farhelm_args}".into())),
        yolo: Some(false),
        command: Some("tool {farhelm_args}".into()),
        wrapper_prefix: Some(Some(vec!["env".into(), "FOO='a b'".into()])),
        permission_flags: Some(vec!["--sandbox".into(), "read only".into()]),
        permissions: Some(Some(LaunchPermission::Custom)),
        effort: Some(Some(LaunchEffort::High)),
        model: Some(Some("gpt-6-luna".into())),
        agent: Some(LaunchHarness::Codex),
        kind: Some(LauncherKind::Agent),
        prompt: Some("hello".into()),
        ..TemplateFields::default()
    };
    assert_eq!(
        plain(&fields),
        "Initial prompt · Agent launch · Codex · gpt-6-luna · Effort High · Approvals Custom · --sandbox 'read only' · \
         Wrapper prefix env FOO='a b' · Command · Not YOLO · Resume command · Host · Destination · Name review"
    );
}

/// Values a template carries are host-supplied text and come back as host parts, shell words in the monospace face,
/// so the list draws them escaped; the app's own words around them stay words.
#[test]
fn template_values_are_host_text() {
    let fields = TemplateFields {
        model: Some(Some("model\u{200B}x".into())),
        wrapper_prefix: Some(Some(vec!["nice".into()])),
        name: Some("review".into()),
        ..TemplateFields::default()
    };
    assert_eq!(
        parts(&fields),
        vec![
            Part::Host("model\u{200B}x".into()),
            Part::Words(" · ".into()),
            Part::Words("Wrapper prefix ".into()),
            Part::Mono("nice".into()),
            Part::Words(" · ".into()),
            Part::Words("Name ".into()),
            Part::Host("review".into()),
        ]
    );
}

/// Explicit resets read as defaults, and the permission names what the launch would request: with no agent type a
/// reset depends on the agent; with Claude it is Claude's guarded mode by its own name; with Cursor, which has no
/// guarded mode, the agent's own setting.
#[test]
fn resets_and_permissions_read_as_the_launch_would_request() {
    let reset = |agent| TemplateFields {
        agent,
        model: Some(None),
        effort: Some(None),
        permissions: Some(None),
        wrapper_prefix: Some(None),
        resume_command: Some(None),
        ..TemplateFields::default()
    };
    assert_eq!(
        plain(&reset(None)),
        "Model default · Effort default · Approvals Agent-dependent mode · Wrapper prefix none · Resume default"
    );
    assert_eq!(
        plain(&reset(Some(LaunchHarness::Claude))),
        "Claude · Model default · Effort default · Approvals Guarded · Claude auto mode · Wrapper prefix none · \
         Resume default"
    );
    assert!(
        plain(&reset(Some(LaunchHarness::Cursor))).contains("Approvals Agent's own setting"),
        "Cursor has no guarded mode"
    );
    let yolo = TemplateFields {
        permissions: Some(Some(LaunchPermission::Yolo)),
        yolo: Some(true),
        ..TemplateFields::default()
    };
    assert_eq!(plain(&yolo), "Approvals YOLO · YOLO");
}

/// A template's placement is how a user tells a container template from a sandbox one in the list, so it reads as the
/// environment it makes, in the launcher's words: the old summary's "New " and `target_label` with no hosts, so a
/// container's place is its host's id ("New web container on host 1") and a sandbox reads "New web sandbox". The
/// label, which holds the recipe, is drawn as host text.
#[test]
fn a_placement_reads_as_the_environment_it_makes() {
    let placed = |place| TemplateFields {
        target: Some(LaunchTarget::Environment {
            recipe: "web".into(),
            place,
        }),
        ..TemplateFields::default()
    };
    assert_eq!(
        plain(&placed(Place::PodmanHost(1))),
        "New web container on host 1"
    );
    assert_eq!(plain(&placed(Place::TensorlakeAccount)), "New web sandbox");
    assert_eq!(
        parts(&placed(Place::TensorlakeAccount)),
        vec![Part::Words("New ".into()), Part::Host("web sandbox".into())]
    );
}

/// The retired workspace-trust field is never mentioned, since applying a template ignores it, and a retired container
/// field is named so the user can find and remove it.
#[test]
fn retired_fields_are_named_only_where_they_still_matter() {
    let fields = TemplateFields {
        workspace_trust: Some(Some(true)),
        container: Some(None),
        ..TemplateFields::default()
    };
    assert_eq!(plain(&fields), "Retired containerized reset");
}
