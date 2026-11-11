//! Update discovery shares the rail's eligibility, not the broader Update all offer.
//!
//! Prompts retain the host snapshots they name. Activation may drop a stale or
//! newly busy target, but cannot acquire a host that appeared after rendering.
//! Dismissal belongs to the mounted list only; no preference or helm state is written.

use std::collections::{HashMap, HashSet};

use super::{host_update_kind, remote_update_available};
use crate::api::ProvisioningOperation;
use crate::provisioning::{ActionRequest, HostBinding, ProvisioningMenuState};
use crate::{Host, HostId};

/// The exact hosts whose rail avatars carry an update arrow, split by urgency.
/// Input order is retained for names and action targets; no sorting or age
/// inference is done here, including for a dev helm whose age is unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HostUpdates {
    pub(crate) optional: Vec<Host>,
    pub(crate) required: Vec<Host>,
}

impl HostUpdates {
    /// Count each classified row once for the Hosts toggle's accessible name.
    pub(crate) fn len(&self) -> usize {
        self.optional.len() + self.required.len()
    }

    /// A rail avatar uses the same membership as every fleet-wide prompt.
    pub(super) fn contains(&self, id: HostId) -> bool {
        self.optional
            .iter()
            .chain(&self.required)
            .any(|host| host.id == id)
    }

    /// Keep the familiar label when no update waits, and name the count otherwise.
    pub(crate) fn toggle_label(&self) -> String {
        match self.len() {
            0 => "Hosts".into(),
            1 => "Hosts: 1 host has an update".into(),
            count => format!("Hosts: {count} hosts have updates"),
        }
    }
}

/// Classify only idle remote rows that both offer Update and need the helm's build.
/// Missing menu state, a live run, a local host, and a too-new peer all withdraw
/// discovery even when old connection facts still describe an update.
pub(crate) fn classify(
    hosts: &[Host],
    menus: &HashMap<HostId, ProvisioningMenuState>,
    busy: &HashSet<HostId>,
) -> HostUpdates {
    let mut updates = HostUpdates::default();
    for host in hosts {
        if !remote_update_available(host, menus.get(&host.id), busy.contains(&host.id)) {
            continue;
        }
        match host_update_kind(&host.state) {
            Some("optional") => updates.optional.push(host.clone()),
            Some("required") => updates.required.push(host.clone()),
            _ => {}
        }
    }
    updates
}

/// Capture the identity set dismissed in this interface, for one helm build.
/// Temporary disappearance during a run does not make a known host new again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UpdateDismissal {
    hosts: HashSet<HostId>,
    helm_build: String,
}

impl UpdateDismissal {
    /// Remember all update identities shown at dismissal, without persisting them.
    pub(crate) fn new(updates: &HostUpdates, helm_build: String) -> Self {
        Self {
            hosts: updates
                .optional
                .iter()
                .chain(&updates.required)
                .map(|host| host.id)
                .collect(),
            helm_build,
        }
    }

    /// A new host or changed helm build permanently retires this dismissal.
    /// Required updates count as new too; dismissal only conceals optional rows.
    pub(crate) fn invalidated_by(&self, updates: &HostUpdates, helm_build: &str) -> bool {
        self.helm_build != helm_build
            || updates
                .optional
                .iter()
                .chain(&updates.required)
                .any(|host| !self.hosts.contains(&host.id))
    }
}

/// Queue only still-eligible members of the rendered prompt, with its original binding.
/// A retargeted, reconnected or newly busy host is dropped; a newly eligible
/// host is never added. Mounted provisioning panels and the helm revalidate again.
pub(crate) fn requests_for_shown(
    shown: &[Host],
    current: &[Host],
    menus: &HashMap<HostId, ProvisioningMenuState>,
    busy: &HashSet<HostId>,
) -> HashMap<HostId, ActionRequest> {
    let eligible = classify(current, menus, busy);
    shown
        .iter()
        .filter_map(|host| {
            let binding = HostBinding::from(host);
            let live = current.iter().find(|live| live.id == host.id)?;
            (eligible.contains(host.id) && binding == HostBinding::from(live)).then_some((
                host.id,
                ActionRequest {
                    operation: ProvisioningOperation::Update,
                    binding,
                    return_to_rail: false,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HostKind, HostPhase, RefreshHealth};

    /// Give classification cases a real remote binding without provisioning or IO.
    fn host(id: HostId, state: HostPhase) -> Host {
        Host {
            id,
            kind: HostKind::Ssh,
            destination: Some("worker".into()),
            alias: None,
            name: format!("worker-{id}").into(),
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            state,
            incarnation: 1,
            trash_epoch: 0,
            yolo_without_asking: false,
            commands_without_asking: false,
            own_sessions_without_asking: false,
            is_session_owned: false,
            links_without_asking: false,
            local_links_without_asking: false,
            icon: Default::default(),
            color: Default::default(),
            tensorlake: None,

            environment: None,
        }
    }

    /// Optional age is the helm's explicit fact, not a UI version-string comparison.
    fn optional(id: HostId) -> Host {
        host(
            id,
            HostPhase::Connected {
                identity: None,
                build_version: "0.1.0".into(),
                old_version: true,
                newer_version: false,
                refresh: RefreshHealth::Pending,
            },
        )
    }

    /// Required updates are established by protocol direction, even without a connection.
    fn required(id: HostId) -> Host {
        host(
            id,
            HostPhase::VersionSkew {
                peer_protocol: 1,
                our_protocol: 2,
                peer_build: "0.1.0".into(),
                our_build: "0.2.0".into(),
                remediation: "update".into(),
            },
        )
    }

    /// Offers deliberately include rows that Update all accepts but discovery must omit.
    fn menus(hosts: &[Host]) -> HashMap<HostId, ProvisioningMenuState> {
        hosts
            .iter()
            .map(|host| {
                (
                    host.id,
                    ProvisioningMenuState {
                        update: true,
                        ..Default::default()
                    },
                )
            })
            .collect()
    }

    /// Crossing age, offers, busy ownership and host kind keeps every arrow honest.
    #[farhelm_testtrace::test]
    fn classifier_excludes_non_updates_and_unavailable_rows() {
        let mut current = optional(3);
        if let HostPhase::Connected { old_version, .. } = &mut current.state {
            *old_version = false;
        }
        let mut newer = optional(4);
        if let HostPhase::Connected { newer_version, .. } = &mut newer.state {
            *newer_version = true;
        }
        let mut local = optional(5);
        local.kind = HostKind::Local;
        let unreachable = host(
            6,
            HostPhase::Unreachable {
                cause: "transport".into(),
                last_error: "offline".into(),
            },
        );
        let connecting = host(
            9,
            HostPhase::Connecting {
                attempt: 1,
                stage: None,
                last_error: None,
            },
        );
        let mut newer_protocol = required(11);
        if let HostPhase::VersionSkew { peer_protocol, .. } = &mut newer_protocol.state {
            *peer_protocol = 3;
        }
        let hosts = vec![
            optional(1),
            required(2),
            current,
            newer,
            local,
            unreachable,
            optional(7),
            optional(8),
            connecting,
            optional(10),
            newer_protocol,
        ];
        let mut offers = menus(&hosts);
        offers.remove(&8);
        offers.get_mut(&10).unwrap().update = false;
        let updates = classify(&hosts, &offers, &HashSet::from([7]));
        assert_eq!(
            updates
                .optional
                .iter()
                .map(|host| host.id)
                .collect::<Vec<_>>(),
            [1]
        );
        assert_eq!(
            updates
                .required
                .iter()
                .map(|host| host.id)
                .collect::<Vec<_>>(),
            [2]
        );
        assert_eq!(updates.toggle_label(), "Hosts: 2 hosts have updates");
        assert_eq!(HostUpdates::default().toggle_label(), "Hosts");
        assert_eq!(
            classify(&hosts[..1], &offers, &HashSet::new()).toggle_label(),
            "Hosts: 1 host has an update"
        );
    }

    /// A temporarily absent update does not revive a dismissed reminder.
    /// New identities or builds retire the dismissal, including a required host.
    #[farhelm_testtrace::test]
    fn dismissal_tracks_membership_and_build() {
        let updates = HostUpdates {
            optional: vec![optional(1), optional(2)],
            ..Default::default()
        };
        let dismissal = UpdateDismissal::new(&updates, "build-a".into());
        assert!(!dismissal.invalidated_by(&updates, "build-a"));
        let subset = HostUpdates {
            optional: vec![optional(1)],
            ..Default::default()
        };
        assert!(!dismissal.invalidated_by(&subset, "build-a"));
        assert!(dismissal.invalidated_by(&updates, "build-b"));
        let added = HostUpdates {
            optional: vec![optional(3)],
            ..Default::default()
        };
        assert!(dismissal.invalidated_by(&added, "build-a"));
        let mixed = HostUpdates {
            optional: updates.optional.clone(),
            required: vec![required(3)],
        };
        assert!(dismissal.invalidated_by(&mixed, "build-a"));
        assert!(!dismissal.invalidated_by(&HostUpdates::default(), "build-a"));
    }

    /// A stale prompt can shrink safely, but never pick up new or retargeted hosts.
    #[farhelm_testtrace::test]
    fn prompt_requests_bind_exact_rendered_hosts() {
        let shown = vec![
            optional(1),
            optional(2),
            required(3),
            optional(4),
            required(6),
        ];
        let mut current = shown.clone();
        current[1].destination = Some("different-worker".into());
        current[2].incarnation += 1;
        current.push(optional(5));
        let requests = requests_for_shown(&shown, &current, &menus(&current), &HashSet::from([4]));
        assert_eq!(
            requests.keys().copied().collect::<HashSet<_>>(),
            HashSet::from([1, 6])
        );
        assert_eq!(requests[&1].binding, HostBinding::from(&shown[0]));
        assert_eq!(requests[&1].operation, ProvisioningOperation::Update);
        assert!(!requests[&1].return_to_rail);
        assert_eq!(requests[&6].binding, HostBinding::from(&shown[4]));
    }
}
