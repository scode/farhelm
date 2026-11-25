//! Automatic updates reuse manual planning and execution, including every refusal.
//!
//! The fleet revision is the wake source: connection transitions, provisioning
//! completion and preference writes all publish there. A shared sign-in's handoff expiry also
//! wakes this scan so an otherwise idle host can become eligible again.

use super::super::setup_sign_in::{AutomaticSignIn, Pending, SetupSettlement, SignInIntent};
use super::{HostPlanResponse, ProvisionRequest, ProvisioningRequestError, ProvisioningService};
use crate::manager::{HostSnapshot, HostState};
use crate::store::{HostId, HostKind};
use farhelm_proto::ui_channel::SshSignInOutcome;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::oneshot;

/// Application-owned listener. A weak service reference breaks the waiting task's
/// ownership cycle. Drop requests cooperative cancellation of pending listener
/// work; already accepted runs retain ordinary provisioning ownership.
pub(crate) struct AutomaticUpdates {
    task: tokio::task::JoinHandle<()>,
    /// Observe completed scans without adding any scheduling or wake source.
    #[cfg(test)]
    completed_scans: tokio::sync::watch::Receiver<u64>,
}

impl Drop for AutomaticUpdates {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl AutomaticUpdates {
    /// Request listener cancellation before manager shutdown. Pending admission
    /// waits belong to this listener; accepted runs retain ordinary ownership.
    /// Tokio applies abort when the task yields, not as a synchronous barrier.
    pub(crate) fn stop(&self) {
        self.task.abort();
    }
    /// A completed initial off scan proves a later opt-in cannot be mistaken
    /// for startup work. Revision tests subscribe before triggering the event.
    #[cfg(test)]
    pub(crate) fn completed_scans(&self) -> tokio::sync::watch::Receiver<u64> {
        self.completed_scans.clone()
    }
    /// Subscribe before the initial scan so startup and enabling cannot miss a
    /// host transition between observing the fleet and waiting for a revision.
    pub(crate) fn start(service: &Arc<ProvisioningService>) -> Self {
        if !service.payloads.allows_automatic_updates() {
            tracing::info!(
                "automatic remote-host updates are skipped with locally staged payloads"
            );
        }
        let mut events = service.manager.events().subscribe();
        let service = Arc::downgrade(service);
        #[cfg(test)]
        let (scan_notices, completed_scans) = tokio::sync::watch::channel(0_u64);
        let task = tokio::spawn(async move {
            let mut attempts = HashMap::new();
            loop {
                events.borrow_and_update();
                let Some(service) = service.upgrade() else {
                    return;
                };
                if let Err(error) = scan(&service, &mut attempts).await {
                    tracing::warn!(error = %error, "automatic remote-host update scan failed");
                }
                drop(service);
                #[cfg(test)]
                scan_notices.send_modify(|count| *count += 1);
                if events.changed().await.is_err() {
                    return;
                }
            }
        });
        Self {
            task,
            #[cfg(test)]
            completed_scans,
        }
    }
}

/// Automatic work belongs to the connection that made it eligible. A pending check owns only its
/// interest in sign-in. Dropping it on opt-out, retarget or shutdown leaves any attached manual work alive.
pub(in crate::provisioning) enum AttemptState {
    PendingSignIn {
        incarnation: u64,
        work: PendingUpdate,
    },
    Attempted(u64),
}

/// Keep Ready work with the listener's per-host reservation. Batch planning can use a shared planning
/// slot while the fleet scan probes another host. Dropping the reservation cancels unadmitted work.
pub(in crate::provisioning) struct PendingUpdate {
    sign_in: AutomaticSignIn,
    resume: tokio::task::JoinHandle<()>,
    attempted: oneshot::Receiver<bool>,
}

impl Drop for PendingUpdate {
    fn drop(&mut self) {
        self.resume.abort();
    }
}

impl AttemptState {
    /// Preserve a failure the SSH task published before opt-out or temporary ineligibility.
    /// A check released by policy has no failure to preserve.
    fn retain_for(&mut self, host: &HostSnapshot, eligible: bool) -> bool {
        match self {
            Self::Attempted(_) => true,
            Self::PendingSignIn { incarnation, .. } if *incarnation != host.incarnation => false,
            Self::PendingSignIn { work, .. } => match work.sign_in.outcome() {
                Some(SetupSettlement::Released | SetupSettlement::ReadyForManualAction) => false,
                Some(SetupSettlement::Finished(SshSignInOutcome::Failed { reason })) => {
                    *self = Self::sign_in_failed(host, &reason);
                    true
                }
                Some(SetupSettlement::Finished(SshSignInOutcome::Ready)) => {
                    // The continuation can set busy itself while it admits the run. Keep its reservation
                    // until it reports the result, even if the host snapshot now looks busy or disabled.
                    match work.attempted.try_recv() {
                        Ok(true) => {
                            *self = Self::Attempted(host.incarnation);
                            true
                        }
                        Ok(false) => false,
                        Err(oneshot::error::TryRecvError::Empty) => true,
                        Err(oneshot::error::TryRecvError::Closed) => {
                            *self = Self::sign_in_failed(
                                host,
                                "the automatic update continuation ended",
                            );
                            true
                        }
                    }
                }
                None => eligible || work.sign_in.retain_while_ineligible(),
            },
        }
    }

    /// A failed setup connection leaves the ordinary Update badge as the manual retry surface.
    fn sign_in_failed(host: &HostSnapshot, reason: &str) -> Self {
        tracing::warn!(host = host.id, incarnation = host.incarnation, %reason,
            "automatic remote-host update stopped at SSH sign-in");
        Self::Attempted(host.incarnation)
    }
}

/// Use the rail's update badge rules with an exemption for development peers.
///
/// A developer may have installed the build under test on that host. Its
/// protocol can be older than the helm's, and manual Update can still restore
/// the release. An unparseable non-development build can qualify by protocol.
fn eligible(kind: HostKind, state: &HostState, busy: bool, helm_build: &str) -> bool {
    kind.panel_updates() && !busy && connection_needs_update(state, helm_build)
}

/// A skewed connection can become ineligible without changing incarnation.
/// Apply the same connection rule when scouting and when admitting a prepared update.
pub(super) fn connection_needs_update(state: &HostState, helm_build: &str) -> bool {
    match state {
        HostState::Connected { build_version, .. } => {
            !farhelm_proto::is_development_build(build_version)
                && crate::hosts::build_is_older(build_version, helm_build)
        }
        HostState::VersionSkew {
            peer_protocol,
            our_protocol,
            peer_build,
            ..
        } => {
            peer_protocol < our_protocol
                && !farhelm_proto::is_development_build(peer_build)
                && !crate::hosts::build_is_newer(peer_build, helm_build)
        }
        _ => false,
    }
}

/// Scan the fleet without waiting for a user to answer SSH. Setup sign-in returns the planning slot first.
/// A successful sign-in dispatches batch work directly through a listener-owned continuation.
/// Reserve the attempt before starting work, since a run can publish completion before the start caller resumes.
pub(super) async fn scan(
    service: &Arc<ProvisioningService>,
    attempts: &mut HashMap<HostId, AttemptState>,
) -> anyhow::Result<()> {
    if !service.payloads.allows_automatic_updates() {
        return Ok(());
    }
    let hosts = service.manager.snapshots();
    let live: HashMap<_, _> = hosts.iter().map(|host| (host.id, host)).collect();
    let enabled = service.store.preferences().await?.auto_update_remote_hosts != Some(false);
    let helm_build = service.helm_build();
    {
        // Retire invalid pending checks even when the host cannot enter the scan below.
        let memory = service.memory.lock().await;
        attempts.retain(|id, attempt| {
            let Some(host) = live.get(id) else {
                return false;
            };
            attempt.retain_for(
                host,
                enabled
                    && eligible(
                        host.kind,
                        &host.state,
                        memory.busy.contains(id) || memory.manual_ssh_handoff_active(*id),
                        &helm_build,
                    ),
            )
        });
    }
    if !enabled {
        return Ok(());
    }
    for host in hosts {
        if service.store.preferences().await?.auto_update_remote_hosts == Some(false) {
            // The preference commit cancelled auto-only checks. The next scan settles
            // any observed failures before it forgets the listener's interest.
            return Ok(());
        }
        let (busy, failed) = {
            let memory = service.memory.lock().await;
            (
                memory.busy.contains(&host.id) || memory.manual_ssh_handoff_active(host.id),
                memory
                    .automatic_runs
                    .get(&host.id)
                    .is_some_and(|run| run.failed_incarnation == Some(host.incarnation)),
            )
        };
        if !eligible(host.kind, &host.state, busy, &helm_build) || failed {
            continue;
        }
        match attempts.get(&host.id) {
            Some(AttemptState::Attempted(incarnation)) if *incarnation == host.incarnation => {
                continue;
            }
            Some(AttemptState::PendingSignIn { .. }) => continue,
            _ => {}
        }
        attempts.insert(host.id, AttemptState::Attempted(host.incarnation));
        let plan = match service
            .plan_update_guarded(host.id, Some(SignInIntent::AutomaticUpdate))
            .await
        {
            Err(error) if error.is::<Pending>() => {
                match begin_sign_in(service, &host, error.downcast::<Pending>()?, &helm_build).await
                {
                    Ok(Some(work)) => {
                        attempts.insert(
                            host.id,
                            AttemptState::PendingSignIn {
                                incarnation: host.incarnation,
                                work,
                            },
                        );
                    }
                    Ok(None) => {
                        attempts.remove(&host.id);
                    }
                    Err(error) => {
                        if let Some(incarnation) = refused(service, &host, &error) {
                            attempts.insert(host.id, AttemptState::Attempted(incarnation));
                        }
                    }
                }
                continue;
            }
            result => result,
        };
        if let Some(incarnation) = admit(service, &host, plan).await {
            attempts.insert(host.id, AttemptState::Attempted(incarnation));
        } else {
            attempts.remove(&host.id);
        }
    }
    Ok(())
}

/// Neither an old plan nor a successful sign-in from an old connection can authorize new work.
async fn still_eligible(
    service: &ProvisioningService,
    host: &HostSnapshot,
    helm_build: &str,
) -> anyhow::Result<bool> {
    if service.store.preferences().await?.auto_update_remote_hosts == Some(false) {
        return Ok(false);
    }
    let memory = service.memory.lock().await;
    Ok(service.manager.status(host.id).is_some_and(|current| {
        current.entry_uuid == host.entry_uuid
            && current.incarnation == host.incarnation
            && eligible(
                host.kind,
                &current.state,
                memory.busy.contains(&host.id) || memory.manual_ssh_handoff_active(host.id),
                helm_build,
            )
    }))
}

/// Once planning yields its slot, order prompt activation against a committing opt-out. The Ready continuation
/// waits for this specific check and dispatches immediately, behind only the service's ordinary planning limits.
async fn begin_sign_in(
    service: &Arc<ProvisioningService>,
    host: &HostSnapshot,
    pending: Pending,
    helm_build: &str,
) -> anyhow::Result<Option<PendingUpdate>> {
    let Some(setup) = &service.setup_sign_in else {
        return Ok(None);
    };
    let _policy = service.automatic_update_admission.lock().await;
    if !still_eligible(service, host, helm_build).await? {
        return Ok(None);
    }
    let sign_in = setup.start_automatic(pending, service);
    let ready = sign_in.wait_ready();
    let service = Arc::downgrade(service);
    let host = host.clone();
    let helm_build = helm_build.to_owned();
    let (finished, attempted) = oneshot::channel();
    let resume = tokio::spawn(async move {
        if !ready.await {
            return;
        }
        let Some(service) = service.upgrade() else {
            return;
        };
        let disposition = match still_eligible(&service, &host, &helm_build).await {
            Ok(true) => {
                let plan = service.plan_update(host.id).await;
                admit(&service, &host, plan).await
            }
            Ok(false) => None,
            Err(error) => refused(&service, &host, &error),
        };
        // A resumed probe belongs to the authentication that started it. A failure observed after
        // retarget or reconnect cannot consume the replacement connection's opportunity.
        let _ = finished.send(disposition == Some(host.incarnation));
        service.manager.events().bump();
    });
    Ok(Some(PendingUpdate {
        sign_in,
        resume,
        attempted,
    }))
}

/// Fresh plans and resumed plans share the final preference, connection and busy checks.
async fn admit(
    service: &Arc<ProvisioningService>,
    host: &HostSnapshot,
    plan: anyhow::Result<HostPlanResponse>,
) -> Option<u64> {
    let result = async {
        service
            .start_automatic_update(
                host.id,
                host.entry_uuid.clone(),
                host.incarnation,
                ProvisionRequest {
                    probe_id: plan?.probe_id,
                },
            )
            .await
    }
    .await;
    match result {
        Ok(Some(_)) => Some(host.incarnation),
        Ok(None) => None,
        Err(error) => refused(service, host, &error),
    }
}

/// A busy host or an obsolete route consumes no attempt. The run's completion or replacement
/// connection wakes the scan. Other refusals belong to the connection observed when the failure became known.
fn refused(
    service: &ProvisioningService,
    host: &HostSnapshot,
    error: &anyhow::Error,
) -> Option<u64> {
    if matches!(
        error.downcast_ref::<ProvisioningRequestError>(),
        Some(ProvisioningRequestError::Busy(_) | ProvisioningRequestError::SshRouteChanged)
    ) {
        None
    } else {
        tracing::warn!(host = host.id, incarnation = host.incarnation, %error,
            "automatic remote-host update refused before start");
        service
            .manager
            .status(host.id)
            .map(|current| current.incarnation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Even if the new connection is already update-eligible when a queued reader returns,
    /// the obsolete route's refusal cannot spend the replacement's opportunity.
    #[farhelm_testtrace::test]
    async fn an_obsolete_route_refusal_leaves_the_replacement_connection_eligible() {
        use crate::provisioning::tests::{FakeBackend, service};
        use crate::rest_harness::{FleetBuilder, HostScript};

        let (builder, host) = FleetBuilder::new()
            .await
            .ssh(
                "toggle.example",
                HostScript {
                    build: "1.0.0".into(),
                    ..Default::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.state._automatic_updates.stop();
        harness.await_refreshed(host).await;
        let root = tempfile::tempdir().unwrap();
        let service = service(
            &harness,
            FakeBackend::absent(root.path().into()),
            root.path(),
        );
        let previous = harness
            .manager
            .snapshots()
            .into_iter()
            .find(|snapshot| snapshot.id == host)
            .unwrap();
        assert!(connection_needs_update(&previous.state, "2.0.0"));
        let row = service.host_row(host).await.unwrap();
        let writer = harness.manager.ssh_route_lock(host).write_owned().await;
        let queued = service.lock_ssh_plan(Some(&row));
        tokio::pin!(queued);
        assert!(futures_util::poll!(&mut queued).is_pending());

        assert!(harness.store.set_ssh_tailscale(&row, true).await.unwrap());
        harness.manager.sync_registry().await.unwrap();
        harness
            .await_refreshed_after(host, previous.incarnation)
            .await;
        assert!(connection_needs_update(
            &harness.manager.status(host).unwrap().state,
            "2.0.0"
        ));
        drop(writer);

        let error = queued
            .await
            .expect_err("the queued operation must recheck its stale route");
        assert!(matches!(
            error.downcast_ref::<ProvisioningRequestError>(),
            Some(ProvisioningRequestError::SshRouteChanged)
        ));
        assert_eq!(refused(&service, &previous, &error), None);
    }

    /// Automatic targets must agree with the update badge, including release
    /// ordering, protocol urgency, busy exclusion and the reserved local row.
    /// An older protocol still exempts a host running a development build,
    /// since Automatic Update would otherwise undo the devbuild under test.
    #[farhelm_testtrace::test]
    fn eligibility_uses_age_and_protocol_without_downgrading() {
        let connected = |build: &str| HostState::Connected {
            identity: None,
            build_version: build.into(),
            last_refresh: crate::manager::RefreshHealth::Pending,
        };
        assert!(eligible(HostKind::Ssh, &connected("1.0.0"), false, "2.0.0"));
        assert!(!eligible(
            HostKind::Local,
            &connected("1.0.0"),
            false,
            "2.0.0"
        ));
        assert!(!eligible(HostKind::Ssh, &connected("1.0.0"), true, "2.0.0"));
        for peer in ["2.0.0", "3.0.0", "unknown", "0.0.0-unreleased"] {
            assert!(!eligible(HostKind::Ssh, &connected(peer), false, "2.0.0"));
        }
        assert!(!eligible(
            HostKind::Ssh,
            &connected("1.0.0"),
            false,
            "0.0.0-unreleased"
        ));
        let skew = |protocol, build: &str| HostState::VersionSkew {
            peer_protocol: protocol,
            our_protocol: 2,
            peer_build: build.into(),
            our_build: "2.0.0".into(),
            remediation: "update".into(),
        };
        assert!(eligible(HostKind::Ssh, &skew(1, "1.0.0"), false, "2.0.0"));
        assert!(!eligible(
            HostKind::Ssh,
            &skew(1, "0.0.0-unreleased"),
            false,
            "2.0.0"
        ));
        assert!(eligible(
            HostKind::Ssh,
            &skew(1, "unknown"),
            false,
            "0.0.0-unreleased"
        ));
        assert!(!eligible(HostKind::Ssh, &skew(1, "3.0.0"), false, "2.0.0"));
        for protocol in [2, 3] {
            assert!(!eligible(
                HostKind::Ssh,
                &skew(protocol, "1.0.0"),
                false,
                "2.0.0"
            ));
        }
    }
}
