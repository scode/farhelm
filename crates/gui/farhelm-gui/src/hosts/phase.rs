//! What a host row says about its host: the phase words beside the dot and their color, the hover on them, and the
//! detail lines under the row (the destination behind an alias, the evidence for the phase, and the remedy).
//!
//! Ported word for word from the old app's `hosts.rs` at reference commit `24bc0580` (`phase_display_label`,
//! `phase_class`, `too_new_title`, `state_detail`, `refresh_detail`, `state_remedy`, `host_row_lines`). The old
//! `Unrecognized` phase is not ported: the GUI decodes the helm's state into a closed type, and a state it cannot
//! decode breaks the channel instead of reaching a row.
//!
//! Everything a host or the helm reported (builds, identities, errors, the skew remedy, a twin's name) stays host text:
//! a detail line is a list of [`Part`]s, the app's words and host values each their own part, drawn each as its own
//! text element so a host value's direction cannot reorder the words around it.

use farhelm_proto::http::LOCAL_SUPERVISOR_NOT_RUNNING;
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::{ConnectionStage, HostStateView, HostView, RefreshView};
use gpui::SharedString;

use super::offers::runs_newer_version;
use crate::host_text::{HostSentence, HostText};

#[cfg(test)]
mod tests;

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
            .host(&HostText::from_host(build_version.as_str()))
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
            .host(&HostText::from_host(peer_build.as_str()))
            .text(format!(
                " (protocol {peer_protocol}), which is newer than this helm's "
            ))
            .host(&HostText::from_host(our_build.as_str()))
            .text(format!(" (protocol {our_protocol}); update the helm"))
            .accessible_name(),
        _ => words(state).into(),
    }
}

// ---- Detail lines ----------------------------------------------------------------------------------------------------

/// One part of a detail line.
///
/// NOTE: `Part` and [`Line`] are [`HostSentence`] plus a monospace host part, which `HostSentence` does not have yet
/// (the old app's `DetailPart::MonoPeer`). A monospace part for it has been requested from the host text foundation's
/// owner; once it lands, a detail line becomes a `HostSentence` and these two types, with the panel's part-drawing loop,
/// go away.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Part {
    /// The app's own words.
    Text(SharedString),
    /// A value a host or the helm reported, in the interface font.
    Host(HostText),
    /// A reported value that reads as code (a build, an identity, a destination), in the monospace font.
    Mono(HostText),
}

/// A detail line: its parts in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Line(pub(crate) Vec<Part>);

impl Line {
    fn text(mut self, words: impl Into<SharedString>) -> Self {
        self.0.push(Part::Text(words.into()));
        self
    }

    fn host(mut self, value: &str) -> Self {
        self.0.push(Part::Host(HostText::from_host(value)));
        self
    }

    fn mono(mut self, value: &str) -> Self {
        self.0.push(Part::Mono(HostText::from_host(value)));
        self
    }

    /// The line as assistive technology reads it, with every host value escaped as the screen shows it.
    pub(crate) fn accessible_name(&self) -> SharedString {
        self.0
            .iter()
            .fold(
                HostSentence::new("host-detail"),
                |sentence, part| match part {
                    Part::Text(words) => sentence.text(words.clone()),
                    Part::Host(value) | Part::Mono(value) => sentence.host(value),
                },
            )
            .accessible_name()
    }
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
pub(crate) fn detail_lines(
    host: &HostView,
    twin: impl Fn(i64) -> Option<HostText>,
) -> Vec<(Kind, Line)> {
    let mut lines = Vec::new();
    // Only behind an alias: an unaliased host's name already is its destination.
    if let (Some(_), Some(destination)) = (&host.alias, &host.destination) {
        lines.push((
            Kind::Evidence,
            Line::default().text("Destination: ").mono(destination),
        ));
    }
    lines.push((Kind::Evidence, evidence(&host.state, &twin)));
    lines.extend(remedy(&host.state, &twin).map(|line| (Kind::Remedy, line)));
    lines
}

/// What the helm knows about the host's phase (the old `state_detail`). For a connected host this one line is its
/// version, identity and session count.
fn evidence(state: &HostStateView, twin: &impl Fn(i64) -> Option<HostText>) -> Line {
    let line = Line::default();
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
            // One piece of the app's words before the error: parts wrap as whole pieces, so the lead and "; the last
            // one failed: " would otherwise break apart at their seam.
            match last_error {
                Some(error) => line
                    .text(format!("{lead}; the last one failed: "))
                    .host(error),
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
                line.host(last_error)
            }
        }
        HostStateView::Connected {
            identity,
            build_version,
            refresh,
            ..
        } => {
            let line = line.text("Farhelm ").mono(build_version);
            let line = match identity {
                Some(identity) => line.text("; identity ").mono(identity),
                None => line.text("; no identity reported"),
            };
            let line = line.text("; ");
            match refresh {
                RefreshView::Pending => line.text("the first session refresh is still in flight"),
                RefreshView::Ok { sessions: 1 } => line.text("1 session"),
                RefreshView::Ok { sessions } => line.text(format!("{sessions} sessions")),
                RefreshView::Failed { error } => line
                    .text("the last session refresh failed, so its sessions are last-known: ")
                    .host(error),
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
            .mono(peer_build)
            .text(format!("); this helm speaks {our_protocol} (Farhelm "))
            .mono(our_build)
            .text(")"),
        HostStateView::IdentityMismatch { recorded, reported } => line
            .text("Recorded as install ")
            .host(recorded)
            .text("; the destination now reports ")
            .host(reported)
            .text(", so nothing is connected until this is decided"),
        HostStateView::IdentityUnverified { recorded } => line
            .text(
                "The host answered without an identity, so this helm cannot confirm it is still the install \
                 recorded as ",
            )
            .host(recorded),
        HostStateView::Duplicate { twin: id, identity } => {
            let line = line.text("This entry reaches install ").host(identity);
            let line = match twin(*id) {
                Some(name) => {
                    let mut line = line.text(", which the entry ");
                    line.0.push(Part::Host(name));
                    line.text(" already holds")
                }
                None => line.text(format!(", which another entry (host {id}) already holds")),
            };
            line.text(", so nothing is connected through this one")
        }
        HostStateView::Retired { reason } => {
            if reason.trim().is_empty() {
                line.text("No connection actor is running for this entry")
            } else {
                line.host(reason)
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
            .host(reason),
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
                Some(reason) => line.text(". Resume failed: ").host(reason),
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
                Some(reason) => line.text(" Start failed: ").host(reason),
                None => line,
            }
        }
    }
}

/// What to do about the phase, where waiting will not help (the old `state_remedy`). Connecting and unreachable hosts
/// get none: one is still trying and the other is re-probed forever, so telling the user to act would invent work. The
/// one exception is the helm's own machine with no supervisor, whose remedy is a command on the machine the user sits
/// at: the helm's own dial failure spells it out with the right state directory, so that is shown rather than words of
/// the app's, which cannot know the directory.
fn remedy(state: &HostStateView, twin: &impl Fn(i64) -> Option<HostText>) -> Option<Line> {
    let line = Line::default();
    match state {
        HostStateView::Unreachable { cause, last_error } if cause == LOCAL_SUPERVISOR_NOT_RUNNING => {
            Some(if last_error.trim().is_empty() {
                line.text(
                    "Start a supervisor on this machine with `farhelm supervisor run`, passing the same `--state-dir` \
                     this helm was started with if it has one",
                )
            } else {
                line.text("The helm reports: ").host(last_error)
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
            (!remediation.trim().is_empty()).then(|| line.host(remediation))
        }
        HostStateView::IdentityMismatch { .. } => Some(line.text(
            "Adopt the identity the host reports, or fix the destination; adopting resets this host to asking before \
             YOLO launches",
        )),
        HostStateView::IdentityUnverified { .. } => Some(line.text(
            "Fix the host so it reports its identity, or retarget or remove this entry — it is re-probed meanwhile, \
             so a host that starts identifying itself again recovers on its own",
        )),
        HostStateView::Duplicate { twin: id, .. } => Some(match twin(*id) {
            Some(name) => {
                let mut line = line.text("Remove ");
                line.0.push(Part::Host(name));
                line.text(" or change this entry's destination, then press Retry")
            }
            None => line.text("Remove the other entry or change this entry's destination, then press Retry"),
        }),
        HostStateView::Retired { .. } => {
            Some(line.text("Retry to start a fresh connection actor for this entry"))
        }
        HostStateView::Deleting { failure: Some(_) } => Some(line.text("Retry to delete it again")),
    }
}

/// `secs` since the epoch as `YYYY-MM-DD HH:MM:SS UTC`, the old app's unambiguous stamp (`activity::utc_stamp`): a
/// zone-qualified time needs no time zone database, which the GUI does not carry.
fn utc_stamp(secs: i64) -> String {
    const SECS_PER_DAY: i64 = 86_400;
    // Euclidean division, so a stamp before 1970 still lands on its own day rather than reflecting around the epoch.
    let (year, month, day) = farhelm_proto::time::civil_from_days(secs.div_euclid(SECS_PER_DAY));
    let of_day = secs.rem_euclid(SECS_PER_DAY);
    let (hour, minute, second) = (of_day / 3_600, (of_day % 3_600) / 60, of_day % 60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}
