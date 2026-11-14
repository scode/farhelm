//! Session controls adapt existing whole-machine operations for disposable
//! environments. The user's Delete of a session asks the environment's own
//! supervisor, which deletes the session and, if it was the last, seals
//! itself so the helm can destroy the environment (SPEC_impl.md, "Environment
//! Delete and the seal"); the helm never counts sessions itself. When that
//! supervisor cannot be asked, the page offers the warned whole-environment
//! Delete, which destroys the resource without any count. Replace's internal
//! source removal remains a supervisor session operation that never seals.

use super::*;

/// Recipe placement has its own endpoint so an older helm cannot silently
/// ignore it and create the session on its ordinary default host.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SandboxCreateRequest {
    recipe: String,
    #[serde(default)]
    confirmed: bool,
    name_kind: Option<farhelm_proto::launcher::TemplateNameKind>,
    request: CreateReq,
}

/// Start one billed resource only after the user has accepted its consequence.
/// Response headers name existing host progress immediately. The ordinary JSON
/// body arrives when the owned setup task finishes, including after successful
/// failure cleanup has removed that host's progress. No retained job registry is
/// needed, and dropping this response cannot cancel the provisioning task.
pub(crate) async fn create_sandbox_session(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<SandboxCreateRequest>,
) -> axum::response::Response {
    create_environment_session(
        State(state),
        axum::Json(EnvironmentCreateRequest {
            recipe: request.recipe,
            place: farhelm_proto::Place::TensorlakeAccount,
            confirmed: request.confirmed,
            name_kind: request.name_kind,
            request: request.request,
        }),
    )
    .await
}

/// Protected admission never falls through to an older ordinary-host decoder.
/// Provider I/O and task lifetime remain in the existing provisioning service.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EnvironmentCreateRequest {
    recipe: String,
    place: farhelm_proto::Place,
    #[serde(default)]
    confirmed: bool,
    name_kind: Option<farhelm_proto::launcher::TemplateNameKind>,
    request: CreateReq,
}

/// Admit one captured environment target and stream the existing task result.
/// Headers identify recovery progress before the ordinary session reply is ready.
pub(crate) async fn create_environment_session(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<EnvironmentCreateRequest>,
) -> axum::response::Response {
    let result = crate::run_owned(async move {
        if request.place.provider().asks_before_creating()
            && !request.confirmed
            && !state.tensorlake.settings().await?.create_without_asking
        {
            return Err(invalid_request(
                "confirm creation of a billed sandbox before launching".into(),
            ));
        }
        Arc::clone(&state.provisioning)
            .start_session_environment(
                state,
                request.recipe,
                request.place,
                request.request,
                request.name_kind,
            )
            .await
    })
    .await;
    let (accepted, result) = match result {
        Ok(accepted) => accepted,
        Err(error) => return crate::provisioning::provisioning_error(error),
    };
    use futures_util::StreamExt as _;
    // A first whitespace frame flushes progress headers while leaving one
    // valid JSON value for the client's ordinary decoder to read at the end.
    let prefix = futures_util::stream::once(async { Ok::<_, std::io::Error>("\n".to_string()) });
    let body = futures_util::stream::once(async move {
        let reply = match result.await {
            Ok(Ok(session)) => serde_json::json!({ "session": session }),
            Ok(Err(error)) => serde_json::json!({ "error": error.to_string() }),
            Err(_) => {
                serde_json::json!({ "error": "The launch task ended without a result. Check Hosts for the named environment before launching again.", "uncertain": true })
            }
        };
        Ok::<_, std::io::Error>(reply.to_string())
    });
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .header("x-farhelm-environment-host", accepted.host_id.to_string())
        .header("x-farhelm-environment-run", accepted.run_id.clone())
        .header("x-farhelm-sandbox-host", accepted.host_id.to_string())
        .header("x-farhelm-sandbox-run", accepted.run_id)
        .body(axum::body::Body::from_stream(prefix.chain(body)))
        .expect("launch headers contain only a host number and a generated UUID")
}

/// Nothing was decided about an environment session's Delete: its
/// environment's supervisor could not be asked (the environment is suspended,
/// paused or unreachable, or its Farhelm is a version this helm cannot talk
/// to), or the request got no answer in time. The HTTP edge marks it with
/// `ENVIRONMENT_UNASKED_HEADER`, which is what makes the page offer the
/// warned whole-environment Delete; the helm itself never escalates.
///
/// The text says what is known: an unanswered request may still have been
/// acted on, which the next listing shows (a seal included, which the helm
/// then finishes).
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct EnvironmentUnasked(String);

/// How long the helm waits for its turn among a host's environment session
/// Deletes before answering "busy" (`ConnectionManager::environment_delete_turn`).
/// A turn lasts one supervisor request, so a whole queue of a bulk Delete's
/// sessions fits; a turn held longer means something is wrong, and the user
/// should hear so rather than wait without a bound.
pub(crate) const ENVIRONMENT_DELETE_TURN_WAIT: std::time::Duration =
    std::time::Duration::from_secs(60);

/// How long the helm waits for an environment's supervisor to answer an
/// environment session Delete before reporting that nothing was decided.
///
/// Above everything the supervisor may spend before it answers: its bounded
/// wait for its own locks (`SEAL_DELETE_ADMISSION_WAIT` there, 30 seconds),
/// then a teardown whose terminal-host settling and ending each have their
/// own budget (`DELETE_HOST_END_BUDGET`, 60 seconds each), and the process
/// sweep and checkout move, which are ordinarily seconds. Four minutes leaves
/// room for those on a slow machine; a request still unanswered then has
/// almost certainly lost its connection.
pub(crate) const ENVIRONMENT_DELETE_REPLY_DEADLINE: std::time::Duration =
    std::time::Duration::from_secs(240);

/// The user's Delete of a session in a disposable environment, after the
/// confirmation that states both outcomes (SPEC.md, "Sandbox sessions").
///
/// In order: deny the session's pending approval cards (a spawn card still
/// waiting would hold the agent fence the supervisor waits for); wait for
/// this host's turn among environment Deletes; take the host's provisioning
/// lock without waiting, so a setup, update, Suspend, Resume or
/// whole-environment Delete answers "busy"; re-read the row under that lock
/// and require a session-owned host whose removal deletes its machine; then,
/// only while the helm is connected to the environment's supervisor, send
/// `DeleteSessionOrSeal` with a reply deadline.
///
/// `Deleted` forgets the session locally and reports the supervisor's own
/// count; `Sealed` destroys the environment under the lock already held
/// (`hosts::finish_sealed_environment`); a `NotFound` from the supervisor
/// means an earlier attempt deleted only this session and is forgotten as a
/// success. Not connected, not sent, unanswered or past the deadline is
/// [`EnvironmentUnasked`]. Any other refusal from the supervisor (its own
/// "busy, nothing was deleted") is passed on as it is.
pub(super) async fn delete_environment_session(
    state: &AppState,
    key: &store::SessionKey,
) -> anyhow::Result<farhelm_proto::SessionDeleteOutcome> {
    if !crate::hosts::registry_row(state, key.host)
        .await?
        .is_session_owned()
    {
        return Err(anyhow::Error::new(SupervisorError::helm(
            ErrorKind::Conflict,
            "this session no longer belongs to a disposable environment".into(),
        )));
    }
    let _denied = state.approvals.deny_session(key);
    let _turn = tokio::time::timeout(
        ENVIRONMENT_DELETE_TURN_WAIT,
        state.manager.environment_delete_turn(key.host),
    )
    .await
    .map_err(|_| anyhow::Error::new(crate::environment::lifecycle::HostBusy))?;
    let provisioning =
        crate::environment::lifecycle::provision_lock_or_busy(&state.manager, key.host)?;
    let row = crate::hosts::registry_row(state, key.host).await?;
    if !row.is_session_owned() || !row.kind.removal_deletes_machine() {
        return Err(anyhow::Error::new(SupervisorError::helm(
            ErrorKind::Conflict,
            "this session no longer belongs to a disposable environment".into(),
        )));
    }
    // One capture decides both whether the helm can ask and which connection
    // the claim is made on. No session-membership check: after a lost reply
    // the helm's list may already omit the session, and whether it exists is
    // the environment's supervisor's to answer (`NotFound` below).
    let status = state
        .manager
        .status(key.host)
        .ok_or_else(|| anyhow::Error::new(crate::store::HostStoreError::HostNotFound(key.host)))?;
    let Some(client) = status.client.clone() else {
        return Err(anyhow::Error::new(EnvironmentUnasked(
            "Farhelm cannot reach this environment's own Farhelm right now, so nothing was \
             deleted"
                .into(),
        )));
    };
    let claim = crate::manager::SessionClaim {
        host: key.host,
        entry_uuid: status.entry_uuid.clone(),
        incarnation: status.incarnation,
        identity: match &status.state {
            crate::manager::HostState::Connected { identity, .. } => identity.clone(),
            _ => None,
        },
    };
    let id = key.session_id.as_str();
    let reply = tokio::time::timeout(
        ENVIRONMENT_DELETE_REPLY_DEADLINE,
        client.delete_session_or_seal(id),
    )
    .await;
    match reply {
        Err(_elapsed) => Err(anyhow::Error::new(EnvironmentUnasked(
            "this environment's own Farhelm did not answer in time; Farhelm cannot tell whether \
             the session was deleted"
                .into(),
        ))),
        Ok(Err(error)) => {
            if !crate::error_kind_is_a_supervisor_reply(&error) {
                // The request was not sent, or its connection ended before
                // the answer: nothing this side knows was decided.
                return Err(anyhow::Error::new(EnvironmentUnasked(format!(
                    "the connection to this environment's own Farhelm ended before it answered, \
                     so Farhelm cannot tell whether the session was deleted: {error:#}"
                ))));
            }
            if crate::error_kind(&error) == ErrorKind::NotFound {
                // An unsealed supervisor without the session: an earlier
                // attempt, whose reply was lost, deleted only this session.
                forget_deleted_session(state, &claim, id).await;
                return Ok(farhelm_proto::SessionDeleteOutcome::default());
            }
            Err(error)
        }
        Ok(Ok((farhelm_proto::SealOutcome::Deleted { remaining }, notice))) => {
            forget_deleted_session(state, &claim, id).await;
            Ok(farhelm_proto::SessionDeleteOutcome {
                notice,
                archived: false,
                environment_remaining: Some(remaining),
            })
        }
        Ok(Ok((farhelm_proto::SealOutcome::Sealed, _notice))) => {
            // A notice from the supervisor here is about cleanup inside an
            // environment that is now destroyed with everything in it.
            finish_seal_locked(state, key.host, provisioning).await?;
            Ok(farhelm_proto::SessionDeleteOutcome::default())
        }
    }
}

/// How often the finisher looks again at sealed hosts it could not start on
/// (their provisioning lock was busy), besides being woken by a new seal.
pub(crate) const SEAL_FINISH_RETRY: std::time::Duration = std::time::Duration::from_secs(10);

/// Destroy the sealed environment of `host`, recording how it went, for the
/// finisher, Retry and the user's own Delete alike. `Ok(false)` when the
/// host's provisioning lock is busy (nothing tried; try again later).
///
/// A failure releases the host's connection hold, so its actor reconnects to
/// the sealed supervisor, but the failure is recorded in the manager's seal
/// record, which keeps the host shown as being deleted with the reason and no
/// client, and keeps the finisher from retrying it until the user's Retry. A
/// host that is not a disposable environment is refused before anything is
/// destroyed and is not retried either; this helm never sends such a host the
/// request that seals.
pub(crate) async fn finish_seal(state: &AppState, host: store::HostId) -> anyhow::Result<bool> {
    let Some(provisioning) = state.manager.try_host_provision_lock(host) else {
        return Ok(false);
    };
    finish_seal_locked(state, host, provisioning)
        .await
        .map(|()| true)
}

/// [`finish_seal`] with the provisioning lock already held.
async fn finish_seal_locked(
    state: &AppState,
    host: store::HostId,
    provisioning: crate::manager::ProvisionLockGuard,
) -> anyhow::Result<()> {
    // Recorded before the attempt: the record is what shows the host as being
    // deleted (and, afterwards, a failure's reason until Retry).
    state.manager.sealed_attempt(host);
    let result = crate::hosts::finish_sealed_environment(state, host, provisioning).await;
    state.manager.sealed_outcome(
        host,
        result.as_ref().err().map(|error| format!("{error:#}")),
    );
    result
}

/// `POST /api/hosts/{id}/retry-delete`: destroy again a sealed environment
/// whose destruction failed, the Retry its host shows in Hosts and the
/// sidebar attention area.
///
/// Refused unless the host currently shows that failure
/// (`HostState::Deleting` with a reason): a seal is what authorizes this
/// destruction without a fresh confirmation, so a stale or forged click must
/// never reach an environment that is not sealed. Answers once the attempt
/// is over; a new failure shows on the host again.
pub(crate) async fn retry_environment_delete(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<store::HostId>,
) -> axum::response::Response {
    let result = crate::run_owned(async move {
        if !state.manager.sealed_failed(host) {
            return Err(anyhow::Error::new(SupervisorError::helm(
                ErrorKind::Conflict,
                "this host is not an environment whose deletion failed; nothing was retried".into(),
            )));
        }
        if !finish_seal(&state, host).await? {
            return Err(anyhow::Error::new(crate::environment::lifecycle::HostBusy));
        }
        Ok(())
    })
    .await;
    match result {
        Ok(()) => axum::Json(serde_json::json!({})).into_response(),
        Err(error) => http_error(error),
    }
}

/// Start the helm's finisher of sealed environments: whenever a listing
/// reports a seal (`ConnectionManager::note_sealed`), destroy that
/// environment, whether or not this helm sent the Delete that sealed it (a
/// lost reply, a failed destruction retried, a restart of the helm). Runs for
/// the helm's lifetime, one environment at a time.
pub(crate) fn spawn_seal_finisher(state: Arc<AppState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = state.manager.sealed_noted() => {}
                () = tokio::time::sleep(SEAL_FINISH_RETRY) => {}
            }
            for host in state.manager.sealed_pending() {
                match finish_seal(&state, host).await {
                    Ok(true) => tracing::info!(host, "destroyed a sealed environment"),
                    Ok(false) => {}
                    Err(error) => tracing::warn!(host, error = %format!("{error:#}"),
                        "could not destroy a sealed environment; it waits for Retry"),
                }
            }
        }
    })
}

/// Refuse session-only deletion of a resource whose files belong to its user.
/// The same refusal closes the agent helper and an older page's Delete path.
pub(super) fn user_only_delete() -> anyhow::Error {
    anyhow::Error::new(SupervisorError::helm(
        ErrorKind::Conflict,
        "Only the user can delete a session in a disposable environment, from Farhelm's own Delete"
            .to_string(),
    ))
}

/// Return None for an ordinary host; an owned host requires explicit consent
/// to end every process and discard every file, including unlisted leftovers.
/// Reuse host removal's lock, bounded vendor cleanup and cascading local cleanup.
pub(super) async fn delete_if_owned(
    state: &AppState,
    key: &store::SessionKey,
    confirmed: bool,
    guard: farhelm_proto::DeleteGuard,
) -> anyhow::Result<Option<farhelm_proto::SessionDeleteOutcome>> {
    let row = crate::hosts::registry_row(state, key.host).await?;
    if !row.is_session_owned() {
        if confirmed {
            return Err(anyhow::Error::new(SupervisorError::helm(
                ErrorKind::Conflict,
                "this session no longer belongs to a disposable environment".into(),
            )));
        }
        return Ok(None);
    }
    if !confirmed || guard != farhelm_proto::DeleteGuard::Unconditional {
        return Err(user_only_delete());
    }
    // Resolve the selected session's existing reference, without contacting a
    // suspended supervisor or interpreting cache absence as a membership proof.
    resolve_owner(state, key).await?;
    crate::hosts::remove_host_checked(state, key.host, |now| {
        if now.is_session_owned() {
            Ok(())
        } else {
            Err(user_only_delete())
        }
    })
    .await?;
    Ok(Some(farhelm_proto::SessionDeleteOutcome::default()))
}

/// Stop and suspend is one helm-owned user operation. A dropped browser must
/// not abandon the second half after it has already stopped the user's agent.
pub(crate) async fn stop_and_suspend(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(request): axum::Json<farhelm_proto::SessionStopRequest>,
) -> impl IntoResponse {
    let result = crate::run_owned(async move {
        let key = resolve_session_ref(&state, &id).await?;
        if !crate::hosts::registry_row(&state, key.host)
            .await?
            .is_session_owned()
        {
            return Err(invalid_request("Stop and suspend is available only for environment sessions".into()));
        }
        let outcome = do_stop_session(
            &state,
            &key,
            request.refuse_if_tabs_open,
            request.refuse_if_agent_running,
        )
        .await?;
        if let Some(notice) = outcome.notice {
            return Err(anyhow::Error::new(SupervisorError::helm(
                ErrorKind::Conflict, format!("The agent stopped, but cleanup was incomplete; the environment was not suspended: {notice}"),
            )));
        }
        crate::environment::lifecycle::change(&state, key.host, false).await
            .context("The agent stopped, but the environment was not suspended")
    })
    .await;
    match result {
        Ok(()) => axum::Json(farhelm_proto::SessionStopOutcome::default()).into_response(),
        Err(error) => http_error(error),
    }
}
