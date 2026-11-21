//! Tests of the composer search's rules: what a query means, which rows it offers in which order, which row is
//! preselected and what an empty result says. Ported from the old app's `launch_composer.rs` search tests at reference
//! commit `24bc0580` (named in each test's docs), minus the rows whose data the app does not read yet (Browse and
//! repositories).
//!
//! Every test calls [`search`] with plain inputs, no window: the search is a function of the query and the draft's
//! values, so the view's tests only need to show that a row it draws applies.

use farhelm_proto::launcher::LaunchCatalogModel;
use farhelm_proto::session_requests::{FolderHistoryEntry, LaunchHistoryEntry, LaunchHistoryReply};
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection};

use super::draft::Tab;
use super::search::{
    Group, Inputs, Message, Results, Row, Scope, SearchPermission, scoped, search,
};
use crate::host_text::{HostSentence, HostText};
use crate::hosts::tests::{aliased, connected, connecting, host};
use crate::templates::rows::Row as TemplateRow;

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// A catalog with two Codex models (one named so it contains an effort's word) and a Claude model named exactly as
/// its agent type, so the exact-match priority between groups can be pinned.
fn catalog() -> Vec<LaunchCatalogModel> {
    vec![
        LaunchCatalogModel {
            id: "gpt-fast".into(),
            family: "gpt-fast".into(),
            latest: true,
            harness: LaunchHarness::Codex,
            efforts: vec![LaunchEffort::Low, LaunchEffort::High],
        },
        LaunchCatalogModel {
            id: "medium-context".into(),
            family: "medium-context".into(),
            latest: true,
            harness: LaunchHarness::Codex,
            efforts: vec![LaunchEffort::Medium],
        },
        LaunchCatalogModel {
            id: "claude".into(),
            family: "sonnet".into(),
            latest: true,
            harness: LaunchHarness::Claude,
            efforts: vec![LaunchEffort::Medium, LaunchEffort::Max],
        },
    ]
}

/// The local host under an alias; an ssh host that is not connected, so its list name carries its phase; and a
/// connected ssh host named `build`, listed after `build.example` so an exact `host:build` must pick the second row.
fn hosts() -> Vec<HostView> {
    let mut local = host(1, "local", connected());
    local.name = "workstation alias".to_owned();
    vec![
        local,
        aliased(
            host(2, "ssh", connecting(1, None)),
            "build.example",
            "dev@build.example",
        ),
        aliased(host(3, "ssh", connected()), "build", "dev@build"),
    ]
}

/// What a search reads: the tab, the chosen agent type and model, the catalog and whether its read is still out, the
/// registry's hosts, the templates' rows and the chosen host's launch history. The constructors start from [`catalog`]
/// as read and [`hosts`], with no model chosen, no templates and no history read.
struct Draft {
    tab: Tab,
    harness: Option<LaunchHarness>,
    model: Option<&'static str>,
    catalog: Option<Vec<LaunchCatalogModel>>,
    catalog_pending: bool,
    hosts: Vec<HostView>,
    templates: Result<Vec<TemplateRow>, HostSentence>,
    history: Option<LaunchHistoryReply>,
}

impl Draft {
    fn agent(harness: Option<LaunchHarness>) -> Self {
        Draft {
            tab: Tab::Agent,
            harness,
            model: None,
            catalog: Some(catalog()),
            catalog_pending: false,
            hosts: hosts(),
            templates: Ok(Vec::new()),
            history: None,
        }
    }

    fn command() -> Self {
        Draft {
            tab: Tab::Command,
            ..Draft::agent(None)
        }
    }

    fn search(&self, query: &str) -> Results {
        let placement = placement();
        search(&Inputs {
            query,
            tab: self.tab,
            harness: self.harness,
            model: self.model,
            catalog: self.catalog.as_deref(),
            catalog_pending: self.catalog_pending,
            hosts: &self.hosts,
            templates: self.templates.as_deref(),
            history: self.history.as_ref(),
            host: Some(&placement),
            repositories: &[],
        })
    }

    fn rows(&self, query: &str) -> Vec<Row> {
        self.search(query).rows
    }
}

fn permissions(list: &[SearchPermission]) -> Vec<Row> {
    list.iter().copied().map(Row::Permission).collect()
}

fn model(id: &str, harness: LaunchHarness) -> Row {
    Row::Model {
        id: id.to_owned(),
        harness,
        latest: true,
    }
}

/// A family word can prefer a later option without moving it in search. With no agent selected,
/// its first offering wins unless the word is itself an exact agent name.
#[test]
fn exact_family_words_preselect_current_models_without_reordering() {
    let row = |id: &str, harness, family: &str, latest| LaunchCatalogModel {
        id: id.into(),
        family: family.into(),
        latest,
        harness,
        efforts: Vec::new(),
    };
    let mut draft = Draft {
        catalog: Some(vec![
            row("gpt-6.1-sol", LaunchHarness::Codex, "sol", false),
            row("solstice", LaunchHarness::Codex, "solstice", true),
            row("gpt-6.2-sol", LaunchHarness::Codex, "sol", true),
            row("openai/gpt-6.2-sol", LaunchHarness::Pi, "sol", true),
            row("x-ai/grok-4.5", LaunchHarness::Pi, "grok", false),
            row("x-ai/grok-4.6", LaunchHarness::Pi, "grok", true),
        ]),
        ..Draft::agent(None)
    };
    let exact = draft.search("SoL");
    assert_eq!(exact.rows[0], model("solstice", LaunchHarness::Codex));
    assert_eq!(exact.default, 1);
    assert_eq!(
        exact.rows[exact.default],
        model("gpt-6.2-sol", LaunchHarness::Codex)
    );
    assert_eq!(draft.search("so").default, 0);
    let grok = draft.search("GrOk");
    let current = model("x-ai/grok-4.6", LaunchHarness::Pi);
    assert!(
        grok.rows.contains(&current),
        "the model family also matches"
    );
    assert_eq!(grok.rows[grok.default], Row::Harness(LaunchHarness::Grok));
    for query in ["grok-", "grok-4.6", "model:grok"] {
        let result = draft.search(query);
        assert_eq!(result.rows[result.default], current, "{query}");
    }
    draft.harness = Some(LaunchHarness::Codex);
    assert!(draft.rows("grok").is_empty(), "Codex has no Grok model");
    draft.harness = Some(LaunchHarness::Pi);
    let result = draft.search("grok");
    assert_eq!(result.rows[result.default], current);
}

/// The reserved model family only defers the spelling when the user means a model.
#[test]
fn bare_grok_can_choose_its_agent_before_the_catalog_arrives() {
    let draft = Draft {
        catalog: None,
        catalog_pending: true,
        ..Draft::agent(None)
    };
    let result = draft.search("grok");
    assert_eq!(
        result.rows[result.default],
        Row::Harness(LaunchHarness::Grok)
    );
    let model = draft.search("model:grok");
    assert!(model.rows.is_empty());
    assert_eq!(model.message.unwrap().text, "Loading models…");
}

/// An exact family query still means that family before the catalog arrives. It must not preselect an older
/// launch or set the session name. Other typed names still have their usual fallback.
#[test]
fn a_family_search_waits_for_the_release_catalog() {
    let draft = Draft {
        catalog: None,
        catalog_pending: true,
        ..with_history(
            Some(LaunchHarness::Codex),
            vec![launch("/a", LaunchHarness::Codex, Some("gpt-6.1-sol"))],
            &[],
        )
    };
    assert!(
        draft
            .rows("gpt-6.1-sol")
            .contains(&history_model("gpt-6.1-sol", LaunchHarness::Codex)),
        "the premise: the older Sol is still selectable by its ID"
    );
    let pending = draft.search("sol");
    assert!(pending.rows.is_empty());
    assert_eq!(
        pending.message,
        Some(Message {
            text: "Loading models…",
            warning: false
        })
    );
    assert_eq!(draft.rows("scratch"), vec![Row::Name("scratch".to_owned())]);
}

/// Exact older IDs can be selected from model search. An earlier setup remains available as history even when
/// the older catalog row is also present. The grouping is based on catalog generation, not table order.
#[test]
fn older_model_search_groups_known_rows_and_keeps_past_launches() {
    let older = LaunchCatalogModel {
        id: "gpt-fast-classic".into(),
        family: "gpt-fast".into(),
        latest: false,
        harness: LaunchHarness::Codex,
        efforts: vec![],
    };
    let past = launch("/a", LaunchHarness::Codex, Some(&older.id));
    let draft = Draft {
        catalog: Some([older].into_iter().chain(catalog()).collect()),
        ..with_history(Some(LaunchHarness::Codex), vec![past.clone()], &[])
    };
    let broad = draft.search("model:gpt-fast");
    assert_eq!(
        broad
            .groups()
            .into_iter()
            .map(|(group, _)| group)
            .collect::<Vec<_>>(),
        [Group::Models, Group::OlderModels]
    );
    assert_eq!(broad.rows[0], model("gpt-fast", LaunchHarness::Codex));
    assert_eq!(
        broad.rows[1],
        history_model("gpt-fast-classic", LaunchHarness::Codex)
    );
    let older_row = Row::Model {
        id: "gpt-fast-classic".into(),
        harness: LaunchHarness::Codex,
        latest: false,
    };
    assert_eq!(broad.rows[2], older_row);
    assert_eq!(draft.rows("recent:gpt-fast-classic"), [recent(past)]);

    let without_history = Draft {
        history: None,
        ..draft
    };
    let exact = without_history.search("model:gpt-fast-classic");
    assert_eq!(exact.rows[exact.default], older_row);
    let claude = Draft {
        harness: Some(LaunchHarness::Claude),
        ..without_history
    };
    assert!(claude.rows("model:gpt-fast-classic").is_empty());
}

fn efforts(list: &[Option<LaunchEffort>]) -> Vec<Row> {
    list.iter().copied().map(Row::Effort).collect()
}

fn name(text: &str) -> Vec<Row> {
    vec![Row::Name(text.to_owned())]
}

/// A draft on the Agent tab with no agent type, holding the templates `names` in that order, as the templates' rows
/// give them (only the name matters to the search).
fn with_templates(names: &[&str]) -> Draft {
    Draft {
        templates: Ok(names
            .iter()
            .map(|name| TemplateRow {
                name: (*name).to_owned(),
                summary: Vec::new(),
                marked: false,
                key: None,
            })
            .collect()),
        ..Draft::agent(None)
    }
}

/// The template rows of `rows`, by name.
fn template_names(rows: &[Row]) -> Vec<&str> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Template(name) => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

// ---- The query -----------------------------------------------------------------------------------------------------

/// Only a known label before the first colon scopes a query, case-insensitively and with both sides trimmed;
/// anything else keeps the whole query unlabelled. Why: a model id such as `vendor:model:v2`, or a folder typed with a
/// space before its colon, must keep its ordinary meaning rather than turn into an empty scope (old
/// `scoped_query_recognizes_known_labels_without_reparsing_unknown_ones`).
#[test]
fn only_known_labels_before_the_first_colon_scope_a_query() {
    assert_eq!(scoped("  HARNESS: cod "), (Scope::Harness, "cod"));
    assert_eq!(
        scoped("model:provider/model:v2"),
        (Scope::Model, "provider/model:v2")
    );
    assert_eq!(scoped("Perms:"), (Scope::Perms, ""));
    assert_eq!(scoped("TL:x"), (Scope::Templates, "x"));
    assert_eq!(scoped("gh:owner/repo"), (Scope::Github, "owner/repo"));
    assert_eq!(scoped("folder : /tmp"), (Scope::All, "folder : /tmp"));
    assert_eq!(scoped("vendor:model:v2"), (Scope::All, "vendor:model:v2"));
    assert_eq!(scoped(" plain "), (Scope::All, "plain"));
}

/// The retired `trust:` labels and words that look like commands are ordinary text: they offer nothing but the name
/// fallback. Why: search only ever changes the draft, so no text typed into it may act on its own (old
/// `trust_labels_no_longer_offer_launch_actions` and `search_never_treats_its_text_as_a_command`).
#[test]
fn unknown_labels_and_command_words_only_name_the_session() {
    let draft = Draft::agent(Some(LaunchHarness::Codex));
    for query in ["trust:", "trust:true", "trust:false", "command", "run-this"] {
        assert_eq!(draft.rows(query), name(query), "{query}");
    }
}

// ---- The name fallback and the missing harness ---------------------------------------------------------------------

/// Unlabelled text that matched nothing offers "Set session name" with the trimmed text, colons and all; blank text,
/// labelled text and text that matched something offer no fallback. Why: Enter on a typed name must name the session,
/// but a scoped query with no match has to say so rather than quietly become a name (old
/// `unmatched_search_offers_only_explicit_name_fallbacks`).
#[test]
fn only_unmatched_unlabelled_text_falls_back_to_a_name() {
    let draft = Draft::agent(None);
    for query in ["asdf", "  foo:bar  ", "highway", "yolanda"] {
        assert_eq!(draft.rows(query), name(query.trim()), "{query}");
    }
    for query in [
        "",
        "   ",
        "host:missing",
        "name:",
        "gh:bad",
        "model:bad",
        "tl:",
        "folder:x",
        "recent:",
    ] {
        assert!(draft.rows(query).is_empty(), "{query}");
    }
    assert_eq!(
        draft.rows("codex"),
        vec![Row::Harness(LaunchHarness::Codex)],
        "a match leaves no fallback"
    );
}

/// On the Agent tab with no agent type, an effort or permission word (whole, any case) and every `effort:` or `perms:`
/// query offers nothing and says "Choose a harness first" as a warning; on the Command tab the same bare words name
/// the session. Why: the user typed a choice that cannot apply yet, and turning it into a session name would hide that
/// (old `search_choice_words_explain_the_missing_harness`).
#[test]
fn choice_words_before_a_harness_say_to_choose_one() {
    // No catalog models, so a word such as `medium` cannot match a model's id (which it rightly would) and only the
    // choice-word rule decides.
    let agent = Draft {
        catalog: Some(Vec::new()),
        ..Draft::agent(None)
    };
    let command = Draft {
        catalog: Some(Vec::new()),
        ..Draft::command()
    };
    let warning = Some(Message {
        text: "Choose a harness first",
        warning: true,
    });
    for query in [
        "off",
        "minimal",
        "low",
        "medium",
        "HIGH",
        " xhigh ",
        "max",
        "ultra",
        "default",
        "own",
        "custom",
        "approve",
        "smart approve",
        "chat",
        "effort:",
        "perms:xyz",
    ] {
        let results = agent.search(query);
        assert!(results.rows.is_empty(), "{query}");
        assert_eq!(results.message, warning, "{query}");
        if scoped(query).0 == Scope::All {
            assert_eq!(command.rows(query), name(query.trim()), "{query}");
        }
    }
}

// ---- Empty results -------------------------------------------------------------------------------------------------

/// Every label's empty result has its own words, and only a hint (an empty `name:` or `gh:`, an unlabelled blank,
/// models still loading) is drawn in the ordinary color; a query that matched nothing is a warning; an empty box says
/// nothing; a catalog read that failed is not loading. Why: a refusal presented as help, or a pending read as a failure
/// (or a failure as pending), misleads (old `empty_search_messages_cover_labels_and_loading`).
#[test]
fn empty_results_say_why_and_warn_only_on_a_miss() {
    let draft = Draft::agent(Some(LaunchHarness::Codex));
    let cases = [
        (
            "   ",
            "Type a search or clear the search box to launch",
            false,
        ),
        ("name:", "Type a session name after name:", false),
        ("host:nope", "No host matches", true),
        ("harness:nope", "No harness matches", true),
        (
            "model:nope",
            "No model matches; enter a custom model ID in the model field",
            true,
        ),
        ("effort:nope", "No effort matches", true),
        ("perms:nope", "No permissions match", true),
        ("folder:nope", "No folder matches", true),
        ("recent:nope", "No recent setup matches", true),
        ("gh:", "Type owner/repo to choose a repository", false),
        ("gh:nope", "No repository matches; enter owner/repo", true),
        ("tl:nope", "No template matches", true),
    ];
    for (query, text, warning) in cases {
        assert_eq!(
            draft.search(query).message,
            Some(Message { text, warning }),
            "{query}"
        );
    }
    assert_eq!(draft.search("").message, None);
    let loading = Draft {
        catalog: None,
        catalog_pending: true,
        ..Draft::agent(Some(LaunchHarness::Codex))
    };
    assert_eq!(
        loading.search("model:x").message,
        Some(Message {
            text: "Loading models…",
            warning: false
        })
    );
    let failed = Draft {
        catalog: None,
        catalog_pending: false,
        ..Draft::agent(Some(LaunchHarness::Codex))
    };
    assert_eq!(
        failed.search("model:x").message,
        Some(Message {
            text: "No model matches; enter a custom model ID in the model field",
            warning: true
        }),
        "a failed read has nothing to wait for"
    );
    assert_eq!(draft.search("codex").message, None, "rows leave no message");
}

// ---- Names and hosts -----------------------------------------------------------------------------------------------

/// `name:` takes its whole trimmed value as the name, colons included; `host:` filters the registry by its name or its
/// listed label, `host:local` finds the local row under any alias, `host:` alone lists every host, and an exact name
/// preselects its row. Why: the two labels exist so that a name or a host can be chosen without the other kinds
/// competing (old `name_and_host_labels_offer_only_their_intended_actions`).
#[test]
fn name_and_host_labels_offer_only_their_own_rows() {
    let draft = Draft::agent(None);
    assert_eq!(draft.rows("NAME:  fix: parser  "), name("fix: parser"));
    let ids = |query: &str| -> Vec<i64> {
        draft
            .rows(query)
            .into_iter()
            .map(|row| match row {
                Row::Host { id, .. } => id,
                other => panic!("{query}: not a host row: {other:?}"),
            })
            .collect()
    };
    assert_eq!(ids("host:local"), vec![1]);
    assert_eq!(ids("host:LOCAL"), vec![1]);
    assert_eq!(ids("host:BUILD"), vec![2, 3]);
    assert_eq!(
        ids("host:connecting"),
        vec![2],
        "the label's phase matches too"
    );
    assert_eq!(ids("host:"), vec![1, 2, 3]);
    assert!(ids("host:missing").is_empty());
    assert!(
        draft
            .rows("build")
            .iter()
            .all(|row| row.group() != Group::Hosts),
        "hosts need host:"
    );
    // `build.example` comes first in the registry, but `build` is the exact name.
    assert_eq!(draft.search("host:build").default, 1);
}

/// Session-owned hosts (a container or sandbox made for one session) are never offered. Why: a new session cannot be
/// placed on another session's own environment (the host list shows one only while it is already the chosen host).
#[test]
fn session_owned_hosts_are_not_offered() {
    let mut draft = Draft::agent(None);
    draft.hosts[1].is_session_owned = true;
    assert_eq!(draft.rows("host:").len(), 2);
}

// ---- Permissions ---------------------------------------------------------------------------------------------------

/// `perms:` lists exactly the choices the chosen agent type offers, in the fixed order, matching each choice's word by
/// its start. Why: the search must never offer a mode the helm would refuse for that agent, and the table that decides
/// is the shared one in `farhelm-proto` (old `permission_search_actions_follow_harness_capabilities`).
#[test]
fn permission_rows_follow_the_agents_capabilities() {
    use SearchPermission::{Approve, Chat, Custom, Guarded, OwnSetting, SmartApprove, Yolo};
    let perms = |harness: LaunchHarness, query: &str| Draft::agent(Some(harness)).rows(query);
    assert_eq!(
        perms(LaunchHarness::Codex, "perms:"),
        permissions(&[Yolo, Guarded, Custom])
    );
    assert_eq!(perms(LaunchHarness::Codex, "perms:y"), permissions(&[Yolo]));
    assert_eq!(
        perms(LaunchHarness::Codex, "perms:g"),
        permissions(&[Guarded])
    );
    assert_eq!(perms(LaunchHarness::Pi, "perms:"), permissions(&[Yolo]));
    assert_eq!(
        perms(LaunchHarness::Goose, "perms:"),
        permissions(&[Yolo, Approve, SmartApprove, Chat])
    );
    assert_eq!(
        perms(LaunchHarness::Omp, "perms:"),
        permissions(&[Yolo, Guarded, Custom, Approve])
    );
    assert!(perms(LaunchHarness::Cursor, "perms:").contains(&Row::Permission(OwnSetting)));
    assert!(perms(LaunchHarness::Pi, "perms:default").is_empty());
    assert_eq!(
        perms(LaunchHarness::Goose, "perms:smart"),
        permissions(&[SmartApprove])
    );
    assert_eq!(
        perms(LaunchHarness::Goose, "perms:smart approve"),
        permissions(&[SmartApprove])
    );
    // By the word's start only: these are inside `guarded` and `approve`.
    assert!(perms(LaunchHarness::Codex, "perms:ar").is_empty());
    assert!(perms(LaunchHarness::Goose, "perms:prove").is_empty());
    assert!(Draft::command().rows("perms:yolo").is_empty());
}

/// Before an agent type is chosen, `yolo` and `guarded`, bare (by any part of the word) or after `perms:`, offer
/// exactly their one row on the Agent tab and nothing on the Command tab; other permission words need a harness. Why:
/// YOLO and Guarded are the two choices a draft can hold before its agent, and the Command tab declares YOLO separately
/// (old `permissions_before_a_harness_belong_only_to_the_agent_tab` and
/// `bare_yolo_and_guarded_offer_permissions_and_preselect_exactly`).
#[test]
fn bare_yolo_and_guarded_work_before_a_harness_on_the_agent_tab_only() {
    let agent = Draft::agent(None);
    assert_eq!(agent.rows("yolo"), permissions(&[SearchPermission::Yolo]));
    assert_eq!(
        agent.rows("GUARDED"),
        permissions(&[SearchPermission::Guarded])
    );
    assert_eq!(agent.rows("ar"), permissions(&[SearchPermission::Guarded]));
    assert_eq!(
        agent.rows("perms:y"),
        permissions(&[SearchPermission::Yolo])
    );
    assert_eq!(
        agent.rows("perms:guarded"),
        permissions(&[SearchPermission::Guarded])
    );
    assert!(agent.rows("approve").is_empty());
    assert!(agent.rows("perms:approve").is_empty());
    let command = Draft::command();
    assert_eq!(command.rows("yolo"), name("yolo"));
    let codex = Draft::agent(Some(LaunchHarness::Codex));
    assert!(
        !codex
            .rows("custom")
            .contains(&Row::Permission(SearchPermission::Custom)),
        "an unlabelled query offers only YOLO and Guarded"
    );
}

// ---- Efforts and models --------------------------------------------------------------------------------------------

/// Effort rows are offered only with an agent type chosen, from the chosen model's own efforts or else every effort
/// the agent's catalog models offer; `effort:` lists Default first when the agent has an effort control at all, with or
/// without a catalog, and an unlabelled `default` never offers it. Why: an effort the model does not take would be
/// refused at launch (old `search_offers_efforts_only_for_a_selected_compatible_harness` and
/// `scoped_default_effort_follows_harness_capability_without_a_catalog`).
#[test]
fn effort_rows_follow_the_agent_and_its_model() {
    let codex = Draft::agent(Some(LaunchHarness::Codex));
    assert_eq!(
        codex.rows("effort:"),
        efforts(&[
            None,
            Some(LaunchEffort::Low),
            Some(LaunchEffort::Medium),
            Some(LaunchEffort::High)
        ])
    );
    let fast = Draft {
        model: Some("gpt-fast"),
        ..Draft::agent(Some(LaunchHarness::Codex))
    };
    assert_eq!(
        fast.rows("effort:"),
        efforts(&[None, Some(LaunchEffort::Low), Some(LaunchEffort::High)])
    );
    assert_eq!(fast.rows("hi"), efforts(&[Some(LaunchEffort::High)]));
    let no_catalog = Draft {
        catalog: None,
        ..Draft::agent(Some(LaunchHarness::Codex))
    };
    assert_eq!(no_catalog.rows("effort:"), efforts(&[None]));
    assert!(
        Draft::agent(Some(LaunchHarness::Grok))
            .rows("effort:")
            .is_empty()
    );
    assert!(!codex.rows("default").contains(&Row::Effort(None)));
    assert!(Draft::command().rows("effort:").is_empty());
}

/// With an agent type chosen on the Agent tab, model rows are that agent's catalog models; with none, or on the
/// Command tab, every agent's, each with its owner, in catalog order. An unlabelled colon id still finds its model, and
/// under `model:` the exact id is preselected even when a longer one comes first. Why: choosing another agent's model
/// switches the agent, which the search must offer only when no agent is chosen (old
/// `search_models_follow_the_selected_harness` and `scoped_harness_and_model_exact_words_select_the_matching_row`).
#[test]
fn model_rows_follow_the_chosen_agent() {
    assert_eq!(
        Draft::agent(Some(LaunchHarness::Claude)).rows("model:"),
        vec![model("claude", LaunchHarness::Claude)]
    );
    let every = vec![
        model("gpt-fast", LaunchHarness::Codex),
        model("medium-context", LaunchHarness::Codex),
        model("claude", LaunchHarness::Claude),
    ];
    assert_eq!(Draft::agent(None).rows("model:"), every);
    assert_eq!(
        Draft {
            harness: Some(LaunchHarness::Claude),
            ..Draft::command()
        }
        .rows("model:"),
        every
    );
    let mut colon = Draft::agent(None);
    colon.catalog = Some(vec![LaunchCatalogModel {
        id: "vendor:model:v2".into(),
        family: "vendor:model:v2".into(),
        latest: true,
        harness: LaunchHarness::Codex,
        efforts: vec![],
    }]);
    assert_eq!(
        colon.rows("vendor:model:v2"),
        vec![model("vendor:model:v2", LaunchHarness::Codex)]
    );
    assert_eq!(
        colon.rows("model:vendor:model:v2"),
        vec![model("vendor:model:v2", LaunchHarness::Codex)]
    );
    let mut longer_first = Draft::agent(None);
    longer_first.catalog = Some(vec![
        LaunchCatalogModel {
            id: "gpt-fast-x".into(),
            family: "gpt-fast-x".into(),
            latest: true,
            harness: LaunchHarness::Codex,
            efforts: vec![],
        },
        LaunchCatalogModel {
            id: "gpt-fast".into(),
            family: "gpt-fast".into(),
            latest: true,
            harness: LaunchHarness::Codex,
            efforts: vec![],
        },
    ]);
    assert_eq!(longer_first.search("model:GPT-FAST").default, 1);
}

/// Every agent type stays discoverable by search, and `harness:` alone lists
/// them in the launcher's order. An agent type missing from search could only be chosen by hand (old
/// `scoped_and_unscoped_search_offer_omp`).
#[test]
fn every_agent_type_is_discoverable() {
    let draft = Draft::agent(None);
    let harnesses = draft.rows("harness:");
    assert_eq!(harnesses.len(), LaunchHarness::ALL.len());
    assert!(harnesses.windows(2).any(|pair| {
        pair == [
            Row::Harness(LaunchHarness::Claude),
            Row::Harness(LaunchHarness::Antigravity),
        ]
    }));
    assert_eq!(draft.rows("omp"), vec![Row::Harness(LaunchHarness::Omp)]);
    assert_eq!(draft.rows("grok"), vec![Row::Harness(LaunchHarness::Grok)]);
    assert_eq!(
        draft.rows("antigravity"),
        vec![Row::Harness(LaunchHarness::Antigravity)]
    );
    assert_eq!(
        draft.rows("harness:open"),
        vec![Row::Harness(LaunchHarness::OpenCode)]
    );
}

// ---- Order and the preselected row ---------------------------------------------------------------------------------

/// Rows come grouped in the fixed order (names, hosts, agent types, models, efforts, permissions, folders), and an
/// exact unlabelled word preselects an effort before a permission, a model and an agent type, else the first row.
/// Why: `medium` must land on the effort and not a model named `medium-context`, while `claude` lands on the model
/// named exactly that (old `grouped_search_results_places_efforts_between_models_and_folders` and
/// `default_search_index_prefers_exact_specific_words`).
#[test]
fn rows_are_grouped_and_the_exact_word_is_preselected() {
    let codex = Draft::agent(Some(LaunchHarness::Codex));
    let medium = codex.search("medium");
    assert_eq!(
        medium.rows,
        vec![
            model("medium-context", LaunchHarness::Codex),
            Row::Effort(Some(LaunchEffort::Medium))
        ]
    );
    assert_eq!(medium.default, 1);
    let groups: Vec<Group> = medium
        .groups()
        .into_iter()
        .map(|(group, _)| group)
        .collect();
    assert_eq!(groups, vec![Group::Models, Group::Efforts]);

    let claude = Draft::agent(None).search("claude");
    assert_eq!(
        claude.rows,
        vec![
            Row::Harness(LaunchHarness::Claude),
            model("claude", LaunchHarness::Claude)
        ]
    );
    assert_eq!(claude.default, 1);

    let partial = Draft::agent(None).search("c");
    assert!(partial.rows.len() > 1);
    assert_eq!(partial.default, 0, "no exact word: the first row");
}

/// A path-shaped value (starting with `/`, `~` or `.`, or holding a slash) offers "Use this path" with the value as
/// typed, unlabelled or under `folder:`, in the Folders group after every other kind. Why: typing a folder and pressing
/// Enter must set it, and `folder:` is how to say a word is a folder; `gh:` names a repository, so it never offers a
/// local path (old `path_search_groups_explicit_actions_before_folder_history`, without the history rows, and
/// `scoped_search_empty_kinds_are_bounded_and_gh_is_local_free`).
#[test]
fn a_path_offers_use_this_path() {
    let draft = Draft::agent(None);
    assert_eq!(
        draft.rows("~/src"),
        vec![
            Row::UsePath("~/src".to_owned()),
            Row::BrowsePath("~/src".to_owned())
        ]
    );
    assert_eq!(
        draft.rows("folder: ./a b "),
        vec![
            Row::UsePath("./a b".to_owned()),
            Row::BrowsePath("./a b".to_owned())
        ]
    );
    assert!(draft.rows("folder:plain").is_empty());
    assert!(
        draft.rows("gh:gh:owner/repo").is_empty(),
        "gh: offers no local rows"
    );
    assert!(draft.rows("gh:/tmp").is_empty(), "gh: offers no local rows");
    let mixed = Draft {
        catalog: Some(vec![LaunchCatalogModel {
            id: "org/model".into(),
            family: "model".into(),
            latest: true,
            harness: LaunchHarness::Codex,
            efforts: vec![],
        }]),
        ..Draft::agent(None)
    };
    assert_eq!(
        mixed.rows("org/"),
        vec![
            model("org/model", LaunchHarness::Codex),
            Row::UsePath("org/".to_owned()),
            Row::BrowsePath("org/".to_owned())
        ]
    );
}

// ---- The launch history: saved folders, recent setups and past models ----------------------------------------------

/// The chosen host as the history rows name it.
fn placement() -> HostText {
    HostText::from_host("Local (this machine)")
}

/// A launch of `harness` with `model` (else its default) in the folder `cwd`, every other choice left to its default.
fn launch(cwd: &str, harness: LaunchHarness, model: Option<&str>) -> LaunchHistoryEntry {
    LaunchHistoryEntry {
        host: 1,
        target: None,
        github_repo: None,
        canonical_cwd: Some(cwd.to_owned()),
        cwd: cwd.to_owned(),
        selection: LaunchSelection {
            harness,
            model: model.map(str::to_owned),
            effort: None,
            permissions: None,
            permission_flags: None,
            workspace_trust: None,
            wrapper_prefix: None,
            container: None,
        },
        created_at: 0,
        creation_seq: None,
    }
}

/// A saved folder, as typed.
fn folder(display: &str) -> FolderHistoryEntry {
    FolderHistoryEntry {
        host: 1,
        canonical_cwd: display.to_owned(),
        canonical_proven: true,
        display_cwd: display.to_owned(),
        created_at: 0,
        creation_seq: None,
    }
}

/// A draft on the Agent tab with `harness` chosen, whose host's history holds `launches` and `folders`, newest first.
fn with_history(
    harness: Option<LaunchHarness>,
    launches: Vec<LaunchHistoryEntry>,
    folders: &[&str],
) -> Draft {
    Draft {
        history: Some(LaunchHistoryReply {
            launches,
            folders: folders.iter().map(|path| folder(path)).collect(),
            checkout_config_revision: 1,
        }),
        ..Draft::agent(harness)
    }
}

fn recent(entry: LaunchHistoryEntry) -> Row {
    Row::Recent {
        entry: Box::new(entry),
        placement: placement(),
    }
}

fn history_model(id: &str, harness: LaunchHarness) -> Row {
    Row::HistoryModel {
        id: id.to_owned(),
        harness,
    }
}

/// A typed path offers "Use this path" first, then every saved folder holding the text, in the history's order
/// (newest first) and uncapped. Why: the typed path is what Enter takes, and the saved folders are the next best
/// guesses (old `path_search_groups_explicit_actions_before_folder_history`, less its Browse row, which is not built).
#[test]
fn a_typed_path_comes_before_the_saved_folders() {
    let draft = with_history(
        None,
        Vec::new(),
        &["~/work/helm", "/srv/other", "~/work/helm-old"],
    );
    assert_eq!(
        draft.rows("~/work/helm"),
        vec![
            Row::UsePath("~/work/helm".to_owned()),
            Row::BrowsePath("~/work/helm".to_owned()),
            Row::Folder("~/work/helm".to_owned()),
            Row::Folder("~/work/helm-old".to_owned()),
        ]
    );
    // A word that is not a path still finds saved folders, and is not taken as a session name.
    assert_eq!(
        draft.rows("srv"),
        vec![Row::Folder("/srv/other".to_owned())]
    );
}

/// `folder:` alone lists every saved folder and `recent:` alone every recent setup, while an empty unlabelled box
/// offers nothing and `gh:` offers nothing local. Why: a bare label is how to browse one kind, and an empty box must not
/// cover the launcher (old `scoped_search_empty_kinds_are_bounded_and_gh_is_local_free`).
#[test]
fn a_bare_label_lists_its_whole_kind() {
    let codex = launch("/a", LaunchHarness::Codex, None);
    let draft = with_history(None, vec![codex.clone()], &["/a", "/b"]);
    assert_eq!(
        draft.rows("folder:"),
        vec![Row::Folder("/a".to_owned()), Row::Folder("/b".to_owned())]
    );
    assert_eq!(draft.rows("RECENT:"), vec![recent(codex)]);
    assert!(draft.rows("").is_empty());
    assert!(draft.rows("gh:/a").is_empty());
}

/// `folder:` offers only folders and `recent:` only recent setups, while unlabelled text offers both. Why: a label
/// narrows the search to one kind; a setup that started in a folder is not a folder (old
/// `folder_scope_does_not_broaden_into_unlabelled_recent_search`).
#[test]
fn a_label_does_not_broaden_into_the_other_history_kind() {
    let codex = launch("/home/u/project", LaunchHarness::Codex, None);
    let draft = with_history(None, vec![codex.clone()], &["/home/u/project"]);
    let saved = Row::Folder("/home/u/project".to_owned());
    assert_eq!(draft.rows("folder:project"), vec![saved.clone()]);
    assert_eq!(draft.rows("recent:project"), vec![recent(codex.clone())]);
    assert_eq!(draft.rows("project"), vec![saved, recent(codex)]);
}

/// Saved folders and recent setups match the folder as it was typed, so the visible `<U+202E>` spelling of a hidden
/// character matches only a folder that literally holds that text. Why: what is drawn escaped must not stand for the
/// character it hides, or typing the escape would find a folder whose name reads differently (old
/// `folder_and_recent_search_match_raw_paths`).
#[test]
fn history_rows_match_the_raw_folder() {
    let hidden = launch("/tmp/\u{202e}x", LaunchHarness::Codex, None);
    let literal = launch("/tmp/<U+202E>y", LaunchHarness::Codex, None);
    let draft = with_history(
        None,
        vec![hidden, literal.clone()],
        &["/tmp/\u{202e}x", "/tmp/<U+202E>y"],
    );
    assert_eq!(
        draft.rows("folder:<u+202e>"),
        vec![Row::Folder("/tmp/<U+202E>y".to_owned())]
    );
    assert_eq!(draft.rows("recent:<u+202e>"), vec![recent(literal)]);
    assert_eq!(
        draft.rows("folder:\u{202e}"),
        vec![Row::Folder("/tmp/\u{202e}x".to_owned())],
        "the hidden character itself finds the folder holding it"
    );
}

/// A recent setup matches on where it started, its agent type and its model, but not on its effort or permissions,
/// and it sits last, after the agent types and models the same word finds. Why: each row is one kind of choice, and a
/// word naming an agent type should offer the type itself before whole setups (old `search_keeps_choice_kinds_separate`).
#[test]
fn recent_setups_match_their_place_agent_and_model() {
    let mut claude = launch("/w", LaunchHarness::Claude, None);
    claude.selection.effort = Some(LaunchEffort::Max);
    let draft = with_history(None, vec![claude.clone()], &[]);
    assert_eq!(
        draft.rows("clau"),
        vec![
            Row::Harness(LaunchHarness::Claude),
            model("claude", LaunchHarness::Claude),
            recent(claude),
        ]
    );
    let recents = |query: &str| {
        draft
            .rows(query)
            .into_iter()
            .filter(|row| row.group() == Group::RecentSetups)
            .count()
    };
    assert_eq!(recents("recent:max"), 0, "the effort is not matched");
    assert_eq!(recents("recent:/w"), 1);
    let custom = launch("/v", LaunchHarness::Codex, Some("zz-model"));
    let by_model = with_history(None, vec![custom.clone()], &[]);
    assert_eq!(
        by_model.rows("recent:zz"),
        vec![recent(custom)],
        "the model is matched"
    );
}

/// Recent setups in search are every setup the history holds, in the strip's order (most launched, then newest),
/// neither capped at the strip's three nor narrowed by the draft's choices. Why: the strip shows the likely few, and
/// search is how to reach the rest (old `search_and_ordinary_recents_share_complete_ranking_before_the_cap`).
#[test]
fn recent_setups_are_ranked_and_uncapped() {
    let newest = launch("/1", LaunchHarness::Codex, None);
    let often = launch("/2", LaunchHarness::Claude, None);
    let older = launch("/3", LaunchHarness::Codex, None);
    let oldest = launch("/4", LaunchHarness::Codex, None);
    let draft = with_history(
        // A chosen Codex narrows the strip, not these rows.
        Some(LaunchHarness::Codex),
        vec![
            newest.clone(),
            often.clone(),
            older.clone(),
            often.clone(),
            oldest.clone(),
        ],
        &[],
    );
    assert_eq!(
        draft.rows("recent:/"),
        vec![recent(often), recent(newest), recent(older), recent(oldest)]
    );
}

/// A recent setup that made a new environment names that environment's recipe and place, not the chosen host, beside
/// the host's own setup naming the host. Why: SPEC.md, "Creation", recent setups restore "recipe and place together",
/// and taking the row goes there, so a row naming the chosen host would say it launches somewhere it does not (the old
/// search row's `recent_placement_label`).
#[test]
fn an_environment_setup_names_its_own_placement() {
    let mut sandbox = launch("~", LaunchHarness::Codex, None);
    sandbox.target = Some(farhelm_proto::LaunchTarget::Environment {
        recipe: "web".into(),
        place: farhelm_proto::Place::TensorlakeAccount,
    });
    let plain = launch("~", LaunchHarness::Claude, None);
    let draft = with_history(None, vec![sandbox.clone(), plain.clone()], &[]);
    assert_eq!(
        draft.rows("recent:"),
        vec![
            Row::Recent {
                entry: Box::new(sandbox),
                placement: HostText::from_host("web sandbox"),
            },
            recent(plain),
        ]
    );
}

/// A model an earlier launch used that the catalog does not list is offered after the catalog's models, with the agent
/// type it ran with, once however often it ran, and only for the chosen agent type when one is chosen. A current
/// model is offered only as the catalog's choice, including when history has an alternate spelling. A custom model
/// ID typed once stays within reach, and a selected agent type narrows every model row (old
/// `search_offers_a_used_custom_model_with_its_harness` and `search_models_follow_the_selected_harness`).
#[test]
fn past_models_follow_the_catalogs() {
    let launches = vec![
        launch("/a", LaunchHarness::Codex, Some("release/candidate.42")),
        launch("/a", LaunchHarness::Claude, Some("candidate-x")),
        launch("/b", LaunchHarness::Codex, Some("release/candidate.42")),
        launch("/a", LaunchHarness::Codex, Some("gpt-fast")),
        launch("/a", LaunchHarness::OpenCode, Some("big")),
    ];
    let models = |draft: &Draft, query: &str| -> Vec<Row> {
        draft
            .rows(query)
            .into_iter()
            .filter(|row| row.group() == Group::Models)
            .collect()
    };
    let any = Draft {
        catalog: Some(
            catalog()
                .into_iter()
                .chain([LaunchCatalogModel {
                    id: "opencode/big".into(),
                    family: "big".into(),
                    latest: true,
                    harness: LaunchHarness::OpenCode,
                    efforts: vec![],
                }])
                .collect(),
        ),
        ..with_history(None, launches.clone(), &[])
    };
    assert_eq!(
        models(&any, "candidate"),
        vec![
            history_model("release/candidate.42", LaunchHarness::Codex),
            history_model("candidate-x", LaunchHarness::Claude),
        ]
    );
    assert_eq!(
        models(&any, "model:gpt-fast"),
        vec![model("gpt-fast", LaunchHarness::Codex)],
        "a catalog model is not offered twice"
    );
    assert_eq!(
        models(&any, "model:big"),
        vec![model("opencode/big", LaunchHarness::OpenCode)],
        "the catalog's spelling of a past model is the catalog's row"
    );
    let codex = with_history(Some(LaunchHarness::Codex), launches, &[]);
    assert_eq!(
        models(&codex, "model:candidate"),
        vec![history_model("release/candidate.42", LaunchHarness::Codex)]
    );
}

/// A custom model recalled from history can share a shell's name. A bare
/// search word keeps the shell shortcut, while a model search still finds
/// the saved agent choice.
#[test]
fn exact_shell_search_takes_priority_over_a_past_model() {
    let draft = with_history(
        None,
        vec![launch("/a", LaunchHarness::Codex, Some("bash"))],
        &[],
    );
    let results = draft.search("bash");
    assert!(
        results
            .rows
            .contains(&history_model("bash", LaunchHarness::Codex))
    );
    assert_eq!(
        results.rows[results.default],
        Row::Harness(LaunchHarness::Bash)
    );
    assert_eq!(
        draft.rows("model:bash"),
        vec![history_model("bash", LaunchHarness::Codex)]
    );
}

/// Without a history read (none yet, or none for this host), no saved folder, recent setup or past model is offered,
/// and a `folder:` word that is not a path says it has no match. Why: these rows belong to the chosen host's history
/// only; offering a stale one would fill another host's folder.
#[test]
fn no_history_offers_no_history_rows() {
    let draft = Draft::agent(None);
    assert!(draft.rows("recent:").is_empty());
    assert!(draft.rows("folder:").is_empty());
    assert_eq!(
        draft.search("folder:src").message,
        Some(Message {
            text: "No folder matches",
            warning: true
        })
    );
}

/// A recent setup's row names the whole setup, host values escaped, and its hover help says the same without the
/// lead; a saved folder reads as a typed path does. Why: the row's accessible name is what a screen reader and the old
/// end-to-end tests found it by (old `.launch-composer-search-recent` `aria-label` and `search_result_tooltip`).
#[test]
fn history_row_words() {
    let mut codex = launch("/w\u{202e}", LaunchHarness::Codex, Some("model\u{202e}one"));
    codex.selection.effort = Some(LaunchEffort::High);
    codex.selection.permissions = Some(LaunchPermission::Yolo);
    let row = recent(codex);
    assert_eq!(
        row.words("w").accessible_name().as_ref(),
        "Recent setup: /w<U+202E> · Local (this machine) · Codex · Model: model<U+202E>one · Effort: High · Permissions: YOLO"
    );
    assert_eq!(
        row.help(None, "h").accessible_name().as_ref(),
        "/w<U+202E> · Local (this machine) · Codex · Model: model<U+202E>one · Effort: High · Permissions: YOLO"
    );
    assert_eq!(
        Row::Folder("/srv".to_owned())
            .words("w")
            .accessible_name()
            .as_ref(),
        "Use this path: /srv"
    );
    assert_eq!(
        history_model("custom", LaunchHarness::Claude)
            .words("w")
            .accessible_name()
            .as_ref(),
        "Model: custom (Claude, past launch)"
    );
}

// ---- Words and hover help ------------------------------------------------------------------------------------------

/// A row's words and hover help name host values escaped, as every host text is, and use the old app's wording. Why:
/// a session name or a model id with a hidden character must read the same in a tooltip as on screen (old
/// `search_result_tooltips_escape_names`).
#[test]
fn row_words_and_help_escape_host_values() {
    let hidden = "a\u{202e}b";
    let row = Row::Name(hidden.to_owned());
    assert_eq!(
        row.words("w").accessible_name().as_ref(),
        "Set session name: a<U+202E>b"
    );
    assert_eq!(
        row.help(None, "h").accessible_name().as_ref(),
        "Use \"a<U+202E>b\" as the session's name"
    );
    assert_eq!(
        model("m", LaunchHarness::Codex)
            .words("w")
            .accessible_name()
            .as_ref(),
        "Model: m (Codex)"
    );
    assert_eq!(
        Row::Permission(SearchPermission::Guarded)
            .help(None, "h")
            .accessible_name()
            .as_ref(),
        "Guarded: choose an agent to see its automatic safety mode"
    );
    assert_eq!(
        Row::Effort(None).words("w").accessible_name().as_ref(),
        "Effort: Default"
    );
}

// ---- Templates -------------------------------------------------------------------------------------------------------

/// SPEC.md: "`tl:name` in the search box offers templates by name". Only the `tl:` scope offers them, by a
/// case-insensitive part of the name (`tl:` alone lists them all), and an exact name is preselected: an unlabelled
/// query is about the launch being built, so template names there would crowd out its agent types and models (old
/// `templates_are_offered_only_under_tl`). Specified: with `my-codex` and `Web-Builder`, `tl:web` offers
/// `Web-Builder` alone; `TL:` offers both in order; `codex` offers no template; `tl:ÉCR` finds `écrire-project` (Unicode
/// case folding); `tl:my-codex` offers it under the Templates group, preselected.
#[test]
fn templates_are_offered_only_under_tl() {
    let draft = with_templates(&["my-codex", "Web-Builder"]);
    assert_eq!(draft.rows("tl:web"), [Row::Template("Web-Builder".into())]);
    assert_eq!(
        template_names(&draft.rows("TL:")),
        ["my-codex", "Web-Builder"]
    );
    assert!(
        template_names(&draft.rows("codex")).is_empty(),
        "unlabelled: no templates"
    );
    // The filter folds case as Rust's Unicode lowercase does, which ASCII folding would not: `ÉCR` finds `écrire`.
    let accented = with_templates(&["écrire-project", "web"]);
    assert_eq!(template_names(&accented.rows("tl:ÉCR")), ["écrire-project"]);
    let results = draft.search("tl:my-codex");
    assert_eq!(results.groups()[0].0, Group::Templates);
    assert_eq!(results.default, 0);
}

/// The exact-name preselection must fold case the way template names are matched, Unicode lowercase both ways, or
/// Enter would apply the first partial name rather than the full name typed (old
/// `template_exact_priority_uses_unicode_lowercase`). Specified: with `<name>-project` listed before `<name>`, both
/// match, and the second is preselected, for `écrire` against `Écrire` and `ÉCRIRE` against `écrire`.
#[test]
fn the_exact_template_name_is_preselected_by_unicode_lowercase() {
    for (query, exact) in [("écrire", "Écrire"), ("ÉCRIRE", "écrire")] {
        let project = format!("{exact}-project");
        let draft = with_templates(&[project.as_str(), exact]);
        let results = draft.search(&format!("tl:{query}"));
        assert_eq!(results.rows.len(), 2, "{query}: both names match");
        assert_eq!(results.default, 1, "{query}: the exact name is preselected");
    }
}

/// The groups keep the old order, Templates between Harnesses and Models, so a later group added in its place never
/// moves the template rows. Specified: the group order ranks Harnesses, Templates, Models; the group is headed
/// "Templates"; a template row reads "Template: " and the name, escaped, with the old hover help.
#[test]
fn template_rows_sit_between_harnesses_and_models_in_the_old_words() {
    assert_eq!(Group::Harnesses as u8 + 1, Group::Shells as u8);
    assert_eq!(Group::Shells as u8 + 1, Group::Templates as u8);
    assert_eq!(Group::Templates as u8 + 1, Group::Models as u8);
    assert_eq!(Group::Templates.label(), "Templates");
    let row = Row::Template("a\u{202e}b".into());
    assert_eq!(
        row.words("w").accessible_name().as_ref(),
        "Template: a<U+202E>b"
    );
    assert_eq!(
        row.help(None, "h").accessible_name().as_ref(),
        "Apply this template's settings to the draft"
    );
}

/// SPEC.md: a template list that cannot be read is "a reason shown in the results area". Saying "No template matches"
/// instead would tell the user their template is gone. Specified: with the templates unavailable, `tl:` offers no rows
/// and no message, and its reason is the one the templates give ("Couldn't load templates: busy").
#[test]
fn unavailable_templates_are_the_reason_under_tl() {
    let reason = HostSentence::new("r")
        .text("Couldn't load templates: ")
        .host(&HostText::from_host("busy"));
    let draft = Draft {
        templates: Err(reason),
        ..Draft::agent(None)
    };
    let results = draft.search("tl:");
    assert_eq!((results.rows.len(), results.message), (0, None));
    assert_eq!(
        results
            .reason
            .map(|reason| reason.accessible_name().to_string())
            .as_deref(),
        Some("Couldn't load templates: busy")
    );
    assert_eq!(draft.search("codex").reason, None, "only under tl:");
}
