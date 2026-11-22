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

use std::time::{SystemTime, UNIX_EPOCH};

use farhelm_proto::http::LOCAL_SUPERVISOR_NOT_RUNNING;
use farhelm_proto::tensorlake::{LifecycleDirection, TunnelState, TunnelView};
use farhelm_proto::ui_channel::{ConnectionStage, HostStateView, HostView, RefreshView};
use gpui::SharedString;

use crate::environment_ui::ProviderUi;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::offers::{HostKind, runs_newer_version};
use crate::local_time::{self, Zone};

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
        HostStateView::Connecting {
            stage: ConnectionStage::WaitingForSignIn,
            ..
        } => "Waiting for sign-in",
        HostStateView::SignInNeeded => "Sign-in needed",
        HostStateView::Connecting { .. } => "Connecting",
        HostStateView::Unreachable { .. } => "Unreachable, retrying",
        HostStateView::Connected {
            old_version: true, ..
        } => "Old version",
        HostStateView::Connected { .. } => "Connected",
        HostStateView::VersionSkew { .. } => "Needs update",
        HostStateView::IdentityMismatch { .. } => "Identity mismatch",
        HostStateView::OwnedElsewhere { .. } => "Managed by another Farhelm",
        HostStateView::IdentityUnverified { .. } => "Identity unverified",
        HostStateView::Duplicate { .. } => "Duplicate",
        HostStateView::TensorlakeChanging { starting: true, .. } => "Starting…",
        HostStateView::TensorlakeChanging { direction, .. } => changing_words(*direction),
        HostStateView::Suspended { .. } => "Suspended",
        HostStateView::Stopped { .. } => "Stopped",
        HostStateView::Retired { .. } => "Retired",
        HostStateView::Deleting { failure: None } => "Deleting…",
        HostStateView::Deleting { failure: Some(_) } => "Deleting failed",
    }
}

/// What a sandbox or container says while its provider changes it in `direction`: "Suspending…" or "Resuming…". The
/// one source of those words, which the pushed phase ([`words`]), the Hosts row's request in flight
/// (`crate::hosts::offers::Lifecycle::pending_words`) and a session's pending control
/// (`crate::session_list::environment::Pending::label`) all show, so the row and the session never word it apart.
pub(crate) fn changing_words(direction: LifecycleDirection) -> &'static str {
    match direction {
        LifecycleDirection::Suspending => "Suspending…",
        LifecycleDirection::Resuming => "Resuming…",
    }
}

/// How long ago Farhelm asked the provider to change `host`'s machine, as its changing row says it after the phase's
/// words (SPEC.md, "Topology": "**Suspending…**, with a spinner and how long ago Tensorlake was asked"): "asked
/// Tensorlake 12 s ago", the old `TensorlakeChangingStatus`'s elapsed, in the provider's words. `None` while the host
/// is not changing or has no provider. A viewer clock behind the helm's reads 0 s rather than wrapping, as the old
/// `lifecycle_age_secs` clamped it; the row redraws from the ticker's one-second clock while it shows (`crate::hosts`).
pub(crate) fn asked_ago(host: &HostView, now: SystemTime) -> Option<String> {
    let HostStateView::TensorlakeChanging { requested_at, .. } = host.state else {
        return None;
    };
    let provider = HostKind::of(host).provider()?;
    let now = now.duration_since(UNIX_EPOCH).map_or(0, |since| {
        i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
    });
    let seconds = now.saturating_sub(requested_at).max(0) / 1_000;
    Some(format!(
        "asked {} {seconds} s ago",
        ProviderUi(provider).lifecycle_asked()
    ))
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
        | HostStateView::Unreachable { .. }
        | HostStateView::SignInNeeded
        | HostStateView::Connecting {
            stage: ConnectionStage::WaitingForSignIn,
            ..
        } => Tone::Advisory,
        HostStateView::Connected { .. } => Tone::Connected,
        HostStateView::Connecting { .. }
        | HostStateView::TensorlakeChanging { .. }
        | HostStateView::Deleting { failure: None } => Tone::Pending,
        HostStateView::Suspended { .. } => Tone::Quiet,
        HostStateView::VersionSkew { .. }
        | HostStateView::IdentityMismatch { .. }
        | HostStateView::OwnedElsewhere { .. }
        | HostStateView::IdentityUnverified { .. }
        | HostStateView::Duplicate { .. }
        | HostStateView::Retired { .. }
        | HostStateView::Stopped { .. }
        | HostStateView::Deleting { failure: Some(_) } => Tone::Attention,
    }
}

/// The tone of `host`'s phase as its row and its rail avatar show it: [`tone`], except that a host whose credentials
/// were not delivered ([`credential_warning`]), or a connected host whose forwards failed ([`tunnels_failed`]), reads
/// [`Tone::Advisory`] whatever its phase, as the old row and rail turned such a host amber. The words stay the phase's:
/// delivery and forwarding are independent of the supervisor's connection, and the details say what failed.
pub(crate) fn host_tone(host: &HostView) -> Tone {
    if credential_warning(host).is_some() || tunnels_failed(host) {
        return Tone::Advisory;
    }
    tone(&host.state)
}

/// Why the helm could not deliver a sandbox's or a container's credentials, when it could not (the old
/// `credential_delivery_failed`, whose reason the details show): an agent there may run without the keys it needs, and
/// delivery can fail during setup too, so this does not wait for a connection.
pub(crate) fn credential_warning(host: &HostView) -> Option<&str> {
    host.environment
        .as_ref()
        .and_then(|environment| environment.credential_warning.as_deref())
        .or_else(|| {
            host.tensorlake
                .as_ref()
                .and_then(|sandbox| sandbox.credential_warning.as_deref())
        })
}

/// The forwards a sandbox or an environment keeps for its recipe's tunnels, with their health, where the helm reports
/// any: the environment's when the host is one, else the sandbox's.
fn tunnels(host: &HostView) -> Option<&TunnelView> {
    host.environment
        .as_ref()
        .and_then(|environment| environment.tunnels.as_ref())
        .or_else(|| {
            host.tensorlake
                .as_ref()
                .and_then(|sandbox| sandbox.tunnels.as_ref())
        })
}

/// Whether a connected host's forwards failed or are retrying after a failure (the old `tunnels::failed`): agents may
/// work while their inference endpoint is unreachable, so the row and the rail show it beside a connection that is
/// fine. Forwards still starting up, with no failure yet, claim nothing.
pub(crate) fn tunnels_failed(host: &HostView) -> bool {
    matches!(host.state, HostStateView::Connected { .. })
        && tunnels(host).is_some_and(|view| {
            matches!(
                view.state,
                TunnelState::Failed { .. }
                    | TunnelState::Retrying {
                        reason: Some(_),
                        ..
                    }
            )
        })
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

/// The lines a row shows when its details are open, in the old order: for a sandbox or an environment, the machine
/// behind it (its name, recipe, container host and size), its forwards' health with one line per forward, and why its
/// credentials were not delivered; the destination behind an alias; the evidence for the phase; and the remedy where
/// waiting will not help. `twin` names another host by id, as the duplicate lines mention the entry that holds the
/// same machine, or `None` when the registry does not list it. `zone` is the viewer's time zone, for the times the
/// evidence gives.
///
/// Each line is drawn under `host-detail-<host id>-<n>`, numbered from 0 in this order, which is how the panel's tests
/// and the agent bridge find a row's lines.
pub(crate) fn detail_lines(
    host: &HostView,
    twin: impl Fn(i64) -> Option<HostText>,
    zone: Zone,
) -> Vec<(Kind, HostSentence)> {
    let id = host.id;
    let numbered =
        |n: usize| HostSentence::new(SharedString::from(format!("host-detail-{id}-{n}")));
    let mut lines = Vec::new();
    lines.extend(machine_line(numbered(lines.len()), host).map(|line| (Kind::Evidence, line)));
    if let Some(view) = tunnels(host) {
        let first = lines.len();
        lines
            .extend(tunnel_lines(view, |n| numbered(first + n)).map(|line| (Kind::Evidence, line)));
    }
    // The helm's reason, which may quote the provider or the machine, so host text.
    if let Some(reason) = credential_warning(host) {
        let line = numbered(lines.len())
            .text("Credentials not delivered: ")
            .host(&pushed(reason));
        lines.push((Kind::Evidence, line));
    }
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
        host_evidence(numbered(lines.len()), host, &twin, zone),
    ));
    let last = remedy(numbered(lines.len()), &host.state, &twin);
    lines.extend(last.map(|line| (Kind::Remedy, line)));
    lines
}

/// The progress card's connection detail for `host`, drawn under `id`: what the helm knows about its phase, the same
/// line its row's details show ([`detail_lines`]' evidence), or `None` while it is connected, when the card has no
/// connection section (the old card's `host_state_detail`, shown only while `!is_connected`). The card follows the host
/// entry, not the run, so a host that reconnects mid-run loses the section at the next push. `twin` names a duplicate's
/// other entry, as for the row, and `zone` is the viewer's, for the times the line gives.
pub(crate) fn connection_line(
    host: &HostView,
    id: impl Into<gpui::ElementId>,
    twin: impl Fn(i64) -> Option<HostText>,
    zone: Zone,
) -> Option<HostSentence> {
    match host.state {
        HostStateView::Connected { .. } => None,
        _ => Some(host_evidence(HostSentence::new(id), host, &twin, zone)),
    }
}

/// The details line of the machine behind a sandbox or an environment (the old `host-environment-detail` and
/// `host-tensorlake-detail`): "Sandbox: <name> · Recipe: <recipe>", with " · Container host: <host>" for a container
/// placed on one, and the size the sandbox was created with (" · 2 CPUs · 4 GB memory"). The word that leads it is the
/// provider's ([`ProviderUi::locality_label`]). `None` for a host with no machine of its own, or of a kind this build
/// does not know.
fn machine_line(line: HostSentence, host: &HostView) -> Option<HostSentence> {
    let provider = HostKind::of(host).provider()?;
    let (name, recipe, place) = match (&host.environment, &host.tensorlake) {
        (Some(environment), _) => (
            environment.name.as_str(),
            environment.target.recipe(),
            environment.place_name.as_deref(),
        ),
        (None, Some(sandbox)) => (sandbox.sandbox.as_str(), sandbox.recipe.as_str(), None),
        (None, None) => return None,
    };
    let mut line = line
        .text(format!("{}: ", ProviderUi(provider).locality_label()))
        .host(&pushed(name))
        .text(" · Recipe: ")
        .host(&pushed(recipe));
    if let Some(place) = place {
        line = line.text(" · Container host: ").host(&pushed(place));
    }
    if let Some(size) = host.tensorlake.as_ref().and_then(|sandbox| sandbox.size) {
        line = line.text(format!(
            " · {} CPUs · {} GB memory",
            size.cpus, size.memory_gb
        ));
    }
    Some(line)
}

/// A host's forwards as details lines (the old `TunnelDetails`): their health in one sentence ("Tunnels up", "Tunnels
/// failed: <reason>", "Tunnels failed; retrying in 4s: <reason>", "Tunnels connecting…"), then one line per forward,
/// "127.0.0.1:<sandbox port> → <host>:<port>", each started from `line(n)`, the empty line numbered `n` from 0. The
/// reasons and the forwards' hosts come from the recipe or from SSH, so they are host text.
fn tunnel_lines<'a>(
    view: &'a TunnelView,
    line: impl Fn(usize) -> HostSentence + 'a,
) -> impl Iterator<Item = HostSentence> + 'a {
    let health = match &view.state {
        TunnelState::Up => line(0).text("Tunnels up"),
        TunnelState::Failed { reason } => line(0).text("Tunnels failed: ").host(&pushed(reason)),
        TunnelState::Retrying {
            reason: Some(reason),
            delay_secs,
        } => line(0)
            .text(format!("Tunnels failed; retrying in {delay_secs}s: "))
            .host(&pushed(reason)),
        TunnelState::Retrying { reason: None, .. } => line(0).text("Tunnels connecting…"),
    };
    let forwards = view.tunnels.iter().enumerate().map(move |(n, tunnel)| {
        line(n + 1)
            .text(format!("127.0.0.1:{} → ", tunnel.sandbox_port))
            .host(&pushed(&tunnel.host))
            .text(format!(":{}", tunnel.port))
    });
    std::iter::once(health).chain(forwards)
}

/// The generic provider line describes the direction alone. The host-specific
/// line can also distinguish starting a stopped container from resuming it.
fn lifecycle_infinitive(direction: &LifecycleDirection, starting: bool) -> &'static str {
    match (direction, starting) {
        (LifecycleDirection::Suspending, _) => "suspend",
        (LifecycleDirection::Resuming, true) => "start",
        (LifecycleDirection::Resuming, false) => "resume",
    }
}

/// [`evidence`] for `host`: a machine's change ("Asked Tensorlake to resume at …") and its deletion ("Farhelm is
/// deleting this sandbox, its files and its snapshots", or the failure and Retry, then the helm's reason) are worded
/// for the host's provider. A stopped container's start has its own verb even though the lifecycle direction is
/// Resuming. The unavailable session card uses the same line as Hosts. Every other phase, and a host with no
/// provider, reads as [`evidence`] says.
pub(crate) fn host_evidence(
    line: HostSentence,
    host: &HostView,
    twin: &impl Fn(i64) -> Option<HostText>,
    zone: Zone,
) -> HostSentence {
    match (&host.state, HostKind::of(host).provider()) {
        // The old `lifecycle_request_detail`: whom Farhelm asked, in the provider's words.
        (
            HostStateView::TensorlakeChanging {
                starting,
                direction,
                requested_at,
            },
            Some(provider),
        ) => line.text(format!(
            "Asked {} to {} at {}; waiting for confirmation",
            ProviderUi(provider).lifecycle_asked(),
            lifecycle_infinitive(direction, *starting),
            local_time::stamp(requested_at / 1000, zone)
        )),
        (HostStateView::Deleting { failure }, Some(provider)) => {
            let line = line.text(ProviderUi(provider).deleting_detail(failure.is_some()));
            match failure {
                Some(reason) => line.text(": ").host(&pushed(reason)),
                None => line,
            }
        }
        _ => evidence(line, &host.state, twin, zone),
    }
}

/// What the helm knows about the host's phase (the old `state_detail`), appended to `line`. For a connected host this
/// one line is its version, identity and session count. Visible to the crate for the session pane's unavailable card,
/// whose smaller diagnostics line is this one after the host's name, as the old card's was. The progress card reaches it
/// through [`connection_line`]. The times it gives (when a suspension was noticed, when a provider was asked) are the
/// viewer's local time in `zone`.
pub(crate) fn evidence(
    line: HostSentence,
    state: &HostStateView,
    twin: &impl Fn(i64) -> Option<HostText>,
    zone: Zone,
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
                ConnectionStage::WaitingForSignIn => "waiting for sign-in…",
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
        // The local row's evidence is the helm's dial failure, as any unreachable host's is, and what to do goes in the
        // remedy ([`remedy`]). The dial failure is never blank in practice; the fallback only keeps the line honest if
        // it ever is.
        HostStateView::Unreachable { cause, last_error } if cause == LOCAL_SUPERVISOR_NOT_RUNNING => {
            if last_error.trim().is_empty() {
                line.text("No supervisor is running on this machine")
            } else {
                line.host(&pushed(last_error))
            }
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
        HostStateView::OwnedElsewhere { owner, .. } => {
            let line = line.text("Helm ").mono(&pushed(&owner.tag));
            if owner.connected {
                line.text(", connected now")
            } else {
                line.text(format!(", last connected {}", local_time::stamp(owner.last_connected_at, zone)))
            }
        }
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
        HostStateView::SignInNeeded => line.text("SSH sign-in ended while the host was asking for an answer"),
        // A host with a provider gets its provider's words ([`host_evidence`]); these are the old app's words without
        // one, for the unavailable session card, which words its host's phase from the state alone.
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
                local_time::stamp(suspended_by_farhelm_at.unwrap_or(*observed_at) / 1000, zone)
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
            lifecycle_infinitive(direction, false),
            local_time::stamp(requested_at / 1000, zone)
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
/// one exception is the helm's own machine with no supervisor, whose remedy is the app's sentence alone, "Quit Farhelm
/// and open it again; it starts this machine's supervisor when it opens", because the desktop app starts that
/// supervisor whenever it opens and none answers. The helm's dial failure stays on the evidence line ([`evidence`]), as
/// what happened, and never joins the remedy. (The helm's dial of its own machine carries no hand-start advice; for a
/// remote host with no supervisor, the helm names the start command itself, from that host entry's program and state
/// directory.)
///
/// The remedy is appended to `line`. Visible to the crate for the session pane's unavailable card, which explains an
/// unreachable session with it where there is one, as the old card did.
pub(crate) fn remedy(
    line: HostSentence,
    state: &HostStateView,
    twin: &impl Fn(i64) -> Option<HostText>,
) -> Option<HostSentence> {
    match state {
        HostStateView::Connecting { stage: ConnectionStage::WaitingForSignIn, .. } => {
            Some(line.text("The host is waiting for an answer to its SSH sign-in question"))
        }
        HostStateView::Unreachable { cause, .. } if cause == LOCAL_SUPERVISOR_NOT_RUNNING => {
            Some(line.text("Quit Farhelm and open it again; it starts this machine's supervisor when it opens"))
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
        HostStateView::OwnedElsewhere { .. } => Some(line.text(
            "Use Take over in Hosts to manage this host and its sessions from this Farhelm.",
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
        HostStateView::SignInNeeded => Some(line.text("Retry in Hosts to start another SSH sign-in")),
        HostStateView::Deleting { failure: Some(_) } => Some(line.text("Retry to delete it again")),
    }
}
