//! What a host row says about its host: the phase words beside the dot and their color, the hover on them, and the
//! detail lines under the row (the destination behind an alias, the evidence for the phase, and the remedy).
//!
//! Ported word for word from the old app's `hosts.rs` at reference commit `24bc0580` (`phase_display_label`,
//! `phase_class`, `too_new_title`, `state_detail`, `refresh_detail`, `state_remedy`, `host_row_lines`). The old
//! `Unrecognized` phase is not ported: the GUI decodes the helm's state into a closed type, and a state it cannot
//! decode breaks the channel instead of reaching a row.
//!
//! Everything a host or the helm reported (builds, identities, errors, the skew remedy, a twin's name) stays host text:
//! a detail line is a [`HostSentence`], the app's words and host values each their own part, drawn each as its own
//! text element so a host value's direction cannot reorder the words around it. Builds, a connected host's identity and
//! the destination are its monospace parts, as the old app's `DetailPart::MonoPeer` runs were.

use farhelm_proto::http::LOCAL_SUPERVISOR_NOT_RUNNING;
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::{ConnectionStage, HostStateView, HostView, RefreshView};
use gpui::SharedString;

use super::utc_stamp;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::offers::runs_newer_version;

#[cfg(test)]
mod tests;

/// The build a host reports itself running: a connected host's, or the one a skewed host gave at the handshake. `None`
/// in every other phase, where nothing the host said is current. The host settings' header and the host menu's header
/// both show it, so they cannot disagree on which phases count.
pub(crate) fn reported_build(state: &HostStateView) -> Option<&str> {
    match state {
        HostStateView::Connected { build_version, .. } => Some(build_version),
        HostStateView::VersionSkew { peer_build, .. } => Some(peer_build),
        _ => None,
    }
}

/// The phase words of a host, as the row shows them beside its dot. A host running a newer Farhelm reads "Too new"
/// whatever its phase (connected or refused at the handshake), before its own words.
pub(crate) fn words(state: &HostStateView) -> &'static str {
    if runs_newer_version(state) {
        return "Too new";
    }
    match state {
        HostStateView::Connecting { .. } => "Connecting",
        HostStateView::Unreachable { .. } => "Unreachable, retrying",
        HostStateView::Connected {
            old_version: true, ..
        } => "Old version",
        HostStateView::Connected { .. } => "Connected",
        HostStateView::VersionSkew { .. } => "Needs update",
        HostStateView::IdentityMismatch { .. } => "Identity mismatch",
        HostStateView::IdentityUnverified { .. } => "Identity unverified",
        HostStateView::Duplicate { .. } => "Duplicate",
        HostStateView::TensorlakeChanging { starting: true, .. } => "Starting…",
        HostStateView::TensorlakeChanging { direction, .. } => match direction {
            LifecycleDirection::Suspending => "Suspending…",
            LifecycleDirection::Resuming => "Resuming…",
        },
        HostStateView::Suspended { .. } => "Suspended",
        HostStateView::Stopped { .. } => "Stopped",
        HostStateView::Retired { .. } => "Retired",
        HostStateView::Deleting { failure: None } => "Deleting…",
        HostStateView::Deleting { failure: Some(_) } => "Deleting failed",
    }
}

/// The color group a phase's dot and words take, by what someone watching the panel should do about it. Deliberately
/// coarser than the words: the color is a category signal and the exact phase is right beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    /// All is well (`--ok`).
    Connected,
    /// Usable or recovering by itself, but worth knowing (`--warn`): an older or newer build, or a host being
    /// re-probed.
    Advisory,
    /// Nothing to do yet (the quiet ink): connecting, or waiting on the provider. The old app set these in italics; the
    /// GUI bundles no italic face, so the port draws them upright (the parity file records it).
    Pending,
    /// Nothing is wrong and nothing changes by itself (the quiet ink): a suspended sandbox.
    Quiet,
    /// Stays as it is until someone acts (`--danger-strong`): a skew, either identity state, a duplicate, a retired
    /// entry, a stopped environment, a failed delete.
    Attention,
}

/// The tone of a phase (the old `phase_class`). A skewed host stays [`Tone::Attention`] even when it is the newer side,
/// since it cannot be used until one side is updated.
pub(crate) fn tone(state: &HostStateView) -> Tone {
    match state {
        HostStateView::Connected {
            newer_version: true,
            ..
        }
        | HostStateView::Connected {
            old_version: true, ..
        }
        | HostStateView::Unreachable { .. } => Tone::Advisory,
        HostStateView::Connected { .. } => Tone::Connected,
        HostStateView::Connecting { .. }
        | HostStateView::TensorlakeChanging { .. }
        | HostStateView::Deleting { failure: None } => Tone::Pending,
        HostStateView::Suspended { .. } => Tone::Quiet,
        HostStateView::VersionSkew { .. }
        | HostStateView::IdentityMismatch { .. }
        | HostStateView::IdentityUnverified { .. }
        | HostStateView::Duplicate { .. }
        | HostStateView::Retired { .. }
        | HostStateView::Stopped { .. }
        | HostStateView::Deleting { failure: Some(_) } => Tone::Attention,
    }
}

/// The hover on a phase's words: for a host running a newer Farhelm, both builds (and both protocols for a skew) and
/// the remedy, which is updating the helm; for any other host, the words themselves. Host builds are escaped, as the
/// screen shows host text.
///
/// The helm's own build for a connected host is this build's (`farhelm_proto::BUILD_VERSION`): the desktop app embeds
/// the helm it shows, so the two are one build, as the old page was served by its own helm. A skewed host's versions
/// come from the helm's refusal.
pub(crate) fn hover(state: &HostStateView) -> SharedString {
    match state {
        HostStateView::Connected {
            newer_version: true,
            build_version,
            ..
        } => HostSentence::new("host-too-new")
            .text("This host runs Farhelm ")
            .host(&pushed(build_version))
            .text(format!(
                ", which is newer than this helm's {} (both speak protocol {}); update the helm",
                farhelm_proto::BUILD_VERSION,
                farhelm_proto::PROTOCOL_VERSION,
            ))
            .accessible_name(),
        HostStateView::VersionSkew {
            peer_protocol,
            peer_build,
            our_protocol,
            our_build,
            ..
        } if peer_protocol > our_protocol => HostSentence::new("host-too-new")
            .text("This host runs Farhelm ")
            .host(&pushed(peer_build))
            .text(format!(
                " (protocol {peer_protocol}), which is newer than this helm's "
            ))
            .host(&pushed(our_build))
            .text(format!(" (protocol {our_protocol}); update the helm"))
            .accessible_name(),
        _ => words(state).into(),
    }
}

// ---- Detail lines ----------------------------------------------------------------------------------------------------

/// A value the helm pushed for this host (an error, a build), taken as host text where the detail lines and the hover
/// read it.
fn pushed(value: &str) -> HostText {
    HostText::from_host(value)
}

/// An install id the helm pushed for this host, in the identity spelling ([`HostText::identity`]): every space and
/// other non-ASCII or look-alike character written out, so two ids the user compares (recorded against reported, this
/// entry's against its twin's) never look alike, as SPEC.md asks of identities and the old app's `display_identity`
/// did.
fn install_id(value: &str) -> HostText {
    HostText::identity(value)
}

/// What a detail line is, which decides its ink: what the helm knows in the quiet ink, what to do in the ordinary one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Evidence,
    Remedy,
}

/// The lines a row shows when its details are open, in the old order: the destination behind an alias, the evidence
/// for the phase, and the remedy where waiting will not help. `twin` names another host by id, as the duplicate lines
/// mention the entry that holds the same machine, or `None` when the registry does not list it.
///
/// Each line is drawn under `host-detail-<host id>-<n>`, numbered from 0 in this order, which is how the panel's tests
/// and the agent bridge find a row's lines.
pub(crate) fn detail_lines(
    host: &HostView,
    twin: impl Fn(i64) -> Option<HostText>,
) -> Vec<(Kind, HostSentence)> {
    let id = host.id;
    let numbered =
        |n: usize| HostSentence::new(SharedString::from(format!("host-detail-{id}-{n}")));
    let mut lines = Vec::new();
    // Only behind an alias: an unaliased host's name already is its destination.
    if let (Some(_), Some(destination)) = (&host.alias, &host.destination) {
        lines.push((
            Kind::Evidence,
            numbered(lines.len())
                .text("Destination: ")
                .mono(&pushed(destination)),
        ));
    }
    lines.push((
        Kind::Evidence,
        evidence(numbered(lines.len()), &host.state, &twin),
    ));
    let last = remedy(numbered(lines.len()), &host.state, &twin);
    lines.extend(last.map(|line| (Kind::Remedy, line)));
    lines
}

/// The progress card's connection detail for `host`, drawn under `id`: what the helm knows about its phase, the same
/// line its row's details show ([`detail_lines`]' evidence), or `None` while it is connected, when the card has no
/// connection section (the old card's `host_state_detail`, shown only while `!is_connected`). The card follows the host
/// entry, not the run, so a host that reconnects mid-run loses the section at the next push. `twin` names a duplicate's
/// other entry, as for the row.
pub(crate) fn connection_line(
    host: &HostView,
    id: impl Into<gpui::ElementId>,
    twin: impl Fn(i64) -> Option<HostText>,
) -> Option<HostSentence> {
    match host.state {
        HostStateView::Connected { .. } => None,
        _ => Some(evidence(HostSentence::new(id), &host.state, &twin)),
    }
}

/// What the helm knows about the host's phase (the old `state_detail`), appended to `line`. For a connected host this
/// one line is its version, identity and session count. Visible to the crate for the session pane's unavailable card,
/// whose smaller diagnostics line is this one after the host's name, as the old card's was. The progress card reaches it
/// through [`connection_line`].
pub(crate) fn evidence(
    line: HostSentence,
    state: &HostStateView,
    twin: &impl Fn(i64) -> Option<HostText>,
) -> HostSentence {
    match state {
        HostStateView::Connecting {
            attempt,
            stage,
            last_error,
        } => {
            // These phrases continue after the attempt prefix, so they start in lower case.
            let progress = match stage {
                ConnectionStage::WaitingToConnect => "waiting for the next connection attempt…",
                ConnectionStage::ConnectingToHost => "connecting to the host…",
                ConnectionStage::ConnectingToSupervisor => "connecting to the local supervisor…",
                ConnectionStage::WaitingForHello => "waiting for the supervisor to answer…",
                ConnectionStage::SettingUp => "finishing connection setup…",
            };
            let lead = if *attempt == 0 {
                format!("The first connection attempt: {progress}")
            } else {
                format!("Attempt {attempt}: {progress}")
            };
            match last_error {
                Some(error) => line
                    .text(format!("{lead}; the last one failed: "))
                    .host(&pushed(error)),
                None => line.text(lead),
            }
        }
        // The helm's dial failure for the local row carries the exact command that fixes it, so it goes in the remedy,
        // and this line says only what happened.
        HostStateView::Unreachable { cause, .. } if cause == LOCAL_SUPERVISOR_NOT_RUNNING => {
            line.text("No supervisor is running on this machine")
        }
        HostStateView::Unreachable { last_error, .. } => {
            if last_error.trim().is_empty() {
                line.text("The host did not answer")
            } else {
                line.host(&pushed(last_error))
            }
        }
        HostStateView::Connected {
            identity,
            build_version,
            refresh,
            ..
        } => {
            let line = line.text("Farhelm ").mono(&pushed(build_version));
            let line = match identity {
                Some(identity) => line.text("; identity ").mono(&install_id(identity)),
                None => line.text("; no identity reported"),
            };
            let line = line.text("; ");
            match refresh {
                RefreshView::Pending => line.text("the first session refresh is still in flight"),
                RefreshView::Ok { sessions: 1 } => line.text("1 session"),
                RefreshView::Ok { sessions } => line.text(format!("{sessions} sessions")),
                RefreshView::Failed { error } => line
                    .text("the last session refresh failed, so its sessions are last-known: ")
                    .host(&pushed(error)),
            }
        }
        HostStateView::VersionSkew {
            peer_protocol,
            peer_build,
            our_protocol,
            our_build,
            ..
        } => line
            .text(format!("The host speaks protocol {peer_protocol} (Farhelm "))
            .mono(&pushed(peer_build))
            .text(format!("); this helm speaks {our_protocol} (Farhelm "))
            .mono(&pushed(our_build))
            .text(")"),
        HostStateView::IdentityMismatch { recorded, reported } => line
            .text("Recorded as install ")
            .host(&install_id(recorded))
            .text("; the destination now reports ")
            .host(&install_id(reported))
            .text(", so nothing is connected until this is decided"),
        HostStateView::IdentityUnverified { recorded } => line
            .text(
                "The host answered without an identity, so this helm cannot confirm it is still the install \
                 recorded as ",
            )
            .host(&install_id(recorded)),
        HostStateView::Duplicate { twin: id, identity } => {
            let line = line.text("This entry reaches install ").host(&install_id(identity));
            let line = match twin(*id) {
                Some(name) => line
                    .text(", which the entry ")
                    .host(&name)
                    .text(" already holds"),
                None => line.text(format!(", which another entry (host {id}) already holds")),
            };
            line.text(", so nothing is connected through this one")
        }
        HostStateView::Retired { reason } => {
            if reason.trim().is_empty() {
                line.text("No connection actor is running for this entry")
            } else {
                line.host(&pushed(reason))
            }
        }
        // The environment phases are worded per provider by the environment pieces of the port; until then they read
        // as the old app worded them without a provider.
        HostStateView::Deleting { failure: None } => {
            line.text("Farhelm is deleting this host's environment")
        }
        HostStateView::Deleting {
            failure: Some(reason),
        } => line
            .text("Deleting this host's environment failed: ")
            .host(&pushed(reason)),
        HostStateView::Suspended {
            observed_at,
            suspended_by_farhelm_at,
            resume_failed,
        } => {
            let line = line.text(format!(
                "Suspension {} at {}",
                if suspended_by_farhelm_at.is_some() {
                    "completed from Farhelm"
                } else {
                    "noticed"
                },
                utc_stamp(suspended_by_farhelm_at.unwrap_or(*observed_at) / 1000)
            ));
            match resume_failed {
                Some(reason) => line.text(". Resume failed: ").host(&pushed(reason)),
                None => line,
            }
        }
        HostStateView::TensorlakeChanging {
            direction,
            requested_at,
            ..
        } => line.text(format!(
            "Asked the environment provider to {} at {}; waiting for confirmation",
            match direction {
                LifecycleDirection::Suspending => "suspend",
                LifecycleDirection::Resuming => "resume",
            },
            utc_stamp(requested_at / 1000)
        )),
        HostStateView::Stopped {
            start_failed,
            waiting_for_place,
        } => {
            let line = line.text(if *waiting_for_place {
                "The environment is stopped; waiting for its container host to reconnect."
            } else {
                "The environment is stopped. Resume starts it again; its files remain."
            });
            match start_failed {
                Some(reason) => line.text(" Start failed: ").host(&pushed(reason)),
                None => line,
            }
        }
    }
}

/// What to do about the phase, where waiting will not help (the old `state_remedy`). Connecting and unreachable hosts
/// get none: one is still trying and the other is re-probed forever, so telling the user to act would invent work. The
/// one exception is the helm's own machine with no supervisor, whose remedy is a command on the machine the user sits
/// at: the helm's own dial failure spells it out with the right state directory, so that is shown rather than words of
/// the app's, which cannot know the directory. The remedy is appended to `line`. Visible to the crate for the session
/// pane's unavailable card, which explains an unreachable session with it where there is one, as the old card did.
pub(crate) fn remedy(
    line: HostSentence,
    state: &HostStateView,
    twin: &impl Fn(i64) -> Option<HostText>,
) -> Option<HostSentence> {
    match state {
        HostStateView::Unreachable { cause, last_error } if cause == LOCAL_SUPERVISOR_NOT_RUNNING => {
            Some(if last_error.trim().is_empty() {
                line.text(
                    "Start a supervisor on this machine with `farhelm supervisor run`, passing the same `--state-dir` \
                     this helm was started with if it has one",
                )
            } else {
                line.text("The helm reports: ").host(&pushed(last_error))
            })
        }
        HostStateView::Unreachable { .. }
        | HostStateView::Connecting { .. }
        | HostStateView::Connected { .. }
        | HostStateView::Suspended { .. }
        | HostStateView::Stopped { .. }
        | HostStateView::TensorlakeChanging { .. }
        | HostStateView::Deleting { failure: None } => None,
        // The helm's own sentence, verbatim: it knows which side is behind.
        HostStateView::VersionSkew { remediation, .. } => {
            (!remediation.trim().is_empty()).then(|| line.host(&pushed(remediation)))
        }
        HostStateView::IdentityMismatch { .. } => Some(line.text(
            "Adopt the identity the host reports, or fix the destination; adopting resets all five of this host's \
             permissions to asking",
        )),
        HostStateView::IdentityUnverified { .. } => Some(line.text(
            "Fix the host so it reports its identity, or retarget or remove this entry — it is re-probed meanwhile, \
             so a host that starts identifying itself again recovers on its own",
        )),
        HostStateView::Duplicate { twin: id, .. } => Some(match twin(*id) {
            Some(name) => line
                .text("Remove ")
                .host(&name)
                .text(" or change this entry's destination, then press Retry"),
            None => line.text("Remove the other entry or change this entry's destination, then press Retry"),
        }),
        HostStateView::Retired { .. } => {
            Some(line.text("Retry to start a fresh connection actor for this entry"))
        }
        HostStateView::Deleting { failure: Some(_) } => Some(line.text("Retry to delete it again")),
    }
}
