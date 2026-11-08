//! Hosts that need something stay visible without opening host management.
//!
//! Classification consumes the same observations as the rail. It grants no new
//! authority: Update callbacks retain their rendered host bindings, and Details
//! merely opens the permanently mounted management row. Progress changes paint
//! inside a fixed footprint; only changes to visible copy or membership move
//! the session anchors below this list.

use std::collections::HashMap;

use dioxus::prelude::*;

use super::attention::HostAttention;
use super::updates::{HostUpdates, UpdateDismissal};
use super::{host_update_title, phase_display_label, rail_identity};
use crate::api::ProvisioningOperation;
use crate::icons::HostMark;
use crate::provisioning::HostUpdateProgress;
use crate::{Host, HostId, HostPhase};

/// A host has one reason to appear, even when several observations need attention.
/// Classification gives Working precedence so a temporarily unreachable update
/// remains one working row, independently of the groups' display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttentionKind {
    Problem,
    Working,
    Update { required: bool },
}

impl AttentionKind {
    /// Stable browser handles distinguish the three visual groups without text matching.
    fn attribute(self) -> &'static str {
        match self {
            Self::Problem => "problem",
            Self::Working => "working",
            Self::Update { .. } => "update",
        }
    }
}

/// One rendered host snapshot, not a lookup key to resolve again at activation.
/// Counts repaint the progress bar but never change its layout footprint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AttentionRow {
    pub(crate) host: Host,
    pub(crate) kind: AttentionKind,
    pub(crate) words: String,
    pub(crate) progress: Option<(usize, usize)>,
}

/// Select one row per host, then group Problems, Working and Updates in rail order.
/// An optional dismissal never conceals working or problem rows, even when that
/// host also has an update. Eligibility itself remains `updates::classify`'s job.
pub(crate) fn classify(
    hosts: &[Host],
    updates: &HostUpdates,
    progress: &HashMap<HostId, HostUpdateProgress>,
    attention: &HashMap<HostId, HostAttention>,
    dismissal: Option<&UpdateDismissal>,
    helm_build: &str,
) -> Vec<AttentionRow> {
    let offers: HashMap<_, _> = updates
        .optional
        .iter()
        .map(|host| (host.id, false))
        .chain(updates.required.iter().map(|host| (host.id, true)))
        .collect();
    let optional_dismissed =
        dismissal.is_some_and(|dismissal| !dismissal.invalidated_by(updates, helm_build));
    let mut rows = Vec::new();
    for host in hosts {
        let update = offers.get(&host.id).copied();
        let (kind, words, counts) = if let Some(progress) = progress.get(&host.id) {
            let (verb, counts) = match progress {
                HostUpdateProgress::Pending(_) => ("Updating", None),
                HostUpdateProgress::PendingConfirmed(operation) => (
                    match operation {
                        ProvisioningOperation::Add => "Setting up",
                        ProvisioningOperation::Update => "Updating",
                        ProvisioningOperation::Uninstall => "Uninstalling",
                    },
                    None,
                ),
                HostUpdateProgress::Running(summary) => {
                    (summary.verb(), Some((summary.done, summary.total)))
                }
            };
            (AttentionKind::Working, verb.into(), counts)
        } else if let Some(words) = problem_words(
            host,
            update,
            attention.get(&host.id).is_some_and(HostAttention::unseen),
        ) {
            (AttentionKind::Problem, words, None)
        } else if let Some(required) = update {
            if !required && optional_dismissed {
                continue;
            }
            (
                AttentionKind::Update { required },
                if required {
                    "Update required"
                } else {
                    "Update available"
                }
                .into(),
                None,
            )
        } else {
            continue;
        };
        rows.push(AttentionRow {
            host: host.clone(),
            kind,
            words,
            progress: counts,
        });
    }
    // Stable grouping retains registry order, including optional and required
    // updates mixed in their common group rather than sorting by urgency again.
    rows.sort_by_key(|row| match row.kind {
        AttentionKind::Problem => 0,
        AttentionKind::Working => 1,
        AttentionKind::Update { .. } => 2,
    });
    rows
}

/// Keep every refusal phase explicit; a newer usable connection alone is healthy.
/// Secondary warning causes remain visible beside a phase problem instead of
/// disappearing behind it. No remote diagnostic is copied into this compact line.
fn problem_words(host: &Host, update: Option<bool>, unseen: bool) -> Option<String> {
    let phase = match &host.state {
        HostPhase::Unreachable { .. }
        | HostPhase::IdentityMismatch { .. }
        | HostPhase::IdentityUnverified { .. }
        | HostPhase::Duplicate { .. }
        | HostPhase::Retired { .. }
        | HostPhase::Unrecognized => Some(phase_display_label(&host.state)),
        HostPhase::VersionSkew {
            peer_protocol,
            our_protocol,
            ..
        } => {
            if peer_protocol > our_protocol {
                Some("Too new; update the helm")
            } else if update != Some(true) {
                Some(phase_display_label(&host.state))
            } else {
                None
            }
        }
        HostPhase::Suspended { resume_failed, .. } => {
            resume_failed.as_ref().map(|_| "Resume failed")
        }
        // An explicit lifecycle request is routine work. Unseen setup warnings
        // still qualify below; live tunnel failures retain their connected-host gate.
        HostPhase::TensorlakeChanging { .. }
        | HostPhase::Connecting { .. }
        | HostPhase::Connected { .. } => None,
    };
    let mut words: Vec<_> = phase.into_iter().collect();
    if super::credential_delivery_failed(host) {
        words.push("Credentials not delivered");
    }
    if crate::tensorlake::tunnels::failed(host) {
        words.push("Tunnels failed");
    }
    if unseen {
        words.push("Setup or update needs attention");
    }
    (!words.is_empty()).then(|| words.join("; "))
}

/// Describe only geometry-bearing copy, not every observation used to classify it.
/// Run ids, step names, elapsed time and counts must not close a session menu on
/// each poll. The stable operation verb stays here because it is actual row copy.
pub(crate) fn layout_key(rows: &[AttentionRow]) -> Vec<(HostId, AttentionKind, String, String)> {
    rows.iter()
        .map(|row| {
            (
                row.host.id,
                row.kind,
                row.host.short_name().escaped(),
                row.words.clone(),
            )
        })
        .collect()
}

/// Render only the snapshots the classifier admitted, with separate open/action controls.
/// The independently scrolling body leaves Update all and optional dismissal
/// reachable. Event propagation cannot turn an Update click into a Details click.
#[component]
pub(crate) fn HostAttentionList(
    rows: Vec<AttentionRow>,
    on_update: EventHandler<Vec<Host>>,
    on_show_host: EventHandler<HostId>,
    on_dismiss: EventHandler<()>,
) -> Element {
    if rows.is_empty() {
        return rsx! {};
    }
    let updates: Vec<_> = rows
        .iter()
        .filter(|row| matches!(row.kind, AttentionKind::Update { .. }))
        .map(|row| row.host.clone())
        .collect();
    let optional = rows
        .iter()
        .any(|row| row.kind == (AttentionKind::Update { required: false }));
    let count = rows.len();
    let heading = if count == 1 {
        "1 host needs attention".into()
    } else {
        format!("{count} hosts need attention")
    };
    rsx! {
        section { class: "host-attention-list", aria_label: "Hosts needing attention",
            div { class: "host-attention-header",
                span { class: "host-attention-heading", "{heading}" }
                if updates.len() > 1 {
                    button { r#type: "button", class: "btn btn-neutral",
                        "data-tooltip": "Update all: install this helm's version on the listed update hosts",
                        onclick: move |_| on_update.call(updates.clone()), "Update all"
                    }
                }
                if optional {
                    button { r#type: "button", class: "btn btn-neutral host-attention-dismiss",
                        aria_label: "Dismiss optional host updates",
                        "data-tooltip": "Dismiss optional updates until another host has an update or the helm build changes",
                        onclick: move |_| on_dismiss.call(()), "✕"
                    }
                }
            }
            div { class: "host-attention-body", role: "region",
                aria_label: "Hosts needing attention", tabindex: "0",
                for row in rows {
                    AttentionHostRow { key: "{row.host.id}", row, on_update, on_show_host }
                }
            }
        }
    }
}

/// The identity opens management; the adjacent action keeps its own exact target.
/// Required-update prose may wrap, while a changing progress value has fixed height.
#[component]
fn AttentionHostRow(
    row: AttentionRow,
    on_update: EventHandler<Vec<Host>>,
    on_show_host: EventHandler<HostId>,
) -> Element {
    let id = row.host.id;
    let identity = rail_identity(
        row.host.name.raw(),
        row.host
            .destination
            .as_ref()
            .map(|destination| destination.raw()),
        row.host.kind.is_this_machine(),
    );
    let tooltip = format!("{identity}: {}; open Hosts for details", row.words);
    let update_tooltip = host_update_title(&row.host.state)
        .unwrap_or_else(|| "Update: install this helm's version on this host".into());
    let update_kind = match row.kind {
        AttentionKind::Update { required: true } => Some("required"),
        AttentionKind::Update { required: false } => Some("optional"),
        AttentionKind::Problem | AttentionKind::Working => None,
    };
    rsx! {
        div { class: "host-attention-row", "data-attention-host-id": "{id}",
            "data-attention-kind": row.kind.attribute(), "data-update-kind": update_kind,
            onclick: move |_| on_show_host.call(id),
            button { r#type: "button", class: "btn host-attention-open",
                aria_label: tooltip.clone(), "data-tooltip": tooltip.clone(),
                onclick: move |event| { event.stop_propagation(); on_show_host.call(id); },
                HostMark {
                    icon: row.host.icon, color: row.host.color,
                    local: row.host.kind.is_this_machine(), tooltip: tooltip.clone(),
                }
                span { class: "host-attention-copy",
                    span { class: "host-attention-name peer-value", dir: "ltr",
                        "{row.host.short_name().escaped()}"
                    }
                    span { class: "host-attention-state", "{row.words}" }
                    if row.kind == (AttentionKind::Update { required: true }) {
                        span { class: "host-attention-required",
                            "Sessions stay unreachable until this host is updated."
                        }
                    }
                }
            }
            match row.kind {
                AttentionKind::Problem => rsx! {
                    button { r#type: "button", class: "btn btn-neutral host-attention-action",
                        "data-tooltip": "Details: open host management at this host",
                        onclick: move |event| { event.stop_propagation(); on_show_host.call(id); }, "Details"
                    }
                },
                AttentionKind::Update { .. } => rsx! {
                    button { r#type: "button", class: "btn btn-neutral host-attention-action",
                        "data-tooltip": update_tooltip,
                        onclick: move |event| {
                            event.stop_propagation();
                            on_update.call(vec![row.host.clone()]);
                        }, "Update"
                    }
                },
                AttentionKind::Working => rsx! {
                    if let Some((done, total)) = row.progress.filter(|(_, total)| *total > 0) {
                        progress { class: "host-attention-progress", max: "{total}", value: "{done.min(total)}",
                            aria_valuemin: "0", aria_valuenow: "{done.min(total)}", aria_valuemax: "{total}",
                            aria_label: "Host operation progress", "data-tooltip": "{done} of {total} steps finished"
                        }
                    } else {
                        progress { class: "host-attention-progress", aria_label: "Host operation in progress" }
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ProvisioningOperation;
    use crate::provisioning::{UpdatePendingPhase, UpdateProgressSummary};
    use crate::{HostKind, RefreshHealth, TensorlakeHost};
    use farhelm_proto::tensorlake::{TunnelState, TunnelView};
    use std::time::Instant;

    /// A remote host carries a real routing identity while every observation stays local.
    fn host(id: HostId, state: HostPhase) -> Host {
        Host {
            id,
            kind: HostKind::Ssh,
            destination: Some(format!("worker-{id}").into()),
            alias: None,
            name: format!("worker-{id}").into(),
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            state,
            incarnation: 1,
            yolo_without_asking: false,
            commands_without_asking: false,
            own_sessions_without_asking: false,
            links_without_asking: false,
            local_links_without_asking: false,
            icon: Default::default(),
            color: Default::default(),
            tensorlake: None,
        }
    }

    /// Connected age alone does not invent an available Update action.
    fn connected() -> HostPhase {
        HostPhase::Connected {
            identity: None,
            build_version: "old".into(),
            old_version: false,
            newer_version: false,
            refresh: RefreshHealth::Pending,
        }
    }

    /// Protocol direction and an explicit Update offer together distinguish a required update.
    fn skew(peer: u32) -> HostPhase {
        HostPhase::VersionSkew {
            peer_protocol: peer,
            our_protocol: 2,
            peer_build: "peer".into(),
            our_build: "helm".into(),
            remediation: "update".into(),
        }
    }

    /// Readable ids and classes make precedence/grouping failures show the wrong row directly.
    fn identities(rows: &[AttentionRow]) -> Vec<(HostId, AttentionKind)> {
        rows.iter().map(|row| (row.host.id, row.kind)).collect()
    }

    /// All phases deliberately choose whether they need attention; usable newer
    /// connections, ordinary Connecting and suspended sandboxes remain absent.
    #[test]
    fn phases_do_not_turn_every_unavailable_host_into_a_problem() {
        let phases = vec![
            HostPhase::Connecting {
                attempt: 1,
                stage: None,
                last_error: None,
            },
            HostPhase::Unreachable {
                cause: "transport".into(),
                last_error: "offline".into(),
            },
            connected(),
            skew(3),
            HostPhase::IdentityMismatch {
                recorded: "old".into(),
                reported: "new".into(),
            },
            HostPhase::IdentityUnverified {
                recorded: "old".into(),
            },
            HostPhase::Duplicate {
                twin: 99,
                identity: "same".into(),
                twin_name: None,
            },
            HostPhase::Suspended {
                observed_at: 1,
                suspended_by_farhelm_at: None,
                resume_failed: None,
            },
            HostPhase::Retired {
                reason: "stopped".into(),
            },
            HostPhase::Unrecognized,
        ];
        let mut hosts: Vec<_> = phases
            .into_iter()
            .enumerate()
            .map(|(index, state)| host(index as HostId, state))
            .collect();
        let mut newer = connected();
        if let HostPhase::Connected { newer_version, .. } = &mut newer {
            *newer_version = true;
        }
        hosts.push(host(10, newer));
        hosts.push(host(11, skew(1)));
        hosts.push(host(
            12,
            HostPhase::Suspended {
                observed_at: 1,
                suspended_by_farhelm_at: Some(1),
                resume_failed: Some("refused".into()),
            },
        ));
        let rows = classify(
            &hosts,
            &HostUpdates::default(),
            &HashMap::new(),
            &HashMap::new(),
            None,
            "a",
        );
        assert_eq!(
            rows.iter().map(|row| row.host.id).collect::<Vec<_>>(),
            [1, 3, 4, 5, 6, 8, 9, 11, 12]
        );
        assert_eq!(rows[0].words, "Unreachable, retrying");
        assert_eq!(rows[1].words, "Too new; update the helm");
        assert_eq!(rows.last().unwrap().words, "Resume failed");
        assert!(rows.iter().all(|row| row.kind == AttentionKind::Problem));
    }

    /// Routine lifecycle work cannot hide a durable unseen setup warning.
    /// Stale forwarding health is ignored while the connection is held: its
    /// stopped forwards are not evidence of a current connected-host failure.
    #[test]
    fn tensorlake_transitions_preserve_independent_attention() {
        use farhelm_proto::tensorlake::LifecycleDirection;
        for direction in [LifecycleDirection::Suspending, LifecycleDirection::Resuming] {
            let mut host = host(
                1,
                HostPhase::TensorlakeChanging {
                    direction,
                    requested_at: 1,
                },
            );
            assert_eq!(problem_words(&host, None, false), None);
            assert_eq!(
                problem_words(&host, None, true).as_deref(),
                Some("Setup or update needs attention")
            );
            host.kind = HostKind::Tensorlake;
            host.tensorlake = Some(TensorlakeHost {
                credential_warning: None,
                sandbox: "sandbox".into(),
                recipe: "recipe".into(),
                size: None,
                tunnels: Some(TunnelView {
                    state: TunnelState::Failed {
                        reason: "refused".into(),
                    },
                    tunnels: Vec::new(),
                }),
            });
            assert_eq!(problem_words(&host, None, false), None);
            assert_eq!(
                problem_words(&host, None, true).as_deref(),
                Some("Setup or update needs attention")
            );
        }
    }

    /// A live operation wins even over a refusal or unseen failure. Grouping is
    /// stable within each class, with required/optional updates sharing rail order.
    #[test]
    fn progress_wins_and_display_order_keeps_registry_order_within_groups() {
        let hosts = vec![
            host(1, connected()),
            host(2, skew(1)),
            host(3, skew(3)),
            host(4, connected()),
            host(5, connected()),
        ];
        let updates = HostUpdates {
            optional: vec![hosts[0].clone()],
            required: vec![hosts[1].clone()],
        };
        let mut warning = HostAttention::default();
        warning.failed_attempt(ProvisioningOperation::Update, false);
        let progress =
            HashMap::from([(3, HostUpdateProgress::Pending(UpdatePendingPhase::Planning))]);
        let attention = HashMap::from([(3, warning.clone()), (4, warning)]);
        let rows = classify(&hosts, &updates, &progress, &attention, None, "a");
        assert_eq!(
            identities(&rows),
            [
                (4, AttentionKind::Problem),
                (3, AttentionKind::Working),
                (1, AttentionKind::Update { required: false }),
                (2, AttentionKind::Update { required: true }),
            ]
        );
        assert_eq!(rows[0].words, "Setup or update needs attention");
        assert_eq!(rows[1].words, "Updating");
    }

    /// A dismissal never hides a required update or an unrelated problem, and
    /// losing a known host temporarily does not make its optional update new.
    #[test]
    fn only_optional_update_rows_obey_the_existing_dismissal() {
        let hosts = vec![
            host(1, connected()),
            host(2, skew(1)),
            host(3, skew(3)),
            host(99, connected()),
        ];
        let updates = HostUpdates {
            optional: vec![hosts[0].clone()],
            required: vec![hosts[1].clone()],
        };
        let dismissal = UpdateDismissal::new(&updates, "a".into());
        let classify_with = |offers: &HostUpdates, build| {
            classify(
                &hosts,
                offers,
                &HashMap::new(),
                &HashMap::new(),
                Some(&dismissal),
                build,
            )
        };
        assert_eq!(
            identities(&classify_with(&updates, "a")),
            [
                (3, AttentionKind::Problem),
                (2, AttentionKind::Update { required: true }),
            ]
        );
        assert_eq!(classify_with(&updates, "b").len(), 3);
        let mut changed = updates.clone();
        changed.optional.push(hosts[3].clone());
        assert_eq!(classify_with(&changed, "a").len(), 4);
        let subset = HostUpdates {
            optional: updates.optional.clone(),
            required: Vec::new(),
        };
        assert!(
            !classify_with(&subset, "a")
                .iter()
                .any(|row| row.host.id == 1)
        );
        let progress = HashMap::from([(
            1,
            HostUpdateProgress::PendingConfirmed(ProvisioningOperation::Uninstall),
        )]);
        let working = classify(
            &hosts,
            &updates,
            &progress,
            &HashMap::new(),
            Some(&dismissal),
            "a",
        );
        assert!(
            working
                .iter()
                .any(|row| row.host.id == 1 && row.kind == AttentionKind::Working)
        );
    }

    /// A connected supervisor may still have failed recipe forwards; its
    /// tunnel warning is independent of connection age and update eligibility.
    #[test]
    fn failed_tunnels_override_an_optional_update() {
        let mut sandbox = host(1, connected());
        sandbox.kind = HostKind::Tensorlake;
        sandbox.tensorlake = Some(TensorlakeHost {
            credential_warning: None,
            sandbox: "sandbox".into(),
            recipe: "recipe".into(),
            size: None,
            tunnels: Some(TunnelView {
                state: TunnelState::Failed {
                    reason: "refused".into(),
                },
                tunnels: Vec::new(),
            }),
        });
        let updates = HostUpdates {
            optional: vec![sandbox.clone()],
            required: Vec::new(),
        };
        let rows = classify(
            &[sandbox],
            &updates,
            &HashMap::new(),
            &HashMap::new(),
            None,
            "a",
        );
        assert_eq!(identities(&rows), [(1, AttentionKind::Problem)]);
        assert_eq!(rows[0].words, "Tunnels failed");
    }

    /// A credential warning outranks an optional update but must not hide a
    /// connection failure or a separate tunnel warning. Compact copy never
    /// repeats the potentially long peer diagnostic from the host details.
    #[test]
    fn credential_warnings_preserve_other_problem_reasons() {
        let mut sandbox = host(1, connected());
        sandbox.kind = HostKind::Tensorlake;
        sandbox.tensorlake = Some(TensorlakeHost {
            sandbox: "sandbox".into(),
            recipe: "recipe".into(),
            size: None,
            tunnels: Some(TunnelView {
                state: TunnelState::Failed {
                    reason: "fixture tunnel reason".into(),
                },
                tunnels: Vec::new(),
            }),
            credential_warning: Some("fixture credential reason".into()),
        });
        let updates = HostUpdates {
            optional: vec![sandbox.clone()],
            required: Vec::new(),
        };
        let rows = classify(
            &[sandbox.clone()],
            &updates,
            &HashMap::new(),
            &HashMap::new(),
            None,
            "a",
        );
        assert_eq!(identities(&rows), [(1, AttentionKind::Problem)]);
        assert_eq!(rows[0].words, "Credentials not delivered; Tunnels failed");
        sandbox.state = HostPhase::Unreachable {
            cause: "transport".into(),
            last_error: "fixture connection reason".into(),
        };
        let rows = classify(
            &[sandbox],
            &updates,
            &HashMap::new(),
            &HashMap::new(),
            None,
            "a",
        );
        assert_eq!(identities(&rows), [(1, AttentionKind::Problem)]);
        assert_eq!(
            rows[0].words,
            "Unreachable, retrying; Credentials not delivered"
        );
    }

    /// Stable verbs and fixed-size bars keep session popovers open across polls;
    /// a real change of row membership or operation copy still invalidates them.
    #[test]
    fn layout_ignores_polling_details_but_keeps_visible_copy() {
        let hosts = vec![host(1, connected())];
        let mut summary = UpdateProgressSummary {
            run_id: "first".into(),
            operation: ProvisioningOperation::Update,
            current_step: Some("upload".into()),
            steps: Vec::new(),
            done: 1,
            total: 4,
            started_at: Instant::now(),
        };
        let working = |summary| {
            classify(
                &hosts,
                &HostUpdates::default(),
                &HashMap::from([(1, HostUpdateProgress::Running(summary))]),
                &HashMap::new(),
                None,
                "a",
            )
        };
        let before = working(summary.clone());
        summary.run_id = "second".into();
        summary.current_step = Some("restart".into());
        summary.done = 3;
        summary.total = 5;
        summary.started_at = Instant::now();
        let after = working(summary.clone());
        assert_ne!(before[0].progress, after[0].progress);
        assert_eq!(layout_key(&before), layout_key(&after));
        summary.operation = ProvisioningOperation::Uninstall;
        assert_ne!(layout_key(&before), layout_key(&working(summary)));
        assert_ne!(layout_key(&before), layout_key(&[]));
        let pending = HashMap::from([(
            1,
            HostUpdateProgress::PendingConfirmed(ProvisioningOperation::Add),
        )]);
        let rows = classify(
            &hosts,
            &HostUpdates::default(),
            &pending,
            &HashMap::new(),
            None,
            "a",
        );
        assert_eq!(rows[0].words, "Setting up");
        assert_eq!(rows[0].progress, None);
    }
}
