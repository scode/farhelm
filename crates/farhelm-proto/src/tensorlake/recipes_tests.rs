//! Validation tables for recipes. The helm and the UI both run these checks,
//! so a regression here is a regression on both sides of the wire.

use super::*;

/// A valid managed recipe that each test varies in one field.
fn recipe() -> RecipeSave {
    RecipeSave {
        size: RecipeSize {
            cpus: 2,
            memory_gb: 4,
            disk_gb: 20,
        },
        source: RecipeSource::Packages {
            packages: vec!["ripgrep".into()],
        },
        claude: None,
        codex: None,
        github_token_credential: None,
        claude_login_credential: None,
        codex_login_credential: None,
        tunnels: vec![],
    }
}

/// Spec: names are `^[a-z][a-z0-9-]*$`, not ending in `-`, 1 to 32 bytes.
///
/// Why: the name goes verbatim into the builder sandbox's name and into URL
/// paths. A name outside the grammar would either need sanitizing (letting
/// two recipes share a builder) or break the path.
#[test]
fn recipe_names_follow_the_sandbox_name_grammar() {
    for good in ["rust-dev", "a", "py3", "a-b-c", &"x".repeat(32)] {
        assert_eq!(check_recipe_name(good), Ok(()), "{good:?}");
    }
    for bad in [
        "",
        "Rust",
        "-x",
        "x-",
        "9x",
        ".",
        "..",
        "rust dev",
        "rust_dev",
        "rüst",
        &"x".repeat(33),
    ] {
        assert!(check_recipe_name(bad).is_err(), "{bad:?}");
    }
}

/// Spec: package text splits on any ASCII whitespace, keeps order, and drops
/// exact duplicates.
///
/// Why: the editor is one text box where people paste lists separated by
/// spaces or newlines; the stored order is what apt receives.
#[test]
fn package_text_splits_on_whitespace_and_drops_duplicates() {
    assert_eq!(
        parse_packages("  git\tripgrep\n\njq git  ripgrep\r\nclang "),
        vec!["git", "ripgrep", "jq", "clang"]
    );
    assert!(parse_packages(" \n\t").is_empty());
}

/// Spec: package words are Debian names with an optional `:arch` and
/// `=version`; anything starting with `-` or carrying shell punctuation is
/// refused.
///
/// Why: the build passes these to apt-get. Quoting stops the shell, but
/// apt-get still reads a quoted `-o...` word as an option, so the leading
/// character rule is the actual boundary.
#[test]
fn package_words_are_debian_names_only() {
    for good in [
        "ripgrep",
        "g++",
        "libssl-dev",
        "python3.12",
        "pkg:amd64",
        "pkg=1.2-3ubuntu1",
        "pkg:arm64=1:2.0~rc1+dfsg",
        "0ad",
    ] {
        let mut save = recipe();
        save.source = RecipeSource::Packages {
            packages: vec![good.into()],
        };
        assert_eq!(check_recipe(&save), Ok(()), "{good:?}");
    }
    for bad in [
        "-oAPT::Get::foo",
        "a;b",
        "a b",
        "$(id)",
        "Ripgrep",
        "x",
        "pkg:",
        "pkg=",
        "pkg=1 2",
        "",
        &"a".repeat(129),
    ] {
        let mut save = recipe();
        save.source = RecipeSource::Packages {
            packages: vec![bad.into()],
        };
        assert!(check_recipe(&save).is_err(), "{bad:?}");
    }
    let mut save = recipe();
    save.source = RecipeSource::Packages {
        packages: (0..=PACKAGES_CAP).map(|n| format!("pkg{n}")).collect(),
    };
    assert!(check_recipe(&save).is_err(), "more than the cap");
}

/// Spec: sizes are whole numbers within loose sanity bounds, never zero.
///
/// Why: a zero size can only be a form mistake; the real plan limits are
/// Tensorlake's to enforce, so the bounds are deliberately wide.
#[test]
fn sizes_are_bounded_and_non_zero() {
    for (cpus, memory_gb, disk_gb) in [
        (0, 4, 20),
        (2, 0, 20),
        (2, 4, 0),
        (65, 4, 20),
        (2, 513, 20),
        (2, 4, 4097),
    ] {
        let mut save = recipe();
        save.size = RecipeSize {
            cpus,
            memory_gb,
            disk_gb,
        };
        assert!(check_recipe(&save).is_err(), "{cpus}/{memory_gb}/{disk_gb}");
    }
    let mut save = recipe();
    save.size = RecipeSize {
        cpus: 64,
        memory_gb: 512,
        disk_gb: 4096,
    };
    assert_eq!(check_recipe(&save), Ok(()));
}

/// Spec: image names are 1 to 200 bytes of `[A-Za-z0-9._/:-]` not starting
/// with `-`; snapshot IDs are 1 to 64 lowercase letters and digits.
///
/// Why: both reach Tensorlake's API and the UI verbatim; refusing punctuation
/// early gives a clearer message than a create failure minutes later.
#[test]
fn reused_images_and_snapshots_have_narrow_shapes() {
    let check = |source: RecipeSource| {
        let mut save = recipe();
        save.source = source;
        check_recipe(&save)
    };
    for good in ["team-base", "tensorlake/ubuntu-systemd", "org/img:v1.2_3"] {
        assert_eq!(
            check(RecipeSource::Image { image: good.into() }),
            Ok(()),
            "{good:?}"
        );
    }
    for bad in ["", "-x", "a b", "a;b", &"a".repeat(201)] {
        assert!(
            check(RecipeSource::Image { image: bad.into() }).is_err(),
            "{bad:?}"
        );
    }
    assert_eq!(
        check(RecipeSource::Snapshot {
            snapshot_id: "ov8u0zj19vjgt1n66pnid".into()
        }),
        Ok(())
    );
    for bad in ["", "ABC", "a-b", &"a".repeat(65)] {
        assert!(
            check(RecipeSource::Snapshot {
                snapshot_id: bad.into()
            })
            .is_err(),
            "{bad:?}"
        );
    }
}

/// Spec: an endpoint's base URL is http(s) with a host, optional port and
/// path, and no credentials, query or fragment; named endpoint credentials
/// use the same immutable-name grammar as recipes.
///
/// Why: URLs must not smuggle a second secret, and credential names must be
/// usable identities. Secret validation belongs to the separate catalog.
#[test]
fn endpoint_urls_and_credential_names_are_checked() {
    for good in [
        "http://127.0.0.1:8080",
        "https://lb.internal/v1",
        "HTTP://Example.COM:443/",
        "http://[::1]:8080/path",
    ] {
        assert_eq!(check_base_url(good), Ok(()), "{good:?}");
    }
    for bad in [
        "",
        "ftp://host",
        "http://",
        "http://user:pw@host",
        "http://host?x=1",
        "http://host#frag",
        "http://host:0",
        "http://host:65536",
        "http://host:abc",
        "http://ho st",
        "http://[::1",
        "http://[::1]x",
        "http://-host",
        "127.0.0.1:8080",
    ] {
        assert!(check_base_url(bad).is_err(), "{bad:?}");
    }
    let with_token = |name: Option<&str>| {
        let mut save = recipe();
        save.claude = Some(EndpointSave {
            base_url: "http://127.0.0.1:8080".into(),
            token_credential: name.map(str::to_string),
        });
        check_recipe(&save)
    };
    assert_eq!(with_token(None), Ok(()));
    assert!(
        with_token(Some("")).is_err(),
        "None is explicit; an empty identity is invalid"
    );
    assert_eq!(with_token(Some("work-endpoint")), Ok(()));
    assert!(with_token(Some("has space")).is_err());
    assert!(with_token(Some(&"t".repeat(RECIPE_NAME_CAP + 1))).is_err());
    let mut save = recipe();
    save.codex = Some(EndpointSave {
        base_url: "http://user@host".into(),
        token_credential: None,
    });
    assert!(check_recipe(&save).unwrap_err().starts_with("Codex: "));
}

/// Spec: at most 32 tunnels; sandbox ports are non-zero, unique and not 22;
/// target ports are non-zero; hosts are DNS names, IPv4 literals or
/// bracketed IPv6 literals without a leading `-`.
///
/// Why: the tunnels PR builds `-R 127.0.0.1:<p>:<host>:<p>` ssh arguments from
/// these values. Validating here keeps that argv safe (no option-shaped host,
/// no spaces), and port 22 is the sandbox's own sshd.
#[test]
fn tunnels_are_checked() {
    let check = |tunnels: Vec<Tunnel>| {
        let mut save = recipe();
        save.tunnels = tunnels;
        check_recipe(&save)
    };
    let tunnel = |sandbox_port: u16, host: &str, port: u16| Tunnel {
        sandbox_port,
        host: host.into(),
        port,
    };
    for host in [
        "lb.internal",
        "127.0.0.1",
        "[::1]",
        "LB-1.Example.com",
        "localhost",
    ] {
        assert_eq!(check(vec![tunnel(8080, host, 8080)]), Ok(()), "{host:?}");
    }
    for host in [
        "",
        "-x",
        "a b",
        "a..b",
        "999.1.1.1",
        "::1",
        "[::1",
        "a_b",
        "-oProxyCommand=x",
    ] {
        assert!(check(vec![tunnel(8080, host, 8080)]).is_err(), "{host:?}");
    }
    assert!(check(vec![tunnel(0, "h", 1)]).is_err());
    assert!(check(vec![tunnel(1, "h", 0)]).is_err());
    assert!(check(vec![tunnel(22, "h", 22)]).is_err());
    assert!(check(vec![tunnel(8080, "a", 1), tunnel(8080, "b", 2)]).is_err());
    assert_eq!(
        check(vec![tunnel(8080, "a", 22), tunnel(8081, "a", 22)]),
        Ok(())
    );
    assert!(
        check(
            (1..=TUNNELS_CAP as u16 + 1)
                .map(|p| tunnel(1000 + p, "h", p))
                .collect()
        )
        .is_err()
    );
}

/// Spec: the source's JSON is tagged by `kind` in snake case, and a save
/// refuses unknown fields at every level (top, size, source, tunnel,
/// endpoint, and inside a create) while a view tolerates them, nested ones
/// included.
///
/// Why: the UI and helm share these types, but a browser tab can outlive a
/// helm upgrade; reads must stay tolerant, while a save must not silently
/// drop a field the user set.
#[test]
fn wire_shapes_are_tagged_and_saves_are_strict() {
    assert_eq!(
        serde_json::to_value(RecipeSource::Snapshot {
            snapshot_id: "abc".into()
        })
        .unwrap(),
        serde_json::json!({"kind": "snapshot", "snapshot_id": "abc"})
    );
    let strict = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut save = serde_json::to_value(recipe()).unwrap();
        save["claude"] =
            serde_json::json!({"base_url": "http://127.0.0.1:8080", "token_credential": null});
        save["tunnels"] = serde_json::json!([{"sandbox_port": 8080, "host": "h", "port": 8080}]);
        assert!(
            serde_json::from_value::<RecipeSave>(save.clone()).is_ok(),
            "premise: the unedited save decodes"
        );
        edit(&mut save);
        serde_json::from_value::<RecipeSave>(save).is_err()
    };
    assert!(
        strict(&|save| save["surprise"] = serde_json::json!(true)),
        "top level"
    );
    assert!(
        strict(&|save| save["size"]["gpus"] = serde_json::json!(1)),
        "size"
    );
    assert!(
        strict(&|save| save["source"]["base_image"] = serde_json::json!("team-base")),
        "source"
    );
    assert!(
        strict(&|save| save["tunnels"][0]["bind"] = serde_json::json!("0.0.0.0")),
        "tunnel"
    );
    assert!(
        strict(&|save| save["claude"]["model"] = serde_json::json!("x")),
        "endpoint"
    );
    let mut create = serde_json::to_value(CreateRecipe {
        name: "a".into(),
        recipe: recipe(),
    })
    .unwrap();
    create["recipe"]["size"]["gpus"] = serde_json::json!(1);
    assert!(
        serde_json::from_value::<CreateRecipe>(create).is_err(),
        "a create's nested recipe is just as strict"
    );
    let view = serde_json::json!({
        "name": "a", "size": {"cpus": 1, "memory_gb": 1, "disk_gb": 1},
        "source": {"kind": "image", "image": "x", "later": 1}, "claude": null, "codex": null,
        "tunnels": [{"sandbox_port": 1, "host": "h", "port": 2, "later": 1}],
        "snapshot": {"id": "s1", "built_at": 1, "base_image": "b", "disk_gb": 1, "packages": [],
            "claude_version": null, "codex_version": null, "later": 1},
        "build": {"state": "building", "step": "codex", "started_at": 1, "later": 1},
        "cleanup_pending": false, "added_later": {"id": "later"}
    });
    let view = serde_json::from_value::<RecipeView>(view).expect("unknown fields are tolerated");
    assert_eq!(
        view.snapshot.map(|snapshot| snapshot.id).as_deref(),
        Some("s1")
    );
    assert_eq!(
        view.build,
        BuildView::Building {
            step: BuildStep::Codex,
            started_at: 1
        }
    );
    let older = serde_json::json!({
        "name": "a", "size": {"cpus": 1, "memory_gb": 1, "disk_gb": 1},
        "source": {"kind": "image", "image": "x"}, "claude": null, "codex": null, "tunnels": []
    });
    let older = serde_json::from_value::<RecipeView>(older).expect("build fields may be absent");
    assert_eq!(
        (older.snapshot, older.build, older.cleanup_pending),
        (None, BuildView::Idle, false),
        "a reply without build fields reads as nothing built and nothing running"
    );
}

/// Codex has one active authentication choice. Older endpoint-only requests
/// still decode, but a save cannot select both kinds or an invalid login name.
#[test]
fn codex_subscription_and_endpoint_are_exclusive_named_choices() {
    let mut save = recipe();
    save.codex_login_credential = Some("codex-work".into());
    assert!(check_recipe(&save).is_ok());
    let decoded: RecipeSave = serde_json::from_value(serde_json::to_value(&save).unwrap()).unwrap();
    assert_eq!(
        decoded.codex_login_credential.as_deref(),
        Some("codex-work")
    );
    save.codex = Some(EndpointSave {
        base_url: "https://endpoint.test/v1".into(),
        token_credential: None,
    });
    assert!(check_recipe(&save).is_err());
    save.codex = None;
    for invalid in ["", "Uppercase", "../login", "name-"] {
        save.codex_login_credential = Some(invalid.into());
        assert!(check_recipe(&save).is_err());
    }
    let mut legacy = serde_json::to_value(recipe()).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("codex_login_credential");
    assert!(
        serde_json::from_value::<RecipeSave>(legacy)
            .unwrap()
            .codex_login_credential
            .is_none()
    );
}

/// Both decode directions retain only the login name. Legacy saves can omit
/// it, while a modern save cannot combine a Claude endpoint with a subscription
/// or smuggle an invalid credential identity into the stored recipe.
#[test]
fn claude_subscription_is_an_exclusive_named_choice() {
    let mut save = recipe();
    save.claude = None;
    save.claude_login_credential = Some("claude-work".into());
    assert!(check_recipe(&save).is_ok());
    let decoded: RecipeSave = serde_json::from_value(serde_json::to_value(&save).unwrap()).unwrap();
    assert_eq!(
        decoded.claude_login_credential.as_deref(),
        Some("claude-work")
    );
    save.claude = Some(EndpointSave {
        base_url: "https://endpoint.test/v1".into(),
        token_credential: None,
    });
    assert!(check_recipe(&save).is_err());
    save.claude = None;
    for invalid in ["", "Uppercase", "../login", "name-"] {
        save.claude_login_credential = Some(invalid.into());
        assert!(check_recipe(&save).is_err());
    }
    let mut legacy = serde_json::to_value(recipe()).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("claude_login_credential");
    assert!(
        serde_json::from_value::<RecipeSave>(legacy)
            .unwrap()
            .claude_login_credential
            .is_none()
    );
}

/// Spec: build status is tagged by `state` in snake case, steps are numbered
/// 1 to 9 in build order, and every step has a label.
///
/// Why: the list shows "step n of 9" from these numbers and builds its
/// sentences from the labels; a renumbered or unlabeled step would show a
/// wrong or empty progress line.
#[test]
fn build_status_is_tagged_and_steps_are_numbered_in_order() {
    assert_eq!(
        serde_json::to_value(BuildView::Building {
            step: BuildStep::ClaudeCode,
            started_at: 7
        })
        .unwrap(),
        serde_json::json!({"state": "building", "step": "claude_code", "started_at": 7})
    );
    let steps = [
        BuildStep::Cleaning,
        BuildStep::Creating,
        BuildStep::Starting,
        BuildStep::Connecting,
        BuildStep::Packages,
        BuildStep::ClaudeCode,
        BuildStep::Codex,
        BuildStep::Snapshotting,
        BuildStep::Finishing,
    ];
    assert_eq!(steps.len(), usize::from(BuildStep::COUNT));
    for (index, step) in steps.iter().enumerate() {
        assert_eq!(usize::from(step.number()), index + 1, "{step:?}");
        assert!(!step.label().is_empty(), "{step:?}");
    }
}
