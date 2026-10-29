//! Update discovery shares the rail's eligibility, not the broader Update all offer.
//!
//! Prompts retain the host snapshots they name. Activation may drop a stale or
//! newly busy target, but cannot acquire a host that appeared after rendering.
//! Dismissal belongs to the mounted list only; no preference or helm state is written.

use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;

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
    /// Required updates count as new too, even while their banner hides the card.
    pub(crate) fn invalidated_by(&self, updates: &HostUpdates, helm_build: &str) -> bool {
        self.helm_build != helm_build
            || updates
                .optional
                .iter()
                .chain(&updates.required)
                .any(|host| !self.hosts.contains(&host.id))
    }
}

/// Whether optional updates deserve a card in the currently visible session list.
/// Required updates take precedence and never consult the dismissal choice.
pub(crate) fn card_visible(
    updates: &HostUpdates,
    dismissal: Option<&UpdateDismissal>,
    helm_build: &str,
    hosts_open: bool,
) -> bool {
    !hosts_open
        && updates.required.is_empty()
        && !updates.optional.is_empty()
        && dismissal.is_none_or(|dismissal| dismissal.invalidated_by(updates, helm_build))
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

/// Name the scope of a required update without implying that optional hosts join it.
fn required_action(count: usize) -> String {
    let number = match count {
        1 => return "↑ Update now".into(),
        2 => "two",
        3 => "three",
        4 => "four",
        5 => "five",
        6 => "six",
        7 => "seven",
        8 => "eight",
        9 => "nine",
        _ => return format!("↑ Update all {count}"),
    };
    format!("↑ Update all {number}")
}

/// Render the nondismissible prerequisite to using protocol-incompatible hosts.
/// The single peer name stays escaped and direction-isolated like host-row names.
#[component]
pub(crate) fn HostUpdateBanner(
    hosts: Vec<Host>,
    on_update: EventHandler<Vec<Host>>,
    on_show_hosts: EventHandler<()>,
) -> Element {
    let count = hosts.len();
    let action = required_action(count);
    rsx! {
        section { class: "host-update-banner", "data-update-kind": "required",
            aria_label: "Host updates required",
            p { class: "host-update-heading",
                if count == 1 {
                    span { class: "peer-value", "{hosts[0].name.escaped()}" }
                    " must be updated before you can use it"
                } else { "{count} hosts must be updated before you can use them" }
            }
            p { if count == 1 {
                "Its supervisor speaks an older protocol than this helm. Its sessions show as unreachable until it is updated."
            } else {
                "Their supervisors speak an older protocol than this helm. Their sessions show as unreachable until they are updated."
            } }
            div { class: "host-update-actions",
                button { r#type: "button", class: "btn btn-neutral",
                    "data-tooltip": "Update required hosts: install this helm's version on exactly these hosts",
                    onclick: move |_| on_update.call(hosts.clone()), "{action}"
                }
                button { r#type: "button", class: "btn btn-neutral",
                    "data-tooltip": "Show hosts: open host management",
                    onclick: move |_| on_show_hosts.call(()), "Show hosts"
                }
            }
        }
    }
}

/// Keep optional updates discoverable next to the Hosts toggle without blocking work.
/// Dismissal is owned by ListView so replacing this card with a required banner
/// cannot accidentally forget the person's choice.
#[component]
pub(crate) fn HostUpdateCard(
    hosts: Vec<Host>,
    on_update: EventHandler<Vec<Host>>,
    on_show_hosts: EventHandler<()>,
    on_dismiss: EventHandler<()>,
) -> Element {
    let count = hosts.len();
    rsx! {
        section { class: "host-update-card", "data-update-kind": "optional",
            aria_label: "Host updates available",
            button { r#type: "button", class: "btn btn-neutral host-update-dismiss",
                aria_label: "Dismiss host update reminder",
                "data-tooltip": "Dismiss: hide until another host has an update or the helm build changes",
                onclick: move |_| on_dismiss.call(()), "✕"
            }
            p { class: "host-update-heading",
                if count == 1 { "1 host has an update" } else { "{count} hosts have updates" }
            }
            div { class: "host-update-body", role: "region",
                aria_label: "Hosts with available updates", tabindex: "0",
                ul { class: "host-update-names",
                    for host in &hosts {
                        li { key: "{host.id}",
                            span { class: "peer-value", "{host.name.escaped()}" }
                        }
                    }
                }
            }
            div { class: "host-update-actions",
                button { r#type: "button", class: "btn btn-neutral",
                    "data-tooltip": "Update all now: install this helm's version on the listed hosts",
                    onclick: move |_| on_update.call(hosts.clone()), "Update all now"
                }
                button { r#type: "button", class: "btn btn-neutral",
                    "data-tooltip": "Show hosts: open host management",
                    onclick: move |_| on_show_hosts.call(()), "Show hosts"
                }
            }
        }
    }
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
            yolo_without_asking: false,
            commands_without_asking: false,
            links_without_asking: false,
            local_links_without_asking: false,
            icon: Default::default(),
            color: Default::default(),
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

    /// Losing a known update temporarily must not resurrect a dismissed card.
    /// New identities and helm builds invalidate it; required updates still take precedence.
    #[farhelm_testtrace::test]
    fn dismissal_tracks_membership_and_build_without_hiding_required_updates() {
        let updates = HostUpdates {
            optional: vec![optional(1), optional(2)],
            ..Default::default()
        };
        let dismissal = UpdateDismissal::new(&updates, "build-a".into());
        assert!(card_visible(&updates, None, "build-a", false));
        assert!(!card_visible(&updates, Some(&dismissal), "build-a", false));
        let subset = HostUpdates {
            optional: vec![optional(1)],
            ..Default::default()
        };
        assert!(!dismissal.invalidated_by(&subset, "build-a"));
        assert!(!card_visible(&subset, Some(&dismissal), "build-a", false));
        assert!(card_visible(&updates, Some(&dismissal), "build-b", false));
        let added = HostUpdates {
            optional: vec![optional(3)],
            ..Default::default()
        };
        assert!(card_visible(&added, Some(&dismissal), "build-a", false));
        assert!(!card_visible(&updates, None, "build-a", true));
        let mixed = HostUpdates {
            optional: updates.optional.clone(),
            required: vec![required(3)],
        };
        assert!(dismissal.invalidated_by(&mixed, "build-a"));
        assert!(!card_visible(&mixed, None, "build-a", false));
        assert!(!card_visible(
            &HostUpdates::default(),
            None,
            "build-a",
            false
        ));
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

    /// Small counts read naturally; larger fleets retain an unambiguous exact number.
    #[farhelm_testtrace::test]
    fn required_button_names_its_scope() {
        assert_eq!(required_action(1), "↑ Update now");
        assert_eq!(required_action(3), "↑ Update all three");
        assert_eq!(required_action(9), "↑ Update all nine");
        assert_eq!(required_action(10), "↑ Update all 10");
    }
}
