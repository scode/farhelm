//! Session controls adapt existing whole-machine operations for disposable
//! environments. The user's Delete of a session asks the environment's own
//! supervisor, which deletes the session and, if it was the last, seals
//! itself so the helm can destroy the environment (SPEC_impl.md, "Environment
//! Delete and the seal"); the helm never counts sessions itself. When that
//! supervisor cannot be asked, the helm's marker asks a client to offer the
//! warned whole-environment Delete, which destroys the resource without any
//! count. Replace's internal
//! source removal remains a supervisor session operation that never seals.
//! Stop and suspend chains the ordinary Stop and the host's Suspend under one
//! reply bound ([`stop_and_suspend`]). Both environment Deletes and the Retry
//! of a failed destruction answer within the same bound, their work carrying
//! on past it ([`environment_session_delete_carries_on`],
//! [`whole_environment_delete_carries_on`], [`retry_environment_delete`]).

use super::*;
use farhelm_proto::recipes::RecipeSize;

/// The body of an environment create. Provider I/O and task lifetime remain in
/// the existing provisioning service.
///
/// `confirmed_size` stays optional: a launch the place does not ask about (a
/// container, or a Tensorlake account set to create without asking) sends
/// neither a confirmation nor a size. A launch that does confirm must name the
/// size the user saw ([`create_environment_session`]).
///
/// The GUI builds this body from
/// `farhelm_proto::session_requests::EnvironmentCreateBody`; the fields are
/// visible to the sessions module so its client-body tests can check that the
/// two still agree.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EnvironmentCreateRequest {
    pub(super) recipe: String,
    pub(super) place: farhelm_proto::Place,
    #[serde(default)]
    pub(super) confirmed: bool,
    #[serde(default)]
    pub(super) confirmed_size: Option<RecipeSize>,
    pub(super) name_kind: Option<farhelm_proto::launcher::TemplateNameKind>,
    pub(super) request: CreateReq,
}

/// `POST /api/environment-sessions`: admit one launch into a new disposable
/// environment and answer 202 with its run, `{host_id, run_id}`, as soon as
/// it is admitted.
///
/// The launch takes minutes (preparing the recipe, creating the sandbox or
/// container, starting Farhelm in it), far longer than the UI channel lets one
/// command hold up the commands behind it, so the route takes the accepted
/// form (SPEC_impl.md, "The UI channel"). The outcome lands in the new
/// environment host's run in the pushed host progress: completed, with the
/// session already in the session listing under that host, or failed with the
/// "Not launched." reason, which stays listed even after the launch's cleanup
/// has removed the host (`ProvisioningService::runs`). A failed run's message
/// is the whole text to show, its "Not launched." lead included; a failed run
/// marked `outcome_unknown` (a panicked task) may have left its environment
/// behind.
///
/// What is refused before admission is an ordinary error reply with nothing
/// registered. Examples include missing or stale consent (a confirmation that
/// names no size, or a size that no longer matches the recipe), an unreadable
/// request, and an unavailable recipe or place. The helm owns launch and
/// cleanup after admission, so both survive a client disconnect.
pub(crate) async fn create_environment_session(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<EnvironmentCreateRequest>,
) -> axum::response::Response {
    let result = crate::run_owned(async move {
        if request.confirmed && request.confirmed_size.is_none() {
            return Err(invalid_request(
                "confirm the sandbox size before launching".into(),
            ));
        }
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
                request.confirmed_size,
                None,
            )
            .await
    })
    .await;
    match result {
        // The run publishes the launch's end, so the task's own result,
        // which only an in-process caller (an agent's request) waits for, is
        // not needed here; dropping it cancels nothing.
        Ok((accepted, _result)) => {
            (axum::http::StatusCode::ACCEPTED, axum::Json(accepted)).into_response()
        }
        Err(error) => crate::provisioning::provisioning_error(error),
    }
}

/// The helm has no confirmed decision for an environment session's Delete.
/// The request may have stayed unsent, for instance because the environment
/// was suspended, paused or unreachable, or it may have gone unanswered.
/// The HTTP edge marks the reply with
/// `ENVIRONMENT_UNASKED_HEADER`, which asks a client to offer the warned
/// whole-environment Delete; the helm itself never escalates.
///
/// The text says what is known: an unanswered request may still have been
/// acted on, which the next listing shows (a seal included, which the helm
/// then finishes).
///
/// The route answers within the host reply bound, so the marker reaches the
/// caller only when this is decided in time: at once when the helm is not
/// connected, which is the common case, or when the connection ends early.
/// One decided later (a connection that ends after the bound, or the reply
/// deadline below) reaches only the helm's log, and the caller has the
/// bound's 504 instead ([`environment_session_delete_carries_on`]).
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct EnvironmentUnasked(String);

/// How long the helm waits for its turn among a host's environment session
/// Deletes before answering "busy" (`ConnectionManager::environment_delete_turn`).
/// A turn lasts one supervisor request, so a whole queue of a bulk Delete's
/// sessions fits; a turn held longer means something is wrong, and the Delete
/// gives up rather than wait without a bound. The route's own reply comes
/// sooner, at the host reply bound, so a "busy" decided here reaches only the
/// helm's log and the session stays listed.
pub(crate) const ENVIRONMENT_DELETE_TURN_WAIT: std::time::Duration =
    std::time::Duration::from_secs(60);

/// How long the helm waits for an environment's supervisor to answer an
/// environment session Delete before reporting an unconfirmed outcome.
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
///
/// Unbounded itself: the route runs it on a task of its own under the host
/// reply bound, so it finishes, seal and destruction included, whether or not
/// anyone still waits ([`environment_session_delete_carries_on`]).
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
        is_session_owned: status.is_session_owned,
        entry_uuid: status.entry_uuid.clone(),
        incarnation: status.incarnation,
        identity: match &status.state {
            crate::manager::HostState::Connected { identity, .. } => identity.clone(),
            _ => None,
        },
    };
    let id = key.session_id.as_str();
    let mut receiver = None;
    let reply = tokio::time::timeout(
        ENVIRONMENT_DELETE_REPLY_DEADLINE,
        client.delete_session_or_seal(id, &mut receiver),
    )
    .await;
    match reply {
        Err(_elapsed) => {
            if let Some(receiver) = receiver {
                observe_expired_delete_notice(state, key.clone(), receiver);
            }
            Err(anyhow::Error::new(EnvironmentUnasked(
                "this environment's own Farhelm did not answer in time; Farhelm cannot tell whether \
                 the session was deleted"
                    .into(),
            )))
        }
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

/// Keep the checkout location if the original non-final Delete answers after
/// its inner deadline. The wait ends on reply or connection close and owns
/// only the notice write. A late seal belongs to the listing-driven finisher.
fn observe_expired_delete_notice(
    state: &AppState,
    key: store::SessionKey,
    receiver: tokio::sync::oneshot::Receiver<farhelm_proto::ControlMsg>,
) {
    let store = state.store.clone();
    let events = Arc::clone(state.manager.events());
    tokio::spawn(async move {
        if let Ok(farhelm_proto::ControlMsg::SessionDeletedOrSealed {
            outcome: farhelm_proto::SealOutcome::Deleted { .. },
            notice: Some(detail),
            ..
        }) = receiver.await
            && let Err(error) = store.record_late_delete(key, detail, events).await
        {
            warn!(%error, "could not record the late Delete notice");
        }
    });
}

/// How often the finisher looks again at sealed hosts it could not start on
/// (their provisioning lock was busy), besides being woken by a new seal.
pub(crate) const SEAL_FINISH_RETRY: std::time::Duration = std::time::Duration::from_secs(10);

/// Finish a queued seal if it still awaits a provider attempt. A busy host stays queued, and a failure
/// that appeared since the queue was read waits for the user's Retry.
async fn finish_pending_seal(state: &AppState, host: store::HostId) -> anyhow::Result<bool> {
    let Some(provisioning) = state.manager.try_host_provision_lock(host) else {
        return Ok(false);
    };
    if !state.manager.seal_is_pending(host) {
        return Ok(false);
    }
    finish_seal_locked(state, host, provisioning)
        .await
        .map(|()| true)
}

/// Share the seal attempt across the user's Delete, Retry and the finisher. Keeping the lock through recording
/// the outcome prevents the finisher from taking over the seal between a failed provider call and its Retry state.
pub(crate) async fn finish_seal_locked(
    state: &AppState,
    host: store::HostId,
    provisioning: crate::manager::ProvisionLockGuard,
) -> anyhow::Result<()> {
    // Recorded before the attempt: the record is what shows the host as being
    // deleted (and, afterwards, a failure's reason until Retry).
    state.manager.sealed_attempt(host);
    let result = crate::hosts::finish_sealed_environment(state, host, &provisioning).await;
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
/// is over, or at the host reply bound (below); a new failure shows on the
/// host either way.
///
/// ## Answering within the host reply bound
///
/// The destruction makes the same provider calls as host removal, so it can
/// outlast the GUI's channel watchdog. Its two refusals (not a failed
/// deletion, a busy host) come from local state and answer as before; an
/// attempt still running at the host reply bound answers
/// [`retry_delete_carries_on`]'s 504 and carries on, on a task of its own,
/// which also keeps a dropped caller from cancelling it partway (the route
/// sits in the router's cancellable group). The seal record shows the host
/// as being deleted during the attempt. On completion, the host leaves the
/// lists or shows the new failure with Retry.
pub(crate) async fn retry_environment_delete(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<store::HostId>,
) -> axum::response::Response {
    within_host_reply_bound_saying(
        async move {
            match retry_failed_seal(&state, host).await {
                Ok(()) => axum::Json(serde_json::json!({})).into_response(),
                Err(error) => http_error(error),
            }
        },
        retry_delete_carries_on(),
    )
    .await
}

/// The body of [`retry_environment_delete`], unbounded: refuse a host whose
/// record holds no failed deletion, or whose provisioning lock is busy, and
/// otherwise destroy the sealed environment again.
async fn retry_failed_seal(state: &AppState, host: store::HostId) -> anyhow::Result<()> {
    let provisioning = state.manager.try_host_provision_lock(host);
    if !state.manager.sealed_failed(host) {
        return Err(anyhow::Error::new(SupervisorError::helm(
            ErrorKind::Conflict,
            "this host is not an environment whose deletion failed; nothing was retried".into(),
        )));
    }
    let Some(provisioning) = provisioning else {
        return Err(anyhow::Error::new(crate::environment::lifecycle::HostBusy));
    };
    finish_seal_locked(state, host, provisioning).await
}

/// The text of the 504 [`retry_environment_delete`] answers when its attempt
/// is still running at the host reply bound. The seal record makes it true
/// whichever way the attempt ends.
fn retry_delete_carries_on() -> String {
    format!(
        "retrying the deletion did not finish within {} seconds; it carries on, and the host's \
         state in the hosts list will show the result",
        HOST_REPLY_BOUND.as_secs()
    )
}

/// Start destroying remembered seals at once, including when the environment cannot reconnect. Later listings
/// wake the same finisher, and a busy host gets another chance after the retry interval. A failed destruction
/// waits for the user's Retry for the rest of this helm run. Runs one environment at a time.
pub(crate) fn spawn_seal_finisher(state: Arc<AppState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            for host in state.manager.sealed_pending() {
                match finish_pending_seal(&state, host).await {
                    Ok(true) => tracing::info!(host, "destroyed a sealed environment"),
                    Ok(false) => {}
                    Err(error) => tracing::warn!(host, error = %format!("{error:#}"),
                        "could not destroy a sealed environment; it waits for Retry"),
                }
            }
            tokio::select! {
                () = state.manager.sealed_noted() => {}
                () = tokio::time::sleep(SEAL_FINISH_RETRY) => {}
            }
        }
    })
}

/// Refuse session-only deletion of a resource whose files belong to its user.
/// The same refusal closes the agent helper and a plain Delete without the environment confirmation.
pub(super) fn user_only_delete() -> anyhow::Error {
    anyhow::Error::new(SupervisorError::helm(
        ErrorKind::Conflict,
        "Only the user can delete a session in a disposable environment, from Farhelm's own Delete"
            .to_string(),
    ))
}

/// The text of the 504 the environment Delete of one session
/// ([`delete_environment_session`]) answers when it is still running at the
/// host reply bound.
///
/// It is true whichever way the Delete ends, and wherever it stalled: still
/// waiting for this host's turn among environment Deletes (when the
/// environment's own Farhelm has not even been asked yet, which is why the
/// words name the Delete rather than an unanswered Farhelm), or for that
/// Farhelm to answer. A late `Deleted` takes the session out of the session
/// list, and a late `Sealed` shows the host being deleted and then gone (or
/// its failure, with Retry); a late "unasked", "busy" or other refusal decided
/// nothing, which "may" covers, and leaves the session listed to delete again.
/// What the bound loses is the count of sessions left and the "unasked"
/// marker, so the warned whole-environment Delete is no longer offered at
/// once; the next Delete offers it from the pushed host state once the host
/// shows as not connected (see `HOST_REPLY_BOUND` for the decision).
pub(super) fn environment_session_delete_carries_on() -> String {
    format!(
        "deleting this session did not finish within {} seconds; it may still delete the session, \
         along with the environment if this was its last session, and the session list will show \
         the result",
        HOST_REPLY_BOUND.as_secs()
    )
}

/// The text of the 504 the warned whole-environment Delete
/// ([`delete_whole_environment`]) answers when it is still running at the
/// host reply bound.
///
/// The destruction carries on under the host's provisioning lock, with the
/// host shown as being deleted meanwhile. A late success takes the host and its
/// sessions out of the lists. A sealed host keeps the latest failure visible
/// with Retry. An unsealed host returns to its ordinary state and the reason
/// goes to the helm's log.
pub(super) fn whole_environment_delete_carries_on() -> String {
    format!(
        "deleting the environment did not finish within {} seconds; the deletion carries on, and \
         the hosts list will show the result",
        HOST_REPLY_BOUND.as_secs()
    )
}

/// The user's warned whole-environment Delete, sent from a session of the
/// environment (`?delete_sandbox=true`): end every process and discard every
/// file, including unlisted leftovers. The confirmation it needs is the
/// warned one, so a guard flag beside it, which asks for something that
/// confirmation never promised, is refused.
///
/// Reuses host removal's lock, bounded vendor cleanup and cascading local
/// cleanup. The route runs it under the host reply bound
/// ([`whole_environment_delete_carries_on`]).
pub(super) async fn delete_whole_environment(
    state: &AppState,
    key: &store::SessionKey,
    guard: farhelm_proto::DeleteGuard,
) -> anyhow::Result<farhelm_proto::SessionDeleteOutcome> {
    let row = crate::hosts::registry_row(state, key.host).await?;
    if !row.is_session_owned() {
        return Err(anyhow::Error::new(SupervisorError::helm(
            ErrorKind::Conflict,
            "this session no longer belongs to a disposable environment".into(),
        )));
    }
    if guard != farhelm_proto::DeleteGuard::Unconditional {
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
    Ok(farhelm_proto::SessionDeleteOutcome::default())
}

/// `POST /api/sessions/{id}/stop-and-suspend`: the ordinary Stop of an
/// environment session, then its host's Suspend, as one helm-owned user
/// operation. A dropped client must not abandon the second half after the
/// first has already stopped the user's agent, and a Stop that fails or leaves
/// cleanup undone never suspends.
///
/// ## Answering within the host reply bound
///
/// One bound covers the Stop and the Suspend together, counted from the
/// handler's start: the route is one command with one reply, and the GUI's
/// ordered channel needs that reply within its watchdog, which a bound per
/// half (up to 40 seconds) would not keep. Refusals that come first (not an
/// environment session, an unknown session) and quick outcomes answer as
/// before. The helm marks its own deadline reply so the client can show that
/// the outcome is still unknown. Either half can settle while the reply is
/// in flight. The environment suspends only if Stop finishes cleanly, and a
/// late failure reaches the helm's log while the session list continues to
/// show the current session and host state. Decided by the migration's
/// orchestrator on 2026-11-15 together with the host's Suspend and Resume
/// (`crate::tensorlake::lifecycle::suspend`), whose outcome likewise lands in
/// the pushed host state.
pub(crate) async fn stop_and_suspend(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(request): axum::Json<farhelm_proto::SessionStopRequest>,
) -> axum::response::Response {
    within_host_reply_bound_marked(
        async move {
            match stop_then_suspend(&state, &id, request).await {
                Ok(()) => axum::Json(farhelm_proto::SessionStopOutcome::default()).into_response(),
                Err(error) => http_error(error),
            }
        },
        stop_and_suspend_timeout_message(),
        HelmMarker::StopAndSuspendReplyBound,
    )
    .await
}

/// The reply can reach the user after the operation has already finished.
/// Direct them to the session list for the current state.
fn stop_and_suspend_timeout_message() -> String {
    format!(
        "Stop and suspend did not finish within {} seconds and may or may not still be running. \
         The environment suspends only if Stop finishes cleanly. Check the session list for the result.",
        HOST_REPLY_BOUND.as_secs()
    )
}

/// The body of [`stop_and_suspend`], unbounded: refuse a session outside a
/// disposable environment, Stop it, and Suspend its host only after a Stop
/// with nothing left over.
async fn stop_then_suspend(
    state: &AppState,
    id: &str,
    request: farhelm_proto::SessionStopRequest,
) -> anyhow::Result<()> {
    let key = resolve_session_ref(state, id).await?;
    if !crate::hosts::registry_row(state, key.host)
        .await?
        .is_session_owned()
    {
        return Err(invalid_request(
            "Stop and suspend is available only for environment sessions".into(),
        ));
    }
    let outcome = do_stop_session(
        state,
        &key,
        request.refuse_if_tabs_open,
        request.refuse_if_agent_running,
        None,
    )
    .await?;
    if let Some(notice) = outcome.notice {
        return Err(anyhow::Error::new(SupervisorError::helm(
            ErrorKind::Conflict,
            format!(
                "The session stopped. Cleanup was incomplete, so Farhelm skipped suspension: \
                 {notice}"
            ),
        )));
    }
    crate::environment::lifecycle::change(state, key.host, false)
        .await
        .context("The session stopped. Farhelm could not suspend the environment")
}
