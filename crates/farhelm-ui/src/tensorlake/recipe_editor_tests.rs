//! The editor's pure parts: turning a draft into a save request, reopening a
//! saved recipe, and retaining an explicit named endpoint selection. The
//! browser spec covers the rendered form; these pin the rules it relies on.

use super::*;

/// The staged Tensorlake editor must preserve a Podman-only base when reopening
/// the shared catalog. Saving another field cannot silently change its image.
#[test]
fn reopening_a_container_base_preserves_its_reference_and_user() {
    let mut view = saved_view();
    view.base = RecipeBase::ContainerImage {
        reference: "example.test/team/dev@sha256:abc123".into(),
        user: "dev".into(),
    };
    let mut draft = Draft::from_view(&view);
    assert_eq!(draft.base, Base::ContainerImage);
    draft.packages = "git jq".into();
    let save = draft.to_save().unwrap();
    assert_eq!(save.base, view.base);
    assert_eq!(save.packages, ["git", "jq"]);
}

/// A saved recipe as the list hands it to the editor, with a Claude Code
/// endpoint that names a saved credential.
fn saved_view() -> RecipeView {
    RecipeView {
        name: "rust-dev".into(),
        size: RecipeSize {
            cpus: 4,
            memory_gb: 8,
            disk_gb: 50,
        },
        base: RecipeBase::ManagedUbuntu,
        packages: vec!["git".into(), "ripgrep".into()],
        claude: Some(EndpointView {
            base_url: "http://127.0.0.1:8080".into(),
            token_credential: Some("work-endpoint".into()),
        }),
        codex: None,
        github_token_credential: None,
        claude_login_credential: None,
        codex_login_credential: None,
        tunnels: vec![Tunnel {
            sandbox_port: 8080,
            host: "127.0.0.1".into(),
            port: 8080,
        }],
        snapshot: None,
        build: farhelm_proto::tensorlake::recipes::BuildView::Idle,
        prepared: None,
        preparation: farhelm_proto::tensorlake::recipes::PrepareView::Idle,
        cleanup_pending: false,
    }
}

/// Spec: reopening a saved recipe and saving it unchanged sends the same
/// definition back, including the chosen credential's name.
///
/// Why: Edit followed by Save must be a no-op on the helm. A draft that lost
/// a field on the way in would silently stop authenticating the endpoint.
#[test]
fn an_unchanged_reopened_recipe_saves_back_unchanged() {
    let view = saved_view();
    let draft = Draft::from_view(&view);
    let save = draft.to_save().expect("a saved recipe is valid");
    assert_eq!(save.size, view.size);
    assert_eq!(save.base, view.base);
    assert_eq!(save.packages, view.packages);
    assert_eq!(save.tunnels, view.tunnels);
    let claude = save.claude.expect("the endpoint is kept");
    assert_eq!(claude.base_url, "http://127.0.0.1:8080");
    assert_eq!(claude.token_credential.as_deref(), Some("work-endpoint"));
    assert!(save.codex.is_none());
    assert_eq!(Draft::from_view(&view), draft, "reopening is stable");
}

/// Spec: package text splits on spaces and newlines; switching bases keeps
/// each base's text, and the request carries only the selected base plus packages;
/// ports and sizes must be whole numbers; shared rules (names aside) apply
/// before anything is sent.
///
/// Why: the form is text, and a refusal it can explain locally is one the
/// user sees at once instead of after a round trip.
#[test]
fn drafts_become_requests_or_local_refusals() {
    let mut draft = Draft::new_recipe();
    draft.packages = "git ripgrep\njq  git".into();
    let save = draft.to_save().unwrap();
    assert_eq!(save.base, RecipeBase::ManagedUbuntu);
    assert_eq!(save.packages, vec!["git", "ripgrep", "jq"]);
    draft.base = Base::Image;
    draft.image = " team-base ".into();
    assert_eq!(
        draft.to_save().unwrap().base,
        RecipeBase::TensorlakeImage {
            image: "team-base".into()
        }
    );
    draft.base = Base::Snapshot;
    draft.snapshot_id = " snapshot1 ".into();
    let save = draft.to_save().unwrap();
    assert_eq!(
        save.base,
        RecipeBase::TensorlakeSnapshot {
            snapshot_id: "snapshot1".into()
        }
    );
    assert_eq!(save.packages, vec!["git", "ripgrep", "jq"]);
    draft.base = Base::TensorlakeUbuntu;
    assert_eq!(draft.image, " team-base ");
    assert_eq!(draft.snapshot_id, " snapshot1 ");
    assert_eq!(
        draft.packages, "git ripgrep\njq  git",
        "switching kept the text"
    );

    let mut bad = Draft::new_recipe();
    bad.cpus = "two".into();
    assert!(bad.to_save().err().expect("refused").contains("CPUs"));
    let mut bad = Draft::new_recipe();
    bad.tunnels.push(TunnelDraft {
        sandbox_port: "70000".into(),
        host: "lb".into(),
        port: "1".into(),
    });
    assert!(bad.to_save().is_err());
    let mut bad = Draft::new_recipe();
    bad.packages = "-oAPT::x".into();
    assert!(bad.to_save().is_err(), "the shared package rule applies");
    bad.base = Base::Image;
    bad.image = "team-base".into();
    assert!(
        bad.to_save().is_err(),
        "custom bases still validate packages"
    );
}

/// Every base must reopen with its packages intact. Otherwise an unchanged
/// edit of a custom base could silently remove its next build's packages.
#[test]
fn all_bases_reopen_with_their_packages() {
    for base in [
        RecipeBase::ManagedUbuntu,
        RecipeBase::TensorlakeImage {
            image: "team-base".into(),
        },
        RecipeBase::TensorlakeSnapshot {
            snapshot_id: "snapshot1".into(),
        },
    ] {
        let view = RecipeView {
            base,
            ..saved_view()
        };
        let draft = Draft::from_view(&view);
        assert_eq!(draft.packages, "git\nripgrep");
        let save = draft.to_save().expect("saved base stays valid");
        assert_eq!(save.base, view.base);
        assert_eq!(save.packages, view.packages);
    }
}

/// A stale draft carries a chosen name rather than an unseen saved secret.
/// Changing servers must retain that explicit choice; only selecting None or
/// removing the endpoint removes its reference, without deleting the catalog.
#[test]
fn named_endpoint_selection_survives_origin_changes_and_can_be_removed() {
    let mut draft = Draft::from_view(&saved_view());
    draft.claude.base_url = "https://another.example/v1".into();
    let selected = draft.to_save().unwrap().claude.unwrap();
    assert_eq!(selected.token_credential.as_deref(), Some("work-endpoint"));
    assert_eq!(selected.base_url, "https://another.example/v1");
    draft.claude.token_credential.clear();
    assert!(
        draft
            .to_save()
            .unwrap()
            .claude
            .unwrap()
            .token_credential
            .is_none()
    );
    draft.claude.token_credential = "work-endpoint".into();
    draft.claude.base_url.clear();
    assert!(draft.to_save().err().unwrap().contains("base URL"));
    draft.claude_mode = AgentMode::None;
    assert!(draft.to_save().unwrap().claude.is_none());
}

/// Claude's endpoint and login drafts survive mode changes, but only the
/// explicit mode is submitted. Missing subscription input is a local refusal;
/// reopening a saved login must not silently reactivate a hidden endpoint.
#[test]
fn claude_modes_keep_drafts_and_submit_only_the_selected_credentials() {
    let mut draft = Draft::from_view(&saved_view());
    assert_eq!(draft.claude_mode, AgentMode::Endpoint);
    draft.claude_mode = AgentMode::Subscription;
    assert!(
        draft
            .to_save()
            .err()
            .unwrap()
            .contains("choose a Claude login")
    );
    draft.claude_login_credential = "claude-work".into();
    let save = draft.to_save().unwrap();
    assert!(save.claude.is_none());
    assert_eq!(save.claude_login_credential.as_deref(), Some("claude-work"));
    draft.claude_mode = AgentMode::None;
    let save = draft.to_save().unwrap();
    assert!(save.claude.is_none() && save.claude_login_credential.is_none());
    assert!(!draft.claude.base_url.is_empty());
    assert_eq!(draft.claude_login_credential, "claude-work");
    let mut view = saved_view();
    view.claude = None;
    view.claude_login_credential = Some("claude-work".into());
    let reopened = Draft::from_view(&view);
    assert_eq!(reopened.claude_mode, AgentMode::Subscription);
    assert_eq!(
        reopened.to_save().unwrap().claude_login_credential,
        view.claude_login_credential
    );
}

/// Switching modes retains text but submits only the selected authentication.
/// Empty mode-specific input must fail locally rather than silently save None.
#[test]
fn codex_modes_keep_drafts_and_submit_exclusive_selections() {
    let mut draft = Draft::new_recipe();
    draft.codex_mode = AgentMode::Endpoint;
    assert!(draft.to_save().err().unwrap().contains("base URL"));
    draft.codex.base_url = "https://codex.example/v1".into();
    draft.codex.token_credential = "endpoint-work".into();
    let save = draft.to_save().unwrap();
    assert!(save.codex.is_some() && save.codex_login_credential.is_none());
    draft.codex_mode = AgentMode::Subscription;
    assert!(
        draft
            .to_save()
            .err()
            .unwrap()
            .contains("choose a Codex login")
    );
    draft.codex_login_credential = "codex-work".into();
    let save = draft.to_save().unwrap();
    assert!(save.codex.is_none());
    assert_eq!(save.codex_login_credential.as_deref(), Some("codex-work"));
    draft.codex_mode = AgentMode::None;
    let save = draft.to_save().unwrap();
    assert!(save.codex.is_none() && save.codex_login_credential.is_none());
    assert_eq!(draft.codex.base_url, "https://codex.example/v1");
    assert_eq!(draft.codex_login_credential, "codex-work");
    let mut view = saved_view();
    view.codex_login_credential = Some("codex-work".into());
    let reopened = Draft::from_view(&view);
    assert_eq!(reopened.codex_mode, AgentMode::Subscription);
    assert_eq!(
        reopened.to_save().unwrap().codex_login_credential,
        view.codex_login_credential
    );
}

/// A new recipe may select and edit its container base; reopening support alone
/// would leave the new provider impossible to configure through Settings.
#[test]
fn a_new_container_recipe_saves_edited_reference_and_user() {
    let mut draft = Draft::new_recipe();
    draft.name = "container-dev".into();
    draft.base = Base::ContainerImage;
    draft.container_reference = "registry.example.test/team/dev:stable".into();
    draft.container_user = "developer".into();
    let save = draft.to_save().unwrap();
    assert_eq!(
        save.base,
        RecipeBase::ContainerImage {
            reference: draft.container_reference,
            user: draft.container_user
        }
    );
}
