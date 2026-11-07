//! Exercise the actual add orchestrator with filesystem-controlled API and
//! backend fakes. No shell or live sandbox is used; ownership and step state
//! remain the production implementations.

use super::*;
use crate::rest_harness::{FleetBuilder, Harness};
use crate::tensorlake::e2e::{E2eTensorlake, FakeModel};
use farhelm_proto::tensorlake::{SaveSettings, recipes::RecipeSave};

/// Own all fake paths alongside the helm fixture. Supplying these explicitly
/// keeps concurrent tests isolated without changing process environment.
struct Fixture {
    harness: Harness,
    root: tempfile::TempDir,
    service: Arc<ProvisioningService>,
    tensorlake: Tensorlake,
}

/// Assemble a real provisioning service over the repository's two controlled
/// fakes; managed fixtures additionally exercise the snapshot build engine.
async fn fixture(managed: bool) -> Fixture {
    let harness = FleetBuilder::new().await.start().await;
    let root = tempfile::tempdir().unwrap();
    let api_dir = root.path().join("api");
    let backend_dir = root.path().join("backend");
    for (path, marker) in [
        (&api_dir, "farhelm-e2e-tensorlake-v1\n"),
        (&backend_dir, "farhelm-e2e-provisioning-v1\n"),
    ] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("ENABLED"), marker).unwrap();
        std::fs::write(path.join("config.json"), "{}").unwrap();
    }
    let api = Arc::new(E2eTensorlake::new(api_dir, root.path()).unwrap());
    std::fs::write(
        root.path().join("api/state.json"),
        serde_json::to_vec(&FakeModel::default()).unwrap(),
    )
    .unwrap();
    let tensorlake = Tensorlake::production(harness.store.clone(), root.path())
        .unwrap()
        .with_api(api.clone())
        .with_shell(api);
    harness
        .store
        .save_tensorlake_config(SaveSettings {
            api_key: Some("fixture-key".into()),
            ..Default::default()
        })
        .await
        .ok()
        .unwrap();
    let mut recipe: RecipeSave = crate::tensorlake::recipes::tests_support::save();
    if !managed {
        recipe.source = RecipeSource::Image {
            image: "example/systemd".into(),
        };
    }
    assert!(
        harness
            .store
            .create_tensorlake_recipe("test-recipe".into(), recipe)
            .await
            .unwrap()
            .is_ok()
    );
    let service = ProvisioningService::injected(
        harness.store.clone(),
        Arc::clone(&harness.manager),
        Arc::new(E2eProvisioningBackend::new(backend_dir.clone(), root.path()).unwrap()),
        Arc::new(E2ePayloads(backend_dir.join("ENABLED"))),
        PlanLayout::production(root.path().to_path_buf()),
        "/fixture/farhelm".into(),
    );
    Fixture {
        harness,
        root,
        service,
        tensorlake,
    }
}

impl Fixture {
    /// A probe has no network side effects; its opaque id is the sole
    /// authority the subsequent POST consumes.
    async fn probe(&self, target: ProbeDestination) -> ProbeResponse {
        self.service
            .probe_tensorlake(
                ProbeRequest {
                    target,
                    remote_farhelm: None,
                    remote_state_dir: None,
                    new_entry: None,
                },
                self.tensorlake.clone(),
            )
            .await
            .unwrap()
    }

    async fn add(&self) -> RunAccepted {
        let ProbeResponse::TensorlakeProvisionable { probe_id, .. } = self
            .probe(ProbeDestination::Tensorlake {
                name: "Test host".into(),
                recipe: "test-recipe".into(),
            })
            .await
        else {
            panic!("expected a creation plan")
        };
        self.service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap()
    }

    /// The run's final state, never the idle view before its admission.
    async fn finish(&self, host: HostId) -> ProvisioningView {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let view = self.service.view(host).await.unwrap();
                if view.run_id.is_some() && view.status != RunStatus::Running {
                    return view;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("add run finishes")
    }

    /// Read what the API fake durably owns, rather than inferring billed
    /// resources from a successful setup reply alone.
    fn model(&self) -> FakeModel {
        serde_json::from_slice(&std::fs::read(self.root.path().join("api/state.json")).unwrap())
            .unwrap()
    }
}

/// Endpoint delivery shares setup's injected backend. A credential failure
/// remains a warning while the supervisor installation completes, so an image
/// with unreadable agent settings can still become a usable Farhelm host.
#[farhelm_testtrace::test]
async fn credential_delivery_warning_does_not_abort_tensorlake_setup() {
    use farhelm_proto::credentials::{CreateCredential, CredentialKind};
    let f = fixture(false).await;
    f.harness
        .store
        .create_credential(CreateCredential {
            name: "endpoint".into(),
            kind: CredentialKind::EndpointToken,
            secret: "fictional-setup-token".into(),
        })
        .await
        .unwrap();
    let mut recipe = crate::tensorlake::recipes::tests_support::save();
    recipe.source = RecipeSource::Image {
        image: "example/systemd".into(),
    };
    recipe.claude = Some(farhelm_proto::tensorlake::recipes::EndpointSave {
        base_url: "https://endpoint.example/v1".into(),
        token_credential: Some("endpoint".into()),
    });
    f.harness
        .store
        .update_tensorlake_recipe("test-recipe".into(), recipe)
        .await
        .unwrap()
        .unwrap();
    std::fs::write(f.root.path().join("backend/config.json"),
        r#"{"default":{"fail_action":"read-agent-settings","message":"fictional-private-diagnostic"}}"#).unwrap();
    let accepted = f.add().await;
    let view = f.finish(accepted.host_id).await;
    assert_eq!(view.status, RunStatus::Completed);
    let configure = view
        .steps
        .iter()
        .find(|step| step.step == "configure-agents")
        .unwrap();
    assert_eq!(configure.status, StepStatus::Completed);
    assert_eq!(
        configure.message.as_deref(),
        Some("could not read the agent settings")
    );
    assert_eq!(
        f.service
            .credential_delivery()
            .warning(accepted.host_id)
            .as_deref(),
        Some("could not read the agent settings")
    );
    let events = std::fs::read_to_string(f.root.path().join("backend/events.jsonl")).unwrap();
    let read = events.find("read-agent-settings").unwrap();
    let enable = events.find("enable-supervisor").unwrap();
    assert!(
        read < enable,
        "the warning must precede continued installation: {events}"
    );
    assert!(
        !serde_json::to_string(&view)
            .unwrap()
            .contains("fictional-private-diagnostic")
    );
    let host = accepted.host_id;
    let record: String = f
        .harness
        .store
        .conn()
        .call("inspect fixture delivery home", move |conn| {
            Ok(conn.query_row(
                "SELECT tensorlake_credential_delivery FROM hosts WHERE id = ?1",
                [host],
                |row| row.get(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&record).unwrap()["home"],
        "/home/farhelm-e2e"
    );
}

/// Codex modes need setup's credential step even without any API endpoint.
/// Subscription seeds before supervisor startup; None still reports a failed
/// config cleanup as a warning while installation continues.
#[farhelm_testtrace::test]
async fn codex_subscription_and_none_enter_setup_without_an_endpoint() {
    use farhelm_proto::credentials::{CreateCredential, CredentialKind};
    for subscription in [true, false] {
        let f = fixture(false).await;
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.source = RecipeSource::Image {
            image: "example/systemd".into(),
        };
        if subscription {
            f.harness
                .store
                .create_credential(CreateCredential {
                    name: "codex".into(),
                    kind: CredentialKind::CodexLogin,
                    secret: r#"{"opaque":"fictional-setup-codex-secret"}"#.into(),
                })
                .await
                .unwrap();
            recipe.codex_login_credential = Some("codex".into());
        } else {
            std::fs::write(f.root.path().join("backend/config.json"), r#"{"default":{"fail_action":"read-agent-settings","message":"fictional-private-diagnostic"}}"#).unwrap();
        }
        f.harness
            .store
            .update_tensorlake_recipe("test-recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
        let accepted = f.add().await;
        let view = f.finish(accepted.host_id).await;
        assert_eq!(view.status, RunStatus::Completed);
        let step = view
            .steps
            .iter()
            .find(|step| step.step == "configure-agents")
            .expect("every Codex mode has a configuration step");
        assert_eq!(step.status, StepStatus::Completed);
        assert_eq!(step.message.is_none(), subscription);
        if !subscription {
            assert_eq!(
                step.message.as_deref(),
                Some("could not read the agent settings")
            );
        }
        let events = std::fs::read_to_string(f.root.path().join("backend/events.jsonl")).unwrap();
        assert!(
            events.find("check-supervisor-state").unwrap()
                < events.find("read-agent-settings").unwrap()
        );
        assert!(
            events.find("read-agent-settings").unwrap() < events.find("enable-supervisor").unwrap()
        );
        assert!(
            !events.contains("fictional-setup-codex-secret")
                && !events.contains("fictional-private-diagnostic")
        );
        let host = accepted.host_id;
        let json: String = f
            .harness
            .store
            .conn()
            .call("inspect fixture Codex stamp", move |conn| {
                Ok(conn.query_row(
                    "SELECT tensorlake_credential_delivery FROM hosts WHERE id = ?1",
                    [host],
                    |row| row.get(0),
                )?)
            })
            .await
            .unwrap();
        let record: serde_json::Value = serde_json::from_str(&json).unwrap();
        if subscription {
            assert_eq!(record["codex"], "Cleared");
            let auth = record["codex_auth"].as_array().unwrap();
            assert_eq!(auth.len(), 3);
            assert_eq!(auth[0], "codex");
            assert_eq!(auth[1], 1);
            uuid::Uuid::parse_str(auth[2].as_str().unwrap()).unwrap();
        } else {
            assert!(record["codex"].is_null() && record["codex_auth"].is_null());
        }
    }
}

/// GitHub-only recipes must enter setup's credential step without an agent
/// endpoint to trigger it. Reused images without gh still finish installing
/// Farhelm after the token upload, keeping the unsuccessful slot retryable.
#[farhelm_testtrace::test]
async fn github_only_setup_delivers_before_starting_the_supervisor() {
    use farhelm_proto::credentials::{CreateCredential, CredentialKind};
    for github_cli in [true, false] {
        let f = fixture(false).await;
        f.harness
            .store
            .create_credential(CreateCredential {
                name: "github".into(),
                kind: CredentialKind::GithubToken,
                secret: "fictional-setup-github-token".into(),
            })
            .await
            .unwrap();
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.source = RecipeSource::Image {
            image: "example/systemd".into(),
        };
        recipe.github_token_credential = Some("github".into());
        f.harness
            .store
            .update_tensorlake_recipe("test-recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
        std::fs::write(
            f.root.path().join("backend/config.json"),
            serde_json::to_vec(&serde_json::json!({"default": {"github_cli": github_cli}}))
                .unwrap(),
        )
        .unwrap();
        let accepted = f.add().await;
        let view = f.finish(accepted.host_id).await;
        assert_eq!(view.status, RunStatus::Completed);
        let step = view
            .steps
            .iter()
            .find(|step| step.step == "configure-agents")
            .expect("GitHub alone creates the credential step");
        assert_eq!(step.status, StepStatus::Completed);
        let warning = f.service.credential_delivery().warning(accepted.host_id);
        assert_eq!(warning.is_none(), github_cli);
        if !github_cli {
            assert!(warning.unwrap().contains("private clones need gh"));
        }
        let events = std::fs::read_to_string(f.root.path().join("backend/events.jsonl")).unwrap();
        assert!(
            events.find("write-unit").unwrap() < events.find("configure-github-helper").unwrap()
        );
        assert!(
            events.find("configure-github-helper").unwrap()
                < events.find("enable-supervisor").unwrap()
        );
        let host = accepted.host_id;
        let record: String = f
            .harness
            .store
            .conn()
            .call("inspect GitHub setup stamp", move |conn| {
                Ok(conn.query_row(
                    "SELECT tensorlake_credential_delivery FROM hosts WHERE id = ?1",
                    [host],
                    |row| row.get(0),
                )?)
            })
            .await
            .unwrap();
        assert!(!record.contains("fictional-setup-github-token"));
        let record: serde_json::Value = serde_json::from_str(&record).unwrap();
        assert_eq!(!record["github"].is_null(), github_cli);
        if github_cli {
            assert_eq!(record["github"]["Token"]["name"], "github");
            assert_eq!(record["github"]["Token"]["revision"], 1);
        }
    }
}

/// A missing key and malformed outer request fail before API work or host
/// registration. This is the boundary that keeps Continue free of billing.
#[farhelm_testtrace::test]
async fn tensorlake_probe_is_free_and_validates_outer_fields() {
    let f = fixture(false).await;
    let ProbeResponse::TensorlakeProvisionable {
        sandbox,
        confirmation,
        ..
    } = f
        .probe(ProbeDestination::Tensorlake {
            name: "Workspace".into(),
            recipe: "test-recipe".into(),
        })
        .await
    else {
        panic!("expected plan")
    };
    assert!(sandbox.starts_with("farhelm-host-"));
    assert!(confirmation.contains("bills while running"));
    assert!(!f.root.path().join("api/events.jsonl").exists());
    assert_eq!(f.harness.store.list_hosts().await.unwrap().len(), 1);
    for request in [
        ProbeRequest {
            target: ProbeDestination::Tensorlake {
                name: "".into(),
                recipe: "test-recipe".into(),
            },
            remote_farhelm: None,
            remote_state_dir: None,
            new_entry: None,
        },
        ProbeRequest {
            target: ProbeDestination::Tensorlake {
                name: "x".into(),
                recipe: "test-recipe".into(),
            },
            remote_farhelm: Some("/x".into()),
            remote_state_dir: None,
            new_entry: None,
        },
    ] {
        assert!(
            f.service
                .probe_tensorlake(request, f.tensorlake.clone())
                .await
                .is_err()
        );
    }
    f.harness
        .store
        .save_tensorlake_config(SaveSettings {
            clear_api_key: true,
            ..Default::default()
        })
        .await
        .ok()
        .unwrap();
    let error = f
        .service
        .probe_tensorlake(
            ProbeRequest {
                target: ProbeDestination::Tensorlake {
                    name: "x".into(),
                    recipe: "test-recipe".into(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
                new_entry: None,
            },
            f.tensorlake.clone(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("API key"));
}

/// The run builds once, records its host before work, splices the concrete
/// install steps, and defaults only YOLO permission on. Rerun keeps the
/// same sandbox rather than allocating another billed machine.
#[farhelm_testtrace::test]
async fn managed_add_splices_setup_and_rerun_adopts_the_same_sandbox() {
    let f = fixture(true).await;
    let accepted = f.add().await;
    assert!(
        f.harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .any(|row| row.id == accepted.host_id)
    );
    let view = f.finish(accepted.host_id).await;
    assert_eq!(view.status, RunStatus::Completed, "{:?}", view.message);
    assert_eq!(view.steps[0].step, "build-snapshot");
    assert!(view.steps.iter().any(|s| s.step == "attach-supervisor"));
    assert!(!view.steps.iter().any(|s| s.step == "set-up-farhelm"));
    let row = f
        .harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == accepted.host_id)
        .unwrap();
    assert!(row.yolo_without_asking);
    assert!(
        !row.commands_without_asking
            && !row.links_without_asking
            && !row.local_links_without_asking
    );
    let model = f.model();
    assert_eq!(model.live_sandboxes().len(), 1);
    assert_eq!(model.sandboxes.len(), 2, "one builder and one host");
    let sandbox = model.live_sandboxes().pop().unwrap();
    let created = model.sandboxes.iter().find(|s| s.id == sandbox.id).unwrap();
    assert_eq!(created.timeout_secs, 0);
    assert!(!created.auto_resume);
    let ProbeResponse::TensorlakeProvisionable { probe_id, .. } = f
        .probe(ProbeDestination::TensorlakeHost { host: row.id })
        .await
    else {
        panic!("expected rerun plan")
    };
    f.service
        .start_add(ProvisionRequest {
            probe_id: probe_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(f.finish(row.id).await.status, RunStatus::Completed);
    assert!(
        f.service
            .start_add(ProvisionRequest { probe_id })
            .await
            .is_err(),
        "one-use confirmation"
    );
    assert_eq!(
        f.model().sandboxes.len(),
        2,
        "rerun creates neither builder nor host"
    );
}

/// Failure after create must preserve the row and sandbox id, so Delete
/// sandbox can clean up and Rerun can continue. A reuse recipe builds nothing.
#[farhelm_testtrace::test]
async fn failed_prepare_keeps_ownership_and_rerun_continues_without_recreate() {
    let f = fixture(false).await;
    std::fs::write(
        f.root.path().join("backend/config.json"),
        r#"{"default":{"fail_action":"prepare-sandbox","message":"image needs systemd"}}"#,
    )
    .unwrap();
    let accepted = f.add().await;
    let failed = f.finish(accepted.host_id).await;
    assert_eq!(failed.status, RunStatus::Failed);
    assert!(
        failed
            .steps
            .iter()
            .any(|s| s.step == "prepare-sandbox" && s.status == StepStatus::Failed)
    );
    assert_eq!(f.model().sandboxes.len(), 1);
    let row = f
        .harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == accepted.host_id)
        .unwrap();
    assert!(row.tensorlake.as_ref().unwrap().sandbox_id.is_some());
    // Stand in for a create whose sandbox exists but whose ID write was
    // lost. The pre-chosen name must recover that very sandbox on rerun.
    let host = row.id;
    f.harness
        .store
        .conn()
        .call("clear fixture sandbox id", move |conn| {
            conn.execute(
                "UPDATE hosts SET tensorlake_sandbox_id = NULL WHERE id = ?1",
                [host],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    std::fs::write(f.root.path().join("backend/config.json"), "{}").unwrap();
    let ProbeResponse::TensorlakeProvisionable { probe_id, .. } = f
        .probe(ProbeDestination::TensorlakeHost { host: row.id })
        .await
    else {
        panic!("expected rerun")
    };
    f.service
        .start_add(ProvisionRequest { probe_id })
        .await
        .unwrap();
    assert_eq!(f.finish(row.id).await.status, RunStatus::Completed);
    assert_eq!(f.model().sandboxes.len(), 1);
    let deleted = f.tensorlake.delete_sandbox(&row).await.unwrap();
    f.harness
        .store
        .remove_host_row(row.id, Some(&deleted))
        .await
        .unwrap();
    assert!(f.model().live_sandboxes().is_empty());
}

/// Copied supervisor state is a refusal with its own explanation, before
/// installation paths are adopted. It must stay retryable without creating
/// a second sandbox or silently accepting the copied identity.
#[farhelm_testtrace::test]
async fn copied_supervisor_state_refuses_before_installation() {
    let f = fixture(false).await;
    std::fs::write(f.root.path().join("backend/config.json"), r#"{"default":{"fail_action":"check-supervisor-state","message":"image contains Farhelm supervisor state"}}"#).unwrap();
    let accepted = f.add().await;
    let failed = f.finish(accepted.host_id).await;
    assert_eq!(failed.status, RunStatus::Failed);
    assert!(failed.message.unwrap().contains("supervisor state"));
    let row = f
        .harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == accepted.host_id)
        .unwrap();
    assert!(row.remote_state_dir.is_none());
    assert!(row.host_identity.is_none());
    assert_eq!(f.model().live_sandboxes().len(), 1);
    // Naming the same sandbox through the generic SSH probe must not adopt
    // a copied supervisor and thereby write the coordinates that open dialing.
    let refused = f
        .service
        .probe(ProbeRequest {
            target: ProbeDestination::Ssh {
                destination: row.destination.clone().unwrap(),
            },
            remote_farhelm: None,
            remote_state_dir: None,
            new_entry: None,
        })
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("initial Tensorlake setup"));
    assert!(
        f.service
            .plan_update(row.id)
            .await
            .unwrap_err()
            .to_string()
            .contains("initial Tensorlake setup")
    );
    f.harness.fleet.edit(row.id, |script| {
        script.reachable = true;
        script.identity = Some("copied-supervisor".into());
    });
    // Deliberately drop the in-memory hold, as a restart would. The durable
    // coordinates gate must still refuse before any transport/hello runs.
    f.harness.manager.release_hold(row.id, true);
    f.harness.await_state(row.id, |state| matches!(state, HostState::Unreachable { last_error, .. } if last_error.contains("supervisor state check"))).await;
    assert_eq!(f.harness.fleet.dials(row.id), 0);
    let Fixture {
        harness,
        service,
        tensorlake,
        root: _root,
    } = f;
    drop(service);
    drop(tensorlake);
    let harness = harness.restart_with(|_| {}).await;
    harness.await_state(row.id, |state| matches!(state, HostState::Unreachable { last_error, .. } if last_error.contains("supervisor state check"))).await;
    assert_eq!(
        harness.fleet.dials(row.id),
        0,
        "restart must not dial the copied supervisor"
    );
    let restarted = harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == row.id)
        .unwrap();
    assert!(restarted.host_identity.is_none());
    // Positive control: the very same peer answers with the copied identity
    // once this test explicitly admits it. A dead peer cannot make the
    // preceding zero-dial observations pass by accident.
    harness
        .store
        .admit_tensorlake_install(row.id, "/fixture/farhelm", "/fixture/state")
        .await
        .unwrap();
    harness.manager.sync_registry().await.unwrap();
    harness.manager.retry_now(row.id).await.unwrap();
    harness
        .await_refreshed_as(row.id, "copied-supervisor", 0)
        .await;
}

/// A create's failed reply and immediate 404s do not settle ownership. Delete
/// must retain the row across a restart, then adopt and terminate the sandbox
/// when that same outstanding request finally becomes visible by name.
#[farhelm_testtrace::test]
async fn unresolved_create_blocks_delete_across_restart_until_sandbox_appears() {
    let f = fixture(false).await;
    std::fs::write(
        f.root.path().join("api/config.json"),
        r#"{"fail":{"create_sandbox":{"status":500,"message":"reply lost"}}}"#,
    )
    .unwrap();
    let accepted = f.add().await;
    assert_eq!(f.finish(accepted.host_id).await.status, RunStatus::Failed);
    let row = f
        .harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == accepted.host_id)
        .unwrap();
    let binding = row.tensorlake.as_ref().unwrap();
    assert!(binding.sandbox_id.is_none());
    let sent = binding
        .pending_create_at
        .expect("failed create is durably outstanding");
    assert!(
        f.model().live_sandboxes().is_empty(),
        "premise: lookups still answer 404"
    );
    let error = f.tensorlake.delete_sandbox(&row).await.unwrap_err();
    assert!(error.to_string().contains("may still be starting"));
    assert!(
        f.harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .any(|r| r.id == row.id)
    );
    let Fixture {
        harness,
        service,
        tensorlake,
        root,
    } = f;
    drop(service);
    drop(tensorlake);
    let harness = harness.restart_with(|_| {}).await;
    let row = harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == row.id)
        .unwrap();
    assert_eq!(
        row.tensorlake.as_ref().unwrap().pending_create_at,
        Some(sent)
    );
    let api = Arc::new(E2eTensorlake::new(root.path().join("api"), root.path()).unwrap());
    let tensorlake = Tensorlake::production(harness.store.clone(), root.path())
        .unwrap()
        .with_api(api);
    assert!(
        tensorlake
            .delete_sandbox(&row)
            .await
            .unwrap_err()
            .to_string()
            .contains("may still be starting")
    );
    std::fs::write(root.path().join("api/config.json"), "{}").unwrap();
    // Complete the outstanding request in Tensorlake's own model without
    // sending another create through Farhelm.
    let mut model: FakeModel =
        serde_json::from_slice(&std::fs::read(root.path().join("api/state.json")).unwrap())
            .unwrap();
    let created = model
        .create(&CreateSandbox {
            name: row.tensorlake.as_ref().unwrap().sandbox.clone(),
            source: SandboxSource::Image("example/systemd".into()),
            resources: Resources {
                cpus: 2.0,
                memory_mb: 4096,
                disk_mb: 20480,
            },
            timeout_secs: 0,
            auto_resume: false,
            max_pending_secs: 900,
        })
        .unwrap();
    std::fs::write(
        root.path().join("api/state.json"),
        serde_json::to_vec(&model).unwrap(),
    )
    .unwrap();
    let proof = tensorlake.delete_sandbox(&row).await.unwrap();
    let recorded = harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == row.id)
        .unwrap();
    assert_eq!(
        recorded.tensorlake.as_ref().unwrap().sandbox_id.as_deref(),
        Some(created.sandbox_id.as_str())
    );
    assert!(
        recorded
            .tensorlake
            .as_ref()
            .unwrap()
            .pending_create_at
            .is_none()
    );
    harness
        .store
        .remove_host_row(row.id, Some(&proof))
        .await
        .unwrap();
    let model: FakeModel =
        serde_json::from_slice(&std::fs::read(root.path().join("api/state.json")).unwrap())
            .unwrap();
    assert!(model.live_sandboxes().is_empty());
}

/// A missing name becomes conclusive only after the shared settle window.
/// Definite API refusals clear the marker immediately and need no such wait.
#[farhelm_testtrace::test]
async fn settled_absence_and_definite_refusal_allow_delete() {
    for status in [500, 401] {
        let f = fixture(false).await;
        std::fs::write(
            f.root.path().join("api/config.json"),
            format!(r#"{{"fail":{{"create_sandbox":{{"status":{status}}}}}}}"#),
        )
        .unwrap();
        let accepted = f.add().await;
        assert_eq!(f.finish(accepted.host_id).await.status, RunStatus::Failed);
        let row = f
            .harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.id == accepted.host_id)
            .unwrap();
        assert_eq!(
            row.tensorlake.as_ref().unwrap().pending_create_at.is_some(),
            status == 500
        );
        if status == 500 {
            let settled = crate::tensorlake::recipes::now_secs()
                - crate::tensorlake::build::CREATE_SETTLE_SECS;
            f.harness
                .store
                .set_tensorlake_pending_create(row.id, Some(settled))
                .await
                .unwrap();
        }
        let row = f
            .harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.id == accepted.host_id)
            .unwrap();
        let proof = f.tensorlake.delete_sandbox(&row).await.unwrap();
        f.harness
            .store
            .remove_host_row(row.id, Some(&proof))
            .await
            .unwrap();
    }
}

/// Names are ownership tags, not arbitrary display text. Sanitizing Unicode
/// and punctuation must not collide with the browser harness's reserved tag.
#[test]
fn names_fit_tensorlake_grammar() {
    for name in ["", "---", "😀", "GPU Dev / 1", &"a".repeat(300)] {
        let result = sandbox_name("k7q2", name);
        assert!(result.starts_with("farhelm-host-k7q2-"));
        assert!(result.len() <= 63);
        assert!(
            result
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        );
        assert!(!result.ends_with('-'));
        assert!(!result.starts_with("farhelm-sbx-"));
    }
}
