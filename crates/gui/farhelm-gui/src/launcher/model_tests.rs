//! Tests of the model field's rules, ported from the old app's `launch_composer.rs` tests at reference commit
//! `24bc0580`.

use farhelm_proto::LaunchHarness;
use farhelm_proto::launcher::LaunchCatalogModel;

use super::model::{
    ModelEnterTarget, ModelOption, model_enter_target, model_options, owned_elsewhere_message,
};

/// A catalog with one Codex and one Claude model, both named "...-fast".
fn catalog() -> Vec<LaunchCatalogModel> {
    vec![
        LaunchCatalogModel {
            id: "codex-fast".into(),
            family: "codex-fast".into(),
            family_word: None,
            latest: true,
            harness: LaunchHarness::Codex,
            efforts: vec![],
        },
        LaunchCatalogModel {
            id: "claude-fast".into(),
            family: "claude-fast".into(),
            family_word: None,
            latest: true,
            harness: LaunchHarness::Claude,
            efforts: vec![],
        },
    ]
}

fn model(id: &str, harness: LaunchHarness) -> ModelOption {
    ModelOption::Model {
        id: id.into(),
        harness,
        latest: true,
    }
}

/// An older known ID still picks its one agent when typed. With a different explicit agent, it keeps working as
/// that agent's custom ID. The older section is a display grouping and does not change either result.
#[test]
fn an_older_id_is_recognized_when_typed_without_reserving_its_spelling() {
    let older = LaunchCatalogModel {
        id: "codex-classic".into(),
        family: "codex-fast".into(),
        family_word: None,
        latest: false,
        harness: LaunchHarness::Codex,
        efforts: vec![],
    };
    let catalog = [older].into_iter().chain(catalog()).collect::<Vec<_>>();
    let older_option = ModelOption::Model {
        id: "codex-classic".into(),
        harness: LaunchHarness::Codex,
        latest: false,
    };
    assert_eq!(
        model_options(&catalog, None, "", false),
        [
            ModelOption::HarnessDefault,
            model("codex-fast", LaunchHarness::Codex),
            model("claude-fast", LaunchHarness::Claude),
            older_option.clone(),
        ]
    );
    let options = model_options(&catalog, Some(LaunchHarness::Codex), "CLASSIC", false);
    assert_eq!(
        options,
        [
            ModelOption::HarnessDefault,
            older_option.clone(),
            ModelOption::ShowAll
        ]
    );
    assert_eq!(
        model_enter_target(&options, None, "codex-classic", &catalog, None),
        ModelEnterTarget::Option(older_option)
    );
    assert_eq!(
        model_enter_target(
            &options,
            None,
            "codex-classic",
            &catalog,
            Some(LaunchHarness::Cursor)
        ),
        ModelEnterTarget::Custom {
            id: "codex-classic".into(),
            harness: LaunchHarness::Cursor
        }
    );
}

/// The list keeps the default row for every agent type and filters only model ids, case-insensitively; with an agent
/// type chosen it lists that type's models unless shown all, and ends with the show-all toggle; with none chosen every
/// model is already listed, so there is no toggle (its label would change nothing). Why: a renderer that invented a
/// requirement, or a toggle that does nothing, would leave the user unable to pick a model they can launch with.
#[test]
fn model_options_filter_and_keep_harness_defaults() {
    let catalog = catalog();
    assert_eq!(
        model_options(&catalog, Some(LaunchHarness::Codex), "FAST", false),
        vec![
            ModelOption::HarnessDefault,
            model("codex-fast", LaunchHarness::Codex),
            ModelOption::ShowAll,
        ]
    );
    assert_eq!(
        model_options(&catalog, Some(LaunchHarness::Codex), "", true),
        vec![
            ModelOption::HarnessDefault,
            model("codex-fast", LaunchHarness::Codex),
            model("claude-fast", LaunchHarness::Claude),
            ModelOption::ShowAll,
        ]
    );
    assert_eq!(
        model_options(&catalog, None, "", false),
        vec![
            ModelOption::HarnessDefault,
            model("codex-fast", LaunchHarness::Codex),
            model("claude-fast", LaunchHarness::Claude),
        ]
    );
    assert!(
        model_options(&catalog, Some(LaunchHarness::Grok), "", false).is_empty(),
        "an agent type with no model choice has no list"
    );
}

/// Enter applies only a row moved to, or the typed text's own meaning, never the first row by default, which could
/// erase a valid choice (SPEC.md, "Creation"). Specified: a row moved to wins over the text; blank text does nothing;
/// another agent type's model typed under Codex is said, not applied (typing never switches the agent); with no agent
/// chosen a known id names its catalog owner and spelling; a custom id keeps its exact bytes under the chosen agent,
/// and needs an agent when none is chosen; a stale row index does nothing.
#[test]
fn model_enter_target_distinguishes_navigation_and_draft_semantics() {
    let catalog = catalog();
    let codex = Some(LaunchHarness::Codex);
    let options = model_options(&catalog, codex, "fast", false);
    assert_eq!(
        model_enter_target(&options, Some(1), "unfinished-custom", &catalog, codex),
        ModelEnterTarget::Option(model("codex-fast", LaunchHarness::Codex)),
        "navigation wins over the draft"
    );
    assert_eq!(
        model_enter_target(&options, None, "  ", &catalog, codex),
        ModelEnterTarget::Nothing,
        "a blank draft keeps the current choice"
    );
    assert_eq!(
        model_enter_target(&options, None, "CLAUDE-FAST", &catalog, codex),
        ModelEnterTarget::OwnedElsewhere {
            id: "CLAUDE-FAST".into(),
            owners: vec![LaunchHarness::Claude],
        },
        "typing another agent's model does not switch the chosen agent"
    );
    assert_eq!(
        model_enter_target(&options, None, "CLAUDE-FAST", &catalog, None),
        ModelEnterTarget::Option(model("claude-fast", LaunchHarness::Claude)),
        "with no agent chosen, a known id names its canonical catalog owner"
    );
    assert_eq!(
        model_enter_target(&options, None, "CODEX-FAST", &catalog, codex),
        ModelEnterTarget::Option(model("codex-fast", LaunchHarness::Codex)),
        "the chosen agent's own model takes its catalog spelling"
    );
    assert_eq!(
        model_enter_target(&options, None, " custom-id ", &catalog, codex),
        ModelEnterTarget::Custom {
            id: " custom-id ".into(),
            harness: LaunchHarness::Codex,
        },
        "a custom id keeps its bytes and carries its agent"
    );
    assert_eq!(
        model_enter_target(&options, None, "custom-id", &catalog, None),
        ModelEnterTarget::NeedsHarness("custom-id".into()),
        "a custom id has no owner to infer"
    );
    assert_eq!(
        model_enter_target(&options, Some(options.len()), "custom-id", &catalog, codex),
        ModelEnterTarget::Nothing,
        "a stale row index does not fall through to the draft"
    );
}

/// The words for a typed model only other agents offer name each of them, so the user knows where to pick it.
#[test]
fn owned_elsewhere_names_every_owner() {
    assert_eq!(
        owned_elsewhere_message("m", &[LaunchHarness::Claude, LaunchHarness::Codex]),
        "m is offered by Claude or Codex; choose Claude or Codex to use it"
    );
}

/// OpenCode accepts a bare Zen model name as well as its `opencode/` spelling, and some bare names are also another
/// agent's catalog ids; comparing the typed text made Enter treat an OpenCode draft's bare name as that other agent's
/// model. Why this matters: OpenCode's own model would be refused. Specified: under OpenCode the bare spelling is
/// OpenCode's model and keeps the typed text, and the catalog spelling, in any case, is its catalog row; with no agent
/// chosen the bare name names its one plain owner; OpenCode's qualified id typed under Codex is said, not applied.
#[test]
fn bare_opencode_names_stay_opencode_models() {
    let catalog = vec![
        LaunchCatalogModel {
            id: "gpt-6-luna".into(),
            family: "luna".into(),
            family_word: None,
            latest: true,
            harness: LaunchHarness::Codex,
            efforts: vec![],
        },
        LaunchCatalogModel {
            id: "opencode/gpt-6-luna".into(),
            family: "luna".into(),
            family_word: None,
            latest: true,
            harness: LaunchHarness::OpenCode,
            efforts: vec![],
        },
    ];
    let opencode = Some(LaunchHarness::OpenCode);
    assert_eq!(
        model_enter_target(&[], None, "gpt-6-luna", &catalog, opencode),
        ModelEnterTarget::Custom {
            id: "gpt-6-luna".into(),
            harness: LaunchHarness::OpenCode,
        },
        "the bare spelling is OpenCode's model and keeps the typed text"
    );
    assert_eq!(
        model_enter_target(&[], None, "OpenCode/GPT-6-Luna", &catalog, opencode),
        ModelEnterTarget::Option(model("opencode/gpt-6-luna", LaunchHarness::OpenCode)),
        "the catalog spelling canonicalizes to its row"
    );
    assert_eq!(
        model_enter_target(&[], None, "gpt-6-luna", &catalog, None),
        ModelEnterTarget::Option(model("gpt-6-luna", LaunchHarness::Codex)),
        "with no agent chosen the bare name names its plain owner"
    );
    assert_eq!(
        model_enter_target(
            &[],
            None,
            "opencode/gpt-6-luna",
            &catalog,
            Some(LaunchHarness::Codex)
        ),
        ModelEnterTarget::OwnedElsewhere {
            id: "opencode/gpt-6-luna".into(),
            owners: vec![LaunchHarness::OpenCode],
        },
        "OpenCode's qualified id does not switch a Codex draft"
    );
}

/// Under OpenCode, its bare spelling typed in another case still names the catalog row, canonical spelling included,
/// since the helm compares spellings exactly. Why: it is the one input that tells the exact-spelling rule apart from a
/// case-insensitive one (old `bare_opencode_names_stay_opencode_models`, its upper-case case).
#[test]
fn a_bare_opencode_name_in_another_case_is_its_catalog_row() {
    let catalog = vec![LaunchCatalogModel {
        id: "opencode/gpt-6-luna".into(),
        family: "luna".into(),
        family_word: None,
        latest: true,
        harness: LaunchHarness::OpenCode,
        efforts: vec![],
    }];
    assert_eq!(
        model_enter_target(
            &[],
            None,
            "GPT-6-LUNA",
            &catalog,
            Some(LaunchHarness::OpenCode)
        ),
        ModelEnterTarget::Option(model("opencode/gpt-6-luna", LaunchHarness::OpenCode))
    );
}
