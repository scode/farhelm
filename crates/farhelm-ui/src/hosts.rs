//! The hosts surface (PLAN_M6.md item 6): `HostsPanel` — one row per
//! registered host with its connection state always visible — plus the
//! renderer-free wording this UI derives from a `HostPhase`, and the
//! read-state model both this panel and the stale session view are drawn
//! from.
//!
//! ## Why the row status is always present
//!
//! SPEC.md: "Per-host connection state is always visible." Not behind a
//! menu, not only when something is wrong, and not summarized into
//! "online/offline" — because the eight phases the helm distinguishes call
//! for different responses, and collapsing them would leave a user with a
//! red dot and no idea whether to wait (`connecting`), do nothing
//! (`unreachable-reprobing` re-probes forever), upgrade a binary
//! (`version-skew`), make a decision (`identity-mismatch`), or press retry
//! (`retired`). Each row therefore keeps a phase-colored dot in the trailing
//! gutter. Connected is deliberately quiet; every other phase adds a
//! humanized word, while the stable wire token remains on `data-host-phase`
//! for automation and machine-authored diagnostics. A compatible peer with
//! an older parseable build is still connected, but gets the advisory
//! `old version` label and amber status class. On an older remote host with
//! Update available, the inline action replaces the visible words while the
//! accessible status retains them; progress takes that action's place.
//!
//! ## Peer-supplied text is displayed, never trusted to lay itself out
//!
//! Identities, build strings, transport errors and remediations all
//! originate on a machine the helm does not control — under `--ssh`, a
//! genuinely different one. Dioxus interpolation already makes them inert as
//! MARKUP (they become text nodes, never parsed HTML), so the risk left is
//! visual: a bidi override inside an identity can reorder the sentence
//! around it and make an adopt button approve one install while appearing to
//! name another. [`display_peer`] neutralizes that by escaping every
//! directional and invisible control into a visible `<U+XXXX>` form, and
//! [`DetailPart`] keeps each such value in its own direction-isolated
//! element so a strong-RTL run cannot reach past its own span. The RAW value
//! survives in exactly one place: the adopt request body, which is a
//! comparison the helm performs and not something a person reads.
//!
//! Those primitives live in `peer`, not here — this panel is their heaviest
//! user but not their owner, and the rule they enforce governs every surface
//! that mixes this UI's words with someone else's.
//!
//! ## What this module decides, and what it refuses to
//!
//! It decides how a phase READS. It never decides what a phase MEANS: the
//! version-skew remediation is the helm's sentence, printed as given, and a
//! refusal from any host verb is shown as the helm wrote it. The one place
//! that distinction bites is `identity-unverified`, which looks adjacent to
//! `identity-mismatch` and is not: there is nothing to compare and therefore
//! nothing to adopt, the helm refuses an adopt against it, and offering the
//! control anyway would be a lie about what is on the table. [`adoptable`]
//! is where that rule is enforced once instead of at each render.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dioxus::prelude::*;

use crate::api::{
    Commit, PreferenceValue, ProbeResponse, ProvisioningOperation, ProvisioningSubmission,
    adopt_host, probe_ssh_host, provision_host, remove_host, retry_host, set_alias,
    set_host_destination, set_yolo_without_asking, store_preference,
};
use crate::icons::{LocalHostIcon, RemoteHostIcon};
use crate::menu_panel::{
    self, MenuFocusQueue, MenuOpenIntent, PanelPlacement, cancel_menu_focus, clamp_title,
    closed_toggle_key_intent, focus_menu_toggle, forget_menu_focus, handle_menu_key,
    measurement_outcome, remember_menu_item, session_menu_placement_style,
    session_menu_pointer_style, should_measure_on_mount,
};
use crate::ops::{OpGuard, OpLock};
use crate::peer::{DetailPart, PeerLine, display_identity, display_peer};
use crate::provisioning::{
    ActionRequest, HostBinding, HostUpdateProgress, ProvisioningMenuState, ProvisioningPanel,
    ProvisioningTraceShape, SetupPlanConfirmation, UpdateProgressSummary, UpdateStepLine,
};
use crate::{ApiBase, Host, HostId, HostKind, HostPhase, RefreshHealth};

pub(crate) mod settings_dialog;

use settings_dialog::SettingsField;

/// Name a confirmed local host consistently across the GUI without changing its registry name.
///
/// An alias stays the name the user chose. Callers with only an unconfirmed
/// session host must pass `false`, even if its name happens to be `this machine`.
pub(crate) fn gui_host_name(name: &str, local: bool) -> String {
    if local && name == "this machine" {
        "local (this machine)".to_string()
    } else {
        display_peer(name)
    }
}

/// Name each duplicate entry's other entry, from the same host list.
///
/// The duplicate message names the entry that already holds the machine,
/// as the user knows it (SPEC.md: it says which entry holds it, by name).
/// The helm sends only that entry's id, and every host list this UI reads
/// already carries the names, so they are filled in here, once per read,
/// rather than by a second request or a change to the wire format.
pub(crate) fn name_duplicate_twins(hosts: &mut [Host]) {
    let names: std::collections::HashMap<HostId, String> = hosts
        .iter()
        .map(|host| {
            (
                host.id,
                gui_host_name(&host.name, host.kind.is_this_machine()),
            )
        })
        .collect();
    for host in hosts.iter_mut() {
        if let HostPhase::Duplicate {
            twin, twin_name, ..
        } = &mut host.state
        {
            *twin_name = names.get(twin).cloned();
        }
    }
}

// ---------------------------------------------------------------------
// The phase vocabulary
// ---------------------------------------------------------------------

/// The stable wire token for one phase.
///
/// Kept as a total match rather than a derived string so that a new phase
/// forces a deliberate decision here — the alternative, deriving the label
/// from the wire tag, would silently show `Unrecognized` as an empty token.
pub(crate) fn phase_label(state: &HostPhase) -> &'static str {
    match state {
        HostPhase::Connecting { .. } => "connecting",
        HostPhase::Unreachable { .. } => "unreachable-reprobing",
        HostPhase::Connected { .. } => "connected",
        HostPhase::VersionSkew { .. } => "version-skew",
        HostPhase::IdentityMismatch { .. } => "identity-mismatch",
        HostPhase::IdentityUnverified { .. } => "identity-unverified",
        HostPhase::Duplicate { .. } => "duplicate",
        HostPhase::Retired { .. } => "retired",
        HostPhase::Unrecognized => "unrecognized",
    }
}

/// The words this client shows for one host phase.
///
/// Wire tokens remain hyphenated because data attributes, requests, logs,
/// and helm refusals use them as stable handles. Display text has a different
/// job: it should read as ordinary prose beside a status dot. Keeping this a
/// total match makes a newly added phase choose both forms deliberately.
pub(crate) fn phase_display_label(state: &HostPhase) -> &'static str {
    if runs_newer_version(state) {
        // Before the phase's own words: a newer host is not one that needs
        // an update or runs an old version, whatever else its phase says.
        return "too new";
    }
    match state {
        HostPhase::Connecting { .. } => "connecting",
        HostPhase::Unreachable { .. } => "unreachable, retrying",
        HostPhase::Connected {
            old_version: true, ..
        } => "old version",
        HostPhase::Connected { .. } => "connected",
        HostPhase::VersionSkew { .. } => "needs update",
        HostPhase::IdentityMismatch { .. } => "identity mismatch",
        HostPhase::IdentityUnverified { .. } => "identity unverified",
        HostPhase::Duplicate { .. } => "duplicate",
        HostPhase::Retired { .. } => "retired",
        HostPhase::Unrecognized => "unrecognized",
    }
}

/// Whether this host runs a newer farhelm than the helm: a connected host
/// whose build the helm reports as newer, or a version-skewed one whose
/// protocol is the higher of the two.
///
/// Such a host is labelled "too new" rather than "needs update" or "old
/// version", and Update is not offered for it, alone or in "update all":
/// Update installs the helm's own build, which here would be a downgrade
/// (SPEC.md: Update never downgrades a host). The remedy is updating the
/// helm, which the label's hover ([`too_new_title`]) says.
pub(crate) fn runs_newer_version(state: &HostPhase) -> bool {
    match state {
        HostPhase::Connected { newer_version, .. } => *newer_version,
        HostPhase::VersionSkew {
            peer_protocol,
            our_protocol,
            ..
        } => peer_protocol > our_protocol,
        _ => false,
    }
}

/// The hover text of a "too new" label, `None` for any other host.
///
/// The label stays two words for the host list's layout; this carries the
/// rest: the host's build and protocol and the helm's, as far as the helm
/// knows them, and the remedy. A connected host speaks the helm's own
/// protocol, and the helm's build is this page's (the helm serves the page
/// it was built with). A skewed host's versions come from its refusal.
pub(crate) fn too_new_title(state: &HostPhase) -> Option<String> {
    match state {
        HostPhase::Connected {
            newer_version: true,
            build_version,
            ..
        } => Some(format!(
            "this host runs farhelm {}, which is newer than this helm's {} (both speak protocol \
             {}); update the helm",
            display_peer(build_version),
            crate::skew::CLIENT_BUILD,
            farhelm_proto::PROTOCOL_VERSION,
        )),
        HostPhase::VersionSkew {
            peer_protocol,
            peer_build,
            our_protocol,
            our_build,
            ..
        } if peer_protocol > our_protocol => Some(format!(
            "this host runs farhelm {} (protocol {peer_protocol}), which is newer than this \
             helm's {} (protocol {our_protocol}); update the helm",
            display_peer(peer_build),
            display_peer(our_build),
        )),
        _ => None,
    }
}

/// The urgency of an older peer's update, independent of whether a run can start.
///
/// A newer build wins over the old-build hint, just as it does for the status
/// label. Unknown or equal protocol versions do not establish a required update.
fn host_update_kind(state: &HostPhase) -> Option<&'static str> {
    match state {
        HostPhase::Connected {
            old_version: true,
            newer_version: false,
            ..
        } => Some("optional"),
        HostPhase::VersionSkew {
            peer_protocol,
            our_protocol,
            ..
        } if peer_protocol < our_protocol => Some("required"),
        _ => None,
    }
}

/// Explain the inline action using the versions that established its urgency.
///
/// Peer-controlled builds use the same display escaping as the too-new remedy.
/// This describes installing the helm's build, not updating to an arbitrary release.
fn host_update_title(state: &HostPhase) -> Option<String> {
    match state {
        HostPhase::Connected {
            old_version: true,
            newer_version: false,
            build_version,
            ..
        } => Some(format!(
            "this host runs farhelm {}; this helm runs {}; update optional; click to update this host to the helm's version",
            display_peer(build_version),
            crate::skew::CLIENT_BUILD,
        )),
        HostPhase::VersionSkew {
            peer_protocol,
            peer_build,
            our_protocol,
            our_build,
            ..
        } if peer_protocol < our_protocol => Some(format!(
            "this host runs farhelm {} (protocol {peer_protocol}); this helm runs {} (protocol {our_protocol}); update required; click to update this host to the helm's version",
            display_peer(peer_build),
            display_peer(our_build),
        )),
        _ => None,
    }
}

/// Share current update availability between the fleet action and inline button.
///
/// A missing menu offer is not permission to start. The live provisioning busy
/// flag closes the render gap before a panel withdraws its published offer.
fn remote_update_available(host: &Host, menu: Option<&ProvisioningMenuState>, busy: bool) -> bool {
    host.kind.updates_automatically()
        && !runs_newer_version(&host.state)
        && !busy
        && menu.is_some_and(|state| state.update)
}

/// The CSS modifier the row status carries, grouping the phases by what a
/// person watching the panel should do about them: nothing yet
/// (`connecting`), nothing at all (`unreachable-reprobing` — it re-probes
/// forever and recovers unaided), all is well (`connected`), a usable peer is
/// older (`old-version`) or newer (`too-new`, whose remedy is updating the
/// helm), or look at this (everything else, which stays exactly as it is
/// until someone acts; a skewed host stays here even when it is the newer
/// side, since it cannot be used until one side is updated).
///
/// Deliberately coarser than [`phase_label`]: color is a category signal and
/// eight colors would be noise, while the exact phase is right there in
/// words beside it. The last group is `needs-attention` rather than
/// "decide", because only the two identity states are decisions — a skew
/// wants a binary upgraded, a duplicate wants the entry edited or removed,
/// and a retired row wants a retry.
pub(crate) fn phase_class(state: &HostPhase) -> &'static str {
    match state {
        HostPhase::Connected {
            newer_version: true,
            ..
        } => "too-new",
        HostPhase::Connected {
            old_version: true, ..
        } => "old-version",
        HostPhase::Connected { .. } => "connected",
        HostPhase::Connecting { .. } => "connecting",
        HostPhase::Unreachable { .. } => "unreachable",
        HostPhase::VersionSkew { .. }
        | HostPhase::IdentityMismatch { .. }
        | HostPhase::IdentityUnverified { .. }
        | HostPhase::Duplicate { .. }
        | HostPhase::Retired { .. }
        | HostPhase::Unrecognized => "needs-attention",
    }
}

/// A fingerprint of this host's current INCARNATION — everything about it
/// that decides where a session would actually be created.
///
/// A `HostId` is a registry ROW, not a machine. The row survives every edit
/// made to it: a retarget points it at a different address, an adopt binds
/// it to a different install, and the two optional install fields decide
/// which binary and which state directory are reached at that address. So an
/// id alone is a weak identity for anything that has to mean "the same
/// target as before" — which is exactly what a create's idempotency key
/// claims (see `list::CreateSessionForm`). Keying on the id lets a retry
/// after an ambiguous failure carry the first attempt's key to a machine
/// that has never seen it, where it is not idempotent at all: a second real
/// agent, silently.
///
/// Composed from the row's fields rather than taken from any single server
/// token, because none of them means "the same target": `identity` names
/// the install but survives a pure address retarget, which has to change
/// this fingerprint. The row's `incarnation` field
/// ([`crate::Host::incarnation`]) must NOT be folded in either: it is the
/// helm's per-CONNECTION counter, which changes on every reconnect, while
/// this fingerprint has to stay the same across a reconnect to the same
/// target (that counter travels separately, as the create's
/// `expected_incarnation`).
///
/// Serialized as JSON rather than joined with a separator so no field's
/// contents can impersonate a boundary — `a|b` and `a` + `|b` are the same
/// string, and these are peer-supplied values.
///
/// Compared, never parsed or displayed.
pub(crate) fn host_incarnation(host: &Host) -> String {
    serde_json::to_string(&(
        host.id,
        &host.identity,
        &host.destination,
        &host.remote_farhelm,
        &host.remote_state_dir,
    ))
    // Infallible for this shape: integers, strings and options, no map keys
    // and no non-UTF-8 bytes for the serializer to fail on.
    .unwrap_or_default()
}

/// Whether this host is currently connected.
///
/// Deliberately NOT what the create dialog filters on — a create against a
/// non-connected host is offered and refused by the helm, naming the state
/// (see `list::CreateSessionForm`). What this decides is presentation: which
/// hosts the selector labels with their phase, and whether a session row is
/// live or last-known.
pub(crate) fn is_connected(state: &HostPhase) -> bool {
    matches!(state, HostPhase::Connected { .. })
}

/// Capture the remote updates the individual row menus would offer now.
///
/// Each request keeps the rendered row's binding, so a retarget before the
/// mounted provisioning panel consumes it cannot update a different host.
/// Rows occupied by setup or another run have no Update offer and must not
/// acquire a request that starts later when that work finishes. The parent's
/// live busy set is also checked because a panel's published menu offer can
/// lag the run-start notification by one render.
fn available_remote_updates(
    hosts: &[Host],
    menus: &HashMap<HostId, ProvisioningMenuState>,
    busy_hosts: &HashSet<HostId>,
) -> HashMap<HostId, ActionRequest> {
    hosts
        .iter()
        .filter(|host| {
            remote_update_available(host, menus.get(&host.id), busy_hosts.contains(&host.id))
        })
        .map(|host| {
            (
                host.id,
                ActionRequest {
                    operation: ProvisioningOperation::Update,
                    binding: HostBinding::from(host),
                },
            )
        })
        .collect()
}

/// The identity an adopt would approve, RAW, and `None` wherever adopting is
/// not on the table.
///
/// Only `identity-mismatch` yields one, and the value is the `reported`
/// field of the state being RENDERED — which is exactly what the adopt
/// request must carry. The helm compares it against what the host reports
/// when the request lands and answers 409 on a mismatch, so a re-probe
/// between the prompt and the click becomes a refusal the user answers by
/// looking again, rather than a silent adoption of a third install.
///
/// Raw rather than displayable, and that asymmetry is the point: the button
/// LABEL shows [`display_peer`]'s escaped form so it cannot misrepresent
/// what is being approved, while the request body carries the bytes the helm
/// will compare. Sending the escaped form would turn every identity
/// containing an unusual character into a spurious 409.
///
/// `identity-unverified` deliberately yields `None`: the host answered with
/// no identity at all, so there is nothing to adopt and the helm refuses the
/// verb outright. See the module docs.
pub(crate) fn adoptable(state: &HostPhase) -> Option<&str> {
    match state {
        HostPhase::IdentityMismatch { reported, .. } => Some(reported.as_str()),
        _ => None,
    }
}

/// The helm's stable label for the one unreachable cause a user can fix
/// with a command on the machine they are already sitting at
/// (farhelm-helm's `HostStateView::Unreachable::cause`).
///
/// Named rather than spelled out at each of its match arms: the diagnosis and
/// the remedy have to key off exactly the same string or the row would explain
/// one state while prescribing for another. It comes from `farhelm-proto`
/// because the helm writes the same constant, so the two crates cannot drift.
const LOCAL_SUPERVISOR_NOT_RUNNING: &str = farhelm_proto::http::LOCAL_SUPERVISOR_NOT_RUNNING;

/// The evidence behind a phase — the diagnosis, never the remedy (that is
/// [`state_remedy`]'s job).
///
/// Every branch renders values the helm supplied rather than restating the
/// phase in longer words: both versions on a skew, both identities on a
/// mismatch, the transport's own text when a host will not answer. A status
/// that said only "unreachable" would leave a user with nothing to search
/// for.
///
/// Returned as parts rather than a sentence so each of those values is
/// isolated where it renders — see [`DetailPart`]. The mismatch branch is
/// the one this matters most for: its two identities are the entire content
/// of a decision, and a value able to reorder the words between them could
/// make the panel recommend the opposite of what it appears to.
pub(crate) fn state_detail(state: &HostPhase) -> Vec<DetailPart> {
    match state {
        HostPhase::Connecting {
            attempt,
            last_error,
        } => match last_error {
            Some(error) => vec![
                DetailPart::text(format!(
                    "attempt {attempt} is in flight; the last one failed: "
                )),
                DetailPart::peer(error),
            ],
            None => vec![DetailPart::text(
                "the first connection attempt is in flight",
            )],
        },
        // The one unreachable cause whose evidence and whose REMEDY are the
        // same sentence: the helm's dial failure for the local row carries
        // the exact command that fixes it, so the text belongs in the remedy
        // slot ([`state_remedy`]) and this slot says only what happened.
        // Printing the whole chain in both would put one long peer string on
        // two consecutive lines, where the second is the one a user acts on.
        HostPhase::Unreachable { cause, .. } if cause == LOCAL_SUPERVISOR_NOT_RUNNING => {
            vec![DetailPart::text("no supervisor is running on this machine")]
        }
        // The transport's own words, preserved raw and escaped only where
        // they are displayed (see [`DetailPart`]). ssh's stderr is the most
        // informative thing anyone has about why a host will not answer, and
        // no classification this side could invent would beat it — so it is
        // carried unaltered, and made unable to lay out the row around it.
        HostPhase::Unreachable { last_error, .. } => {
            if last_error.trim().is_empty() {
                vec![DetailPart::text("the host did not answer")]
            } else {
                vec![DetailPart::peer(last_error)]
            }
        }
        HostPhase::Connected {
            identity,
            build_version,
            refresh,
            ..
        } => {
            let mut parts = vec![
                DetailPart::text("farhelm "),
                DetailPart::peer(build_version),
            ];
            match identity {
                Some(identity) => {
                    parts.push(DetailPart::text("; identity "));
                    parts.push(DetailPart::peer(identity));
                }
                None => parts.push(DetailPart::text("; no identity reported")),
            }
            parts.push(DetailPart::text("; "));
            parts.extend(refresh_detail(refresh));
            parts
        }
        HostPhase::VersionSkew {
            peer_protocol,
            peer_build,
            our_protocol,
            our_build,
            ..
        } => vec![
            DetailPart::text(format!(
                "the host speaks protocol {peer_protocol} (farhelm "
            )),
            DetailPart::peer(peer_build),
            DetailPart::text(format!("); this helm speaks {our_protocol} (farhelm ")),
            DetailPart::peer(our_build),
            DetailPart::text(")"),
        ],
        HostPhase::IdentityMismatch { recorded, reported } => vec![
            DetailPart::text("recorded as install "),
            DetailPart::peer(display_identity(recorded)),
            DetailPart::text("; the destination now reports "),
            DetailPart::peer(display_identity(reported)),
            DetailPart::text(", so nothing is connected until this is decided"),
        ],
        HostPhase::IdentityUnverified { recorded } => vec![
            DetailPart::text(
                "the host answered without an identity, so this helm cannot confirm it is still \
                 the install recorded as ",
            ),
            DetailPart::peer(display_identity(recorded)),
        ],
        HostPhase::Duplicate {
            twin,
            identity,
            twin_name,
        } => {
            let mut parts = vec![
                DetailPart::text("this entry reaches install "),
                DetailPart::peer(display_identity(identity)),
            ];
            match twin_name {
                Some(name) => {
                    parts.push(DetailPart::text(", which the entry "));
                    parts.push(DetailPart::peer(name.clone()));
                    parts.push(DetailPart::text(" already holds"));
                }
                None => parts.push(DetailPart::text(format!(
                    ", which another entry (host {twin}) already holds"
                ))),
            }
            parts.push(DetailPart::text(
                ", so nothing is connected through this one",
            ));
            parts
        }
        HostPhase::Retired { reason } => {
            if reason.trim().is_empty() {
                vec![DetailPart::text(
                    "no connection actor is running for this entry",
                )]
            } else {
                vec![DetailPart::peer(reason)]
            }
        }
        HostPhase::Unrecognized => vec![DetailPart::text(
            "this build does not know the state the helm is reporting for this host",
        )],
    }
}

/// How a connected host's last cache refresh went, as the tail of
/// [`state_detail`]'s sentence.
///
/// Reported beside the connection rather than folded into it, mirroring the
/// helm's own model: a failed refresh does not disconnect a host, and
/// collapsing the two would make a host that is answering perfectly well
/// read as unreachable.
fn refresh_detail(refresh: &RefreshHealth) -> Vec<DetailPart> {
    match refresh {
        RefreshHealth::Pending => vec![DetailPart::text(
            "the first session refresh is still in flight",
        )],
        RefreshHealth::Ok { sessions: 1 } => vec![DetailPart::text("1 session")],
        RefreshHealth::Ok { sessions } => vec![DetailPart::text(format!("{sessions} sessions"))],
        RefreshHealth::Failed { error } => vec![
            DetailPart::text("the last session refresh failed, so its sessions are last-known: "),
            DetailPart::peer(error),
        ],
        RefreshHealth::Unrecognized => {
            vec![DetailPart::text("refresh state unknown to this build")]
        }
    }
}

/// What to DO about a phase, where there is anything to do.
///
/// `None` is the common and correct answer: a connecting host needs
/// patience, and an unreachable one re-probes forever on its own — telling a
/// user to act on either would be inventing work. The branches that do
/// return something are the ones where waiting genuinely will not help.
///
/// The skew branch prints the helm's `remediation` VERBATIM rather than
/// composing its own sentence. That field exists because SPEC.md requires
/// errors to be actionable, the helm is the side that knows which binary is
/// behind, and a second copy of that advice here would be the one that
/// drifted. It is a `Peer` run because the helm builds it from what the far
/// end reported.
pub(crate) fn state_remedy(state: &HostPhase) -> Option<Vec<DetailPart>> {
    match state {
        // The one unreachable host whose fallback is a command on the
        // machine the user is already sitting at. The provisioning panel
        // owns the automatic offer; keeping this function manual-only lets
        // it render the same value as secondary text under that offer, or as
        // the whole remedy when the local probe says setup is unsupported.
        //
        // CONTRACT-BORNE as of PLAN_M6.md item 7, and that is a correctness
        // fix rather than a tidy-up. The helm reaches its local supervisor
        // over the socket in the state directory it was STARTED with, so a
        // bare `farhelm supervisor run` starts a supervisor that helm never
        // talks to — and the row stays exactly as it was after the user did
        // exactly what it told them. This UI cannot know that directory: it
        // is not on `/api/hosts` and never will be. The helm's own dial
        // failure already contains the answer, spelled out with the real
        // path (`farhelm_supervisor::service::connect`, whose remedy quotes
        // the state dir precisely so it survives a paste into a shell), and
        // surfacing that beats any sentence written here from a version of
        // the facts this side does not have.
        //
        // The hardcoded hint survives only as the fallback for a helm that
        // reported nothing at all — an empty `last_error` — because a remedy
        // slot with nothing in it would be worse than an approximate one.
        HostPhase::Unreachable { cause, last_error } if cause == LOCAL_SUPERVISOR_NOT_RUNNING => {
            if last_error.trim().is_empty() {
                Some(vec![DetailPart::text(
                    "start a supervisor on this machine with `farhelm supervisor run`, passing \
                     the same `--state-dir` this helm was started with if it has one",
                )])
            } else {
                Some(vec![
                    DetailPart::text("the helm reports: "),
                    DetailPart::peer(last_error),
                ])
            }
        }
        HostPhase::Unreachable { .. }
        | HostPhase::Connecting { .. }
        | HostPhase::Connected { .. }
        | HostPhase::Unrecognized => None,
        HostPhase::VersionSkew { remediation, .. } => {
            (!remediation.trim().is_empty()).then(|| vec![DetailPart::peer(remediation)])
        }
        HostPhase::IdentityMismatch { .. } => Some(vec![DetailPart::text(
            "adopt the identity the host reports, or fix the destination; adopting resets this \
             host to asking before YOLO launches",
        )]),
        // Adopt is deliberately absent from this list of remedies, because
        // it is absent from the host's options: see `adoptable`.
        HostPhase::IdentityUnverified { .. } => Some(vec![DetailPart::text(
            "fix the host so it reports its identity, or retarget or remove this entry — it is \
             re-probed meanwhile, so a host that starts identifying itself again recovers on its \
             own",
        )]),
        HostPhase::Duplicate { twin_name, .. } => Some(match twin_name {
            Some(name) => vec![
                DetailPart::text("remove "),
                DetailPart::peer(name.clone()),
                DetailPart::text(" or change this entry's destination, then press Retry"),
            ],
            None => vec![DetailPart::text(
                "remove the other entry or change this entry's destination, then press Retry",
            )],
        }),
        HostPhase::Retired { .. } => Some(vec![DetailPart::text(
            "retry to start a fresh connection actor for this entry",
        )]),
    }
}

// ---------------------------------------------------------------------
// The hosts read, as four states
// ---------------------------------------------------------------------

/// What this client currently knows about the host registry.
///
/// Four states, and each of them is something a surface has to say
/// differently: nothing read yet, a current answer, a current answer that
/// has since failed to refresh, and a failure with nothing behind it. A
/// plain `Option<Result<…>>` — which this replaced — can only express three,
/// and it expresses the wrong three: a failed poll erases the snapshot, so
/// one dropped request blanks every status on the surface SPEC.md requires to
/// always show connection state.
///
/// The two consumers then diverge deliberately, because their honesty
/// requirements differ:
///
/// - **The panel** keeps drawing the last successful snapshot and adds an
///   explicit refresh-failure line. Rows the user can still SEE, marked as
///   possibly out of date, beat an empty panel.
/// - **The stale session view** ([`HostLookup`]) refuses to present a stale
///   phase as current at all. Its whole job is to explain why a session has
///   no terminal right now, and "unreachable-reprobing (as of some earlier
///   poll)" is not an answer to that question.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct HostsRead {
    /// The last read that SUCCEEDED, retained across later failures.
    snapshot: Option<Vec<Host>>,
    /// Set when the most recent read failed, cleared by the next success.
    error: Option<String>,
}

impl HostsRead {
    /// Fold one completed read in, keeping the previous snapshot on failure.
    pub(crate) fn record(&mut self, outcome: Result<Vec<Host>, String>) {
        match outcome {
            Ok(hosts) => {
                self.snapshot = Some(hosts);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    /// The rows to draw, or `None` while no read has ever succeeded.
    pub(crate) fn hosts(&self) -> Option<&[Host]> {
        self.snapshot.as_deref()
    }

    /// The most recent read's failure, if the most recent read failed.
    pub(crate) fn refresh_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether nothing has come back at all yet — the one state where the
    /// honest thing to render is neither rows nor an error.
    pub(crate) fn is_loading(&self) -> bool {
        self.snapshot.is_none() && self.error.is_none()
    }

    /// What can be said about ONE host right now.
    ///
    /// A failed refresh outranks a held snapshot here, which is the opposite
    /// of the panel's rule and is deliberate — see the type's own docs.
    pub(crate) fn lookup(&self, host: Option<HostId>) -> HostLookup<'_> {
        if let Some(error) = &self.error {
            return HostLookup::Failed(error);
        }
        let Some(snapshot) = &self.snapshot else {
            return HostLookup::Pending;
        };
        match host.and_then(|id| snapshot.iter().find(|candidate| candidate.id == id)) {
            Some(host) => HostLookup::Known(host),
            // Covers both "the row named a host the registry no longer has"
            // and "the row named no host at all": either way this client has
            // a current registry in hand and that session's host is not in
            // it.
            None => HostLookup::Absent,
        }
    }
}

/// One host's place in [`HostsRead`], for a surface that needs a single
/// answer rather than a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostLookup<'a> {
    /// No hosts read has completed yet.
    Pending,
    /// A successful read describes it.
    Known(&'a Host),
    /// A successful read does NOT contain it: the host has been removed from
    /// the registry (or never was in it). A confirmed absence, not a gap.
    Absent,
    /// The most recent read failed, so nothing current can be said.
    Failed(&'a str),
}

/// The notice a STALE session's view is drawn behind (SPEC.md: "opening such
/// a session shows its metadata — title, directory, last-known status —
/// behind a clear host-unreachable notice").
///
/// Names the host's ACTUAL state rather than saying "unreachable" for all of
/// them, and that is the requirement rather than a nicety: a generic
/// unreachable notice over a version-skewed host would hide the one thing
/// that fixes it, and over an identity-mismatched host it would hide that a
/// DECISION is being waited on. The remedy rides along for the same reason.
///
/// The three non-`Known` lookups each get their own wording rather than
/// sharing a vague one, because they are three different situations: the
/// state has not been read yet, the state could not be refreshed (so any
/// phase this view still held would be a claim about the past dressed as the
/// present), or the host is confirmed gone from the registry — which is not
/// a connection problem at all and has a different remedy.
pub(crate) fn stale_session_notice(host_name: &str, lookup: HostLookup<'_>) -> Vec<DetailPart> {
    let mut parts = if host_name.trim().is_empty() {
        vec![DetailPart::text("this session's host")]
    } else {
        vec![DetailPart::peer(host_name)]
    };
    match lookup {
        // A CONNECTED host under a stale session is a transient disagreement
        // between two reads, not a state to explain: the session row this
        // view holds was fetched before the host came back, and the next
        // detail poll will clear the staleness. Running the generic sentence
        // here would produce "…is connected, so there is no terminal to
        // show", which is a contradiction the user cannot act on and which
        // reads as a bug in the product rather than as a moment mid-refresh.
        // `SessionView` also drives an immediate detail refresh when it sees
        // this, so the moment is as short as one round trip.
        HostLookup::Known(host) if is_connected(&host.state) => parts.push(DetailPart::text(
            " has reconnected — refreshing this session's state.",
        )),
        HostLookup::Known(host) => {
            parts.push(DetailPart::text(format!(
                ": {phase}, so there is no terminal to show — everything below is the helm's \
                 last-known record of this session. ",
                phase = phase_display_label(&host.state),
            )));
            parts.extend(state_detail(&host.state));
            if let Some(remedy) = state_remedy(&host.state) {
                parts.push(DetailPart::text(" "));
                parts.extend(remedy);
            }
        }
        HostLookup::Pending => parts.push(DetailPart::text(
            " is not connected, so there is no terminal to show — everything below is the helm's \
             last-known record of this session. Its host's current state has not been read yet.",
        )),
        HostLookup::Failed(error) => {
            parts.push(DetailPart::text(
                " is not connected, so there is no terminal to show — everything below is the \
                 helm's last-known record of this session. Its host's state could not be \
                 refreshed, so nothing current can be said about it: ",
            ));
            parts.push(DetailPart::peer(error));
        }
        HostLookup::Absent => parts.push(DetailPart::text(
            " is no longer registered with this helm, so there is no terminal to show and nothing \
             here can be operated on — everything below is the helm's last-known record of this \
             session. Re-adding the destination would reach it again.",
        )),
    }
    parts
}

// ---------------------------------------------------------------------
// The panel
// ---------------------------------------------------------------------

/// One host mutation, already built and ready to await.
///
/// Boxed because the five verbs return five different futures and the
/// bookkeeping around them (`HostsPanel`'s `run`) is identical — a generic
/// helper would have to be a function rather than a closure, and would then
/// need every signal it touches passed in by hand.
type HostRequest = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Commit, String>>>>;

/// The permanent host list: every registered host, its state, and the
/// management verbs SPEC.md's host management consists of.
///
/// ## Where the data and the in-flight state come from
///
/// `hosts` is `ListView`'s signal, not this component's: the create dialog's
/// host selector reads the same list, and two polls of `/api/hosts` for one
/// page would be a second cadence nobody chose. `on_changed` is how a
/// mutation asks for an immediate refetch instead of waiting out the poll —
/// there is nothing honest to paint optimistically, because every host verb
/// changes state this side cannot predict (an add's status is whatever the
/// connection finds; a retarget's is a fresh active-retry window).
///
/// `ops` is the page's single live-operation token (`ops::OpLock`), and it
/// excludes click-scale mutations. The two busy-host sets belong to
/// `ListView` too: one owns ordinary mutations, the other owns provisioning
/// snapshots, and rows draw their union. Keeping ownership separate is what
/// stops a quick mutation completion from erasing a still-running provision.
/// See the `ops` module for why a render-time boolean cannot replace the
/// token at submit.
///
/// ## Committed but unvalidated
///
/// Two verbs answer with a host row, and a 2xx whose body this build cannot
/// decode is NOT a refusal — the registry was written. Those surface through
/// `warnings` as a distinct line rather than through `errors`: telling a
/// user their change was rejected when it demonstrably happened is a worse
/// failure than an unread reply, and the authoritative hosts refresh fires
/// either way.
///
/// ## Why removal uses a modal
///
/// wry ships no native JS dialogs on macOS's WKWebView (observed directly
/// running the desktop build), so `window.confirm()` silently does nothing
/// there. The Dioxus modal keeps the question available in both browsers and
/// the desktop app, and its isolation keeps focus on the safe cancel answer
/// until the user chooses a removal action.
///
/// The consequence wording says FORGET rather than delete on purpose.
/// SPEC.md's contract is that removal touches nothing on the host — its
/// supervisor and every running agent carry on, and re-adding the
/// destination rediscovers all of it — so a prompt threatening deletion
/// would describe an operation this verb does not perform.
///
/// ## One row menu open, across BOTH panels
///
/// `host_menu_open`/`session_menu_open` are two signals this panel does not
/// own: `ListView` holds both (see its own doc), because
/// only the component both the session list and this panel mount underneath
/// is in a position to say "opening yours closes mine". A fully unified
/// `Option<RowMenuKey>` (one signal, tagged by which kind of row it names)
/// was the more elegant-looking alternative and was rejected: the session
/// list's existing `menu_open: Signal<Option<String>>` is read and written
/// in roughly a dozen places in `list/view.rs` — the reorder-detection
/// reconciliation in `commit_listing`, the layout-shift closer, the vanish
/// check — and folding an enum in there would touch every one of those call
/// sites for a session row's own menu, which is exactly the "must pass
/// unchanged" surface this change is not supposed to touch. Two coordinated
/// signals that close each other on open cost one extra line per toggle
/// callback and avoid converting those dozen SIGNAL consumers to a tagged
/// enum.
///
/// That is narrower than saying the session menu's own machinery went
/// untouched, and it did not: the ordering, focus, and measurement helpers
/// both rows now share moved out of the list-local module into
/// `menu_panel.rs`, and `ListView`'s own toggle and dismissal logic changed
/// to read and write `host_menu_open` alongside its existing signal. What
/// this decision actually preserved is the session menu's ACTION SET and
/// its `Signal<Option<String>>` shape — the dozen call sites above keep
/// comparing against a plain session id, never against a variant tag.
#[component]
pub(crate) fn HostsPanel(
    hosts: Signal<HostsRead>,
    mut ops: OpLock,
    mut mutation_busy_hosts: Signal<std::collections::HashSet<HostId>>,
    mut provisioning_busy_hosts: Signal<std::collections::HashSet<HostId>>,
    /// The collapsed provisioning traces that currently contribute row height.
    mut provisioning_trace_shapes: Signal<HashMap<HostId, ProvisioningTraceShape>>,
    /// Rows held open by automatic update disclosure. Each provisioning
    /// panel publishes its own row from its update lifecycle; the parent
    /// ORs this with the global checkbox per row and never writes it.
    mut provisioning_auto_details: Signal<HashSet<HostId>>,
    /// Which host row's "⋯" menu is open, if any — `ListView`'s signal, kept
    /// in step with `session_menu_open` below so at most one row menu is
    /// ever open across the whole sidebar (see this component's own doc).
    mut host_menu_open: Signal<Option<HostId>>,
    /// The session list's own open-menu signal — written (never read) here,
    /// purely to close a session row's menu when a host row's opens.
    mut session_menu_open: Signal<Option<String>>,
    on_changed: EventHandler<()>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut preferences = use_context::<crate::list::SharedPreferences>();
    // The parent owns one compact update status per row and passes it to both
    // the renderer and the provisioning child that publishes it.
    let row_update_progress = use_signal(HashMap::<HostId, HostUpdateProgress>::new);
    // Per-host rather than one shared slot, the discipline `ListView` keeps
    // for its session errors and for the same reason: a retry failing on one
    // host must not blank out an adopt refusal on another that the user has
    // not read yet.
    let mut errors = use_signal(HashMap::<HostId, String>::new);
    // Committed-but-unvalidated replies, kept apart from `errors` because
    // they mean the opposite thing — see this component's docs.
    let mut warnings = use_signal(HashMap::<HostId, String>::new);
    // Which host's removal dialog is open. The modal is panel-owned so a
    // refresh cannot tear down its focus or explanation mid-question.
    let mut remove_open = use_signal(|| None::<HostId>);
    // Which field of which host the settings dialog is editing, if any. One
    // at a time, and only inside the open dialog.
    let mut editing = use_signal(|| None::<(HostId, EditField)>);
    // The host whose settings dialog is open, if any. One at a time: the
    // dialog is modal.
    let mut settings_open = use_signal(|| None::<HostId>);
    // Per host, which setting the dialog last wrote, so that write's outcome
    // renders under that setting instead of on the row. `errors` and
    // `warnings` are per host, not per field, and one of them can be left
    // over from a retry or an adopt; this is what tells the dialog whether a
    // host's current line belongs to one of its own settings, and which.
    // `run` drops a host's entry at the start of every write to that host and
    // the dialog's writes put it back once theirs has started, so an entry
    // always names the write that produced that host's current line. Kept
    // per host so a write to one host never disowns another's line, and kept
    // across closing and reopening the dialog, so a save refused after the
    // user closed it shows under its field again when they come back.
    let mut dialog_write = use_signal(HashMap::<HostId, SettingsField>::new);
    let mut destination_draft = use_signal(String::new);
    let mut adding = use_signal(|| false);
    // The global disclosure is the user's preference: one checkbox for every
    // row. It stays client-local and resets on page mount; a host refresh
    // changes facts, not the user's chosen level of detail. Each row's
    // EFFECTIVE disclosure ORs this with its automatic update disclosure,
    // which the provisioning panels publish per row and which never writes
    // this checkbox.
    let mut details_open = use_signal(|| false);
    // Provisioning stays mounted in each row, while its commands render in
    // the row-owned floating menu. These maps are the narrow bridge between
    // those two render locations: summaries flow out, one-shot requests in.
    let provisioning_menu_states = use_signal(HashMap::<HostId, ProvisioningMenuState>::new);
    let mut provisioning_action_requests = use_signal(HashMap::<HostId, ActionRequest>::new);
    // An add that committed with an unreadable reply, which has no row to
    // sit on — see the form's `on_added`.
    let mut add_warning = use_signal(|| None::<String>);
    // A setup submission the helm refused or that failed in transport. The
    // add dialog has already closed by then (see `on_add_submit`), so the
    // panel is the only place left to say so; opening the dialog again
    // clears it, the same way a fresh attempt clears an in-dialog error.
    let mut add_error = use_signal(|| None::<String>);

    // Closes BOTH row-menu signals every time the add form mounts,
    // unmounts, or (via `on_added` setting `adding` back to `false`)
    // commits: `AddHostForm` sits ABOVE `.host-list` in the rsx below, so
    // toggling it moves the vertical position of every host row beneath it.
    // Any row menu open at that instant is a `position: fixed` panel
    // measured at its toggle's OLD coordinates (see `menu_panel_style`),
    // and this component's own signals are exactly what a click on either
    // menu's items still carries the ORIGINAL host or session id inside —
    // so a stale panel here is not merely misplaced, it is a control aimed
    // at whichever row visually slides underneath it, including this
    // panel's own destructive `remove`.
    //
    // `ListView` closes both signals for layout changes it owns — see its
    // own effect's doc — but `adding` is this component's private state,
    // invisible there. That is why this needs a second, narrower effect.
    use_effect(move || {
        adding();
        host_menu_open.set(None);
        session_menu_open.set(None);
    });

    // One shared shape for the ordinary host-row mutations: claim the page's
    // operation token, clear this host's stale lines, run the request, then
    // ask for a refetch or record what came back — and release the token
    // whatever happened. Written once because the only thing that differs
    // between them is the request, which is why it arrives already built as
    // a boxed future rather than as several copies of this bookkeeping.
    //
    // The claim is the exclusion and it happens HERE, synchronously, inside
    // the handler: the buttons' `disabled` attributes only take effect after
    // a rerender, so a second click queued inside the same frame reaches
    // this function with the page still looking idle to anything computed
    // during render.
    //
    // Returns whether the request was actually STARTED, which the settings
    // dialog's writes act on: only a write that started gets its outcome
    // attributed to a setting (`dialog_write`). A refused start leaves the
    // host's existing line, if any, belonging to whatever produced it.
    let mut run = move |host: HostId, request: HostRequest| -> bool {
        if provisioning_busy_hosts.peek().contains(&host) {
            return false;
        }
        // The guard, not a bare claim, because this panel can unmount
        // with the request still in flight (the browser's token prompt
        // replaces the page when another request is refused), and that
        // drops the task below before it reaches any release of its own.
        // Moved into the task, the guard releases the page lock either way;
        // a bare claim stranded it, and every lock-gated control on the
        // page then refused silently until a reload.
        let Some(claim) = ops.claim_guard() else {
            return false;
        };
        mutation_busy_hosts.write().insert(host);
        errors.write().remove(&host);
        warnings.write().remove(&host);
        // Every write starts unattributed; the settings dialog's own writes
        // claim their outcome right after this returns (see `dialog_write`),
        // so a retry, adopt, or remove never has its refusal shown under a
        // setting the user did not touch.
        dialog_write.write().remove(&host);
        spawn(async move {
            match request.await {
                Ok(Commit::Confirmed) => on_changed.call(()),
                // Committed, unreadable. The refresh still fires — it is the
                // authoritative answer to what happened — and the decode
                // problem is reported as its own thing.
                Ok(Commit::Unvalidated(warning)) => {
                    warnings.write().insert(host, warning);
                    on_changed.call(());
                }
                Err(error) => {
                    errors.write().insert(host, error);
                }
            }
            mutation_busy_hosts.write().remove(&host);
            // Naming the guard here is what moves it into this task, so the
            // lock is held until the request settles and is still released
            // if the task is dropped (see the claim above). Without this
            // line the guard would drop when `run` returns, freeing the lock
            // mid-request. A leaked lock, the opposite failure, leaves the
            // whole page inert with nothing on screen to explain why.
            drop(claim);
        });
        true
    };

    let retry_base = base.clone();
    let on_retry = move |host: HostId| {
        // Closes the menu unconditionally, before the request is even
        // built: a retry that lost the race to an in-flight operation is
        // already a no-op with nothing to undo (see the comment on the
        // ignored `run` outcome below), but the user still chose an action
        // from the menu, and leaving the panel open over whatever the row
        // renders next — including a refusal this same click could produce
        // — would hide it behind the very panel that triggered it. See
        // `on_adopt` just below for the identical reasoning.
        host_menu_open.set(None);
        details_open.set(true);
        let base = retry_base.clone();
        // The started/refused answer is ignored here and in the two verbs
        // below: their controls simply stay as they are, so a click that
        // lost the race to an in-flight operation is already a no-op with
        // nothing to undo.
        // Retry and adopt answer with an empty object, so a 200 is the whole
        // answer and there is nothing for a decode to fail on.
        run(
            host,
            Box::pin(async move { retry_host(&base, host).await.map(|()| Commit::Confirmed) }),
        );
    };

    let adopt_base = base.clone();
    // The `reported` identity travels from the RENDERED state to the
    // request untouched — never re-read from a fresher poll, and never the
    // escaped form the button displays — because that is the whole content
    // of the promise the helm checks (see `api::adopt_host`).
    let on_adopt = move |(host, reported): (HostId, String)| {
        // See `on_retry`'s own comment: the menu closes on the choice
        // itself, not on the request's outcome, so an adopt refused because
        // the identity changed again renders its refusal where the user can
        // actually see it.
        host_menu_open.set(None);
        details_open.set(true);
        let base = adopt_base.clone();
        run(
            host,
            Box::pin(async move {
                adopt_host(&base, host, &reported)
                    .await
                    .map(|()| Commit::Confirmed)
            }),
        );
    };

    let remove_base = base.clone();
    let on_remove_confirm = use_callback(move |(host, skip_future): (HostId, bool)| {
        if *remove_open.peek() != Some(host) {
            return;
        }
        remove_open.set(None);
        if skip_future {
            preferences.0.write().skip_host_remove_confirmation = Some(true);
            store_preference(&remove_base, PreferenceValue::HostRemoveConfirmation(true));
        }
        let base = remove_base.clone();
        run(
            host,
            Box::pin(async move { remove_host(&base, host).await.map(|()| Commit::Confirmed) }),
        );
    });

    let edit_base = base.clone();
    let on_edit_submit = move |(host, field, value): (HostId, EditField, String)| {
        let submission = match resolve_edit_submission(field, &value) {
            Ok(submission) => submission,
            Err(error) => {
                // The local syntax check's refusal is this field's too.
                errors.write().insert(host, error);
                dialog_write
                    .write()
                    .insert(host, SettingsField::from(field));
                return;
            }
        };
        let base = edit_base.clone();
        // The field closes only once the helm has accepted the change, and
        // only if it is still the field this submit came from. A refusal
        // leaves it open with the draft as typed and the helm's reason under
        // it, so the user can correct the value rather than retype it. A
        // second submit while this one is out is not a risk: the editor's
        // controls are disabled for as long as the request holds the page's
        // operation token.
        let started = run(
            host,
            Box::pin(async move {
                let outcome = match submission {
                    EditSubmission::Destination(destination) => {
                        set_host_destination(&base, host, &destination).await
                    }
                    EditSubmission::Alias(alias) => set_alias(&base, host, alias).await,
                };
                if outcome.is_ok() && *editing.peek() == Some((host, field)) {
                    editing.set(None);
                }
                outcome
            }),
        );
        if started {
            dialog_write
                .write()
                .insert(host, SettingsField::from(field));
        }
    };

    // Flip whether YOLO launches are allowed on a host. Runs through the same
    // `run` wrapper as the other registry writes, so it takes the page's
    // operation token, lands its outcome in this host's error line (shown
    // under the checkbox, see `dialog_write`), and refreshes the list the
    // checkbox renders from.
    let yolo_base = base.clone();
    let on_yolo_without_asking = move |(host, yolo_without_asking): (HostId, bool)| {
        let base = yolo_base.clone();
        let started = run(
            host,
            Box::pin(
                async move { set_yolo_without_asking(&base, host, yolo_without_asking).await },
            ),
        );
        if started {
            dialog_write
                .write()
                .insert(host, SettingsField::YoloWithoutAsking);
        } else {
            settings_dialog::reset_yolo_checkbox();
        }
    };

    // ----- The settings dialog's handlers -------------------------------
    //
    // The dialog renders from this panel, not from a row, because a host
    // mutation landing rebuilds the rows (see `HostDestinationForm`'s doc):
    // a dialog, a half-typed draft, or an editor's focus owned by a row would
    // be torn down by the very refresh its own save causes.

    // Open one field's editor inside the dialog, prefilled with its current
    // value. Refused, and nothing changes, while another operation holds the
    // page's token or this host is provisioning: the editor's save could not
    // run anyway.
    let on_edit_start = move |(id, field, value): (HostId, EditField, String)| {
        if ops.busy_now() || provisioning_busy_hosts.peek().contains(&id) {
            return;
        }
        destination_draft.set(value);
        editing.set(Some((id, field)));
    };
    let on_edit_cancel = move |_: ()| editing.set(None);
    // Closing the dialog abandons an open field edit with it: the draft is
    // not a setting until it is saved. Focus goes back to the host row's "⋯"
    // toggle, the control the dialog was opened from, once the dialog's
    // isolation has released it (the toggle is inert until then; see
    // `modal_isolation`).
    let on_settings_close = move |_: ()| {
        let Some(id) = *settings_open.peek() else {
            return;
        };
        settings_dialog::return_focus_to_row(id);
        editing.set(None);
        settings_open.set(None);
    };
    // A host that disappears from the list while its dialog is open (another
    // client removed it) takes the dialog with it; there is nothing left for
    // its settings to change. Only a list that was actually read counts: a
    // failed or pending refresh says nothing about whether the host exists.
    use_effect(move || {
        let Some(id) = *settings_open.read() else {
            return;
        };
        let gone = hosts
            .read()
            .hosts()
            .is_some_and(|list| !list.iter().any(|host| host.id == id));
        if gone {
            editing.set(None);
            settings_open.set(None);
        }
    });

    let read = hosts.read();
    let rendered_hosts = read.hosts().map(|list| {
        list.iter()
            .cloned()
            .map(|host| {
                let local_setup = host.kind.sets_up_locally()
                    && matches!(
                        &host.state,
                        HostPhase::Unreachable { cause, .. }
                            if cause == LOCAL_SUPERVISOR_NOT_RUNNING
                    );
                (host, local_setup)
            })
            .collect::<Vec<_>>()
    });
    let update_all_available = read.hosts().is_some_and(|list| {
        !available_remote_updates(
            list,
            &provisioning_menu_states.read(),
            &provisioning_busy_hosts.read(),
        )
        .is_empty()
    });
    // Cosmetic, not the guard — every handler below claims the token for
    // itself (see the `ops` module).
    let busy = ops.busy();

    // The add dialog's setup submission, run in THIS component's scope
    // rather than the dialog's. A confirmed setup closes the dialog at once
    // (a modal must never hold the user behind a pending request), and a
    // task spawned by the dialog would be dropped with it, stranding the
    // reply. The page token travels in with the submission: the dialog
    // claimed it synchronously in the confirming handler, and dropping it
    // when the POST completes is what re-enables the heading's add button
    // and every other page mutation.
    let add_submit_base = base.clone();
    let on_add_submit = move |submission: AddSubmission| {
        let AddSubmission { probe_id, claim } = submission;
        settings_dialog::return_focus_to_add_button();
        adding.set(false);
        add_error.set(None);
        add_warning.set(None);
        let base = add_submit_base.clone();
        spawn(async move {
            let result = provision_host(&base, &probe_id).await;
            drop(claim);
            match result {
                Ok(ProvisioningSubmission::Accepted(_)) => {}
                Ok(ProvisioningSubmission::Unvalidated(warning)) => add_warning.set(Some(warning)),
                Err(problem) => add_error.set(Some(problem.into_text())),
            }
            // Every outcome refreshes: an accepted add registered a row whose
            // run the row itself follows, and a refused or ambiguous one may
            // still have committed (the probe id is one-use either way).
            on_changed.call(());
        });
    };
    rsx! {
        section { class: "hosts-panel",
            div { class: "hosts-heading",
                // Number and label in separate spans so the stylesheet can
                // draw this as a section heading, label first ("HOSTS 3"),
                // the same way the session list's count is drawn. Number
                // first in the DOM and the space kept inside the label: the
                // element's text content stays "3 hosts", which is what a
                // screen reader announces and what the browser suite
                // asserts. Before the first read there is no number, and the
                // bare word is the whole heading.
                div { class: "host-count",
                    if let Some(hosts) = read.hosts() {
                        span { class: "heading-count", "{hosts.len()}" }
                        span { class: "heading-label", if hosts.len() == 1 { " host" } else { " hosts" } }
                    } else {
                        span { class: "heading-label", "hosts" }
                    }
                }
                label { class: "host-details-control",
                    input {
                    r#type: "checkbox",
                    class: "host-details-toggle",
                    checked: details_open(),
                    onchange: move |event| {
                        // Automatic setup may reveal details before that
                        // render reaches the checkbox. Honor the native
                        // requested state instead of inverting a newer signal.
                        details_open.set(event.checked());
                        // Every row changes height together. A fixed menu is
                        // measured once, so it cannot survive that reflow
                        // with trustworthy geometry.
                        host_menu_open.set(None);
                        session_menu_open.set(None);
                                    },
                    }
                    "details"
                }
                button {
                    r#type: "button",
                    class: "btn btn-neutral update-all-button",
                    disabled: !update_all_available,
                    onclick: move |_| {
                        // Read the latest snapshot at activation, since the
                        // rendered eligibility may already be one frame old.
                        // Each panel and the helm still revalidate its own
                        // binding and one-use plan before a host is touched.
                        let requests = {
                            let snapshot = hosts.peek();
                            let menus = provisioning_menu_states.peek();
                            let busy_hosts = provisioning_busy_hosts.peek();
                            snapshot
                                .hosts()
                                .map(|list| available_remote_updates(list, &menus, &busy_hosts))
                                .unwrap_or_default()
                        };
                        if requests.is_empty() {
                            return;
                        }
                        host_menu_open.set(None);
                        session_menu_open.set(None);
                        provisioning_action_requests.write().extend(requests);
                    },
                    "update all"
                }
                button {
                    r#type: "button",
                    class: "btn btn-neutral add-host-button",
                    // The compact heading spends only the visible word
                    // "add"; assistive technology keeps the object that
                    // action adds, matching the form and existing callers.
                    aria_label: "add host",
                    // Opening the add dialog waits out any live MUTATION,
                    // including the previous add's setup POST, which this
                    // panel runs after that dialog closed (`on_add_submit`):
                    // a second add started under it would only reach a
                    // confirm the token refuses. Reads are not what the token
                    // covers — the page reads constantly and none of those
                    // care whether the dialog exists. While the dialog is
                    // open this button sits behind its modal, so closing is
                    // the dialog's own cancel. The token is read
                    // synchronously in the handler for the same reason every
                    // other guard here is — the attribute is one render
                    // behind.
                    disabled: busy,
                    onclick: move |_| {
                        if ops.busy_now() {
                            return;
                        }
                        let open = adding();
                        if !open {
                            add_error.set(None);
                        }
                        adding.set(!open);
                    },
                    "add"
                }
            }
            if adding() {
                AddHostForm {
                    ops,
                    on_refresh: on_changed,
                    on_submit: on_add_submit,
                    on_cancel: move |_| {
                        settings_dialog::return_focus_to_add_button();
                        adding.set(false);
                    },
                    on_added: move |unvalidated: Option<String>| {
                        settings_dialog::return_focus_to_add_button();
                        adding.set(false);
                        // A committed-but-unreadable add has no row id to
                        // hang a warning on — the reply that would have
                        // named it is the thing that failed to decode — so
                        // it goes on the panel itself, next to the list the
                        // refresh is about to repaint.
                        add_warning.set(unvalidated);
                        on_changed.call(());
                    },
                }
            }
            if let Some(warning) = add_warning() {
                PeerLine {
                    class: "host-warning add-host-warning",
                    parts: vec![DetailPart::Peer(warning)],
                }
            }
            if let Some(error) = add_error() {
                PeerLine {
                    class: "host-error add-host-error",
                    parts: probe_error_parts(error),
                }
            }
            // Two different failures, said differently, decided by whether a
            // snapshot exists. A FIRST load that failed has nothing behind
            // it and must say so plainly; a REFRESH that failed leaves rows
            // on screen that are still worth showing (SPEC.md requires
            // connection state to be visible, and a dropped poll is not
            // evidence that anything changed) but must not let them pass for
            // current. Collapsing the two into one sentence would either
            // promise a "last state read" that does not exist, or describe a
            // populated panel as empty.
            if let Some(error) = read.refresh_error() {
                if read.hosts().is_some() {
                    div { class: "status error hosts-refresh-error",
                        "showing the last state this client read; the refresh failed: {error}"
                    }
                } else {
                    div { class: "status error hosts-load-error",
                        "the hosts list could not be loaded: {error}"
                    }
                }
            }
            if read.is_loading() {
                div { class: "status hosts-status", "loading hosts…" }
            }
            if let Some(list) = rendered_hosts {
                div { class: "host-list",
                    for (host, local_setup) in list {
                        HostRow {
                            key: "{host.id}",
                            controls: HostRowControls {
                                menu_open: *host_menu_open.read() == Some(host.id),
                            },
                            // While this host's settings dialog is open, its
                            // error or warning line shows in the dialog:
                            // under the setting a dialog write changed, or at
                            // the top when something else (a retry, an adopt)
                            // produced it. The row, hidden under the
                            // backdrop, stops showing it meanwhile, so one
                            // line is never on screen twice; it reappears
                            // here once the dialog closes.
                            activity: HostRowActivity {
                                busy: mutation_busy_hosts.read().contains(&host.id)
                                    || provisioning_busy_hosts.read().contains(&host.id)
                                    || busy,
                                error: (*settings_open.read() != Some(host.id))
                                    .then(|| errors.read().get(&host.id).cloned())
                                    .flatten(),
                                warning: (*settings_open.read() != Some(host.id))
                                    .then(|| warnings.read().get(&host.id).cloned())
                                    .flatten(),
                            },
                            details_open: details_open()
                                || provisioning_auto_details.read().contains(&host.id),
                            update_progress: row_update_progress.read().get(&host.id).cloned(),
                            provisioning_menu: provisioning_menu_states
                                .read()
                                .get(&host.id)
                                .copied()
                                .unwrap_or_default(),
                            remote_update_available: remote_update_available(
                                &host,
                                provisioning_menu_states.read().get(&host.id),
                                provisioning_busy_hosts.read().contains(&host.id),
                            ),
                            local_setup,
                            provisioning_section: rsx! {
                                ProvisioningPanel {
                                    host: host.clone(),
                                    ops,
                                    details_open: details_open()
                                        || provisioning_auto_details.read().contains(&host.id),
                                    local_setup,
                                    manual_remedy: state_remedy(&host.state),
                                    action_requests: provisioning_action_requests,
                                    menu_states: provisioning_menu_states,
                                    trace_shapes: provisioning_trace_shapes,
                                    auto_details: provisioning_auto_details,
                                    row_update_progress,
                                    on_reveal_details: move |_| {
                                                                        details_open.set(true);
                                        host_menu_open.set(None);
                                        session_menu_open.set(None);
                                    },
                                    on_running: {
                                        let id = host.id;
                                        move |running: bool| {
                                            if running {
                                                provisioning_busy_hosts.write().insert(id);
                                            } else {
                                                provisioning_busy_hosts.write().remove(&id);
                                            }
                                        }
                                    },
                                    on_changed,
                                }
                            },
                            on_retry: on_retry.clone(),
                            on_adopt: on_adopt.clone(),
                            on_settings_start: move |id: HostId| {
                                if ops.busy_now()
                                    || provisioning_busy_hosts.peek().contains(&id)
                                {
                                    return;
                                }
                                // This is the ONE place that closes the menu
                                // for the settings item: the item's own
                                // click in `HostRow` only requests the
                                // dialog, so there is one state change to
                                // account for rather than two. It runs only
                                // past the guard above, so a request refused
                                // because another operation is in flight
                                // leaves the menu exactly as it was.
                                editing.set(None);
                                host_menu_open.set(None);
                                settings_open.set(Some(id));
                            },
                            on_remove_start: move |id: HostId| {
                                if ops.busy_now()
                                    || provisioning_busy_hosts.peek().contains(&id)
                                {
                                    return;
                                }
                                editing.set(None);
                                settings_open.set(None);
                                // See `on_settings_start` just above: the
                                // same single-owner close, past the same
                                // guard.
                                host_menu_open.set(None);
                                if preferences.0.peek().skip_host_remove_confirmation == Some(true) {
                                    remove_open.set(Some(id));
                                    on_remove_confirm.call((id, false));
                                } else {
                                    remove_open.set(Some(id));
                                }
                            },
                            on_provisioning: move |(id, request): (HostId, ActionRequest)| {
                                if provisioning_busy_hosts.peek().contains(&id) {
                                    return;
                                }
                                // Setup still confirms explicitly, and its
                                // confirmation must be visible: reveal the
                                // global disclosure past the page lock, as
                                // before. Update progress stays in its row's
                                // status spot, and planning never consults the
                                // lock — so neither the reveal nor the lock
                                // check applies to it here.
                                if request.operation == ProvisioningOperation::Add {
                                    if ops.busy_now() {
                                        return;
                                    }
                                    details_open.set(true);
                                }
                                host_menu_open.set(None);
                                provisioning_action_requests.write().insert(id, request);
                            },
                            on_menu_toggle: move |id: HostId| {
                                let currently = *host_menu_open.peek() == Some(id);
                                host_menu_open.set(if currently { None } else { Some(id) });
                                // Opening a host row's menu must close
                                // whichever session row's menu is open —
                                // see this component's own "one row menu
                                // open, across BOTH panels" doc.
                                if !currently {
                                    session_menu_open.set(None);
                                                            }
                            },
                            host,
                        }
                    }
                }
            }
            // Looked up in the list this render already read, so the dialog
            // always shows the host's current settings (a save that lands
            // re-renders it with the new value) and simply does not render
            // for a host that is gone; the effect above then closes it.
            if let Some(host) = settings_open
                .read()
                .and_then(|id| read.hosts()?.iter().find(|host| host.id == id).cloned())
            {
                settings_dialog::HostSettingsDialog {
                    busy: mutation_busy_hosts.read().contains(&host.id)
                        || provisioning_busy_hosts.read().contains(&host.id)
                        || busy,
                    editing: editing
                        .read()
                        .and_then(|(id, field)| (id == host.id).then_some(field)),
                    outcome: settings_dialog::field_outcome(
                        dialog_write.read().get(&host.id).copied(),
                        errors.read().get(&host.id).cloned(),
                        warnings.read().get(&host.id).cloned(),
                    ),
                    draft: destination_draft,
                    on_edit_start,
                    on_edit_submit: on_edit_submit.clone(),
                    on_edit_cancel,
                    on_yolo_without_asking: on_yolo_without_asking.clone(),
                    on_close: on_settings_close,
                    host,
                }
            }
            if let Some(host) = remove_open
                .read()
                .and_then(|id| read.hosts()?.iter().find(|host| host.id == id).cloned())
            {
                settings_dialog::HostRemoveDialog {
                    busy: mutation_busy_hosts.read().contains(&host.id)
                        || provisioning_busy_hosts.read().contains(&host.id)
                        || busy,
                    host: host.clone(),
                    on_remove: {
                        let handler = on_remove_confirm;
                        move |skip_future| {
                            // Copy the id out before calling: an `if let`
                            // over `*remove_open.peek()` keeps the read guard
                            // alive through its body, and the handler's
                            // `remove_open.set(None)` then panics on the
                            // still-borrowed signal, which a release wasm
                            // build reports only as `unreachable`.
                            let open = *remove_open.peek();
                            if let Some(id) = open {
                                handler.call((id, skip_future));
                            }
                        }
                    },
                    on_cancel: move |_| remove_open.set(None),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// One host row
// ---------------------------------------------------------------------

// ===== The row's "⋯" menu ============================================
//
// TODO.md's near-term entry this section closes: `edit destination` and
// `remove` used to sit on `.host-row-main` as ordinary flex children
// beside `adopt`/`retry`/`profiles`, and on an ssh host the five of them
// together ran wider than the 340px sidebar leaves room for — `remove`
// rendered clipped off the right edge by `.app-sidebar`'s
// `overflow: hidden auto`, invisible and unclickable, with nothing in the
// DOM or in Playwright's `toBeVisible` to notice. Folding every verb but
// the always-visible name/status into one "⋯" menu — built the same way as
// the session row's (PR #239, mechanics shared via `menu_panel`) — leaves
// `.host-row-main` exactly three children regardless of host kind, so
// there is no longer a control count for the sidebar's width to run out
// on. Profiles now live beside New in the session list header, but the remaining host verbs
// keep this menu because the same narrow-sidebar constraint still applies.

/// One command in a host row's actions menu, in the order the menu offers
/// them.
///
/// `Retry` is offered in every phase, like the button it replaces; `Adopt`
/// only when [`adoptable`] names an identity; provisioning commands mirror
/// the permanently mounted provisioning component's current offers;
/// `Settings` is offered on every host, because whether YOLO launches need a
/// confirmation is a setting of every host, the local one included (what the
/// settings dialog offers beyond that, a destination and an alias, it decides
/// itself); and `Remove` only appears on an ssh row (see `HostRow`'s own doc
/// for why an unmanageable kind cannot be removed). The separator before
/// `Remove` is drawn in the rsx, not modeled here — see
/// `MenuOrder` in `menu_panel` for why a separator is never counted as an
/// item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HostMenuAction {
    Retry,
    Adopt,
    Rerun,
    AutomaticSetup,
    Update,
    Settings,
    Remove,
}

/// Draw the compact line icon used beside a host command.
///
/// Host and session menus share the same visual language, but their action
/// enums are intentionally different. Keeping this small renderer beside
/// the host enum lets the host menu reuse the session menu's CSS contract
/// without coupling host lifecycle names to session lifecycle code.
#[component]
fn HostMenuActionIcon(action: HostMenuAction) -> Element {
    rsx! {
        svg {
            class: "session-row-menu-icon",
            view_box: "0 0 14 14",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.2",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            match action {
                HostMenuAction::Retry => rsx! { path { d: "M11.5 5.5 A4.6 4.6 0 0 0 3 4.5 M2.5 8.5 A4.6 4.6 0 0 0 11 9.5 M11.8 2.5 V5.8 H8.6 M2.2 11.5 V8.2 H5.4" } },
                HostMenuAction::Adopt => rsx! { path { d: "M2 7 H12 M7 2 V12" } circle { cx: "7", cy: "7", r: "4.5" } },
                HostMenuAction::Rerun => rsx! { path { d: "M11.5 5.5 A4.6 4.6 0 0 0 3 4.5 M2.5 8.5 A4.6 4.6 0 0 0 11 9.5 M11.8 2.5 V5.8 H8.6 M2.2 11.5 V8.2 H5.4" } },
                HostMenuAction::AutomaticSetup => rsx! { path { d: "M7 1.8 L8.3 5.5 L12.2 6.8 L8.3 8.2 L7 12 L5.7 8.2 L1.8 6.8 L5.7 5.5 Z" } },
                HostMenuAction::Update => rsx! { path { d: "M7 2 V10 M4 5 L7 2 L10 5 M3 11.5 H11" } },
                HostMenuAction::Settings => rsx! { path { d: "M5.2 2.2 H8.8 L9.3 4.1 L11 5.1 L12.7 4.2 L14 6.5 L12.5 7.8 V9.8 L14 11.1 L12.7 13.4 L11 12.5 L9.3 13.5 H5.2 L4.7 11.6 L3 10.6 L1.3 11.5 L0 9.2 L1.5 7.9 V5.9 L0 4.6 L1.3 2.3 L3 3.2 L4.7 2.2 Z" } circle { cx: "7", cy: "7.9", r: "1.7" } },
                HostMenuAction::Remove => rsx! { path { d: "M2.5 4 H11.5 M5.5 4 V2.5 H8.5 V4 M3.8 4 L4.5 12 H9.5 L10.2 4" } },
            }
        }
    }
}

/// Every action a host row's menu can offer, in the order it offers them —
/// the host row's counterpart to `list::row`'s `MENU_ACTIONS`, and for the
/// identical reason: the canonical order lives in one place so the
/// rendered list and the navigable list cannot disagree about what "the
/// first item" or "the last item" means.
const HOST_MENU_ACTIONS: [HostMenuAction; 7] = [
    HostMenuAction::Retry,
    HostMenuAction::Adopt,
    HostMenuAction::Rerun,
    HostMenuAction::AutomaticSetup,
    HostMenuAction::Update,
    HostMenuAction::Settings,
    HostMenuAction::Remove,
];

/// One render's host-menu item list — this row's instantiation of the
/// shared, generic `menu_panel::MenuOrder` (see that type's own doc for
/// the packing rule and for why the mechanics live there rather than
/// being copied from the session row). The const generic argument is
/// `HOST_MENU_ACTIONS`'s own length rather than a restated literal, so the
/// array stays the single source of truth for this menu's capacity.
type HostMenuOrder = menu_panel::MenuOrder<HostMenuAction, { HOST_MENU_ACTIONS.len() }>;

/// Handles for this row's mounted menu items — the host row's
/// instantiation of `menu_panel::MenuItemHandles`.
type HostMenuItemHandles = menu_panel::MenuItemHandles<HostMenuAction>;

/// This row's menu wiring, bound to [`HostMenuAction`] and to [`HostId`]
/// (a plain `i64`, unlike the session row's `String` id) — see
/// `menu_panel::MenuWiring`'s own doc for what it bundles.
type HostMenuWiring = menu_panel::MenuWiring<HostMenuAction, HostId, { HOST_MENU_ACTIONS.len() }>;

/// Builds this render's host-menu item list from the row's own state: the
/// bridge between `adoptable`/`manageable`'s booleans and the shared
/// `MenuOrder::pack`'s generic `(action) -> bool` predicate.
fn host_menu_order(
    adoptable: bool,
    manageable: bool,
    provisioning: ProvisioningMenuState,
) -> HostMenuOrder {
    HostMenuOrder::pack(HOST_MENU_ACTIONS, |action| match action {
        HostMenuAction::Retry => true,
        HostMenuAction::Adopt => adoptable,
        HostMenuAction::Rerun => provisioning.rerun.is_some(),
        HostMenuAction::AutomaticSetup => provisioning.automatic_setup,
        HostMenuAction::Update => provisioning.update,
        HostMenuAction::Remove => manageable,
        // Every host has settings: the YOLO-launch setting applies to the
        // local row and to a kind this build does not recognize alike. What
        // else the dialog offers (destination, alias) is decided inside it.
        HostMenuAction::Settings => true,
    })
}

/// Which host field the settings dialog's text editor is changing. Keeping the
/// selection with the edit state makes submit, cancel, and error handling
/// identical while preserving the different API fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditField {
    Destination,
    Alias,
}

/// Match the helm's alias boundary before a request leaves the browser.
///
/// The helm remains authoritative for collisions, but local syntax failures
/// should preserve the draft and avoid a needless round trip.
fn validate_alias_draft(value: &str) -> Result<Option<String>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().any(char::is_control) {
        return Err("host alias must not contain control characters".to_string());
    }
    if value.chars().count() > 64 {
        return Err("host alias must be at most 64 characters".to_string());
    }
    Ok(Some(value.to_string()))
}

/// One inline-editor submission, resolved to the shape its own API call
/// needs — a raw destination string, or a canonicalized (possibly absent)
/// alias.
///
/// Exists so `on_edit_submit`'s dispatch (`match submission { ... }`) is a
/// plain move into an already-decided value rather than a second place that
/// has to re-derive which field is live.
#[derive(Debug, Clone, PartialEq, Eq)]
enum EditSubmission {
    Destination(String),
    Alias(Option<String>),
}

/// The `EditField` switch itself: decide what a submitted draft means before
/// any network call is made.
///
/// Pulled out of the submit handler as a pure function so the branch is
/// unit-testable without an async runtime or a rendered component — the
/// handler's only job past this point is picking the matching API call. The
/// destination field is NOT canonicalized here: the helm is authoritative
/// for ssh syntax (`validate_alias_draft`'s own doc makes the parallel
/// argument for aliases), so a destination draft passes through verbatim
/// and only an alias draft is checked client-side.
fn resolve_edit_submission(field: EditField, value: &str) -> Result<EditSubmission, String> {
    match field {
        EditField::Destination => Ok(EditSubmission::Destination(value.to_string())),
        EditField::Alias => validate_alias_draft(value).map(EditSubmission::Alias),
    }
}

/// The host row menu toggle's accessible name: the host's display name (or
/// ssh destination), escaped and clamped — the host row's counterpart to
/// `list::row::menu_label`, built on the same shared [`clamp_title`]
/// (`menu_panel::clamp_title`) so both rows' accessible names clamp
/// identically.
///
/// Run through [`display_peer`] BEFORE clamping, never after: `name` is
/// peer-supplied (an ssh destination, and under `--ssh` a value the remote
/// end chose), and this label names a menu whose commands include Adopt,
/// Edit, and Remove — a live bidi override or zero-width run here would let
/// assistive technology announce a different host than the row visibly
/// shows, exactly the hazard `display_peer` exists to close everywhere else
/// this value renders. `clamp_title`'s own escape-token safety is what
/// keeps clamping that escaped form from ever cutting a `<U+XXXX>` token in
/// half.
///
/// Named "display name", not "identity": in this codebase IDENTITY is the
/// recorded/reported value [`adoptable`] compares, a distinct thing from
/// the name or destination a menu happens to be labeled with.
fn host_menu_label(name: &str) -> String {
    format!("host actions for {}", clamp_title(&display_peer(name)))
}

/// Split the host-menu header into app-authored separators and peer-owned
/// values so bidi and invisible controls cannot reorder the summary.
fn host_menu_summary_parts(host: &Host) -> Vec<DetailPart> {
    let destination = host.destination.as_deref().unwrap_or("local host");
    let version = match &host.state {
        HostPhase::Connected { build_version, .. } => build_version.as_str(),
        HostPhase::VersionSkew { peer_build, .. } => peer_build.as_str(),
        _ => host.remote_farhelm.as_deref().unwrap_or("not connected"),
    };
    vec![
        DetailPart::peer(destination),
        DetailPart::text(" · Farhelm "),
        DetailPart::peer(version),
    ]
}

/// Build the native tooltip from isolated header runs without allowing peer
/// text to reorder the app-authored separator.
fn host_menu_summary_tooltip(parts: &[DetailPart]) -> String {
    parts
        .iter()
        .map(|part| match part {
            DetailPart::Text(text) => text.clone(),
            DetailPart::Peer(value) => display_peer(value),
        })
        .collect()
}

/// The host row's class list for its one independent visual state beyond
/// its own phase status — the host row's counterpart to
/// `list::row::row_class`, narrower because a host row has neither a
/// `stale` nor a `selected` concept of its own.
fn host_row_class(menu_open: bool) -> &'static str {
    if menu_open {
        "host-row menu-open"
    } else {
        "host-row"
    }
}

/// Which of the host row's optional surfaces the user has opened.
///
/// Grouped because this answers which row-local control surface is open, and
/// because the `host_menu_open` `ListView` keeps at most one row menu open
/// across both the session list and hosts panel. Dialog state remains owned by
/// `HostsPanel`, where it can apply modal isolation and focus restoration.
///
/// State only, like every group here — see [`HostRowActivity`] for why no
/// group may ever carry a callback.
#[derive(Clone, PartialEq, Eq)]
struct HostRowControls {
    /// Whether this row's "⋯" menu is the (at most one, across sessions AND
    /// hosts) open one.
    menu_open: bool,
}

/// The operation-related presentation state of this row: whether its
/// controls are currently disabled, and what the last ordinary mutation left
/// behind.
///
/// Not one lifecycle, and the grouping does not claim one: `busy` is CURRENT
/// disablement, raised by this host's own mutation or provisioning run or by
/// any other row holding the page's operation token, while `error` and
/// `warning` are RETAINED, mutually exclusive outcomes of the last ordinary
/// mutation (transport and authentication failures land in `error` too, not
/// only helm-authored refusals; a confirmed success clears both). They are
/// grouped because they are what the row shows about operations, full stop.
/// The row derives none of them — the panel owns the busy sets and the two
/// per-host maps, and reduces them per row before rendering.
///
/// STATE ONLY. Every `EventHandler` stays a direct prop on [`HostRow`], and
/// no struct here may ever gain one. A struct literal in
/// `rsx!` is where an inline closure naturally gets written, and a handler
/// built fresh each render never compares equal to the last one — so the
/// props compare unequal on every parent render and every row repaints on
/// every fleet refresh, with none of the in-place handler update Dioxus gives
/// a direct `EventHandler` prop. The session list's `RowActions` was withdrawn
/// mid-review for exactly this (lore/PLAN_M7.md item 5), leaving
/// `list::shared::RowState` as the state-only half; this grouping copies the
/// shape that survived. The rule and its real boundary are measured in
/// `grouping_a_callback_is_safe_only_while_its_handle_is_stable` below.
#[derive(Clone, PartialEq, Eq)]
struct HostRowActivity {
    /// Whether this row's controls are disabled because something is in
    /// flight — this host's own mutation or provisioning run, or the page's
    /// operation token held by any other row.
    busy: bool,
    /// The last verb's REFUSAL, as the helm wrote it, or `None` when the last
    /// one succeeded or none has run yet.
    error: Option<String>,
    /// A verb that committed but whose reply this build could not decode.
    /// Kept apart from `error` because it means the opposite thing: the
    /// change HAPPENED and only its confirmation was unreadable.
    warning: Option<String>,
}

#[cfg(test)]
std::thread_local! {
    // How often the real `HostRow` ran, per host id, for the memoization
    // regressions in this module's tests. Per id rather than a single total
    // because a total cannot say WHICH rows rendered — the invalidation test
    // needs to prove the changed rows ran and the unchanged ones did not.
    // Thread-local because each Dioxus virtual DOM is single-threaded while
    // the Rust test harness runs tests concurrently — the session row's
    // counter (`list::row`) is thread-local for the same reason.
    static HOST_ROW_RENDERS: std::cell::RefCell<std::collections::BTreeMap<HostId, usize>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// The per-host render counts as a vector in id order, for assertions.
#[cfg(test)]
fn host_row_renders() -> Vec<(HostId, usize)> {
    HOST_ROW_RENDERS.with(|renders| renders.borrow().iter().map(|(id, n)| (*id, *n)).collect())
}

/// Render one live update's inline progress inside the row that owns it.
///
/// The one-second clock's future belongs to this keyed component: when the
/// run completes, is replaced, or the row unmounts, Dioxus drops the task
/// instead of leaving a page-wide interval behind.
///
/// Each part is its own flex item because the status spot has to fit the
/// fixed-width sidebar beside the host name and the always-present `⋯`
/// toggle (see `.host-status.updating` in `app.css`). Only the step name may
/// give up width: the `N/M` count and the clock are what the user reads at a
/// glance, so they never truncate. The visible text has no `updating:`
/// prefix, because in a sidebar that narrow the prefix was most of the room
/// the step name needed. The word survives as visually hidden text, since it
/// is the only thing telling assistive technology what the bare count means.
/// The leading spaces in the text nodes are for the text content (what
/// assistive technology and tests read); the visible spacing comes from the
/// flex gap, since a flex item's leading space collapses.
///
/// ## The hover popup
///
/// Hovering anywhere on the label shows [`UpdateProgressPopup`] with the
/// count, the run's whole step list with the untruncated current step
/// highlighted, and the clock. It replaces the step
/// span's native `title`, which only covered the step and only appeared
/// after the browser's own delay, and it must not be clipped by the
/// sidebar: `.app-sidebar` scrolls and is positioned, so no absolutely
/// positioned descendant can paint past its edge. The popup is therefore
/// `position: fixed`, anchored to the label's viewport rectangle as
/// measured on `pointerenter`. It is decoration for pointer users only: it
/// takes no pointer events, cannot be focused, and is hidden from assistive
/// technology, which already reads the full text from the label itself.
///
/// The rectangle is read once per hover, not tracked. Scrolling the sidebar
/// under a stationary pointer can leave the popup a few pixels off until the
/// next hover; that was judged not worth a scroll listener for a transient,
/// non-interactive popup.
#[component]
fn UpdateProgressLabel(summary: UpdateProgressSummary) -> Element {
    let started_at = summary.started_at;
    let mut elapsed = use_signal(|| started_at.elapsed().as_secs());
    use_future(move || async move {
        loop {
            crate::reader::sleep_ms(1_000).await;
            elapsed.set(started_at.elapsed().as_secs());
        }
    });
    let elapsed_text = format_elapsed(elapsed());
    let mut anchor = use_signal(|| None::<Rc<MountedData>>);
    // Whether the pointer is over the label right now. Kept apart from the
    // measured position because the measurement is asynchronous: a pointer
    // that leaves before the rectangle arrives must not have the popup
    // appear behind it.
    let mut hovered = use_signal(|| false);
    // The label's viewport rectangle and the viewport height, measured on
    // `pointerenter`; the popup places itself from these and its own
    // rendered height.
    let mut popup_origin = use_signal(|| None::<PopupOrigin>);

    rsx! {
        span {
            class: "host-status-label host-update-running",
            onmounted: move |element| anchor.set(Some(element.data())),
            onpointerenter: move |_| async move {
                hovered.set(true);
                let Some(handle) = anchor.peek().clone() else {
                    return;
                };
                let Ok(rect) = handle.get_client_rect().await else {
                    return;
                };
                // The label's rect alone cannot say whether the popup fits
                // below it; the viewport height decides that, and it is not
                // part of `MountedData`. A failed read falls back to
                // "below", the old behavior, rather than hiding the popup.
                let viewport_height = document::eval("return window.innerHeight;")
                    .join::<f64>()
                    .await
                    .ok();
                if *hovered.peek() {
                    popup_origin.set(Some(PopupOrigin {
                        label: rect,
                        viewport_height,
                    }));
                }
            },
            onpointerleave: move |_| {
                hovered.set(false);
                popup_origin.set(None);
            },
            UpdateProgressDot {}
            span { class: "host-update-count",
                span { class: "visually-hidden", "updating: " }
                "{summary.done}/{summary.total}"
            }
            if let Some(step) = summary.current_step.clone() {
                span { class: "host-update-step", " {step}" }
            }
            span { class: "host-update-elapsed", "aria-hidden": "true", " {elapsed_text}" }
            if let Some(origin) = popup_origin() {
                UpdateProgressPopup {
                    origin,
                    done: summary.done,
                    total: summary.total,
                    steps: summary.steps.clone(),
                    elapsed: elapsed_text.clone(),
                }
            }
        }
    }
}

/// What [`UpdateProgressPopup`] places itself against: the label's viewport
/// rectangle and the viewport's height (`None` when it could not be read),
/// both measured when the pointer entered the label.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PopupOrigin {
    label: dioxus::html::geometry::PixelsRect,
    viewport_height: Option<f64>,
}

/// Where [`UpdateProgressPopup`] is pinned, in viewport pixels.
///
/// The popup hangs below the label unless that would run it off the bottom
/// of the viewport, in which case it sits above the label, and when it fits
/// on neither side it is pinned inside the viewport over the label. The
/// decision uses the popup's own rendered height, measured once it has
/// mounted: it lists every step of the run, so its height varies with the
/// run and with font metrics no estimate here could track. A
/// fixed-position box escapes the sidebar's clipping but not the viewport's,
/// and with the native `title` gone this popup is the only place a truncated
/// step name can be read, so a popup below the fold would hide it outright.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PopupAnchor {
    /// `left`/`top` of the popup's top-left corner.
    Below { left: f64, top: f64 },
    /// `left`, and `bottom` measured up from the viewport's bottom edge.
    Above { left: f64, bottom: f64 },
    /// `left`/`top` of a popup that fits neither below nor above the label,
    /// held inside the viewport instead, covering the label if it must. The
    /// popup takes no pointer events, so covering the label cannot end the
    /// hover, and it repeats everything the label says.
    Pinned { left: f64, top: f64 },
}

impl PopupAnchor {
    /// Gap between the label and the popup, so the popup reads as hanging
    /// off the label rather than overlapping it.
    const GAP_PX: f64 = 4.0;

    /// Closest a pinned popup comes to the viewport's edges.
    const EDGE_PX: f64 = 8.0;

    /// Left-aligned with the label: below it when a popup `reserve` pixels
    /// tall fits there, above it when it fits there instead, and otherwise
    /// pinned as low in the viewport as `reserve` allows (never above
    /// [`Self::EDGE_PX`]). A run's
    /// full step list can outgrow both sides of a label in a short window,
    /// and a popup hanging past either edge hides the steps it exists to
    /// show. No horizontal clamp: the sidebar is the leftmost column, so
    /// there is always the main pane's width to the right for the popup to
    /// spill into. An unknown viewport height keeps the popup below.
    fn beside(
        rect: &dioxus::html::geometry::PixelsRect,
        viewport_height: Option<f64>,
        reserve: f64,
    ) -> Self {
        let left = rect.min_x();
        let top = rect.max_y() + Self::GAP_PX;
        let Some(height) = viewport_height else {
            return Self::Below { left, top };
        };
        if top + reserve <= height {
            Self::Below { left, top }
        } else if reserve <= rect.min_y() - Self::GAP_PX {
            Self::Above {
                left,
                bottom: height - rect.min_y() + Self::GAP_PX,
            }
        } else {
            Self::Pinned {
                left,
                top: (height - reserve - Self::EDGE_PX).max(Self::EDGE_PX),
            }
        }
    }

    /// The inline position for the popup's style. Both vertical properties
    /// are spelled out, the unused one as `auto`: the popup mounts with a
    /// hidden `top` before it is placed, and a style string that merely
    /// omits a property leaves the old value on the element, which for an
    /// `Above` popup would pin both edges and collapse it.
    fn style(self) -> String {
        match self {
            Self::Below { left, top } | Self::Pinned { left, top } => {
                format!("left: {left}px; top: {top}px; bottom: auto;")
            }
            Self::Above { left, bottom } => {
                format!("left: {left}px; top: auto; bottom: {bottom}px;")
            }
        }
    }
}

/// The complete state of a running update, for the pointer hover on
/// [`UpdateProgressLabel`]: the count, every step of the run with the one in
/// progress highlighted, and the clock.
///
/// A user who hovers wants the detail the narrow inline label cannot hold,
/// so this lists the run's whole step list, the same steps and statuses the
/// expanded row's `provisioning-steps` list shows (and styled with its
/// classes), minus the step messages. The step in progress carries
/// `data-current` and its name keeps the `host-update-popup-step` class, the
/// untruncated copy of the name the inline label may have cut short.
///
/// It mounts hidden below the label, measures its own height, and only then
/// takes the placement [`PopupAnchor::beside`] chooses for that height, so it
/// never shows in a spot that runs off the viewport. A failed measurement
/// leaves it hidden rather than placed blind.
///
/// The count and the clock come first and never shrink; only the step list
/// gives up height. In a window too short for the whole list, the list is
/// scrolled so the step in progress stays in view: the popup takes no
/// pointer events, so the user cannot scroll it, and the current step is
/// the one line it must not lose.
#[component]
fn UpdateProgressPopup(
    origin: PopupOrigin,
    done: usize,
    total: usize,
    steps: Vec<UpdateStepLine>,
    elapsed: String,
) -> Element {
    let mut placed = use_signal(|| None::<PopupAnchor>);
    // The mounted element, stored the same way the label stores its own;
    // the effect below measures it once it exists.
    let mut own = use_signal(|| None::<Rc<MountedData>>);
    use_effect(move || {
        let Some(handle) = own() else {
            return;
        };
        spawn(async move {
            if let Ok(rect) = handle.get_client_rect().await {
                placed.set(Some(PopupAnchor::beside(
                    &origin.label,
                    origin.viewport_height,
                    rect.height(),
                )));
            }
        });
    });
    // Once placed (and so at its final height), bring the current step into
    // the list's own view. A no-op when the whole list fits.
    use_effect(move || {
        if placed().is_some() {
            document::eval(
                "const list = document.querySelector('.host-update-popup .host-update-popup-steps');
                 const step = list && list.querySelector('[data-current]');
                 if (list && step) {
                   list.scrollTop = step.offsetTop - list.offsetTop
                     - Math.max(0, (list.clientHeight - step.offsetHeight) / 2);
                 }",
            );
        }
    });
    // `visibility` is spelled out in both states: dropping a style property
    // from the string does not reliably clear it from the element, so a
    // placed popup says `visible` rather than merely not saying `hidden`.
    let style = match placed() {
        Some(at) => format!("{} visibility: visible;", at.style()),
        None => format!(
            "visibility: hidden; left: {}px; top: {}px;",
            origin.label.min_x(),
            origin.label.max_y() + PopupAnchor::GAP_PX,
        ),
    };
    rsx! {
        span {
            class: "host-update-popup",
            "aria-hidden": "true",
            style,
            onmounted: move |element| own.set(Some(element.data())),
            span { "updating: {done} of {total} steps done" }
            span { class: "host-update-popup-elapsed", "{elapsed} elapsed" }
            ol { class: "provisioning-steps host-update-popup-steps",
                for step in steps {
                    li {
                        class: "provisioning-step",
                        "data-step": "{step.name}",
                        "data-status": "{step.status}",
                        "data-current": if step.current { "true" },
                        span {
                            class: if step.current {
                                "provisioning-step-name host-update-popup-step"
                            } else {
                                "provisioning-step-name"
                            },
                            "{step.name}"
                        }
                        span { class: "provisioning-step-status", "{step.status}" }
                    }
                }
            }
        }
    }
}

/// A small status-color dot that uses the app's existing reduced-motion-aware pulse.
#[component]
fn UpdateProgressDot() -> Element {
    rsx! { span { class: "host-update-dot", "aria-hidden": "true" } }
}

/// Keep short elapsed times readable without capping long-running updates.
fn format_elapsed(seconds: u64) -> String {
    let hours = seconds / 3_600;
    let minutes = (seconds / 60) % 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// One host's row: name, state, the evidence, the remedy, and whichever
/// controls that state actually offers.
///
/// The controls are state-driven rather than uniform, which is the point:
/// `adopt` appears only where there is an identity to adopt (never on
/// `identity-unverified` — see [`adoptable`]), and `remove` appears only on
/// ssh rows, because the reserved local row cannot be removed at all (the
/// helm refuses with a 409). A row of an unrecognized KIND is treated as
/// unmanageable for the same reason: offering a verb the helm would refuse
/// teaches the user something false about what is possible. The same KIND
/// split decides whether the settings dialog offers a destination to edit;
/// the alias editor there is gated instead by whether this helm build's
/// reply carries an `alias` field at all (`Host.alias`'s outer option), and
/// is offered on every kind, local row included. See `settings_dialog`.
///
/// `retry` is offered in every state, connected included. It costs one
/// attempt, and the state it is most needed in — `retired`, whose actor is
/// gone — is precisely the one where nothing else brings the host back.
///
/// The host NAME is peer/user-supplied text (an ssh destination, or the
/// locally-typed alias replacing it) and is rendered through the same
/// isolation every other such value gets: a destination or an alias able to
/// reorder the row around it could make a remove button appear to belong to
/// a different host than it does.
///
/// `data-host-id`/`data-host-phase`/`data-host-kind` are the browser suite's
/// handles, on the wrapper rather than on the status so a test can find a row
/// and then assert about anything inside it.
///
/// ## Prop shape
///
/// Provisioning pushed this signature past the standing "regroup once props
/// are actively growing" condition the session row was held to
/// (lore/PLAN_M7.md item 5). The derived per-row STATE is therefore grouped
/// into [`HostRowControls`] and [`HostRowActivity`] — split by what changes
/// together, one for what the user has opened and one for what the helm is
/// doing about it, so that a change to either says which.
///
/// Everything else stays a direct prop, each for its own reason. The event
/// handlers cover host mutations, provisioning, and local UI transitions;
/// they must stay stable for the memoization boundary described above. The two
/// `Element` sections are rendered markup rather than state, and the panel
/// builds them. The draft is a `Signal` handle the destination form writes
/// through, not a value to compare. And `local_setup` is a fact about the HOST
/// — the one unreachable cause with an automatic remedy — derived by the panel
/// because the provisioning section it also builds needs the same answer, so
/// it belongs beside `host` rather than inside a group describing what the row
/// is doing.
///
/// ## The menu, and what stays outside it
///
/// `retry`/`adopt`, the currently truthful provisioning commands,
/// `settings`, and `remove` render inside one "⋯" menu (`.host-row-menu`
/// toggle, `.host-row-menu-panel` panel) built on the same generic mechanics
/// the session row's menu uses (`menu_panel`) — see that module's own doc for
/// what is shared and why. The name, phase status, and muted toggle stay on
/// the row line. `settings` opens the host settings dialog, which `HostsPanel`
/// renders (see `settings_dialog`): the destination and alias editors live
/// there, not in the row. Removal opens a modal dialog owned by
/// `HostsPanel`; while it is open the row remains behind the modal and cannot
/// receive a competing command.
///
/// Every actionable item closes the menu when chosen. Setup commands
/// additionally open the global details disclosure before sending their
/// one-shot request to the permanently mounted provisioning component, so
/// planning feedback and confirmation never appear invisibly. An update
/// needs no confirmation and leaves details alone: it reports through this
/// row's status spot, and only a failure or uncertain outcome opens the row.
///
/// `settings` and `remove` both take over from the menu (modal dialogs), so
/// closing is a correctness requirement:
/// cancelling either flow must not silently revive a menu the user never
/// asked to reopen. `HostsPanel`'s own `on_settings_start`/
/// `on_remove_start` are where that close happens, past their own busy
/// guard — the item's click here only REQUESTS the flow, so there is one
/// state change to account for rather than the item and the panel each
/// closing it.
///
/// `retry` and `adopt` do not replace the row's branch. They close the menu
/// from `HostsPanel`'s own callbacks because either can produce a refusal in
/// the row's error line, exactly where an opaque open panel would cover it.
#[component]
fn HostRow(
    host: Host,
    /// Whether automatic local setup replaces the ordinary remedy slot.
    local_setup: bool,
    /// Which control surface this row is showing: its menu (grouped state).
    /// Host settings and removal are not among them: they live
    /// in `HostsPanel`'s dialog, not in the row.
    controls: HostRowControls,
    /// What the management verbs are doing to this row (grouped state).
    activity: HostRowActivity,
    /// This row's effective details disclosure: the global checkbox OR the
    /// row's automatic update disclosure, ORed by the parent.
    details_open: bool,
    /// The provisioning panel's compact status for a running update, if any.
    update_progress: Option<HostUpdateProgress>,
    /// Provisioning commands currently offered in this row's menu.
    provisioning_menu: ProvisioningMenuState,
    /// The shared update eligibility, including the parent's live provisioning
    /// busy flag; unrelated page operations do not disable update planning.
    remote_update_available: bool,
    /// The feed-driven setup/update surface built by the panel.
    provisioning_section: Element,
    on_retry: EventHandler<HostId>,
    on_adopt: EventHandler<(HostId, String)>,
    /// Open this host's settings dialog (the menu's Settings item).
    on_settings_start: EventHandler<HostId>,
    on_remove_start: EventHandler<HostId>,
    /// Route a provisioning menu command back to this row's permanently
    /// mounted provisioning component. The request carries the binding this
    /// row rendered with, captured at click time.
    on_provisioning: EventHandler<(HostId, ActionRequest)>,
    /// Open or close THIS row's "⋯" menu — `HostsPanel`'s toggle callback,
    /// built the same way the session row's `on_menu_toggle` is (see
    /// `HostsPanel`'s own doc for the single-open discipline it keeps).
    on_menu_toggle: EventHandler<HostId>,
) -> Element {
    let HostRowControls { menu_open } = controls;
    let HostRowActivity {
        busy,
        error,
        warning,
    } = activity;
    #[cfg(test)]
    HOST_ROW_RENDERS.with(|renders| *renders.borrow_mut().entry(host.id).or_insert(0) += 1);
    let id = host.id;
    let update_is_active = update_progress.is_some();
    let inline_update = (remote_update_available && !update_is_active)
        .then(|| host_update_kind(&host.state))
        .flatten();
    // The binding this row rendered with, captured for provisioning clicks:
    // a request queued behind a retarget must not become work against the
    // retargeted row, so each click site below clones this into its own
    // request rather than sharing one.
    let click_binding = HostBinding::from(&host);
    // The local row is not management surface: SPEC.md has it always
    // present, never registered, never removed. An unrecognized kind is not
    // management surface either — see this component's docs.
    let manageable = host.kind.is_manageable();
    let kind_attribute = match host.kind {
        HostKind::Local => "local",
        HostKind::Ssh => "ssh",
        HostKind::Unrecognized => "unrecognized",
    };
    // Raw for the request, escaped for the label — the asymmetry `adoptable`
    // documents.
    let adopt_identity = adoptable(&host.state).map(str::to_string);
    let adopt_label = adopt_identity
        .as_deref()
        .map(|reported| format!("adopt {}", display_identity(reported)));
    let remedy = state_remedy(&host.state);
    let detail = state_detail(&host.state);
    let shown_name = gui_host_name(&host.name, host.kind.is_this_machine());
    // Keep peer-owned destination and version values isolated in the same
    // `PeerLine` contract used by the session menu. The menu header is a
    // compact identity aid, so it follows the live phase when a peer has
    // reported a build and falls back to the cached value otherwise.
    let menu_summary_parts = host_menu_summary_parts(&host);
    // This render's menu item list — see `host_menu_order`'s own doc. Read
    // every render, not only while the menu is open, because the `use_effect`
    // below has to notice an item withdrawn (a poll turning `adoptable` off)
    // even while a menu built against the wider list is still up.
    let adoptable_now = adopt_identity.is_some();
    let menu_order = host_menu_order(adoptable_now, manageable, provisioning_menu);
    let has_provisioning_menu = provisioning_menu.rerun.is_some()
        || provisioning_menu.automatic_setup
        || provisioning_menu.update;
    // Setup still refuses behind the page lock, so its items stay disabled
    // while another operation holds it. Update planning mutates nothing and
    // its submission claim retries reactively, so the update item answers
    // while busy and only a live provisioning lifecycle disables it.
    let setup_disabled = busy || provisioning_menu.planning;
    let update_disabled = provisioning_menu.planning;
    // A failed UPDATE reruns down the automatic path, so it answers while
    // busy like a fresh update; a failed ADD keeps setup's lock discipline.
    let rerun_disabled = provisioning_menu
        .rerun
        .is_some_and(|operation| match operation {
            ProvisioningOperation::Update => update_disabled,
            ProvisioningOperation::Add => setup_disabled,
        });

    // ===== This row's own "⋯" menu state ================================
    //
    // Mirrors `list::row::SessionRow`'s menu apparatus field for field —
    // see that component's own docs for what each signal means and why it
    // is shaped this way; only the names below are host-specific. Row-local
    // (not `ListView`'s or `HostsPanel`'s business): the PARENT decides only
    // WHETHER this row's menu is open (`controls.menu_open`), never where
    // its panel is measured to be or which of its items currently has
    // keyboard focus.
    let mut toggle_handle = use_signal(|| None::<Rc<MountedData>>);
    let placement = use_signal(|| PanelPlacement::Unmeasured);
    let mut item_handles: HostMenuItemHandles = use_signal(HashMap::new);
    let mut menu_focus = use_signal(|| None::<usize>);
    // The last position a keyboard step asked focus to move TO — see
    // `MenuWiring::requested`'s own doc for why this has to exist
    // separately from `menu_focus` (F5/COR-FOCUS-BURST follow-up: an older
    // in-flight focus request's `onfocusin` can land after a newer press
    // already moved `menu_focus` on, and only a signal DOM events never
    // touch survives that). Cleared alongside `menu_focus` wherever the
    // menu opens or closes, below, and wherever the toggle takes focus
    // (`forget_menu_focus`), and NOT reconciled against a mid-open
    // item-set change the way `menu_focus` is: `next_menu_focus`'s existing
    // out-of-range handling already treats a stale index as "not on an
    // item" and re-enters at an end, the same tolerance a stale
    // `event_origin` already relies on, so a request left pointing at a
    // withdrawn action's old slot degrades no worse than that.
    let mut menu_requested = use_signal(|| None::<usize>);
    // The order `menu_focus`'s stored position was last recorded against —
    // seeded from THIS render's own list, so the first run of the
    // reconciliation effect below (on mount) compares a list against
    // itself and correctly finds nothing to reconcile. Updated at the end
    // of that same effect, never anywhere else: this is bookkeeping for one
    // consumer, not a value any other part of the row should read.
    let mut previous_menu_order = use_signal(|| menu_order);
    let mut open_intent = use_signal(|| None::<MenuOpenIntent>);
    let focus_queue = MenuFocusQueue {
        target: use_signal(|| None::<Rc<MountedData>>),
        draining: use_signal(|| false),
    };
    let open_generation = use_signal(|| 0_u64);
    let spawn_measurement = move || {
        let handle = toggle_handle;
        let mut placement = placement;
        let generation = open_generation();
        spawn(async move {
            let measured = match handle.peek().clone() {
                Some(handle) => handle.get_client_rect().await.ok(),
                None => None,
            };
            if let Some(outcome) =
                measurement_outcome(generation, *open_generation.peek(), measured)
            {
                placement.set(outcome);
            }
        });
    };
    let begin_open = move |intent: MenuOpenIntent| {
        let mut open_generation = open_generation;
        let mut placement = placement;
        let mut item_handles = item_handles;
        let mut menu_focus = menu_focus;
        let mut menu_requested = menu_requested;
        let mut open_intent = open_intent;
        open_generation += 1;
        placement.set(PanelPlacement::Unmeasured);
        item_handles.write().clear();
        cancel_menu_focus(focus_queue);
        menu_focus.set(None);
        menu_requested.set(None);
        open_intent.set(Some(intent));
        spawn_measurement();
    };
    let menu_tab_stop = menu_focus()
        .and_then(|position| menu_order.get(position))
        .or_else(|| menu_order.get(0));
    let menu_wiring: HostMenuWiring = menu_panel::MenuWiring {
        order: menu_order,
        handles: item_handles,
        focus: focus_queue,
        focused: menu_focus,
        requested: menu_requested,
        open_intent,
        close_menu: on_menu_toggle,
    };
    // The item set can change UNDER an open menu exactly the way the
    // session row's can: a poll landing while the menu is open can flip
    // `adoptable` (a successful adopt resolves the mismatch, or a retry
    // discovers the recorded identity again — an ordinary background
    // re-probe does NOT, since `IdentityMismatch` is frozen until a user
    // decision resolves it; see `adopt_is_offered_only_for_an_identity_mismatch`
    // and the state's own doc), which is the host row's version of the
    // session row's conditional-action hazard — see
    // that component's own `use_effect` for the stale-handle reasoning this
    // mirrors exactly.
    //
    // Stale FOCUS, though, is reconciled by ACTION identity rather than by
    // comparing the stored position against the new list's length: an
    // action withdrawn from the MIDDLE of the list (Adopt, here) shifts
    // every later action's index down, so the slot Adopt vacates is
    // immediately reoccupied by Settings — a numeric length check never
    // notices that, and would leave the row believing Settings was focused
    // while the browser had already dropped focus off the removed Adopt
    // button, stranding arrow keys and Escape. See
    // `menu_panel::reconcile_menu_focus`'s own doc for the general rule
    // this applies.
    use_effect(use_reactive(
        (&adoptable_now, &manageable, &provisioning_menu),
        move |(adoptable, manageable, provisioning_menu)| {
            let order = host_menu_order(adoptable, manageable, provisioning_menu);
            item_handles
                .write()
                .retain(|action, _| order.position(*action).is_some());
            let focused_position = *menu_focus.peek();
            // `menu_open` is this render's own belief about whether THIS
            // row's menu is the open one — passed through so
            // `reconcile_menu_focus` can gate `Withdrawn` on it
            // (F3/COR-HOST-WITHDRAWAL-REOPEN): `on_menu_toggle` below is an
            // ordinary click TOGGLE, not an idempotent close, and calling
            // it when some OTHER dismissal (a layout closer, a newer
            // session-menu choice) has already closed this row's menu
            // since this prop was computed would reopen it instead.
            match menu_panel::reconcile_menu_focus(
                *previous_menu_order.peek(),
                order,
                focused_position,
                menu_open,
            ) {
                menu_panel::MenuFocusReconciliation::Unchanged => {}
                menu_panel::MenuFocusReconciliation::Moved(position) => {
                    menu_focus.set(Some(position));
                }
                // No surviving item to aim focus at. Left as-is rather than
                // cleared here: closing through the row's own toggle
                // callback is what the dismissal effect below keys its
                // focus-return on (`was_inside`), and clearing `menu_focus`
                // first would make that check see nothing to return focus
                // FROM. Only ever reached while `menu_open` is true (see
                // the call above), so this toggle call is always a genuine
                // close of THIS row's own open menu, never a reopen.
                menu_panel::MenuFocusReconciliation::Withdrawn => {
                    on_menu_toggle.call(id);
                }
            }
            previous_menu_order.set(order);
        },
    ));
    // The dismissal teardown — see `SessionRow`'s own effect for the
    // reasoning behind every line; only the DOM marker and toggle selector
    // handed to `focus_menu_toggle` are host-specific.
    let dismiss_id = id;
    use_effect(use_reactive((&menu_open,), move |(menu_open,)| {
        if menu_open {
            return;
        }
        cancel_menu_focus(focus_queue);
        let was_inside = menu_focus.peek().is_some();
        menu_focus.set(None);
        menu_requested.set(None);
        open_intent.set(None);
        item_handles.write().clear();
        if was_inside {
            focus_menu_toggle("data-host-id", &dismiss_id.to_string(), ".host-row-menu");
        }
    }));

    rsx! {
        div {
            class: host_row_class(menu_open),
            "data-host-id": "{id}",
            "data-host-phase": "{phase_label(&host.state)}",
            "data-host-kind": "{kind_attribute}",
            div { class: "host-row-main",
                // The same locality glyph the session row draws
                // (2026-09-03) — not asked for by the TODO entry that
                // introduced it, but two renderings of "this is a remote
                // host" in two different vocabularies (a session row's
                // icon, a host row's bare name) would be exactly the
                // inconsistency `icons` exists to prevent, and reusing the
                // two components here costs one match arm.
                //
                // `HostKind::Unrecognized` draws neither glyph: it is the
                // forward-compat catch-all for a kind value a newer helm
                // might send that this build cannot interpret, so — unlike
                // `Local`/`Ssh`, which this registry row always knows
                // outright — there is no verdict to assert. Asserting
                // either glyph would be the same invented claim
                // `list::shared::session_locality`'s `Unknown` case refuses
                // to make for a session row.
                match host.kind {
                    HostKind::Local => rsx! {
                        LocalHostIcon {}
                        span { class: "visually-hidden", "local" }
                    },
                    HostKind::Ssh => rsx! {
                        RemoteHostIcon {}
                        span { class: "visually-hidden", "remote" }
                    },
                    HostKind::Unrecognized => rsx! {},
                }
                span { class: "host-name peer-value", dir: "ltr", "{shown_name}" }
                span {
                    // `updating` lets the status group shrink so the step
                    // name, not the `⋯` toggle, absorbs width pressure.
                    class: if update_is_active {
                        "host-status {phase_class(&host.state)} updating"
                    } else {
                        "host-status {phase_class(&host.state)}"
                    },
                    role: "status",
                    aria_label: if update_is_active {
                        None
                    } else {
                        (is_connected(&host.state) || inline_update.is_some())
                            .then(|| phase_display_label(&host.state))
                    },
                    span { class: "status-dot", "aria-hidden": "true" }
                    match update_progress {
                        Some(HostUpdateProgress::Pending(phase)) => rsx! {
                            span {
                                class: "host-status-label host-update-pending",
                                "data-update-phase": phase.attribute(),
                                UpdateProgressDot {}
                                "updating…"
                            }
                        },
                        Some(HostUpdateProgress::Running(summary)) => rsx! {
                            UpdateProgressLabel { key: "{summary.run_id}", summary }
                        },
                        None => rsx! {
                            if inline_update.is_none() && (!is_connected(&host.state)
                                || matches!(&host.state, HostPhase::Connected { old_version: true, .. })
                                || runs_newer_version(&host.state))
                            {
                                span {
                                    class: "host-status-label",
                                    title: too_new_title(&host.state),
                                    "{phase_display_label(&host.state)}"
                                }
                            }
                        },
                    }
                }
                if let Some(urgency) = inline_update {
                    // Outside the status region: the action has its own name,
                    // while the status still announces the outdated-host words.
                    button {
                        r#type: "button",
                        class: "btn host-update-button",
                        "data-update-kind": urgency,
                        title: host_update_title(&host.state),
                        aria_label: format!("update {shown_name}"),
                        onclick: {
                            let binding = click_binding.clone();
                            move |_| {
                                on_provisioning.call((id, ActionRequest {
                                    operation: ProvisioningOperation::Update,
                                    binding: binding.clone(),
                                }));
                                // Progress unmounts this control. Keep keyboard
                                // navigation on the same host instead of body.
                                if let Some(handle) = toggle_handle.peek().clone() {
                                    spawn(async move {
                                        let _ = handle.set_focus(true).await;
                                    });
                                }
                            }
                        },
                        span { "aria-hidden": "true", "↑" }
                        "update"
                    }
                }
                // The toggle stays mounted while a modal dialog takes over.
                // `nowrap` remains load-bearing for the fixed-position panel
                // (F2/COR-HOST-MENU-OFFSCREEN).
                    button {
                        r#type: "button",
                        class: "btn host-row-menu",
                        aria_label: host_menu_label(&host.name),
                        aria_expanded: menu_open,
                        aria_haspopup: "menu",
                        onkeydown: move |evt| {
                            if !menu_open {
                                let Some(intent) = closed_toggle_key_intent(&evt.key()) else {
                                    return;
                                };
                                evt.prevent_default();
                                on_menu_toggle.call(id);
                                begin_open(intent);
                                return;
                            }
                            handle_menu_key(&evt, None, menu_wiring, &id);
                        },
                        // The same invariant the session row's toggle
                        // restores — see `forget_menu_focus`. This row has
                        // no in-panel sub-state to unmount its own items,
                        // but the toggle is still the one place focus can
                        // sit while the panel stays open, and the shared
                        // key handler reads the same two signals either
                        // way.
                        onfocusin: move |_| forget_menu_focus(menu_wiring),
                        onmounted: move |element| {
                            toggle_handle.set(Some(element.data()));
                            if should_measure_on_mount(menu_open, *placement.peek()) {
                                spawn_measurement();
                            }
                        },
                        onclick: move |_| {
                            let opening = !menu_open;
                            on_menu_toggle.call(id);
                            if !opening {
                                return;
                            }
                            begin_open(MenuOpenIntent::First);
                        },
                        "⋯"
                    }
                    if menu_open {
                        div {
                            class: "host-row-menu-flyout",
                            style: session_menu_placement_style(placement()),
                            if let Some(pointer_style) = session_menu_pointer_style(placement()) {
                                span { class: "host-row-menu-pointer", style: pointer_style, "aria-hidden": "true" }
                            }
                            div {
                                class: "host-row-menu-panel",
                                div {
                                    class: "session-row-menu-header",
                                    div {
                                        class: "session-row-menu-title",
                                        title: "{shown_name}",
                                        span { class: "peer-value", dir: "ltr", "{shown_name}" }
                                    }
                                    div {
                                        class: "session-row-menu-summary",
                                        title: "{host_menu_summary_tooltip(&menu_summary_parts)}",
                                        PeerLine {
                                            class: "session-row-menu-summary-runs".to_string(),
                                            parts: menu_summary_parts.clone(),
                                            peer_tooltips: true,
                                        }
                                    }
                                }
                                div {
                                class: "host-row-menu-items session-row-menu-items",
                                role: "menu",
                                aria_label: host_menu_label(&host.name),
                                button {
                                    r#type: "button",
                                    class: "btn session-row-menu-item host-row-menu-item host-retry",
                                    role: "menuitem",
                                    aria_describedby: "host-menu-retry-description",
                                    aria_disabled: if busy { "true" },
                                    tabindex: if menu_tab_stop == Some(HostMenuAction::Retry) { "0" } else { "-1" },
                                    onmounted: move |element| {
                                        remember_menu_item(menu_wiring, HostMenuAction::Retry, element.data())
                                    },
                                    onfocusin: move |_| {
                                        menu_focus.set(menu_order.position(HostMenuAction::Retry));
                                    },
                                    onfocusout: move |_| menu_focus.set(None),
                                    onkeydown: move |evt| {
                                        handle_menu_key(
                                            &evt,
                                            menu_order.position(HostMenuAction::Retry),
                                            menu_wiring,
                                            &id,
                                        );
                                    },
                                    onclick: move |_| {
                                        if busy {
                                            return;
                                        }
                                        on_retry.call(id);
                                    },
                                    HostMenuActionIcon { action: HostMenuAction::Retry }
                                    span { class: "session-row-menu-copy",
                                        span { class: "session-row-menu-label", "retry" }
                                        span { id: "host-menu-retry-description", class: "session-row-menu-description", "try the connection again" }
                                    }
                                }
                                if let (Some(reported), Some(label)) = (adopt_identity, adopt_label) {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item host-row-menu-item host-adopt",
                                        role: "menuitem",
                                        aria_describedby: "host-menu-adopt-description",
                                        aria_disabled: if busy { "true" },
                                        tabindex: if menu_tab_stop == Some(HostMenuAction::Adopt) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(menu_wiring, HostMenuAction::Adopt, element.data())
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(HostMenuAction::Adopt));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(HostMenuAction::Adopt),
                                                menu_wiring,
                                                &id,
                                            );
                                        },
                                        onclick: move |_| {
                                            if busy {
                                                return;
                                            }
                                            on_adopt.call((id, reported.clone()));
                                        },
                                        // Its own isolated run inside the
                                        // button, so an identity cannot
                                        // rearrange the verb around it and
                                        // make "adopt X" read as something
                                        // else.
                                        HostMenuActionIcon { action: HostMenuAction::Adopt }
                                        span { class: "session-row-menu-copy",
                                            span { class: "session-row-menu-label peer-value", dir: "ltr", "{label}" }
                                            span { id: "host-menu-adopt-description", class: "session-row-menu-description", "accept the supervisor's identity" }
                                        }
                                    }
                                }
                                if has_provisioning_menu {
                                    div { class: "host-row-menu-separator", role: "separator" }
                                }
                                if let Some(operation) = provisioning_menu.rerun {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item host-row-menu-item provisioning-rerun",
                                        role: "menuitem",
                                        aria_describedby: "host-menu-rerun-description",
                                        aria_disabled: if rerun_disabled { "true" },
                                        tabindex: if menu_tab_stop == Some(HostMenuAction::Rerun) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(menu_wiring, HostMenuAction::Rerun, element.data())
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(HostMenuAction::Rerun));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(HostMenuAction::Rerun),
                                                menu_wiring,
                                                &id,
                                            );
                                        },
                                        onclick: {
                                            let binding = click_binding.clone();
                                            move |_| {
                                                if rerun_disabled {
                                                    return;
                                                }
                                                on_provisioning.call((id, ActionRequest {
                                                    operation,
                                                    binding: binding.clone(),
                                                }));
                                            }
                                        },
                                        HostMenuActionIcon { action: HostMenuAction::Rerun }
                                        span { class: "session-row-menu-copy",
                                            span { class: "session-row-menu-label", if provisioning_menu.planning { "planning…" } else { "re-run" } }
                                            span {
                                                id: "host-menu-rerun-description",
                                                class: "session-row-menu-description",
                                                match operation {
                                                    ProvisioningOperation::Update => "try the failed update again",
                                                    ProvisioningOperation::Add => "try the failed setup again",
                                                }
                                            }
                                        }
                                    }
                                }
                                if provisioning_menu.automatic_setup {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item host-row-menu-item provisioning-auto-setup",
                                        role: "menuitem",
                                        aria_describedby: "host-menu-automatic-setup-description",
                                        aria_disabled: if setup_disabled { "true" },
                                        tabindex: if menu_tab_stop == Some(HostMenuAction::AutomaticSetup) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(
                                                menu_wiring,
                                                HostMenuAction::AutomaticSetup,
                                                element.data(),
                                            )
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(HostMenuAction::AutomaticSetup));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(HostMenuAction::AutomaticSetup),
                                                menu_wiring,
                                                &id,
                                            );
                                        },
                                        onclick: {
                                            let binding = click_binding.clone();
                                            move |_| {
                                                if setup_disabled {
                                                    return;
                                                }
                                                on_provisioning.call((id, ActionRequest {
                                                    operation: ProvisioningOperation::Add,
                                                    binding: binding.clone(),
                                                }));
                                            }
                                        },
                                        HostMenuActionIcon { action: HostMenuAction::AutomaticSetup }
                                        span { class: "session-row-menu-copy",
                                            span { class: "session-row-menu-label", "set up automatically" }
                                            span { id: "host-menu-automatic-setup-description", class: "session-row-menu-description", "install Farhelm on this host" }
                                        }
                                    }
                                }
                                if provisioning_menu.update {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item host-row-menu-item provisioning-update",
                                        role: "menuitem",
                                        aria_describedby: "host-menu-update-description",
                                        aria_disabled: if update_disabled { "true" },
                                        tabindex: if menu_tab_stop == Some(HostMenuAction::Update) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(menu_wiring, HostMenuAction::Update, element.data())
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(HostMenuAction::Update));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(HostMenuAction::Update),
                                                menu_wiring,
                                                &id,
                                            );
                                        },
                                        onclick: {
                                            let binding = click_binding.clone();
                                            move |_| {
                                                if update_disabled {
                                                    return;
                                                }
                                                on_provisioning.call((id, ActionRequest {
                                                    operation: ProvisioningOperation::Update,
                                                    binding: binding.clone(),
                                                }));
                                            }
                                        },
                                        HostMenuActionIcon { action: HostMenuAction::Update }
                                        span { class: "session-row-menu-copy",
                                            span { class: "session-row-menu-label", if provisioning_menu.planning { "planning…" } else { "update" } }
                                            span { id: "host-menu-update-description", class: "session-row-menu-description", "install the newer Farhelm version" }
                                        }
                                    }
                                }
                                if has_provisioning_menu {
                                    div { class: "host-row-menu-separator", role: "separator" }
                                }
                                button {
                                    r#type: "button",
                                    class: "btn session-row-menu-item host-row-menu-item host-settings",
                                    role: "menuitem",
                                    aria_describedby: "host-menu-settings-description",
                                    aria_disabled: if busy { "true" },
                                    tabindex: if menu_tab_stop == Some(HostMenuAction::Settings) { "0" } else { "-1" },
                                    onmounted: move |element| {
                                        remember_menu_item(menu_wiring, HostMenuAction::Settings, element.data())
                                    },
                                    onfocusin: move |_| menu_focus.set(menu_order.position(HostMenuAction::Settings)),
                                    onfocusout: move |_| menu_focus.set(None),
                                    onkeydown: move |evt| {
                                        handle_menu_key(&evt, menu_order.position(HostMenuAction::Settings), menu_wiring, &id);
                                    },
                                    onclick: move |_| {
                                        // Only REQUESTS the panel; `HostsPanel`'s
                                        // `on_settings_start` closes the menu once
                                        // its own busy guard lets it through, as
                                        // `on_edit_start` does for the editor.
                                        if !busy {
                                            on_settings_start.call(id);
                                        }
                                    },
                                    HostMenuActionIcon { action: HostMenuAction::Settings }
                                    span { class: "session-row-menu-copy",
                                        span { class: "session-row-menu-label", "settings" }
                                        span { id: "host-menu-settings-description", class: "session-row-menu-description", "edit this host's connection" }
                                    }
                                }
                                if manageable {
                                    // The boundary before the destructive
                                    // item — see `list::row`'s own separator
                                    // for the accessibility argument, which
                                    // applies identically here. Not counted
                                    // by `MenuOrder`, so arrow navigation
                                    // steps straight past it.
                                    div { class: "host-row-menu-separator", role: "separator" }
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item host-row-menu-item host-remove",
                                        role: "menuitem",
                                        aria_describedby: "host-menu-remove-description",
                                        aria_disabled: if busy { "true" },
                                        tabindex: if menu_tab_stop == Some(HostMenuAction::Remove) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(menu_wiring, HostMenuAction::Remove, element.data())
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(HostMenuAction::Remove));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(HostMenuAction::Remove),
                                                menu_wiring,
                                                &id,
                                            );
                                        },
                                        onclick: move |_| {
                                            if busy {
                                                return;
                                            }
                                            // See `settings` above:
                                            // only requests the confirm
                                            // prompt; `on_remove_start`
                                            // closes the menu.
                                            on_remove_start.call(id);
                                        },
                                        HostMenuActionIcon { action: HostMenuAction::Remove }
                                        span { class: "session-row-menu-copy",
                                            span { class: "session-row-menu-label", "remove" }
                                            span { id: "host-menu-remove-description", class: "session-row-menu-description", "forget this host" }
                                        }
                                    }
                                }
                            }
                        }
                    }
            }
            }
            if details_open {
                if host.alias.clone().flatten().is_some() && let Some(destination) = host.destination.as_deref() {
                    // The one place an alias never hides the real
                    // destination — which makes it exactly the wrong place
                    // to lose this module's peer-text safety boundary; see
                    // `destination_detail_parts`'s own doc.
                    PeerLine {
                        class: "host-destination-detail",
                        parts: destination_detail_parts(destination),
                    }
                }
                PeerLine { class: "host-detail", parts: detail }
                if !local_setup && let Some(remedy) = remedy {
                    PeerLine { class: "host-remedy", parts: remedy }
                }
            }
            // The refusal is the HELM's sentence and routinely embeds
            // peer-supplied text — an adoption superseded by a re-probe
            // quotes the identity the host is reporting now — so it is
            // rendered through the same escaping and isolation every other
            // peer value gets. A refusal able to lay itself out is the worst
            // place to lose that: it is the message a user acts on, and it
            // arrives exactly when two identities are being compared.
            if let Some(error) = error {
                PeerLine {
                    class: "action-error host-error",
                    parts: vec![DetailPart::Peer(error)],
                }
            }
            // Distinct from an error on purpose: this one says the change
            // HAPPENED and only its confirmation was unreadable.
            if let Some(warning) = warning {
                PeerLine {
                    class: "host-warning",
                    parts: vec![DetailPart::Peer(warning)],
                }
            }
            {provisioning_section}
        }
    }
}

/// The in-place field editor of the host settings dialog — despite the
/// name, shared between the destination edit AND the alias edit (`alias:
/// bool` selects which), one text field with the same submit/cancel/error
/// shape either way. Not renamed to something field-neutral because the CSS classes it
/// renders (`.host-destination-form`, `.host-destination-input`,
/// `.host-save-destination`) are load-bearing browser-suite selectors
/// across several spec files; changing the component's Rust name costs
/// nothing, but changing those strings would touch every test that already
/// drives this form.
///
/// A plain `<input type="text">`, unlike the rename control's textarea:
/// both a destination and an alias are single lines by construction. The
/// two fields' VALIDATION contracts differ, though, and each is enforced
/// where it is authoritative:
/// - **Destination**: nothing is validated here at all. The helm refuses
///   the shapes that matter (empty, option-shaped, NUL-carrying) at the
///   registry boundary, so whatever is typed goes as typed, with the
///   refusal coming from the authority that owns the rule.
/// - **Alias**: `validate_alias_draft` mirrors the helm's own rules
///   (trim, empty-clears, the 64-character cap, no control characters)
///   BEFORE a request leaves the browser, to save a round trip on an
///   obvious mistake — but the helm remains authoritative for collisions,
///   which nothing client-side can check.
///
/// Both fields still turn off every form of browser text "correction"
/// (autocomplete/autocorrect/autocapitalize/spellcheck) for the same
/// underlying reason, reached two different ways: a destination is
/// EXECUTED as part of an ssh command line, where a silently capitalized
/// hostname or a swallowed keystroke dials the wrong machine, while an
/// alias is a label a person chose on purpose — a "helpful" browser rewrite
/// would submit text the user did not type and never asked to see
/// corrected.
///
/// The draft belongs to the panel, not to this form: the panel is the one
/// owner that outlives everything that can come and go around the editor (a
/// refused save, the dialog closing and reopening, host-list refreshes). It
/// first moved there for the reason `rename::RenameForm` records, back when
/// this form lived in a host row that every host mutation rebuilt.
#[component]
fn HostDestinationForm(
    mut draft: Signal<String>,
    busy: bool,
    /// Selects alias-specific affordances while preserving one editor shape.
    alias: bool,
    on_submit: EventHandler<String>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        form {
            class: "host-destination-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                if busy {
                    return;
                }
                on_submit.call(draft());
            },
            input {
                r#type: "text",
                class: "host-destination-input",
                placeholder: if alias { "host alias" } else { "ssh destination" },
                // Every browser text "correction" is off for both fields
                // this input can be — see this component's own doc for why
                // the two reasons differ: a destination gets EXECUTED as
                // part of a command line (a silently capitalized hostname
                // or a swallowed suggestion keystroke dials the wrong
                // machine, the same argument the create form's command
                // fields make), while an alias is a label the user chose on
                // purpose and a "helpful" rewrite would submit text they
                // never typed.
                autocomplete: "off",
                autocorrect: "off",
                autocapitalize: "none",
                spellcheck: "false",
                autofocus: true,
                value: "{draft}",
                disabled: busy,
                oninput: move |evt| draft.set(evt.value()),
            }
            button {
                r#type: "submit",
                class: "btn btn-primary host-save-destination",
                disabled: busy,
                "save"
            }
            button {
                r#type: "button",
                class: "btn btn-neutral host-cancel-edit",
                disabled: busy,
                onclick: move |_| {
                    if busy {
                        return;
                    }
                    on_cancel.call(());
                },
                "cancel"
            }
        }
    }
}

/// The form inputs that one displayed ADD confirmation was planned from.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AddBinding {
    destination: String,
    remote_farhelm: String,
    remote_state_dir: String,
}

/// One-use ADD authority paired with the inputs the helm inspected.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AddOffer {
    probe_id: String,
    confirmation: String,
    binding: AddBinding,
}

/// A confirmed setup, handed from the add dialog to the hosts panel.
///
/// The dialog closes as soon as the user confirms, so the POST cannot live in
/// the dialog's scope. `claim` is the page token the dialog took in the same
/// handler that consumed the offer; it must be held until the POST resolves,
/// which is why it travels with the probe id instead of being re-claimed by
/// the receiver (a re-claim could lose to another page mutation after the
/// offer was already spent).
struct AddSubmission {
    probe_id: String,
    claim: OpGuard,
}

/// Keep probe and manual diagnostics inside the peer-text boundary.
fn probe_error_parts(error: String) -> Vec<DetailPart> {
    vec![DetailPart::Peer(error)]
}

/// The details-view "destination: ..." line an aliased host shows —
/// extracted to a pure function, in the same shape as
/// [`probe_error_parts`], so the peer-text boundary on this specific value
/// is pinned by a plain unit test rather than only by rendering the whole
/// row.
///
/// This is the ONE place an alias never hides the real destination
/// (SPEC.md's Topology paragraph), which makes it exactly the wrong place
/// to let that destination bypass this module's escaping and isolation:
/// the row's own NAME gets the identical treatment (`HostRow`'s own doc),
/// and a destination is exactly as peer/registry-supplied as a name is.
fn destination_detail_parts(destination: &str) -> Vec<DetailPart> {
    vec![
        DetailPart::text("destination: "),
        DetailPart::peer(destination),
    ]
}

/// The add-host form: discover first, then either keep the answering
/// supervisor or offer the exact setup plan retained by the helm.
///
/// The two optional fields are exposed rather than hidden behind a default
/// because discovery needs them to find an existing custom installation, and
/// a retained setup plan uses them as its installation coordinates. They are
/// therefore part of both sides of discovery-first ADD whenever farhelm is
/// not on the remote's `PATH` or its supervisor serves a non-default state
/// directory — the case the e2e harness itself is built on.
///
/// Discovery claims no page token because its network wait must not freeze
/// unrelated page work. It can still mutate the registry when a supervisor
/// answers, so its local re-entry guard and authoritative refresh are part of
/// the contract. Only this form's explicit confirmation (or the remembered
/// permanent answer) starts its provisioning run and claims `OpLock` around
/// its POST; remote updates on existing rows submit automatically from the
/// row's own lifecycle instead.
///
/// The dialog never holds the user behind a pending request. Confirming
/// setup hands the claimed token and the one-use probe id to `on_submit` and
/// the dialog closes; the panel runs the POST and the new row tracks the run.
/// Cancel and Escape stay live throughout, including while discovery is out
/// or another page operation holds the token: closing mid-discovery drops
/// the probe's reply, as closing the inline form did before this was a
/// dialog, and the helm's registry change still reaches this page through
/// the fleet feed if the probe registered a host.
#[component]
fn AddHostForm(
    mut ops: OpLock,
    on_added: EventHandler<Option<String>>,
    on_refresh: EventHandler<()>,
    on_submit: EventHandler<AddSubmission>,
    on_cancel: EventHandler<()>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut preferences = use_context::<crate::list::SharedPreferences>();
    let mut ssh = use_signal(String::new);
    let mut remote_farhelm = use_signal(String::new);
    let mut remote_state_dir = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut probing = use_signal(|| false);
    let mut offer = use_signal(|| None::<AddOffer>);
    let mut auto_submit = use_signal(|| false);
    let page_busy = ops.busy();
    let busy = page_busy || *probing.read();

    let preference_base = base.clone();
    let confirm = use_callback(move |_| {
        let Some(planned) = offer.peek().clone() else {
            return;
        };
        let current = AddBinding {
            destination: ssh.peek().clone(),
            remote_farhelm: remote_farhelm.peek().clone(),
            remote_state_dir: remote_state_dir.peek().clone(),
        };
        if planned.binding != current {
            offer.set(None);
            error.set(Some(
                "the host fields changed after discovery; probe again".to_string(),
            ));
            return;
        }
        let Some(claim) = ops.claim_guard() else {
            return;
        };
        // The helm may consume this id before any later refusal or transport
        // ambiguity reaches the browser. Never present it for a second use.
        offer.set(None);
        error.set(None);
        on_submit.call(AddSubmission {
            probe_id: planned.probe_id,
            claim,
        });
    });

    // A permanent answer skips only the setup question. The probe still
    // produces the one-use authority, and the same binding check protects
    // against fields changing while that authority is in flight.
    use_effect(move || {
        if auto_submit() && offer.peek().is_some() {
            auto_submit.set(false);
            confirm.call(());
        }
    });
    use_effect(move || {
        if offer.read().is_some() && !*auto_submit.peek() {
            settings_dialog::focus_add_cancel();
        }
    });

    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div {
                class: "host-settings-dialog host-add-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "add host",
                tabindex: "-1",
                onmounted: move |_| settings_dialog::install_add_dialog(),
                onkeydown: move |evt: KeyboardEvent| {
                    if evt.key() == Key::Escape && !evt.is_composing() {
                        on_cancel.call(());
                    }
                },
        form {
            class: "add-host-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                // Discovery can register an answering supervisor, but it
                // still stays outside the page lock. This synchronous guard
                // prevents a second Enter in the same browser task from
                // retaining two competing one-use plans.
                if *probing.peek() || offer.peek().is_some() || ops.busy_now() {
                    return;
                }
                error.set(None);
                probing.set(true);
                let base = base.clone();
                let binding = AddBinding {
                    destination: ssh(),
                    remote_farhelm: remote_farhelm(),
                    remote_state_dir: remote_state_dir(),
                };
                let destination = binding.destination.clone();
                let farhelm = binding.remote_farhelm.clone();
                let state_dir = binding.remote_state_dir.clone();
                spawn(async move {
                    match probe_ssh_host(&base, &destination, &farhelm, &state_dir).await {
                        Ok(ProbeResponse::Discovered) => on_added.call(None),
                        Ok(ProbeResponse::Provisionable {
                            probe_id,
                            confirmation,
                        }) => {
                            offer.set(Some(AddOffer { probe_id, confirmation, binding }));
                            auto_submit.set(
                                preferences.0.peek().skip_host_setup_confirmation == Some(true),
                            );
                        }
                        Ok(ProbeResponse::Manual { reason }) => error.set(Some(reason)),
                        Ok(ProbeResponse::Unvalidated(problem)) => {
                            // A successful probe may have registered an
                            // answering supervisor before its unreadable
                            // body reached this client. Refresh before the
                            // user can mistake the still-open form for proof
                            // that nothing committed.
                            error.set(Some(problem));
                            on_refresh.call(());
                        }
                        Err(problem) => error.set(Some(problem)),
                    }
                    probing.set(false);
                });
            },
            // Same total opt-out of browser text mangling the create form's
            // command fields carry, for the same reason: all three of these
            // become part of a command line, and a "corrected" one dials or
            // execs something the user did not type.
            if let Some(planned) = offer.read().clone() {
                SetupPlanConfirmation {
                    confirmation: planned.confirmation,
                    busy: page_busy,
                    on_confirm: move |skip_future| {
                        if skip_future {
                            preferences.0.write().skip_host_setup_confirmation = Some(true);
                            store_preference(
                                &preference_base,
                                PreferenceValue::HostSetupConfirmation(true),
                            );
                        }
                        confirm.call(());
                    },
                    on_cancel: move |_| on_cancel.call(()),
                }
            } else {
                label {
                    "ssh destination"
                    input {
                        r#type: "text",
                        class: "add-host-ssh",
                        required: true,
                        autocomplete: "off",
                        autocorrect: "off",
                        autocapitalize: "none",
                        spellcheck: "false",
                        value: "{ssh}",
                        disabled: busy,
                        oninput: move |evt| ssh.set(evt.value()),
                    }
                }
                label {
                    "remote farhelm (optional)"
                    input {
                        r#type: "text",
                        class: "add-host-farhelm",
                        autocomplete: "off",
                        autocorrect: "off",
                        autocapitalize: "none",
                        spellcheck: "false",
                        value: "{remote_farhelm}",
                        disabled: busy,
                        oninput: move |evt| remote_farhelm.set(evt.value()),
                    }
                }
                label {
                    "remote state dir (optional)"
                    input {
                        r#type: "text",
                        class: "add-host-state-dir",
                        autocomplete: "off",
                        autocorrect: "off",
                        autocapitalize: "none",
                        spellcheck: "false",
                        value: "{remote_state_dir}",
                        disabled: busy,
                        oninput: move |evt| remote_state_dir.set(evt.value()),
                    }
                }
                button {
                    r#type: "submit",
                    class: "btn btn-primary add-host-submit",
                    disabled: busy,
                    if *probing.read() { "probing…" } else { "add" }
                }
            }
            if let Some(err) = error.read().clone() {
                PeerLine {
                    class: "create-session-error add-host-error",
                    parts: probe_error_parts(err),
                }
            }
        }
                if offer.read().is_none() {
                    // Never disabled: see this component's doc.
                    button {
                        r#type: "button",
                        class: "btn btn-neutral add-host-cancel",
                        onclick: move |_| on_cancel.call(()),
                        "cancel"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::detail_text;

    /// A label-sized rect at `top`, 100px wide and 16px tall, starting 40px
    /// from the viewport's left edge.
    fn label_rect_at(top: f64) -> dioxus::html::geometry::PixelsRect {
        dioxus::html::geometry::PixelsRect::new(
            dioxus::html::geometry::euclid::point2(40.0, top),
            dioxus::html::geometry::euclid::size2(100.0, 16.0),
        )
    }

    /// Every placement's style names both vertical edges, the unused one as
    /// `auto`.
    ///
    /// Why: the popup mounts hidden with a `top` and is then re-styled, and
    /// a style string that merely omits a property leaves the old value on
    /// the element. An `Above` popup that kept the mount-time `top` would
    /// have both edges pinned and collapse between them.
    #[farhelm_testtrace::test]
    fn every_popup_placement_names_both_vertical_edges() {
        assert_eq!(
            PopupAnchor::Above {
                left: 1.0,
                bottom: 2.0
            }
            .style(),
            "left: 1px; top: auto; bottom: 2px;"
        );
        for anchor in [
            PopupAnchor::Below {
                left: 1.0,
                top: 2.0,
            },
            PopupAnchor::Pinned {
                left: 1.0,
                top: 2.0,
            },
        ] {
            assert_eq!(anchor.style(), "left: 1px; top: 2px; bottom: auto;");
        }
    }

    /// The update popup is the only place a truncated step name and the
    /// run's full step list can be read, so it must stay inside the
    /// viewport. Specifies, for a popup of a given (measured) height: below
    /// the label when it fits there; above it, bottom-anchored, when it fits
    /// there instead; pinned inside the viewport, over the label, when it
    /// fits on neither side; and below whenever the viewport height is
    /// unknown.
    #[farhelm_testtrace::test]
    fn update_popup_flips_above_only_when_below_would_overflow() {
        assert_eq!(
            PopupAnchor::beside(&label_rect_at(100.0), Some(800.0), 90.0),
            PopupAnchor::Below {
                left: 40.0,
                top: 120.0
            },
        );
        assert_eq!(
            PopupAnchor::beside(&label_rect_at(750.0), Some(800.0), 90.0),
            PopupAnchor::Above {
                left: 40.0,
                bottom: 54.0
            },
        );
        assert_eq!(
            PopupAnchor::beside(&label_rect_at(750.0), None, 90.0),
            PopupAnchor::Below {
                left: 40.0,
                top: 770.0
            },
        );
        // A 60px viewport with the label at 10px: 30px free below, 6px
        // above. Neither fits the reserve, so the popup is pinned inside the
        // viewport, as high as its edge margin allows, over the label.
        assert_eq!(
            PopupAnchor::beside(&label_rect_at(10.0), Some(60.0), 90.0),
            PopupAnchor::Pinned {
                left: 40.0,
                top: 8.0
            },
        );
        // Pinned as low as the reserve allows when the viewport has room for
        // it, just not on either side of the label.
        assert_eq!(
            PopupAnchor::beside(&label_rect_at(100.0), Some(200.0), 150.0),
            PopupAnchor::Pinned {
                left: 40.0,
                top: 42.0
            },
        );
        // Taller popups flip where shorter ones still fit: the label ends
        // at 516px in an 800px viewport, so after the gap 280px are free
        // below it.
        let label = label_rect_at(500.0);
        assert!(matches!(
            PopupAnchor::beside(&label, Some(800.0), 280.0),
            PopupAnchor::Below { .. }
        ));
        assert!(matches!(
            PopupAnchor::beside(&label, Some(800.0), 281.0),
            PopupAnchor::Above { .. }
        ));
    }

    /// Probe and manual diagnostics cannot carry bidi or invisible controls
    /// into the add form even though the helm relays host-produced text.
    #[farhelm_testtrace::test]
    fn probe_errors_cross_the_peer_text_boundary() {
        let shown = detail_text(&probe_error_parts(
            "ssh failed \u{202E}spoof\u{200B}".to_string(),
        ));
        assert_eq!(shown, "ssh failed <U+202E>spoof<U+200B>");
    }

    /// The details-view destination line is the ONE place an alias never
    /// hides the real destination, which makes it exactly the surface a
    /// directional override or an invisible character could misuse to make
    /// that destination read as something other than what it is — pinned
    /// here the same way `probe_errors_cross_the_peer_text_boundary` pins
    /// the add form's peer text, and needed for the same reason: a raw
    /// interpolated string would have let this line bypass the escaping
    /// and isolation the row's own NAME already gets.
    #[farhelm_testtrace::test]
    fn destination_details_cross_the_peer_text_boundary() {
        let shown = detail_text(&destination_detail_parts(
            "host\u{202E}spoof\u{200B}.example",
        ));
        assert_eq!(shown, "destination: host<U+202E>spoof<U+200B>.example");
    }

    /// A host in the given state, with the rest of the row as plain as
    /// possible — every assertion below is about the state alone.
    fn host(state: HostPhase) -> Host {
        Host {
            id: 1,
            kind: HostKind::Ssh,
            destination: Some("user@box".to_string()),
            alias: None,
            name: "user@box".to_string(),
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            state,
            incarnation: 1,
            yolo_without_asking: false,
        }
    }

    /// A duplicate entry is told which entry holds its machine by that
    /// entry's name, taken from the same host list, and its remedy names it
    /// too.
    ///
    /// Why it matters: SPEC.md has the duplicate message say which entry
    /// holds the machine, by name, and tell the user to remove it or change
    /// this entry's destination, then press Retry. The helm sends only the
    /// other entry's id, so a list that lost the naming step would leave the
    /// user looking for a number. An id missing from the list falls back to
    /// naming it by number rather than to nothing.
    #[farhelm_testtrace::test]
    fn a_duplicate_names_the_entry_holding_its_machine() {
        let mut owner = host(HostPhase::Unrecognized);
        owner.id = 7;
        owner.name = "build box".to_string();
        let mut duplicate = host(HostPhase::Duplicate {
            twin: 7,
            identity: "shared-install".to_string(),
            twin_name: None,
        });
        duplicate.id = 8;
        let mut orphan = host(HostPhase::Duplicate {
            twin: 99,
            identity: "other-install".to_string(),
            twin_name: None,
        });
        orphan.id = 9;
        let mut hosts = vec![owner, duplicate, orphan];
        name_duplicate_twins(&mut hosts);

        let detail = detail_text(&state_detail(&hosts[1].state));
        let remedy = detail_text(&state_remedy(&hosts[1].state).expect("a remedy"));
        assert!(detail.contains("build box"), "{detail}");
        assert!(
            remedy.contains("build box") && remedy.contains("Retry"),
            "{remedy}"
        );
        let orphan_detail = detail_text(&state_detail(&hosts[2].state));
        assert!(orphan_detail.contains("host 99"), "{orphan_detail}");
    }

    /// Every phase, with a UNIQUE sentinel in each of its fields, so the
    /// tables below can prove not just that something rendered but that the
    /// RIGHT field rendered in the right place.
    fn every_phase() -> Vec<HostPhase> {
        vec![
            HostPhase::Connecting {
                attempt: 3,
                last_error: Some("sentinel-connecting-error".to_string()),
            },
            HostPhase::Unreachable {
                cause: "transport-failure".to_string(),
                last_error: "sentinel-unreachable-error".to_string(),
            },
            HostPhase::Connected {
                identity: Some("sentinel-connected-identity".to_string()),
                build_version: "sentinel-connected-build".to_string(),
                old_version: false,
                newer_version: false,
                refresh: RefreshHealth::Ok { sessions: 4 },
            },
            HostPhase::VersionSkew {
                peer_protocol: 9,
                peer_build: "sentinel-peer-build".to_string(),
                our_protocol: 8,
                our_build: "sentinel-our-build".to_string(),
                remediation: "sentinel-remediation".to_string(),
            },
            HostPhase::IdentityMismatch {
                recorded: "sentinel-recorded".to_string(),
                reported: "sentinel-reported".to_string(),
            },
            HostPhase::IdentityUnverified {
                recorded: "sentinel-unverified-recorded".to_string(),
            },
            HostPhase::Duplicate {
                twin: 42,
                identity: "sentinel-duplicate-identity".to_string(),
                twin_name: None,
            },
            HostPhase::Retired {
                reason: "sentinel-retired-reason".to_string(),
            },
            HostPhase::Unrecognized,
        ]
    }

    /// Every phase keeps a stable wire token for data attributes and
    /// machine-authored refusals, even though visible row wording is now
    /// humanized separately.
    ///
    /// Exhaustive over the whole taxonomy rather than a sample: a label is a
    /// one-line match arm, and the failure it guards against — a new phase
    /// borrowing another's word — is invisible unless every phase is listed.
    #[farhelm_testtrace::test]
    fn every_phase_keeps_the_helms_wire_token() {
        let labels: Vec<&str> = every_phase().iter().map(phase_label).collect();
        assert_eq!(
            labels,
            vec![
                "connecting",
                "unreachable-reprobing",
                "connected",
                "version-skew",
                "identity-mismatch",
                "identity-unverified",
                "duplicate",
                "retired",
                "unrecognized",
            ]
        );
        // Every label distinct: two phases sharing a word would make the
        // panel and a refusal disagree about which host is which.
        let unique: std::collections::HashSet<&str> = labels.iter().copied().collect();
        assert_eq!(unique.len(), labels.len());
    }

    /// Every phase's detail must carry ITS OWN evidence — the sentinels make
    /// that checkable field by field, which a per-phase spot check cannot: a
    /// detail that rendered some other variant's payload, or dropped a
    /// field, would still contain "plausible" text.
    #[farhelm_testtrace::test]
    fn every_phase_detail_carries_its_own_evidence() {
        let expectations: Vec<(HostPhase, Vec<&str>)> = vec![
            (
                HostPhase::Connecting {
                    attempt: 3,
                    last_error: Some("sentinel-connecting-error".to_string()),
                },
                vec!["3", "sentinel-connecting-error"],
            ),
            (
                HostPhase::Unreachable {
                    cause: "transport-failure".to_string(),
                    last_error: "sentinel-unreachable-error".to_string(),
                },
                vec!["sentinel-unreachable-error"],
            ),
            (
                HostPhase::Connected {
                    identity: Some("sentinel-connected-identity".to_string()),
                    build_version: "sentinel-connected-build".to_string(),
                    old_version: false,
                    newer_version: false,
                    refresh: RefreshHealth::Ok { sessions: 4 },
                },
                vec![
                    "sentinel-connected-identity",
                    "sentinel-connected-build",
                    "4 sessions",
                ],
            ),
            (
                HostPhase::VersionSkew {
                    peer_protocol: 9,
                    peer_build: "sentinel-peer-build".to_string(),
                    our_protocol: 8,
                    our_build: "sentinel-our-build".to_string(),
                    remediation: "sentinel-remediation".to_string(),
                },
                // Both protocols and both builds: without all four a user
                // cannot see which side is behind.
                vec!["9", "8", "sentinel-peer-build", "sentinel-our-build"],
            ),
            (
                HostPhase::IdentityMismatch {
                    recorded: "sentinel-recorded".to_string(),
                    reported: "sentinel-reported".to_string(),
                },
                vec!["sentinel-recorded", "sentinel-reported"],
            ),
            (
                HostPhase::IdentityUnverified {
                    recorded: "sentinel-unverified-recorded".to_string(),
                },
                vec!["sentinel-unverified-recorded"],
            ),
            (
                HostPhase::Duplicate {
                    twin: 42,
                    identity: "sentinel-duplicate-identity".to_string(),
                    twin_name: None,
                },
                vec!["42", "sentinel-duplicate-identity"],
            ),
            (
                HostPhase::Duplicate {
                    twin: 42,
                    identity: "sentinel-duplicate-identity".to_string(),
                    twin_name: Some("sentinel-twin-name".to_string()),
                },
                vec!["sentinel-twin-name", "sentinel-duplicate-identity"],
            ),
            (
                HostPhase::Retired {
                    reason: "sentinel-retired-reason".to_string(),
                },
                vec!["sentinel-retired-reason"],
            ),
            (HostPhase::Unrecognized, vec!["does not know"]),
        ];

        for (state, needles) in expectations {
            let rendered = detail_text(&state_detail(&state));
            for needle in needles {
                assert!(
                    rendered.contains(needle),
                    "{}'s detail must carry {needle:?}: {rendered}",
                    phase_label(&state)
                );
            }
        }
    }

    /// Adopt is offered for exactly one state, and `identity-unverified` —
    /// the one that looks adjacent to it — must never be that state.
    ///
    /// The helm refuses an adopt there (there is no reported identity to
    /// compare against a recorded one), so offering the control would put a
    /// button on screen whose only possible outcome is a refusal, while
    /// implying a decision the user does not actually have. This is the rule
    /// `HostStateView::IdentityUnverified`'s own docs state as a renderer
    /// obligation. Checked across the whole taxonomy so a phase added later
    /// cannot quietly join the adoptable set.
    #[farhelm_testtrace::test]
    fn adopt_is_offered_only_for_an_identity_mismatch() {
        for state in every_phase() {
            let expected =
                matches!(state, HostPhase::IdentityMismatch { .. }).then_some("sentinel-reported");
            assert_eq!(
                adoptable(&state),
                expected,
                "{} offers the wrong adoption",
                phase_label(&state)
            );
        }
    }

    /// The host menu's item order and visibility follow the row's own
    /// state — the host row's version of the fixed-numbering hazard
    /// `list::row`'s
    /// `menu_order_follows_the_retention_state_rather_than_a_fixed_numbering`
    /// pins for the session row, applied to the host menu's management and
    /// provisioning commands.
    ///
    /// Settings is offered on every row, the reserved local row and an
    /// unmanageable one included, because whether YOLO launches are allowed
    /// is a setting of every host (the destination and alias editors it
    /// holds are gated inside the panel instead). Remove stays gated on
    /// `manageable`, and a dropped item moves later items up rather than
    /// leaving a gap, the same packing `MenuOrder::pack` guarantees for the
    /// session row.
    #[farhelm_testtrace::test]
    fn the_host_menu_follows_manageability_and_adoptability() {
        use HostMenuAction::{Adopt, AutomaticSetup, Remove, Rerun, Retry, Settings, Update};

        // Ssh, adoptable: every non-provisioning item, in declared order.
        let ssh_adoptable = host_menu_order(true, true, ProvisioningMenuState::default());
        assert_eq!(ssh_adoptable.len(), 4);
        assert_eq!(ssh_adoptable.get(0), Some(Retry));
        assert_eq!(ssh_adoptable.get(1), Some(Adopt));
        assert_eq!(ssh_adoptable.get(2), Some(Settings));
        assert_eq!(ssh_adoptable.get(3), Some(Remove));

        // Ssh, not adoptable (the ordinary case): `adopt` drops out and the
        // rest shift up to fill the gap.
        let ssh_plain = host_menu_order(false, true, ProvisioningMenuState::default());
        assert_eq!(ssh_plain.len(), 3);
        assert_eq!(ssh_plain.get(0), Some(Retry));
        assert_eq!(ssh_plain.get(1), Some(Settings));
        assert_eq!(ssh_plain.get(2), Some(Remove));
        assert_eq!(ssh_plain.position(Adopt), None);

        // The local row's identity-mismatch shape: unmanageable, so no remove, but settings
        // still (the local host can start without asking for YOLO launches and aliased like
        // any other). A real, reachable state: the local row's actor compares identities
        // exactly like an ssh row's.
        let local = host_menu_order(true, false, ProvisioningMenuState::default());
        assert_eq!(local.len(), 3);
        assert_eq!(local.get(0), Some(Retry));
        assert_eq!(local.get(1), Some(Adopt));
        assert_eq!(local.get(2), Some(Settings));
        assert_eq!(local.position(Remove), None);

        // The ordinary local row: retry and settings, unconditionally.
        let local_plain = host_menu_order(false, false, ProvisioningMenuState::default());
        assert_eq!(local_plain.len(), 2);
        assert_eq!(local_plain.get(0), Some(Retry));
        assert_eq!(local_plain.last(), Some(Settings));

        // A failed remote update offers rerun and update between identity
        // actions and host management; remove remains last, after the
        // row's destructive separator.
        let failed_remote = host_menu_order(
            false,
            true,
            ProvisioningMenuState {
                rerun: Some(ProvisioningOperation::Update),
                update: true,
                ..ProvisioningMenuState::default()
            },
        );
        assert_eq!(failed_remote.len(), 5);
        assert_eq!(failed_remote.get(1), Some(HostMenuAction::Rerun));
        assert_eq!(failed_remote.get(2), Some(HostMenuAction::Update));
        assert_eq!(failed_remote.get(3), Some(Settings));
        assert_eq!(failed_remote.last(), Some(Remove));

        // Structural coverage deliberately enables every conditional action:
        // the canonical array, not the current lifecycle, owns keyboard order.
        let all_actions = host_menu_order(
            true,
            true,
            ProvisioningMenuState {
                rerun: Some(ProvisioningOperation::Update),
                automatic_setup: true,
                update: true,
                planning: false,
            },
        );
        assert_eq!(all_actions.len(), 7);
        assert_eq!(all_actions.get(0), Some(Retry));
        assert_eq!(all_actions.get(1), Some(Adopt));
        assert_eq!(all_actions.get(2), Some(Rerun));
        assert_eq!(all_actions.get(3), Some(AutomaticSetup));
        assert_eq!(all_actions.get(4), Some(Update));
        assert_eq!(all_actions.get(5), Some(Settings));
        assert_eq!(all_actions.get(6), Some(Remove));
    }

    /// The client-side mirror of the helm's alias validation
    /// (`store::validate_alias` — same trim, same empty-clears rule, same
    /// control-character and length checks, same messages) so a syntax
    /// failure is caught before a round trip, without the two ever being
    /// allowed to drift on the RULES: the helm remains the authority that
    /// decides whether a value is accepted at all, since only it can see
    /// every other host's current display name.
    #[farhelm_testtrace::test]
    fn validate_alias_draft_mirrors_the_helms_rules() {
        assert_eq!(
            validate_alias_draft("  My Box  "),
            Ok(Some("My Box".to_string()))
        );
        assert_eq!(validate_alias_draft(""), Ok(None), "empty clears the alias");
        assert_eq!(
            validate_alias_draft("   "),
            Ok(None),
            "whitespace-only input clears the alias, same as empty"
        );
        assert_eq!(
            validate_alias_draft("bad\u{0007}name"),
            Err("host alias must not contain control characters".to_string())
        );
        assert_eq!(
            validate_alias_draft("bad\nname"),
            Err("host alias must not contain control characters".to_string()),
            "a newline is a control character too — the exact byte an alias must never carry, \
             since `farhelm agent` prints host names on stdout"
        );
        let sixty_five = "a".repeat(65);
        assert_eq!(
            validate_alias_draft(&sixty_five),
            Err("host alias must be at most 64 characters".to_string())
        );
        let sixty_four = "a".repeat(64);
        assert_eq!(
            validate_alias_draft(&sixty_four),
            Ok(Some(sixty_four)),
            "the cap is inclusive: exactly 64 characters is accepted"
        );
    }

    /// The `EditField` switch itself, isolated from the async submit
    /// handler that dispatches on it: a destination draft passes through
    /// untouched (the helm is the ssh-syntax authority), while an alias
    /// draft is run through [`validate_alias_draft`] first, and a refused
    /// alias draft must not silently fall back to being treated as a
    /// destination.
    #[farhelm_testtrace::test]
    fn resolve_edit_submission_switches_on_the_field() {
        assert!(matches!(
            resolve_edit_submission(EditField::Destination, "  not trimmed  "),
            Ok(EditSubmission::Destination(d)) if d == "  not trimmed  "
        ));
        assert!(matches!(
            resolve_edit_submission(EditField::Alias, "  My Box  "),
            Ok(EditSubmission::Alias(Some(a))) if a == "My Box"
        ));
        assert!(matches!(
            resolve_edit_submission(EditField::Alias, ""),
            Ok(EditSubmission::Alias(None))
        ));
        assert_eq!(
            resolve_edit_submission(EditField::Alias, "bad\u{0007}name"),
            Err("host alias must not contain control characters".to_string()),
            "a refused alias draft must surface validate_alias_draft's own error, not a \
             destination-shaped one"
        );
    }

    /// The value an adopt SENDS is the raw one; the value it SHOWS is
    /// escaped. Collapsing the two either way is a real failure: sending the
    /// escaped form turns every unusual identity into a spurious 409, and
    /// showing the raw form is exactly the spoofing hole the escaping exists
    /// to close.
    #[farhelm_testtrace::test]
    fn the_adopted_identity_is_raw_while_its_label_is_escaped() {
        let raw = "id-\u{202E}safe";
        let state = HostPhase::IdentityMismatch {
            recorded: "id-old".to_string(),
            reported: raw.to_string(),
        };
        assert_eq!(
            adoptable(&state),
            Some(raw),
            "the request body must carry the bytes the helm will compare"
        );
        let shown = display_peer(raw);
        assert!(
            shown.contains("<U+202E>") && !shown.contains('\u{202E}'),
            "the label must not carry a live directional override: {shown}"
        );
    }

    /// The host menu's accessible names must go through the same
    /// escaping every other rendered peer value does — a live bidi
    /// override or zero-width character surviving into `aria-label` would
    /// let assistive technology announce a host other than the one the
    /// sighted row shows, for a menu whose commands include Adopt, Edit,
    /// and Remove.
    #[farhelm_testtrace::test]
    fn host_menu_label_escapes_bidi_and_zero_width_characters() {
        let name = "safe\u{202E}evil\u{200B}host";
        let label = host_menu_label(name);
        assert!(
            !label.contains('\u{202E}') && !label.contains('\u{200B}'),
            "no live control character may reach the accessible name: {label:?}"
        );
        assert_eq!(
            label,
            format!("host actions for {}", display_peer(name)),
            "short enough not to clamp, so the label is exactly the escaped name"
        );
        assert!(label.contains("<U+202E>") && label.contains("<U+200B>"));
    }

    /// The mismatch's two identities must each be their own isolated run,
    /// with this UI's labels between them.
    ///
    /// This is the structural half of the anti-spoofing rule and cannot be
    /// asserted on flattened text: what makes the evidence tamper-proof is
    /// that `recorded` and `reported` never share an element with the words
    /// that say which is which.
    #[farhelm_testtrace::test]
    fn the_mismatch_evidence_keeps_each_identity_in_its_own_run() {
        let parts = state_detail(&HostPhase::IdentityMismatch {
            recorded: "id-old".to_string(),
            reported: "id-new".to_string(),
        });
        let peers: Vec<&str> = parts
            .iter()
            .filter_map(|part| match part {
                DetailPart::Peer(value) => Some(value.as_str()),
                DetailPart::Text(_) => None,
            })
            .collect();
        assert_eq!(
            peers,
            vec!["id-old", "id-new"],
            "both identities are peer runs, in the order the labels describe"
        );
        assert!(
            parts.iter().any(|part| matches!(
                part,
                DetailPart::Text(text) if text.contains("recorded as install")
            )),
            "this UI's own words carry which is which"
        );
    }

    /// The manual-start fallback belongs to exactly one cause, and it must be
    /// the helm's OWN sentence — which is the one that names the exact
    /// command, `--state-dir` and all (PLAN_M6.md item 7's contract-borne
    /// remedy).
    ///
    /// The state directory is the whole reason this is not written here: a
    /// helm reaches its local supervisor over the socket in the directory it
    /// was started with, that directory is not on `/api/hosts`, and a hint
    /// that said only `farhelm supervisor run` would send the user to start
    /// a supervisor their helm never dials. The realistic fixture is
    /// therefore the real dial failure's shape, and the assertion is that
    /// the command survives into the remedy verbatim rather than being
    /// paraphrased.
    ///
    /// The row must also not print that same long chain twice: the helm's
    /// text is a REMEDY, so the diagnosis line beside it says only what
    /// happened.
    #[farhelm_testtrace::test]
    fn only_the_local_supervisor_cause_gets_the_manual_start_hint() {
        let reported = "no supervisor is running on this machine: supervisor does not appear to \
                        be running (socket /srv/state/supervisor.sock is not accepting \
                        connections); start it with `farhelm supervisor run --state-dir \
                        /srv/state`: Connection refused (os error 111)";
        let down = HostPhase::Unreachable {
            cause: LOCAL_SUPERVISOR_NOT_RUNNING.to_string(),
            last_error: reported.to_string(),
        };
        let hint =
            detail_text(&state_remedy(&down).expect("the one unreachable cause with a remedy"));
        assert!(
            hint.contains("farhelm supervisor run --state-dir /srv/state"),
            "the exact command, state dir included, has to reach the user: {hint}"
        );
        assert!(
            !hint.contains("install"),
            "the automatic offer is rendered from the probe plan, never invented in this \
             fallback: {hint}"
        );
        let diagnosis = detail_text(&state_detail(&down));
        assert!(
            !diagnosis.contains("farhelm supervisor run"),
            "the command belongs to the remedy alone, not to both lines: {diagnosis}"
        );

        // A helm that reported nothing still gets a remedy, since an empty
        // one would be worse than an approximate one — and it is the only
        // case where this UI writes the command itself.
        let silent = detail_text(
            &state_remedy(&HostPhase::Unreachable {
                cause: LOCAL_SUPERVISOR_NOT_RUNNING.to_string(),
                last_error: String::new(),
            })
            .expect("a remedy is offered even with no reported error"),
        );
        assert!(
            silent.contains("farhelm supervisor run") && silent.contains("--state-dir"),
            "the fallback still names the command and the state-dir caveat: {silent}"
        );

        assert!(
            state_remedy(&HostPhase::Unreachable {
                cause: "transport-failure".to_string(),
                last_error: "connection refused".to_string(),
            })
            .is_none(),
            "an ordinary unreachable host re-probes forever on its own, so there is nothing to ask \
             the user to do"
        );
        assert!(
            state_remedy(&HostPhase::Connecting {
                attempt: 2,
                last_error: None,
            })
            .is_none(),
            "a connecting host needs patience, not action"
        );
    }

    /// The skew remedy must be the helm's sentence VERBATIM, as a peer run.
    /// The helm is the side that knows which binary is behind, and a second
    /// copy of that advice written here is the one that would drift.
    #[farhelm_testtrace::test]
    fn the_skew_remedy_is_the_helms_own_sentence() {
        let remedy = state_remedy(&HostPhase::VersionSkew {
            peer_protocol: 9,
            peer_build: "0.2.0".to_string(),
            our_protocol: 8,
            our_build: "0.1.0".to_string(),
            remediation: "update this helm to at least 0.2.0".to_string(),
        })
        .expect("a skew always has a remediation to print");
        assert_eq!(
            remedy,
            vec![DetailPart::Peer(
                "update this helm to at least 0.2.0".to_string()
            )],
        );
    }

    /// The four states have to be distinguishable, because two surfaces make
    /// opposite decisions from them.
    ///
    /// The one that matters most is the third: a failed refresh must keep
    /// the snapshot (so the list can keep drawing statuses) while still
    /// reporting the failure (so nothing claims to be current). A model that
    /// dropped the snapshot on failure blanks the one surface SPEC.md
    /// requires to always show host state.
    #[farhelm_testtrace::test]
    fn a_failed_hosts_read_keeps_the_last_snapshot_and_reports_the_failure() {
        let mut read = HostsRead::default();
        assert!(read.is_loading());
        assert!(read.hosts().is_none());
        assert!(read.refresh_error().is_none());

        read.record(Ok(vec![host(HostPhase::Connected {
            identity: None,
            build_version: "0.1.0".to_string(),
            old_version: false,
            newer_version: false,
            refresh: RefreshHealth::Pending,
        })]));
        assert!(!read.is_loading());
        assert_eq!(read.hosts().map(<[Host]>::len), Some(1));
        assert!(read.refresh_error().is_none());

        read.record(Err("the helm did not answer".to_string()));
        assert_eq!(
            read.hosts().map(<[Host]>::len),
            Some(1),
            "a dropped poll is not evidence that the fleet changed"
        );
        assert_eq!(read.refresh_error(), Some("the helm did not answer"));

        read.record(Ok(Vec::new()));
        assert_eq!(read.hosts().map(<[Host]>::len), Some(0));
        assert!(
            read.refresh_error().is_none(),
            "a success clears the failure it superseded"
        );
    }

    /// A failure with no snapshot behind it is its own state: there is
    /// nothing to draw and something to say. Conflating it with "loading"
    /// would leave a spinner on screen forever while the helm is down.
    #[farhelm_testtrace::test]
    fn a_first_read_that_fails_is_a_failure_rather_than_still_loading() {
        let mut read = HostsRead::default();
        read.record(Err("connection refused".to_string()));
        assert!(!read.is_loading());
        assert!(read.hosts().is_none());
        assert_eq!(read.refresh_error(), Some("connection refused"));
    }

    /// One host's lookup must distinguish "not read yet" from "read, and not
    /// there" from "the read failed" — three different sentences for the
    /// stale session view, and only one of them (`Known`) may ever show a
    /// phase.
    ///
    /// The failure precedence is the load-bearing part: after a failed
    /// refresh the view must NOT keep presenting the phase it last saw, or
    /// it would describe a possibly-recovered host as still down.
    #[farhelm_testtrace::test]
    fn a_host_lookup_separates_pending_absent_and_failed() {
        let mut read = HostsRead::default();
        assert_eq!(read.lookup(Some(1)), HostLookup::Pending);

        read.record(Ok(vec![host(HostPhase::Retired {
            reason: "gone".to_string(),
        })]));
        assert!(matches!(read.lookup(Some(1)), HostLookup::Known(_)));
        assert_eq!(
            read.lookup(Some(99)),
            HostLookup::Absent,
            "a current registry that does not contain it is a confirmed removal"
        );
        assert_eq!(
            read.lookup(None),
            HostLookup::Absent,
            "a session row naming no host has no host in the registry either"
        );

        read.record(Err("the helm did not answer".to_string()));
        assert_eq!(
            read.lookup(Some(1)),
            HostLookup::Failed("the helm did not answer"),
            "a stale phase must never be presented as the current one"
        );
    }

    /// A stale session identifies its host and carries the supplied remedy
    /// rather than leaving the user with a generic connection failure.
    #[farhelm_testtrace::test]
    fn the_stale_notice_identifies_the_host_and_carries_its_remedy() {
        let skewed = host(HostPhase::VersionSkew {
            peer_protocol: 9,
            peer_build: "0.2.0".to_string(),
            our_protocol: 8,
            our_build: "0.1.0".to_string(),
            remediation: "update the host's farhelm binary".to_string(),
        });
        let notice = detail_text(&stale_session_notice(
            "user@box",
            HostLookup::Known(&skewed),
        ));
        assert!(notice.contains("user@box"), "the host is named: {notice}");
        assert!(
            notice.contains("update the host's farhelm binary"),
            "the remedy travels with the notice, or the user is told only that they are stuck: \
             {notice}"
        );
        assert!(
            notice.contains("no terminal"),
            "SPEC.md: there is no terminal to show for such a session: {notice}"
        );
    }

    /// The three non-`Known` lookups each say their own thing. They are
    /// genuinely different situations — nothing read yet, a refresh that
    /// failed, and a host that is gone from the registry — and the last one
    /// is not a connection problem at all, so describing it as one would
    /// send the user looking for a network fault that does not exist.
    #[farhelm_testtrace::test]
    fn the_stale_notice_distinguishes_unread_unrefreshed_and_unregistered() {
        let unread = detail_text(&stale_session_notice("user@box", HostLookup::Pending));
        assert!(unread.contains("has not been read yet"), "{unread}");

        let unrefreshed = detail_text(&stale_session_notice(
            "user@box",
            HostLookup::Failed("connection refused"),
        ));
        assert!(
            unrefreshed.contains("could not be refreshed")
                && unrefreshed.contains("connection refused"),
            "{unrefreshed}"
        );

        let gone = detail_text(&stale_session_notice("user@box", HostLookup::Absent));
        assert!(
            gone.contains("no longer registered") && gone.contains("Re-adding"),
            "a removed host's remedy is registration, not waiting: {gone}"
        );
    }

    /// `is_connected` decides presentation only, and must agree with the
    /// helm about what "connected" means — the session rows' stale marking
    /// and the selector's phase labelling both key off it.
    #[farhelm_testtrace::test]
    fn only_a_connected_host_counts_as_connected() {
        for state in every_phase() {
            assert_eq!(
                is_connected(&state),
                matches!(state, HostPhase::Connected { .. }),
                "{} is misclassified",
                phase_label(&state)
            );
        }
    }

    /// A compatible older peer keeps the connected wire token and all live
    /// host behavior, while the row exposes an amber advisory label. A
    /// version-skew refusal remains the separate red “needs update” case.
    #[farhelm_testtrace::test]
    fn old_connected_hosts_have_advisory_display_without_skew_classification() {
        let old = HostPhase::Connected {
            identity: None,
            build_version: "0.13.0".to_string(),
            old_version: true,
            newer_version: false,
            refresh: RefreshHealth::Pending,
        };
        assert_eq!(phase_label(&old), "connected");
        assert_eq!(phase_display_label(&old), "old version");
        assert_eq!(phase_class(&old), "old-version");
        assert!(is_connected(&old));

        let current = HostPhase::Connected {
            identity: None,
            build_version: "0.14.0-rc.2".to_string(),
            old_version: false,
            newer_version: false,
            refresh: RefreshHealth::Pending,
        };
        assert_eq!(phase_display_label(&current), "connected");
        assert_eq!(phase_class(&current), "connected");

        let skew = HostPhase::VersionSkew {
            peer_protocol: 1,
            peer_build: "0.13.0".to_string(),
            our_protocol: 2,
            our_build: "0.14.0-rc.2".to_string(),
            remediation: "update".to_string(),
        };
        assert_eq!(phase_display_label(&skew), "needs update");
        assert_eq!(phase_class(&skew), "needs-attention");
        assert_eq!(too_new_title(&skew), None);
    }

    /// Inline updates are only for older remote hosts whose menu can start one.
    /// This crosses age with host kind and live availability so an old label
    /// cannot become an action while setup is running or before options load.
    #[farhelm_testtrace::test]
    fn inline_update_eligibility_requires_age_and_remote_availability() {
        let old = HostPhase::Connected {
            identity: None,
            build_version: "0.13.0".into(),
            old_version: true,
            newer_version: false,
            refresh: RefreshHealth::Pending,
        };
        let required = HostPhase::VersionSkew {
            peer_protocol: 1,
            peer_build: "0.13.0".into(),
            our_protocol: 2,
            our_build: "0.14.0".into(),
            remediation: "update".into(),
        };
        let menu = ProvisioningMenuState {
            update: true,
            ..Default::default()
        };
        let mut cases: Vec<_> = every_phase()
            .into_iter()
            .map(|state| (state, None))
            .collect();
        cases.extend([
            (old.clone(), Some("optional")),
            (required, Some("required")),
        ]);
        let mut newer = old.clone();
        if let HostPhase::Connected { newer_version, .. } = &mut newer {
            *newer_version = true;
        }
        let mut newer_host = row_specimen(1);
        newer_host.state = newer.clone();
        assert!(!remote_update_available(&newer_host, Some(&menu), false));
        cases.push((newer, None));
        cases.push((
            HostPhase::VersionSkew {
                peer_protocol: 8,
                peer_build: "0.13.0".into(),
                our_protocol: 8,
                our_build: "0.14.0".into(),
                remediation: "unknown skew".into(),
            },
            None,
        ));
        for (state, expected) in cases {
            assert_eq!(host_update_kind(&state), expected, "{state:?}");
            for kind in [HostKind::Ssh, HostKind::Local, HostKind::Unrecognized] {
                let mut host = row_specimen(1);
                host.kind = kind;
                host.state = state.clone();
                for (offer, busy, available) in [
                    (Some(&menu), false, true),
                    (None, false, false),
                    (Some(&menu), true, false),
                ] {
                    let actual = remote_update_available(&host, offer, busy)
                        .then(|| host_update_kind(&host.state))
                        .flatten();
                    assert_eq!(
                        actual,
                        if kind == HostKind::Ssh && available {
                            expected
                        } else {
                            None
                        }
                    );
                }
                assert!(!remote_update_available(
                    &host,
                    Some(&ProvisioningMenuState::default()),
                    false
                ));
            }
        }
    }

    /// Tooltips must explain both urgency and the installed version, without
    /// allowing a peer's control characters to disguise which side is older.
    #[farhelm_testtrace::test]
    fn inline_update_hover_names_versions_urgency_and_action() {
        let old = HostPhase::Connected {
            identity: None,
            build_version: "old\u{202e}build".into(),
            old_version: true,
            newer_version: false,
            refresh: RefreshHealth::Pending,
        };
        let hover = host_update_title(&old).unwrap();
        for word in [
            display_peer("old\u{202e}build").as_str(),
            crate::skew::CLIENT_BUILD,
            "optional",
            "click to update this host to the helm's version",
        ] {
            assert!(hover.contains(word), "{hover}");
        }
        assert!(!hover.contains('\u{202e}'));
        let skew = HostPhase::VersionSkew {
            peer_protocol: 1,
            peer_build: "peer\u{202e}build".into(),
            our_protocol: 2,
            our_build: "helm\u{202e}build".into(),
            remediation: "update".into(),
        };
        let hover = host_update_title(&skew).unwrap();
        for word in [
            display_peer("peer\u{202e}build").as_str(),
            display_peer("helm\u{202e}build").as_str(),
            "protocol 1",
            "protocol 2",
            "required",
            "click to update this host to the helm's version",
        ] {
            assert!(hover.contains(word), "{hover}");
        }
        assert!(!hover.contains('\u{202e}'));
        assert_eq!(host_update_title(&HostPhase::Unrecognized), None);
    }

    /// A host running a newer farhelm than the helm is labelled "too new",
    /// connected or skewed, and the label's hover carries both versions.
    ///
    /// Why it matters: such a host used to read "needs update" (a skew) or
    /// plain "connected", inviting an Update that would install the helm's
    /// older build over it, and nothing said that the helm was the side
    /// behind. Specified: a connected host the helm calls newer, and a
    /// skewed host with the higher protocol, both read "too new", with a
    /// hover naming the host's build, the helm's and the protocols; the
    /// connected one keeps the amber advisory colour and stays connected.
    #[farhelm_testtrace::test]
    fn a_newer_host_reads_too_new_with_both_versions_on_hover() {
        let connected = HostPhase::Connected {
            identity: None,
            build_version: "9.0.0".to_string(),
            old_version: false,
            newer_version: true,
            refresh: RefreshHealth::Pending,
        };
        assert!(runs_newer_version(&connected));
        assert_eq!(phase_display_label(&connected), "too new");
        assert_eq!(phase_class(&connected), "too-new");
        assert!(is_connected(&connected));
        let hover = too_new_title(&connected).expect("a too-new host has a hover");
        assert!(
            hover.contains("9.0.0") && hover.contains(crate::skew::CLIENT_BUILD),
            "{hover}"
        );
        assert!(hover.contains("update the helm"), "{hover}");

        let skew = HostPhase::VersionSkew {
            peer_protocol: 3,
            peer_build: "9.0.0".to_string(),
            our_protocol: 2,
            our_build: "0.14.0".to_string(),
            remediation: "update this helm".to_string(),
        };
        assert!(runs_newer_version(&skew));
        assert_eq!(phase_display_label(&skew), "too new");
        let hover = too_new_title(&skew).expect("a too-new skew has a hover");
        for expected in ["9.0.0", "protocol 3", "0.14.0", "protocol 2"] {
            assert!(
                hover.contains(expected),
                "{expected:?} missing from {hover:?}"
            );
        }
    }

    /// A connected host's row must report how its last cache refresh went,
    /// beside the connection rather than as part of it: a failed refresh
    /// leaves the host perfectly connected while its listed sessions are
    /// last-known, and reading that as a disconnection would be wrong in
    /// both directions.
    #[farhelm_testtrace::test]
    fn a_connected_hosts_detail_reports_its_refresh_health() {
        let failing = detail_text(&state_detail(&HostPhase::Connected {
            identity: None,
            build_version: "0.1.0".to_string(),
            old_version: false,
            newer_version: false,
            refresh: RefreshHealth::Failed {
                error: "list timed out".to_string(),
            },
        }));
        assert!(
            failing.contains("last-known") && failing.contains("list timed out"),
            "a failed refresh must say what it costs and why: {failing}"
        );
        assert!(
            failing.contains("no identity reported"),
            "an identity-less connected host says so rather than showing a blank: {failing}"
        );

        let pending = detail_text(&state_detail(&HostPhase::Connected {
            identity: Some("id".to_string()),
            build_version: "0.1.0".to_string(),
            old_version: false,
            newer_version: false,
            refresh: RefreshHealth::Pending,
        }));
        assert!(pending.contains("still in flight"), "{pending}");
    }

    // -----------------------------------------------------------------
    // Host-row prop memoization
    // -----------------------------------------------------------------
    //
    // These tests exist because `HostRow`'s twenty props were regrouped into
    // `HostRowControls` and `HostRowActivity`, and the way a grouping breaks
    // — a prop that stops comparing equal when nothing about the row changed
    // — is invisible to every other test in this crate. A rerender leaves no
    // trace in the rendered DOM, so the only oracle is a count.
    //
    // The first two are the host row's copy of the session row's regressions
    // (`list::row`) and drive the real `HostRow` from a parent that behaves:
    // stable `use_callback` handles created outside the row loop, and the two
    // `Element` sections held at `VNode::empty()`, whose backing `Rc` is one
    // reused thread-local so it compares equal to itself. The third pins the
    // half of the direct-prop rule that is about memoization — a nested
    // callback compares equal only while its handle is stable — since that
    // rule had never been executable anywhere; it does NOT exercise the
    // in-place handler update Dioxus gives direct callback props, which is
    // the other reason the handlers stay direct.
    //
    // NOTE on what these do NOT show. That parent is deliberately better
    // behaved than `HostsPanel`, and no host row memoizes in production
    // today. Measured 2026-08-23 against this same row, one build plus eight
    // parent refreshes, so nine renders means no memoization at all:
    // rebuilding either section with `rsx!` instead of holding it stable
    // costs nine, and handing the host verbs to the row as inline closures
    // costs nine. `HostsPanel` does both. Neither is something a state
    // grouping can move — one is the panel's choice to hand the row built
    // markup, the other its choice not to hold `use_callback` handles outside
    // the row loop — so what these tests pin is the row's own end of the
    // contract, which is the end this shape could break.

    /// One host row as the render-count regressions need it: real enough for
    /// `HostRow` to render its ordinary control strip, and stable so that
    /// rebuilding it on every parent render compares equal and memoization
    /// is the only thing the counts can be measuring.
    fn row_specimen(id: HostId) -> Host {
        Host {
            id,
            kind: HostKind::Ssh,
            destination: Some("user@box".to_string()),
            alias: None,
            name: "user@box".to_string(),
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            state: HostPhase::Connected {
                identity: Some("stable".to_string()),
                build_version: "0.1.0".to_string(),
                old_version: false,
                newer_version: false,
                refresh: RefreshHealth::Ok { sessions: 0 },
            },
            incarnation: 1,
            yolo_without_asking: false,
        }
    }

    /// A published Update offer can lag the parent's run-start notice.
    /// The header must skip that busy row even while a separate remote row
    /// remains eligible, or a fleet click can queue work its row refuses.
    #[farhelm_testtrace::test]
    fn fleet_updates_recheck_live_busy_hosts_after_menu_publication() {
        let ready = row_specimen(1);
        let busy = row_specimen(2);
        let mut local = row_specimen(3);
        local.kind = HostKind::Local;
        let hosts = [ready, busy, local];
        let menus = hosts
            .iter()
            .map(|host| {
                (
                    host.id,
                    ProvisioningMenuState {
                        update: true,
                        ..ProvisioningMenuState::default()
                    },
                )
            })
            .collect();
        let busy_hosts = HashSet::from([2]);

        let requests = available_remote_updates(&hosts, &menus, &busy_hosts);
        assert_eq!(requests.keys().copied().collect::<Vec<_>>(), vec![1]);
    }

    /// "Update all" skips a host running a newer farhelm than the helm,
    /// even when its row's menu state would offer Update.
    ///
    /// Why it matters: "update all" took every eligible remote row, and an
    /// Update on a newer host installs the helm's older build over it. The
    /// row's own menu stops offering Update for such a host; this pins the
    /// fleet action independently, since it reads published menu states
    /// that can lag a host's state. Specified: of two eligible remote rows,
    /// the one reporting a newer build is left out.
    #[farhelm_testtrace::test]
    fn update_all_skips_a_host_running_a_newer_farhelm() {
        let current = row_specimen(1);
        let mut newer = row_specimen(2);
        newer.state = HostPhase::Connected {
            identity: Some("stable".to_string()),
            build_version: "9.0.0".to_string(),
            old_version: false,
            newer_version: true,
            refresh: RefreshHealth::Ok { sessions: 0 },
        };
        let hosts = [current, newer];
        let menus = hosts
            .iter()
            .map(|host| {
                (
                    host.id,
                    ProvisioningMenuState {
                        update: true,
                        ..ProvisioningMenuState::default()
                    },
                )
            })
            .collect();

        let requests = available_remote_updates(&hosts, &menus, &HashSet::new());
        assert_eq!(requests.keys().copied().collect::<Vec<_>>(), vec![1]);
    }

    /// A host row whose state did not change must not rerender when its
    /// parent does, however many times the parent does.
    ///
    /// This is the contract the whole prop shape is arranged around, and the
    /// one a regrouping can lose outright: every prop `HostRow` takes has to
    /// be able to compare equal to its own previous value, or the row repaints
    /// on every fleet refresh. Sixty-four refreshes rather than one, because a
    /// prop that compares equal by accident on the first pass — a recycled
    /// allocation, say — will not keep doing it.
    #[farhelm_testtrace::test]
    fn repeated_parent_refreshes_do_not_rerender_an_unchanged_host_row() {
        fn app() -> Element {
            let on_retry = use_callback(|_: HostId| {});
            let on_adopt = use_callback(|_: (HostId, String)| {});
            let on_remove_start = use_callback(|_: HostId| {});
            let on_provisioning = use_callback(|_: (HostId, ActionRequest)| {});
            let on_menu_toggle = use_callback(|_: HostId| {});
            rsx! {
                HostRow {
                    host: row_specimen(1),
                    local_setup: false,
                    controls: HostRowControls {
                        menu_open: false,
                    },
                    activity: HostRowActivity {
                        busy: false,
                        error: None,
                        warning: None,
                    },
                    details_open: false,
                    update_progress: None,
                    provisioning_menu: ProvisioningMenuState::default(),
                    remote_update_available: false,
                    provisioning_section: dioxus::core::VNode::empty(),
                    on_retry,
                    on_adopt,
                    on_settings_start: use_callback(|_: HostId| {}),
                    on_remove_start,
                    on_provisioning,
                    on_menu_toggle,
                }
            }
        }

        HOST_ROW_RENDERS.with(|renders| renders.borrow_mut().clear());
        let mut dom = VirtualDom::new(app);
        dom.rebuild_to_vec();
        for _ in 0..64 {
            dom.mark_dirty(dioxus::core::ScopeId::APP);
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }
        assert_eq!(
            host_row_renders(),
            vec![(1, 1)],
            "an unchanged host row must stay memoized across fleet refreshes"
        );
    }

    /// A change to one host's state rerenders exactly the rows it describes,
    /// and leaves every other row memoized — proved with one representative
    /// field from each of the two state groups.
    ///
    /// This is the cost model the grouping has to preserve. Both structs are
    /// compared by value, so changing the menu state is limited to the row
    /// that owns it and a refusal landing on one host is one render — not a
    /// fleet-wide repaint. A field that
    /// drops out of what memoization compares, or a group that starts
    /// invalidating rows it does not describe, moves these per-row counts.
    /// Exercising `HostRowControls` and `HostRowActivity` in the same virtual
    /// DOM is what makes the test say which of the two broke.
    #[farhelm_testtrace::test]
    fn only_the_host_rows_whose_state_changed_rerender() {
        // Both facts live OUTSIDE the virtual DOM — the test moves them
        // between renders the way `HostsPanel`'s own signals move between
        // its renders — and the app re-derives every row's state from them
        // on each parent render, so memoization alone decides which rows
        // actually run.
        std::thread_local! {
            static REFUSED: std::cell::Cell<Option<HostId>> =
                const { std::cell::Cell::new(None) };
        }

        fn app() -> Element {
            let on_retry = use_callback(|_: HostId| {});
            let on_adopt = use_callback(|_: (HostId, String)| {});
            let on_remove_start = use_callback(|_: HostId| {});
            let on_provisioning = use_callback(|_: (HostId, ActionRequest)| {});
            let on_menu_toggle = use_callback(|_: HostId| {});
            let refused = REFUSED.with(std::cell::Cell::get);
            rsx! {
                for id in [1_i64, 2, 3] {
                    HostRow {
                        key: "{id}",
                        host: row_specimen(id),
                        local_setup: false,
                        controls: HostRowControls { menu_open: false },
                        activity: HostRowActivity {
                            busy: false,
                            error: (refused == Some(id))
                                .then(|| "the helm refused this verb".to_string()),
                            warning: None,
                        },
                        details_open: false,
                        provisioning_menu: ProvisioningMenuState::default(),
                        remote_update_available: false,
                        provisioning_section: dioxus::core::VNode::empty(),
                        on_retry,
                        on_adopt,
                        on_settings_start: use_callback(|_: HostId| {}),
                        on_remove_start,
                        on_provisioning,
                        on_menu_toggle,
                    }
                }
            }
        }

        REFUSED.with(|refused| refused.set(None));
        HOST_ROW_RENDERS.with(|renders| renders.borrow_mut().clear());
        let mut dom = VirtualDom::new(app);
        dom.rebuild_to_vec();
        assert_eq!(
            host_row_renders(),
            vec![(1, 1), (2, 1), (3, 1)],
            "the initial build renders every row once"
        );

        // `HostRowActivity`: a refusal is per-host, so it costs exactly the
        // one row that has to show it.
        REFUSED.with(|refused| refused.set(Some(3)));
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            host_row_renders(),
            vec![(1, 1), (2, 1), (3, 2)],
            "a refusal landing on one host must rerender only that host's row"
        );
    }

    /// Grouping a callback into a props struct keeps memoization only while
    /// the handle behind it is stable; a freshly built one costs it outright.
    ///
    /// This is the memoization half of why [`HostRowControls`] and
    /// [`HostRowActivity`] are state-only, and until now it existed only as
    /// prose — lore/PLAN_M7.md item 5 records eleven reviewers concluding that
    /// a callback props struct "does not survive contact with Dioxus", which
    /// is true of the shape the session list tried and not true as a blanket
    /// rule. What decides the outcome is whether the handler's box is the
    /// same one as last render (within one parent scope, box identity is the
    /// deciding variable; `Callback` equality also requires the same
    /// originating scope), and a struct field is compared by exactly the
    /// equality a direct prop is.
    ///
    /// What this test does NOT measure is the other half: a DIRECT
    /// `EventHandler` prop additionally gets Dioxus's in-place handler update,
    /// so a retained component sees a fresh closure's captured state; a
    /// nested one does not. That freshness is the second reason every
    /// handler stays direct, and it is not covered by a render count — so a
    /// framework change that made fresh handlers memoize would NOT make
    /// nesting them safe on its own. The trap the direct-prop rule removes is
    /// that a struct literal in `rsx!` is where inline closures naturally get
    /// written, each render minting a new box.
    #[farhelm_testtrace::test]
    fn grouping_a_callback_is_safe_only_while_its_handle_is_stable() {
        std::thread_local! {
            static PROBE_RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        }

        /// A minimal stand-in for the `HostRowActions` this row deliberately
        /// does not have. Kept local to the test so nothing in the module can
        /// grow a use for it.
        #[derive(Clone, PartialEq)]
        struct Actions {
            on_verb: EventHandler<()>,
        }

        // The handler is never called; only whether the props compared equal
        // is under test, and the render count is the only way to read that.
        #[component]
        fn Probe(actions: Actions) -> Element {
            let _ = actions;
            PROBE_RENDERS.with(|renders| renders.set(renders.get() + 1));
            rsx! { div {} }
        }

        fn stable_handle() -> Element {
            let on_verb = use_callback(|_: ()| {});
            rsx! { Probe { actions: Actions { on_verb } } }
        }

        fn fresh_handler() -> Element {
            rsx! {
                Probe { actions: Actions { on_verb: EventHandler::new(|_: ()| {}) } }
            }
        }

        fn renders_over_eight_refreshes(app: fn() -> Element) -> usize {
            PROBE_RENDERS.with(|renders| renders.set(0));
            let mut dom = VirtualDom::new(app);
            dom.rebuild_to_vec();
            for _ in 0..8 {
                dom.mark_dirty(dioxus::core::ScopeId::APP);
                dom.render_immediate(&mut dioxus::core::NoOpMutations);
            }
            PROBE_RENDERS.with(std::cell::Cell::get)
        }

        assert_eq!(
            renders_over_eight_refreshes(stable_handle),
            1,
            "a `use_callback` handle inside a props struct still compares equal, \
             so nesting one is not by itself what costs memoization"
        );
        assert_eq!(
            renders_over_eight_refreshes(fresh_handler),
            9,
            "a handler built fresh each render never compares equal, so the child \
             repaints on every parent render — one per refresh plus the build"
        );
    }

    /// Elapsed time stays compact for ordinary updates and remains unambiguous after an hour.
    #[farhelm_testtrace::test]
    fn host_update_elapsed_time_uses_minute_or_hour_clock() {
        assert_eq!(format_elapsed(0), "0:00");
        assert_eq!(format_elapsed(42), "0:42");
        assert_eq!(format_elapsed(3_599), "59:59");
        assert_eq!(format_elapsed(3_600), "1:00:00");
        assert_eq!(format_elapsed(93_784), "26:03:04");
    }
}
