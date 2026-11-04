//! The editor's pure parts: turning a draft into a save request, reopening a
//! saved recipe, and predicting what a save does to a saved token. The
//! browser spec covers the rendered form; these pin the rules it relies on.

use super::*;

/// A saved recipe as the list hands it to the editor, with a Claude Code
/// endpoint whose token is saved.
fn saved_view() -> RecipeView {
    RecipeView {
        name: "rust-dev".into(),
        size: RecipeSize {
            cpus: 4,
            memory_gb: 8,
            disk_gb: 50,
        },
        source: RecipeSource::Packages {
            packages: vec!["git".into(), "ripgrep".into()],
        },
        claude: Some(EndpointView {
            base_url: "http://127.0.0.1:8080".into(),
            token_set: true,
        }),
        codex: None,
        tunnels: vec![Tunnel {
            sandbox_port: 8080,
            host: "127.0.0.1".into(),
            port: 8080,
        }],
        snapshot: None,
        build: farhelm_proto::tensorlake::recipes::BuildView::Idle,
        cleanup_pending: false,
    }
}

/// Spec: reopening a saved recipe and saving it unchanged sends the same
/// definition back, with no token field (keep the saved token).
///
/// Why: Edit followed by Save must be a no-op on the helm. A draft that lost
/// a field on the way in, or sent an empty token, would silently change the
/// recipe or clear its token.
#[test]
fn an_unchanged_reopened_recipe_saves_back_unchanged() {
    let view = saved_view();
    let draft = Draft::from_view(&view);
    let save = draft.to_save().expect("a saved recipe is valid");
    assert_eq!(save.size, view.size);
    assert_eq!(save.source, view.source);
    assert_eq!(save.tunnels, view.tunnels);
    let claude = save.claude.expect("the endpoint is kept");
    assert_eq!(claude.base_url, "http://127.0.0.1:8080");
    assert!(
        claude.token.is_none(),
        "a blank token field keeps the saved token"
    );
    assert!(save.codex.is_none());
    assert_eq!(Draft::from_view(&view), draft, "reopening is stable");
}

/// Spec: package text splits on spaces and newlines; switching the software
/// kind keeps each kind's text, and the request carries only the chosen one;
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
    assert_eq!(
        save.source,
        RecipeSource::Packages {
            packages: vec!["git".into(), "ripgrep".into(), "jq".into()]
        }
    );
    draft.software = Software::Image;
    draft.image = " team-base ".into();
    assert_eq!(
        draft.to_save().unwrap().source,
        RecipeSource::Image {
            image: "team-base".into()
        }
    );
    draft.software = Software::Packages;
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
}

/// Spec: a blank base URL removes the endpoint; a typed token without a base
/// URL is refused; with a base URL, Clear sends an empty token, a typed token
/// replaces, and a blank field sends none.
///
/// Why: these three token outcomes are the helm's write-only token contract;
/// the form must never send an empty string by accident (that clears) or
/// drop a token the user just typed.
#[test]
fn endpoint_drafts_map_to_the_token_contract() {
    let draft = |base_url: &str, token: &str, clear: bool| EndpointDraft {
        base_url: base_url.into(),
        token: token.into(),
        clear,
    };
    assert!(
        endpoint_save("Codex", &draft("", "", false))
            .unwrap()
            .is_none()
    );
    assert!(
        endpoint_save("Codex", &draft(" ", "", true))
            .unwrap()
            .is_none()
    );
    assert!(
        endpoint_save("Codex", &draft("", "secret", false))
            .err()
            .expect("refused")
            .starts_with("Codex: ")
    );
    let token = |d: EndpointDraft| endpoint_save("Codex", &d).unwrap().unwrap().token;
    assert_eq!(token(draft("http://h", "", true)).as_deref(), Some(""));
    assert_eq!(
        token(draft("http://h", "secret", false)).as_deref(),
        Some("secret")
    );
    assert_eq!(token(draft("http://h", "", false)), None);
}

/// Spec: the token field predicts the helm's origin rule: kept for the same
/// origin (a path change included), dropped on an origin change, a clear or a
/// removed endpoint, replaced when typed, and nothing to say without a saved
/// token.
///
/// Why: the form never sees the token, so this prediction is the only way
/// the user learns before saving that a server change will drop it.
#[test]
fn token_fate_follows_the_origin_rule() {
    let saved = EndpointView {
        base_url: "http://127.0.0.1:8080".into(),
        token_set: true,
    };
    let draft = |base_url: &str, token: &str, clear: bool| EndpointDraft {
        base_url: base_url.into(),
        token: token.into(),
        clear,
    };
    let fate = |d: EndpointDraft| token_fate(Some(&saved), &d);
    assert_eq!(
        fate(draft("http://127.0.0.1:8080/v1", "", false)),
        TokenFate::Kept
    );
    assert_eq!(
        fate(draft("http://127.0.0.1:9090", "", false)),
        TokenFate::Dropped
    );
    assert_eq!(
        fate(draft("http://127.0.0.1:8080", "", true)),
        TokenFate::Dropped
    );
    assert_eq!(fate(draft("", "", false)), TokenFate::Dropped);
    assert_eq!(
        fate(draft("http://127.0.0.1:8080", "new", false)),
        TokenFate::Replaced
    );
    let unset = EndpointView {
        token_set: false,
        ..saved.clone()
    };
    assert_eq!(
        token_fate(Some(&unset), &draft("http://other", "", false)),
        TokenFate::None
    );
    assert_eq!(
        token_fate(None, &draft("http://h", "", false)),
        TokenFate::None
    );
}
