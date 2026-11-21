//! Tests of the ⚡ mark's and the number keys' rules, without a window: who holds what, how a key moves, what a write
//! sends, and the hover help.
//!
//! The oracle is SPEC.md ("Quick launch", "Template number shortcuts") and the old app's `bind_number` and hover-help
//! wording in `list/templates.rs` at reference commit `24bc0580`. These rules are easy to get subtly wrong (a key left
//! on two templates, a write that drops someone else's key), and the helm stores whatever the GUI writes, so they are
//! pinned here rather than only through the dialog.

use std::collections::BTreeMap;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::LaunchHarness;
use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateFields};
use serde_json::json;

use crate::operations::{Command, Work};

use super::shortcuts::{
    Bindings, KEYS, bolt_help, key_button_text, key_help, keys_command, keys_renamed, mark_command,
    mark_write, may_mark,
};

fn bindings(marked: Option<&str>, keys: &[(&str, &str)]) -> Bindings {
    Bindings {
        marked: marked.map(str::to_owned),
        keys: keys
            .iter()
            .map(|(key, name)| ((*key).to_owned(), (*name).to_owned()))
            .collect(),
    }
}

fn map(keys: &[(&str, &str)]) -> BTreeMap<String, String> {
    bindings(None, keys).keys
}

/// The menu offers the keys in SPEC.md's order, 1 to 9 then 0.
#[test]
fn the_keys_are_one_to_nine_then_zero() {
    assert_eq!(KEYS, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"]);
}

/// Choosing a key moves it: the template's old key is freed, and the chosen key leaves whichever template held it, so a
/// key always has one template and a template one key. Every other key is kept as it was.
#[test]
fn choosing_a_key_moves_it_and_frees_the_old_one() {
    let before = bindings(None, &[("1", "codex"), ("2", "claude"), ("5", "review")]);
    assert_eq!(
        before.keys_with("codex", Some("2")),
        map(&[("2", "codex"), ("5", "review")]),
        "codex leaves 1 and takes 2 from claude"
    );
    assert_eq!(
        before.keys_with("codex", None),
        map(&[("2", "claude"), ("5", "review")])
    );
    assert_eq!(
        before.keys_with("new", Some("0")),
        map(&[
            ("0", "new"),
            ("1", "codex"),
            ("2", "claude"),
            ("5", "review")
        ])
    );
}

/// The mark and a key are looked up by the template's raw name, and a stored map that somehow gives one template two
/// keys reads as the first in the menu's order, so the button and the menu agree.
#[test]
fn lookups_go_by_raw_name() {
    let held = bindings(Some("codex"), &[("0", "codex"), ("3", "codex")]);
    assert!(held.is_marked("codex"));
    assert!(!held.is_marked("Codex"));
    assert_eq!(held.key_of("codex"), Some("3"));
    assert_eq!(held.key_of("claude"), None);
}

/// A mark write names only the mark's field, as the name or `null`; a key write names only the keys' field, the whole
/// map. The helm merges preference writes per field, so neither may carry another preference.
#[test]
fn writes_patch_only_their_own_field() {
    let expected = |body| {
        Command::new(
            Work::Save,
            Method::Put,
            ApiPath::api(&["preferences"]).unwrap(),
            Some(body),
            "PUT /api/preferences",
        )
    };
    assert_eq!(
        mark_command(Some("codex")).unwrap(),
        expected(json!({"quick_launch_template": "codex"}))
    );
    assert_eq!(
        mark_command(None).unwrap(),
        expected(json!({"quick_launch_template": null}))
    );
    assert_eq!(
        keys_command(&map(&[("1", "codex"), ("0", "review")])).unwrap(),
        expected(json!({"template_shortcuts": {"0": "review", "1": "codex"}}))
    );
}

/// Only a template that can launch on its own may be marked, by the rule the helm shares: an agent launch needs an
/// agent type, a command launch a nonblank command, and no kind is judged as an agent launch.
#[test]
fn only_a_launchable_template_may_be_marked() {
    let template = |fields| LaunchTemplate {
        name: "t".into(),
        fields,
    };
    assert!(may_mark(&template(TemplateFields {
        agent: Some(LaunchHarness::Codex),
        ..TemplateFields::default()
    })));
    assert!(!may_mark(&template(TemplateFields {
        model: Some(Some("gpt-6-luna".into())),
        ..TemplateFields::default()
    })));
    assert!(!may_mark(&template(TemplateFields {
        kind: Some(LauncherKind::Command),
        command: Some("  ".into()),
        ..TemplateFields::default()
    })));
    assert!(
        !may_mark(&template(TemplateFields {
            command: Some("make dev".into()),
            ..TemplateFields::default()
        })),
        "a kindless command-only template is judged as an agent launch"
    );
}

/// The hover help says what each control does, that it belongs to one template, and the current choice, in the old
/// list's words; a partial template's key explains composition with the sequence prefix.
#[test]
fn hover_help_names_the_behavior_and_the_choice() {
    let chord = super::QUICK_LAUNCH_SHORTCUT_NAME;
    assert_eq!(
        bolt_help(true, true),
        format!(
            "{chord} starts a session from this template right away, without opening New. Only one template can hold \
             ⚡. Click to turn off."
        )
    );
    assert!(bolt_help(false, false).ends_with(
        "Can't mark this one: an agent launch needs an agent; a command launch needs a nonblank command."
    ));
    assert!(bolt_help(false, true).ends_with("Click to mark this one."));
    assert_eq!(
        key_help(true, Some("4")),
        format!(
            "A number shortcut starts a session from this template right away, without opening New. Only one \
             template can hold each key. {} is set; click to change or remove it.",
            super::number_key_name("4")
        )
    );
    assert!(key_help(false, None).starts_with(&format!(
        "This template cannot launch on its own. Its key opens New with it applied; {} starts a sequence",
        super::SEQUENCE_PREFIX_NAME
    )));
    assert!(key_help(false, None).ends_with("Click to give this template a number key."));
    assert_eq!(key_button_text(None), "+");
    assert_eq!(
        key_button_text(Some("7")),
        if cfg!(target_os = "macos") {
            "⌘7"
        } else {
            "7"
        }
    );
}

/// A bolt click means what the bolt showed when it was clicked: an unlit bolt marks, a lit one clears. When the store
/// already says what the click asks for (an agent marked the same template just before the redraw), nothing is written,
/// so the click never undoes the agent's change by flipping what the store holds now, and a click meant to clear one
/// template's mark never clears another's.
#[test]
fn a_bolt_click_writes_what_the_drawn_bolt_meant() {
    assert_eq!(mark_write("codex", false, None), Some(Some("codex")));
    assert_eq!(
        mark_write("codex", false, Some("claude")),
        Some(Some("codex")),
        "marking moves the mark"
    );
    assert_eq!(mark_write("codex", true, Some("codex")), Some(None));
    assert_eq!(
        mark_write("codex", false, Some("codex")),
        None,
        "an agent already marked it"
    );
    assert_eq!(
        mark_write("codex", true, None),
        None,
        "the mark is already gone"
    );
    assert_eq!(
        mark_write("codex", true, Some("claude")),
        None,
        "clearing codex's mark never clears the mark an agent moved to claude"
    );
}

/// SPEC.md: "Rename keeps a key". A rename moves the old name's key to the new name and must leave every other key
/// where it is, since the map is written whole and a key dropped here would be unbound for good. Specified: the key the
/// old name held now names the new one, the others are unchanged, and a map in which the old name holds nothing comes
/// back unchanged (so the rename writes nothing).
#[test]
fn a_rename_moves_only_the_old_names_key() {
    let keys = map(&[("1", "old"), ("2", "other")]);
    assert_eq!(
        keys_renamed(&keys, "old", "new"),
        map(&[("1", "new"), ("2", "other")])
    );
    let untouched = map(&[("2", "other")]);
    assert_eq!(keys_renamed(&untouched, "old", "new"), untouched);
}
