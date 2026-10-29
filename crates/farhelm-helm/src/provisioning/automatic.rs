//! Automatic updates reuse manual planning and execution, including every refusal.
//!
//! The fleet revision is the wake source: connection transitions, provisioning
//! completion and preference writes all publish there. There is no retry timer.

use super::{ProvisionRequest, ProvisioningService};
use crate::manager::HostState;
use crate::store::{HostId, HostKind};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

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
        let mut events = service.manager.events().subscribe();
        let service = Arc::downgrade(service);
        #[cfg(test)]
        let (scan_notices, completed_scans) = tokio::sync::watch::channel(0_u64);
        let task = tokio::spawn(async move {
            let mut attempted = HashMap::new();
            loop {
                events.borrow_and_update();
                let Some(service) = service.upgrade() else {
                    return;
                };
                if let Err(error) = scan(&service, &mut attempted).await {
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

/// Match the rail's update badge, rather than Update all's broader idle-host list.
/// Development or unparseable builds have unknown age; only a lower protocol
/// then proves an update is needed. Newer peers are never automatic targets.
fn eligible(kind: HostKind, state: &HostState, busy: bool, helm_build: &str) -> bool {
    if !kind.panel_updates() || busy {
        return false;
    }
    match state {
        HostState::Connected { build_version, .. } => {
            crate::hosts::build_is_older(build_version, helm_build)
        }
        HostState::VersionSkew {
            peer_protocol,
            our_protocol,
            peer_build,
            ..
        } => peer_protocol < our_protocol && !crate::hosts::build_is_newer(peer_build, helm_build),
        _ => false,
    }
}

/// Start at most one attempt per host incarnation for this helm lifetime.
/// Reserve before planning, not after run completion: a failed run clears busy
/// and publishes its own revision before the start caller necessarily resumes.
/// Manual starts still arbitrate through the provisioning service's busy claim.
pub(super) async fn scan(
    service: &Arc<ProvisioningService>,
    attempted: &mut HashMap<HostId, u64>,
) -> anyhow::Result<()> {
    let hosts = service.manager.snapshots();
    let live: HashSet<_> = hosts.iter().map(|host| host.id).collect();
    attempted.retain(|id, _| live.contains(id));
    if service.store.preferences().await?.auto_update_remote_hosts != Some(true) {
        return Ok(());
    }
    for host in hosts {
        if service.store.preferences().await?.auto_update_remote_hosts != Some(true) {
            return Ok(());
        }
        let (busy, failed) = {
            let memory = service.memory.lock().await;
            (
                memory.busy.contains(&host.id),
                memory
                    .automatic_runs
                    .get(&host.id)
                    .is_some_and(|run| run.failed_incarnation == Some(host.incarnation)),
            )
        };
        if !eligible(host.kind, &host.state, busy, &service.helm_build())
            || attempted.get(&host.id) == Some(&host.incarnation)
            || failed
        {
            continue;
        }
        attempted.insert(host.id, host.incarnation);
        let result = async {
            let plan = service.plan_update(host.id).await?;
            // The final policy read and connection check share admission's
            // ordering with a committing opt-out; planning cannot authorize a
            // later start by retaining an earlier true read.
            if service
                .start_automatic_update(
                    host.id,
                    host.entry_uuid.clone(),
                    host.incarnation,
                    ProvisionRequest {
                        probe_id: plan.probe_id,
                    },
                )
                .await?
                .is_none()
            {
                attempted.remove(&host.id);
            }
            Ok::<(), anyhow::Error>(())
        }
        .await;
        if let Err(error) = result {
            // A refused probe can itself outlast a connection transition. The
            // no-retry boundary starts when refusal is known, not before SSH.
            if let Some(current) = service.manager.status(host.id) {
                attempted.insert(host.id, current.incarnation);
            }
            tracing::warn!(host = host.id, incarnation = host.incarnation, error = %error,
                "automatic remote-host update refused before start");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Automatic targets must agree with the update badge, including release
    /// ordering, protocol urgency, busy exclusion and the reserved local row.
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
        for peer in ["2.0.0", "3.0.0", "unknown"] {
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
