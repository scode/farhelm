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
//! for automation and machine-authored diagnostics.
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

use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;

use crate::api::{
    Commit, ProbeResponse, ProvisioningOperation, ProvisioningSubmission, adopt_host,
    probe_ssh_host, provision_host, remove_host, retry_host, set_host_destination,
};
use crate::icons::{LocalHostIcon, RemoteHostIcon};
use crate::menu_panel::{
    self, MenuFocusQueue, MenuOpenIntent, PanelPlacement, cancel_menu_focus, clamp_title,
    closed_toggle_key_intent, focus_menu_toggle, forget_menu_focus, handle_menu_key,
    measurement_outcome, menu_panel_placement_style, remember_menu_item, should_measure_on_mount,
};
use crate::ops::OpLock;
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::provisioning::{
    PlanConfirmation, ProvisioningMenuState, ProvisioningPanel, ProvisioningTraceShape,
};
use crate::{ApiBase, Host, HostId, HostKind, HostPhase, RefreshHealth};

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
    match state {
        HostPhase::Connecting { .. } => "connecting",
        HostPhase::Unreachable { .. } => "unreachable, retrying",
        HostPhase::Connected { .. } => "connected",
        HostPhase::VersionSkew { .. } => "version skew",
        HostPhase::IdentityMismatch { .. } => "identity mismatch",
        HostPhase::IdentityUnverified { .. } => "identity unverified",
        HostPhase::Duplicate { .. } => "duplicate",
        HostPhase::Retired { .. } => "retired",
        HostPhase::Unrecognized => "unrecognized",
    }
}

/// The CSS modifier the row status carries, grouping the phases by what a
/// person watching the panel should do about them: nothing yet
/// (`connecting`), nothing at all (`unreachable-reprobing` — it re-probes
/// forever and recovers unaided), all is well (`connected`), or look at this
/// (everything else, which stays exactly as it is until someone acts).
///
/// Deliberately coarser than [`phase_label`]: color is a category signal and
/// eight colors would be noise, while the exact phase is right there in
/// words beside it. The last group is `needs-attention` rather than
/// "decide", because only the two identity states are decisions — a skew
/// wants a binary upgraded, a duplicate wants the entry edited or removed,
/// and a retired row wants a retry.
pub(crate) fn phase_class(state: &HostPhase) -> &'static str {
    match state {
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
/// Composed from what the hosts snapshot exposes rather than from a server
/// token, because the frozen `/api/hosts` contract has no incarnation of its
/// own; if it grows one, this is the single place that changes. Serialized
/// as JSON rather than joined with a separator so no field's contents can
/// impersonate a boundary — `a|b` and `a` + `|b` are the same string, and
/// these are peer-supplied values.
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
/// Named rather than spelled out at each of its two match arms: the string
/// is a cross-crate coupling, and the diagnosis and the remedy have to key
/// off exactly the same one or the row would explain one state while
/// prescribing for another.
const LOCAL_SUPERVISOR_NOT_RUNNING: &str = "local-supervisor-not-running";

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
            DetailPart::peer(recorded),
            DetailPart::text("; the destination now reports "),
            DetailPart::peer(reported),
            DetailPart::text(", so nothing is connected until this is decided"),
        ],
        HostPhase::IdentityUnverified { recorded } => vec![
            DetailPart::text(
                "the host answered without an identity, so this helm cannot confirm it is still \
                 the install recorded as ",
            ),
            DetailPart::peer(recorded),
        ],
        HostPhase::Duplicate { twin, identity } => vec![
            DetailPart::text("this entry reaches install "),
            DetailPart::peer(identity),
            DetailPart::text(format!(
                ", which host {twin} already holds — the host itself is listed once, under that \
                 entry"
            )),
        ],
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
            "adopt the identity the host reports, or fix the destination",
        )]),
        // Adopt is deliberately absent from this list of remedies, because
        // it is absent from the host's options: see `adoptable`.
        HostPhase::IdentityUnverified { .. } => Some(vec![DetailPart::text(
            "fix the host so it reports its identity, or retarget or remove this entry — it is \
             re-probed meanwhile, so a host that starts identifying itself again recovers on its \
             own",
        )]),
        HostPhase::Duplicate { .. } => Some(vec![DetailPart::text(
            "edit this entry to a different host, or remove it",
        )]),
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
                " is {phase}, so there is no terminal to show — everything below is the helm's \
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
/// ## Why remove confirms in-page
///
/// wry ships no native JS dialogs on macOS's WKWebView (observed directly
/// running the desktop build), so `window.confirm()` silently does nothing
/// there — the same discovery that replaced the session list's delete
/// confirmation with an inline prompt. This one follows that established
/// pattern exactly, down to focusing cancel: consequence first, then the
/// host being forgotten, then confirm/cancel.
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
    /// Which host row's "⋯" menu is open, if any — `ListView`'s signal, kept
    /// in step with `session_menu_open` below so at most one row menu is
    /// ever open across the whole sidebar (see this component's own doc).
    mut host_menu_open: Signal<Option<HostId>>,
    /// The session list's own open-menu signal — written (never read) here,
    /// purely to close a session row's menu when a host row's opens.
    mut session_menu_open: Signal<Option<String>>,
    /// The fixed filter popover must close when details move its session-header
    /// anchor, just as it did when the former full host panel changed height.
    mut filter_open: Signal<bool>,
    on_changed: EventHandler<()>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    // Per-host rather than one shared slot, the discipline `ListView` keeps
    // for its session errors and for the same reason: a retry failing on one
    // host must not blank out an adopt refusal on another that the user has
    // not read yet.
    let mut errors = use_signal(HashMap::<HostId, String>::new);
    // Committed-but-unvalidated replies, kept apart from `errors` because
    // they mean the opposite thing — see this component's docs.
    let mut warnings = use_signal(HashMap::<HostId, String>::new);
    // Which row is showing the in-page removal confirmation, and which is
    // showing its destination field. One at a time for both: they replace
    // the row's controls, so two open at once would be two half-finished
    // decisions competing for the same space.
    let mut confirming_remove = use_signal(|| None::<HostId>);
    let mut editing = use_signal(|| None::<HostId>);
    let mut destination_draft = use_signal(String::new);
    let mut adding = use_signal(|| false);
    // One disclosure controls every row. It stays client-local and resets on
    // page mount; a host refresh changes facts, not the user's chosen level
    // of detail.
    let mut details_open = use_signal(|| false);
    // Provisioning stays mounted in each row, while its commands render in
    // the row-owned floating menu. These maps are the narrow bridge between
    // those two render locations: summaries flow out, one-shot requests in.
    let provisioning_menu_states = use_signal(HashMap::<HostId, ProvisioningMenuState>::new);
    let mut provisioning_action_requests =
        use_signal(HashMap::<HostId, ProvisioningOperation>::new);
    // An add that committed with an unreadable reply, which has no row to
    // sit on — see the form's `on_added`.
    let mut add_warning = use_signal(|| None::<String>);

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
        filter_open.set(false);
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
    // Returns whether the request was actually STARTED, which only the edit
    // path acts on: it closes its field on submit, and closing it for a
    // submit that was refused would throw the draft away with nothing said.
    let mut run = move |host: HostId, request: HostRequest| -> bool {
        if provisioning_busy_hosts.peek().contains(&host) {
            return false;
        }
        if !ops.claim() {
            return false;
        }
        mutation_busy_hosts.write().insert(host);
        errors.write().remove(&host);
        warnings.write().remove(&host);
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
            // Released on every path. A leaked token leaves the whole page
            // inert with nothing on screen to explain why, which is a far
            // worse failure than any of the outcomes above.
            ops.release();
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
        if !*details_open.peek() {
            filter_open.set(false);
        }
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
        if !*details_open.peek() {
            filter_open.set(false);
        }
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
    let on_remove_confirm = move |host: HostId| {
        // Only ever proceeds when this host is still the one being
        // confirmed, which is what keeps a confirm click queued behind a
        // cancel (both fired in one burst) from forgetting a host the user
        // just backed out of.
        if *confirming_remove.peek() != Some(host) {
            return;
        }
        confirming_remove.set(None);
        let base = remove_base.clone();
        // A removal has no host row to report back, so it confirms itself:
        // the 200 IS the whole answer, and there is no body for a decode to
        // fail on.
        run(
            host,
            Box::pin(async move { remove_host(&base, host).await.map(|()| Commit::Confirmed) }),
        );
    };

    let edit_base = base.clone();
    let on_edit_submit = move |(host, destination): (HostId, String)| {
        let base = edit_base.clone();
        let started = run(
            host,
            Box::pin(async move { set_host_destination(&base, host, &destination).await }),
        );
        // Closed once the request is actually OUT: its outcome lands in this
        // row's error line either way, so leaving the field open would only
        // invite a second submit of the same edit. A submit that was refused
        // leaves the field exactly as typed instead — closing it would
        // discard a draft nothing had been done with.
        if started && editing.peek().as_ref() == Some(&host) {
            editing.set(None);
        }
    };

    let read = hosts.read();
    let rendered_hosts = read.hosts().map(|list| {
        list.iter()
            .cloned()
            .map(|host| {
                let local_setup = host.kind == HostKind::Local
                    && matches!(
                        &host.state,
                        HostPhase::Unreachable { cause, .. }
                            if cause == LOCAL_SUPERVISOR_NOT_RUNNING
                    );
                (host, local_setup)
            })
            .collect::<Vec<_>>()
    });
    // Cosmetic, not the guard — every handler below claims the token for
    // itself (see the `ops` module).
    let busy = ops.busy();
    rsx! {
        section { class: "hosts-panel",
            div { class: "hosts-header",
                div { class: "host-count",
                    if let Some(hosts) = read.hosts() {
                        if hosts.len() == 1 { "1 host" } else { "{hosts.len()} hosts" }
                    } else {
                        "hosts"
                    }
                }
                div { class: "hosts-header-controls",
                button {
                    r#type: "button",
                    class: "btn host-details-toggle",
                    aria_expanded: details_open(),
                    onclick: move |_| {
                        details_open.toggle();
                        // Every row changes height together. A fixed menu is
                        // measured once, and the filter popover is fixed to
                        // the session header, so neither can survive that
                        // reflow with trustworthy geometry.
                        host_menu_open.set(None);
                        session_menu_open.set(None);
                        filter_open.set(false);
                    },
                    "details"
                }
                button {
                    r#type: "button",
                    class: "btn add-host-button",
                    // This control UNMOUNTS the add form, so it must not act
                    // while a MUTATION is in flight: dropping the component
                    // mid-request strands the response with nothing left to
                    // act on it. Reads are not what the token covers — the
                    // page reads constantly and none of those care whether
                    // this form exists. The token is read synchronously in
                    // the handler for the same reason every other guard here
                    // is — the attribute is one render behind.
                    disabled: busy,
                    onclick: move |_| {
                        if ops.busy_now() {
                            return;
                        }
                        let open = adding();
                        adding.set(!open);
                    },
                    "add host"
                }
                }
            }
            if adding() {
                AddHostForm {
                    ops,
                    on_refresh: on_changed,
                    on_added: move |unvalidated: Option<String>| {
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
                                confirming_remove: *confirming_remove.read() == Some(host.id),
                                editing: *editing.read() == Some(host.id),
                                menu_open: *host_menu_open.read() == Some(host.id),
                            },
                            activity: HostRowActivity {
                                busy: mutation_busy_hosts.read().contains(&host.id)
                                    || provisioning_busy_hosts.read().contains(&host.id)
                                    || busy,
                                error: errors.read().get(&host.id).cloned(),
                                warning: warnings.read().get(&host.id).cloned(),
                            },
                            details_open: details_open(),
                            provisioning_menu: provisioning_menu_states
                                .read()
                                .get(&host.id)
                                .copied()
                                .unwrap_or_default(),
                            local_setup,
                            provisioning_section: rsx! {
                                ProvisioningPanel {
                                    host: host.clone(),
                                    ops,
                                    details_open: details_open(),
                                    local_setup,
                                    manual_remedy: state_remedy(&host.state),
                                    action_requests: provisioning_action_requests,
                                    menu_states: provisioning_menu_states,
                                    trace_shapes: provisioning_trace_shapes,
                                    on_reveal_details: move |_| {
                                        filter_open.set(false);
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
                            destination_draft,
                            on_retry: on_retry.clone(),
                            on_adopt: on_adopt.clone(),
                            on_edit_start: move |(id, destination): (HostId, String)| {
                                if ops.busy_now()
                                    || provisioning_busy_hosts.peek().contains(&id)
                                {
                                    return;
                                }
                                confirming_remove.set(None);
                                // This is the ONE place that closes the menu
                                // for an edit — the item's own click in
                                // `HostRow` only requests the edit, never
                                // closes anything itself, so there is one
                                // state change to account for rather than
                                // two. It has to run only past the guard
                                // above: an edit refused because a request
                                // is already in flight must leave the menu
                                // exactly as it was, not close it out from
                                // under a click that did nothing.
                                host_menu_open.set(None);
                                if !*details_open.peek() {
                                    filter_open.set(false);
                                }
                                details_open.set(true);
                                destination_draft.set(destination);
                                editing.set(Some(id));
                            },
                            on_edit_submit: on_edit_submit.clone(),
                            on_edit_cancel: move |_| editing.set(None),
                            on_remove_start: move |id: HostId| {
                                if ops.busy_now()
                                    || provisioning_busy_hosts.peek().contains(&id)
                                {
                                    return;
                                }
                                editing.set(None);
                                // See `on_edit_start` just above: the same
                                // single-owner close, past the same guard.
                                host_menu_open.set(None);
                                confirming_remove.set(Some(id));
                            },
                            on_remove_confirm: on_remove_confirm.clone(),
                            on_remove_cancel: move |_| confirming_remove.set(None),
                            on_provisioning: move |(id, operation): (HostId, ProvisioningOperation)| {
                                if ops.busy_now()
                                    || provisioning_busy_hosts.peek().contains(&id)
                                {
                                    return;
                                }
                                if !*details_open.peek() {
                                    filter_open.set(false);
                                }
                                details_open.set(true);
                                host_menu_open.set(None);
                                provisioning_action_requests.write().insert(id, operation);
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
                                    filter_open.set(false);
                                }
                            },
                            host,
                        }
                    }
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
// on. Profiles have since moved to the app bar, but the remaining host verbs
// keep this menu because the same narrow-sidebar constraint still applies.

/// One command in a host row's actions menu, in the order the menu offers
/// them.
///
/// `Retry` is offered in every phase, like the button it replaces; `Adopt`
/// only when [`adoptable`] names an identity; provisioning commands mirror
/// the permanently mounted provisioning component's current offers; and
/// `Edit`/`Remove` only appear on an ssh row (see `HostRow`'s own doc for why
/// an unmanageable kind gets neither). The separator before `Remove` is
/// drawn in the rsx, not modeled here — see `MenuOrder` in `menu_panel`
/// for why a separator is never counted as an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HostMenuAction {
    Retry,
    Adopt,
    Rerun,
    AutomaticSetup,
    Update,
    Edit,
    Remove,
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
    HostMenuAction::Edit,
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
        HostMenuAction::Edit | HostMenuAction::Remove => manageable,
    })
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
/// Grouped because all three answer one question — what does this row offer
/// beyond its ordinary control strip right now — and because `HostsPanel`
/// narrows all three from at-most-one-row signals to a boolean here: its own
/// `confirming_remove` and `editing`, and the `host_menu_open` `ListView`
/// owns to keep at most one row menu open across
/// BOTH the session list and the hosts panel (see `HostsPanel`'s own doc for
/// that single-open discipline). Not one enum because the panel already owns
/// the first two signals while the list owns the cross-panel menu signal.
///
/// State only, like every group here — see [`HostRowActivity`] for why no
/// group may ever carry a callback.
#[derive(Clone, PartialEq, Eq)]
struct HostRowControls {
    /// Whether this row is showing the in-place forget-this-host prompt.
    confirming_remove: bool,
    /// Whether this row is showing its destination field instead of controls.
    editing: bool,
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

/// One host's row: name, state, the evidence, the remedy, and whichever
/// controls that state actually offers.
///
/// The controls are state-driven rather than uniform, which is the point:
/// `adopt` appears only where there is an identity to adopt (never on
/// `identity-unverified` — see [`adoptable`]), and edit/remove appear only
/// on ssh rows, because the reserved local row has no destination to change
/// and cannot be removed at all (the helm refuses both with a 409). A row of
/// an unrecognized KIND is treated as unmanageable for the same reason:
/// offering a verb the helm would refuse teaches the user something false
/// about what is possible.
///
/// `retry` is offered in every state, connected included. It costs one
/// attempt, and the state it is most needed in — `retired`, whose actor is
/// gone — is precisely the one where nothing else brings the host back.
///
/// The host NAME is peer/user-supplied text (an ssh destination) and is
/// rendered through the same isolation every other such value gets: a
/// destination able to reorder the row around it could make a remove button
/// appear to belong to a different host than it does.
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
/// `edit destination`, and `remove` render inside one
/// "⋯" menu (`.host-row-menu` toggle, `.host-row-menu-panel` panel) built on
/// the same generic mechanics the session row's menu uses (`menu_panel`) —
/// see that module's own doc for what is shared and why. The name, phase
/// status, and muted toggle stay on the row line. Destination editing and
/// removal confirmation use full-width blocks below it; while either is
/// open the toggle stays in its trailing gutter but is disabled, preventing
/// a competing command without making the row jump horizontally.
///
/// Every actionable item closes the menu when chosen. Provisioning commands
/// additionally open the global details disclosure before sending their
/// one-shot request to the permanently mounted provisioning component, so
/// planning feedback and confirmation never appear invisibly.
///
/// `edit destination` and `remove` disable the toggle and open subordinate
/// blocks, so closing is a correctness requirement: cancelling either flow
/// must not silently revive a menu the user never asked to reopen.
/// `HostsPanel`'s own `on_edit_start`/
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
    /// Which control surface this row is showing (grouped state).
    controls: HostRowControls,
    /// What the management verbs are doing to this row (grouped state).
    activity: HostRowActivity,
    /// The global details disclosure, shared by every host row.
    details_open: bool,
    /// Provisioning commands currently offered in this row's menu.
    provisioning_menu: ProvisioningMenuState,
    /// The feed-driven setup/update surface built by the panel.
    provisioning_section: Element,
    destination_draft: Signal<String>,
    on_retry: EventHandler<HostId>,
    on_adopt: EventHandler<(HostId, String)>,
    on_edit_start: EventHandler<(HostId, String)>,
    on_edit_submit: EventHandler<(HostId, String)>,
    on_edit_cancel: EventHandler<()>,
    on_remove_start: EventHandler<HostId>,
    on_remove_confirm: EventHandler<HostId>,
    on_remove_cancel: EventHandler<()>,
    /// Route a provisioning menu command back to this row's permanently
    /// mounted provisioning component.
    on_provisioning: EventHandler<(HostId, ProvisioningOperation)>,
    /// Open or close THIS row's "⋯" menu — `HostsPanel`'s toggle callback,
    /// built the same way the session row's `on_menu_toggle` is (see
    /// `HostsPanel`'s own doc for the single-open discipline it keeps).
    on_menu_toggle: EventHandler<HostId>,
) -> Element {
    let HostRowControls {
        confirming_remove,
        editing,
        menu_open,
    } = controls;
    let HostRowActivity {
        busy,
        error,
        warning,
    } = activity;
    #[cfg(test)]
    HOST_ROW_RENDERS.with(|renders| *renders.borrow_mut().entry(host.id).or_insert(0) += 1);
    let id = host.id;
    // The local row is not management surface: SPEC.md has it always
    // present, never registered, never removed. An unrecognized kind is not
    // management surface either — see this component's docs.
    let manageable = host.kind == HostKind::Ssh;
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
        .map(|reported| format!("adopt {}", display_peer(reported)));
    let remedy = state_remedy(&host.state);
    let detail = state_detail(&host.state);
    let shown_name = display_peer(&host.name);
    let edit_start = (id, host.destination.clone().unwrap_or_default());
    // This render's menu item list — see `host_menu_order`'s own doc. Read
    // every render, not only while the menu is open, because the `use_effect`
    // below has to notice an item withdrawn (a poll turning `adoptable` off)
    // even while a menu built against the wider list is still up.
    let adoptable_now = adopt_identity.is_some();
    let menu_order = host_menu_order(adoptable_now, manageable, provisioning_menu);
    let provisioning_disabled = busy || provisioning_menu.planning;

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
    // session row's "archiving withdraws stop and archive" hazard — see
    // that component's own `use_effect` for the stale-handle reasoning this
    // mirrors exactly.
    //
    // Stale FOCUS, though, is reconciled by ACTION identity rather than by
    // comparing the stored position against the new list's length: an
    // action withdrawn from the MIDDLE of the list (Adopt, here) shifts
    // every later action's index down, so the slot Adopt vacates is
    // immediately reoccupied by Edit — a numeric length check never
    // notices that, and would leave the row believing Edit was focused
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
                    class: "host-status {phase_class(&host.state)}",
                    role: "status",
                    aria_label: is_connected(&host.state).then_some("connected"),
                    span { class: "status-dot", "aria-hidden": "true" }
                    if !is_connected(&host.state) {
                        span { class: "host-status-label", "{phase_display_label(&host.state)}" }
                    }
                }
                // The line always keeps its three children. Edit and remove
                // disable the menu rather than replacing its toggle, so the
                // trailing gutter does not jump while their full-width
                // blocks render below. `nowrap` remains load-bearing for the
                // fixed-position panel (F2/COR-HOST-MENU-OFFSCREEN).
                    button {
                        r#type: "button",
                        class: "btn host-row-menu",
                        disabled: editing || confirming_remove,
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
                    if menu_open && !editing && !confirming_remove {
                        div {
                            class: "host-row-menu-panel",
                            style: menu_panel_placement_style(placement()),
                            div {
                                class: "host-row-menu-items",
                                role: "menu",
                                aria_label: host_menu_label(&host.name),
                                button {
                                    r#type: "button",
                                    class: "btn host-row-menu-item host-retry",
                                    role: "menuitem",
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
                                    "retry"
                                }
                                if let (Some(reported), Some(label)) = (adopt_identity, adopt_label) {
                                    button {
                                        r#type: "button",
                                        class: "btn host-row-menu-item host-adopt",
                                        role: "menuitem",
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
                                        span { class: "peer-value", dir: "ltr", "{label}" }
                                    }
                                }
                                if let Some(operation) = provisioning_menu.rerun {
                                    button {
                                        r#type: "button",
                                        class: "btn host-row-menu-item provisioning-rerun",
                                        role: "menuitem",
                                        aria_disabled: if provisioning_disabled { "true" },
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
                                        onclick: move |_| {
                                            if provisioning_disabled {
                                                return;
                                            }
                                            on_provisioning.call((id, operation));
                                        },
                                        if provisioning_menu.planning { "planning…" } else { "re-run" }
                                    }
                                }
                                if provisioning_menu.automatic_setup {
                                    button {
                                        r#type: "button",
                                        class: "btn host-row-menu-item provisioning-auto-setup",
                                        role: "menuitem",
                                        aria_disabled: if provisioning_disabled { "true" },
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
                                        onclick: move |_| {
                                            if provisioning_disabled {
                                                return;
                                            }
                                            on_provisioning.call((id, ProvisioningOperation::Add));
                                        },
                                        "set up automatically"
                                    }
                                }
                                if provisioning_menu.update {
                                    button {
                                        r#type: "button",
                                        class: "btn host-row-menu-item provisioning-update",
                                        role: "menuitem",
                                        aria_disabled: if provisioning_disabled { "true" },
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
                                        onclick: move |_| {
                                            if provisioning_disabled {
                                                return;
                                            }
                                            on_provisioning.call((id, ProvisioningOperation::Update));
                                        },
                                        if provisioning_menu.planning { "planning…" } else { "update" }
                                    }
                                }
                                if manageable {
                                    button {
                                        r#type: "button",
                                        class: "btn host-row-menu-item host-edit",
                                        role: "menuitem",
                                        aria_disabled: if busy { "true" },
                                        tabindex: if menu_tab_stop == Some(HostMenuAction::Edit) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(menu_wiring, HostMenuAction::Edit, element.data())
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(HostMenuAction::Edit));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(HostMenuAction::Edit),
                                                menu_wiring,
                                                &id,
                                            );
                                        },
                                        onclick: move |_| {
                                            if busy {
                                                return;
                                            }
                                            // Only REQUESTS the edit —
                                            // `on_edit_start` (`HostsPanel`)
                                            // is the one place that closes
                                            // the menu, once its own busy
                                            // guard has actually let the
                                            // request through (see this
                                            // component's own doc).
                                            on_edit_start.call(edit_start.clone());
                                        },
                                        "edit destination"
                                    }
                                    // The boundary before the destructive
                                    // item — see `list::row`'s own separator
                                    // for the accessibility argument, which
                                    // applies identically here. Not counted
                                    // by `MenuOrder`, so arrow navigation
                                    // steps straight past it.
                                    div { class: "host-row-menu-separator", role: "separator" }
                                    button {
                                        r#type: "button",
                                        class: "btn host-row-menu-item host-remove",
                                        role: "menuitem",
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
                                            // See `edit destination` above:
                                            // only requests the confirm
                                            // prompt; `on_remove_start`
                                            // closes the menu.
                                            on_remove_start.call(id);
                                        },
                                        "remove"
                                    }
                                }
                            }
                        }
                    }
            }
            // The removal prompt: a full-width block BELOW the name/status
            // header line, not a flex child squeezed onto it (see the
            // guard on that line, just above). It has to fit an
            // unshrinkable warning sentence, a second copy of the
            // (unbounded) host name, AND both buttons, which is more room
            // than the 340px sidebar has on one line regardless of how
            // short the name is — `confirm remove`/`cancel` rendered
            // clipped and unclickable off the sidebar's edge exactly the
            // way `remove` itself used to before the row's other verbs
            // folded into the "⋯" menu (see the section banner above this
            // component). `cancel` is the safe default here, so keeping it
            // reachable is not a cosmetic concern.
            if confirming_remove {
                div { class: "host-confirm-remove-panel",
                    // Consequence first and never truncated, then the
                    // host it is about — the reading order this prompt
                    // established, for the reason recorded there: a
                    // long name must not be able to clip the sentence
                    // that says what the button does.
                    span { class: "confirm-consequence",
                        "forgetting a host leaves its supervisor and sessions running; \
                         re-adding the destination finds them again:"
                    }
                    span { class: "confirm-title peer-value", dir: "ltr", "\"{shown_name}\"" }
                    div { class: "host-confirm-remove-actions",
                        button {
                            r#type: "button",
                            class: "btn confirm-delete host-confirm-remove",
                            disabled: busy,
                            onclick: move |_| on_remove_confirm.call(id),
                            "confirm remove"
                        }
                        button {
                            r#type: "button",
                            class: "btn confirm-cancel host-cancel-remove",
                            // Focus lands on the way OUT of the
                            // destructive action, through the plain
                            // HTML attribute rather than a fallible
                            // `set_focus` whose discarded `Result`
                            // could drop the safety behavior silently.
                            autofocus: true,
                            onclick: move |_| on_remove_cancel.call(()),
                            "cancel"
                        }
                    }
                }
            }
            if editing {
                HostDestinationForm {
                    draft: destination_draft,
                    busy,
                    on_submit: move |destination| on_edit_submit.call((id, destination)),
                    on_cancel: move |_| on_edit_cancel.call(()),
                }
            }
            if details_open {
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

/// The in-place destination field for one ssh row.
///
/// A plain `<input type="text">`, unlike the rename control's textarea: a
/// destination is a single line by construction, and the helm refuses the
/// shapes that matter (empty, option-shaped, NUL-carrying) at the registry
/// boundary — so nothing is validated here and whatever is typed goes as
/// typed, with the refusal coming from the authority that owns the rule.
///
/// The draft belongs to the panel for the reason `rename::RenameForm`
/// records: this form is unmounted by re-renders the user did not cause (a
/// host mutation landing rebuilds the rows), and a draft owned here would be
/// silently discarded with it.
#[component]
fn HostDestinationForm(
    mut draft: Signal<String>,
    busy: bool,
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
                // An ssh destination is literal text that gets EXECUTED as
                // part of a command line, never prose, so every browser
                // "correction" is off for the reason the create form's
                // command fields record: a silently capitalized hostname or
                // a swallowed suggestion keystroke dials the wrong machine.
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
                class: "btn host-save-destination",
                disabled: busy,
                "save"
            }
            button {
                r#type: "button",
                class: "btn host-cancel-edit",
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

/// Keep probe and manual diagnostics inside the peer-text boundary.
fn probe_error_parts(error: String) -> Vec<DetailPart> {
    vec![DetailPart::Peer(error)]
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
/// the contract. Only explicit confirmation starts a provisioning run and
/// claims `OpLock` around its POST.
#[component]
fn AddHostForm(
    mut ops: OpLock,
    on_added: EventHandler<Option<String>>,
    on_refresh: EventHandler<()>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut ssh = use_signal(String::new);
    let mut remote_farhelm = use_signal(String::new);
    let mut remote_state_dir = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut probing = use_signal(|| false);
    let mut offer = use_signal(|| None::<AddOffer>);
    let page_busy = ops.busy();
    let busy = page_busy || *probing.read();

    let confirm_base = base.clone();
    let confirm = move |_| {
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
        let base = confirm_base.clone();
        spawn(async move {
            let result = provision_host(&base, &planned.probe_id).await;
            drop(claim);
            match result {
                Ok(ProvisioningSubmission::Accepted(_)) => on_added.call(None),
                Ok(ProvisioningSubmission::Unvalidated(warning)) => on_added.call(Some(warning)),
                Err(problem) => {
                    error.set(Some(problem));
                    on_refresh.call(());
                }
            }
        });
    };

    rsx! {
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
                        }) => offer.set(Some(AddOffer {
                            probe_id,
                            confirmation,
                            binding,
                        })),
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
                PlanConfirmation {
                    confirmation: planned.confirmation,
                    busy: page_busy,
                    confirm_label: "confirm setup",
                    on_confirm: confirm,
                    on_cancel: move |_| {
                        if !ops.busy_now() {
                            offer.set(None);
                            error.set(None);
                        }
                    },
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::detail_text;

    /// Probe and manual diagnostics cannot carry bidi or invisible controls
    /// into the add form even though the helm relays host-produced text.
    #[test]
    fn probe_errors_cross_the_peer_text_boundary() {
        let shown = detail_text(&probe_error_parts(
            "ssh failed \u{202E}spoof\u{200B}".to_string(),
        ));
        assert_eq!(shown, "ssh failed <U+202E>spoof<U+200B>");
    }

    /// A host in the given state, with the rest of the row as plain as
    /// possible — every assertion below is about the state alone.
    fn host(state: HostPhase) -> Host {
        Host {
            id: 1,
            kind: HostKind::Ssh,
            destination: Some("user@box".to_string()),
            name: "user@box".to_string(),
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            state,
            incarnation: 1,
        }
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
    #[test]
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

    /// Display labels turn wire separators into prose without collapsing
    /// distinct states. This matters because the row spends words only on
    /// non-connected phases, so each word has to identify the state on its
    /// own while `data-host-phase` retains the exact token for automation.
    #[test]
    fn every_phase_has_humanized_display_words() {
        let labels: Vec<&str> = every_phase().iter().map(phase_display_label).collect();
        assert_eq!(
            labels,
            vec![
                "connecting",
                "unreachable, retrying",
                "connected",
                "version skew",
                "identity mismatch",
                "identity unverified",
                "duplicate",
                "retired",
                "unrecognized",
            ]
        );
    }

    /// Every phase's detail must carry ITS OWN evidence — the sentinels make
    /// that checkable field by field, which a per-phase spot check cannot: a
    /// detail that rendered some other variant's payload, or dropped a
    /// field, would still contain "plausible" text.
    #[test]
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
                },
                vec!["42", "sentinel-duplicate-identity"],
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
    #[test]
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
    /// With no provisioning offer, an adoptable ssh host has four commands
    /// and an ordinary ssh host has three. The reserved local row (never `manageable` — see
    /// [`HostRow`]'s own doc) drops `edit` and `remove` entirely regardless,
    /// which must move `adopt`'s position rather than leave a gap where
    /// `edit` would have sat — the same packing `MenuOrder::pack` guarantees
    /// for the session row.
    #[test]
    fn the_host_menu_follows_manageability_and_adoptability() {
        use HostMenuAction::{Adopt, AutomaticSetup, Edit, Remove, Rerun, Retry, Update};

        // Ssh, adoptable: every item, in the declared order.
        let ssh_adoptable = host_menu_order(true, true, ProvisioningMenuState::default());
        assert_eq!(ssh_adoptable.len(), 4);
        assert_eq!(ssh_adoptable.get(0), Some(Retry));
        assert_eq!(ssh_adoptable.get(1), Some(Adopt));
        assert_eq!(ssh_adoptable.get(2), Some(Edit));
        assert_eq!(ssh_adoptable.get(3), Some(Remove));
        assert_eq!(ssh_adoptable.last(), Some(Remove));

        // Ssh, not adoptable (the ordinary case: most phases offer no
        // adopt): `adopt` drops out and `edit`/`remove` shift up to fill
        // the gap rather than leaving one at position 2.
        let ssh_plain = host_menu_order(false, true, ProvisioningMenuState::default());
        assert_eq!(ssh_plain.len(), 3);
        assert_eq!(ssh_plain.get(0), Some(Retry));
        assert_eq!(ssh_plain.get(1), Some(Edit));
        assert_eq!(ssh_plain.get(2), Some(Remove));
        assert_eq!(ssh_plain.position(Adopt), None);

        // The local row's identity-mismatch menu shape: unmanageable, so
        // `edit`/`remove` never appear regardless of `adoptable`. This is a
        // real, reachable state — the local row's connection actor compares
        // its recorded and reported identities exactly like an ssh row's
        // (`farhelm-helm::manager`), so a local supervisor restarted behind
        // a changed install lands here too — not a hypothetical `pack` has
        // to merely tolerate. `host_menu_order` takes `adoptable` and
        // `manageable` as two independent facts rather than encoding "local
        // implies never adoptable" itself, which is what lets this case be
        // exercised directly instead of only through the ssh fixtures above.
        let local = host_menu_order(true, false, ProvisioningMenuState::default());
        assert_eq!(local.len(), 2);
        assert_eq!(local.get(0), Some(Retry));
        assert_eq!(local.get(1), Some(Adopt));
        assert_eq!(local.position(Edit), None);
        assert_eq!(local.position(Remove), None);

        // The ordinary local row: just the unconditional retry item.
        let local_plain = host_menu_order(false, false, ProvisioningMenuState::default());
        assert_eq!(local_plain.len(), 1);
        assert_eq!(local_plain.get(0), Some(Retry));
        assert_eq!(local_plain.last(), Some(Retry));

        // A failed remote update offers rerun and update between identity
        // actions and destination management. Remove remains last, after
        // the visual destructive separator rendered by the row.
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
        assert_eq!(all_actions.get(5), Some(Edit));
        assert_eq!(all_actions.get(6), Some(Remove));
    }

    /// The value an adopt SENDS is the raw one; the value it SHOWS is
    /// escaped. Collapsing the two either way is a real failure: sending the
    /// escaped form turns every unusual identity into a spurious 409, and
    /// showing the raw form is exactly the spoofing hole the escaping exists
    /// to close.
    #[test]
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
    #[test]
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
    #[test]
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
    #[test]
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
    #[test]
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
    #[test]
    fn a_failed_hosts_read_keeps_the_last_snapshot_and_reports_the_failure() {
        let mut read = HostsRead::default();
        assert!(read.is_loading());
        assert!(read.hosts().is_none());
        assert!(read.refresh_error().is_none());

        read.record(Ok(vec![host(HostPhase::Connected {
            identity: None,
            build_version: "0.1.0".to_string(),
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
    #[test]
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
    #[test]
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

    /// The stale-session notice must name the host's ACTUAL state, not a
    /// generic "unreachable" — SPEC.md's host-unreachable notice is the
    /// common case, not the only one, and a skewed host described as
    /// unreachable would hide the upgrade that fixes it.
    #[test]
    fn the_stale_notice_names_the_real_state_and_carries_its_remedy() {
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
            notice.contains("version skew"),
            "the real phase, not a generic unreachable: {notice}"
        );
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
    #[test]
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
    #[test]
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

    /// A connected host's row must report how its last cache refresh went,
    /// beside the connection rather than as part of it: a failed refresh
    /// leaves the host perfectly connected while its listed sessions are
    /// last-known, and reading that as a disconnection would be wrong in
    /// both directions.
    #[test]
    fn a_connected_hosts_detail_reports_its_refresh_health() {
        let failing = detail_text(&state_detail(&HostPhase::Connected {
            identity: None,
            build_version: "0.1.0".to_string(),
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
            name: "user@box".to_string(),
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            state: HostPhase::Connected {
                identity: Some("stable".to_string()),
                build_version: "0.1.0".to_string(),
                refresh: RefreshHealth::Ok { sessions: 0 },
            },
            incarnation: 1,
        }
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
    #[test]
    fn repeated_parent_refreshes_do_not_rerender_an_unchanged_host_row() {
        fn app() -> Element {
            let destination_draft = use_signal(String::new);
            let on_retry = use_callback(|_: HostId| {});
            let on_adopt = use_callback(|_: (HostId, String)| {});
            let on_edit_start = use_callback(|_: (HostId, String)| {});
            let on_edit_submit = use_callback(|_: (HostId, String)| {});
            let on_edit_cancel = use_callback(|_: ()| {});
            let on_remove_start = use_callback(|_: HostId| {});
            let on_remove_confirm = use_callback(|_: HostId| {});
            let on_remove_cancel = use_callback(|_: ()| {});
            let on_provisioning = use_callback(|_: (HostId, ProvisioningOperation)| {});
            let on_menu_toggle = use_callback(|_: HostId| {});
            rsx! {
                HostRow {
                    host: row_specimen(1),
                    local_setup: false,
                    controls: HostRowControls {
                        confirming_remove: false,
                        editing: false,
                        menu_open: false,
                    },
                    activity: HostRowActivity {
                        busy: false,
                        error: None,
                        warning: None,
                    },
                    details_open: false,
                    provisioning_menu: ProvisioningMenuState::default(),
                    provisioning_section: dioxus::core::VNode::empty(),
                    destination_draft,
                    on_retry,
                    on_adopt,
                    on_edit_start,
                    on_edit_submit,
                    on_edit_cancel,
                    on_remove_start,
                    on_remove_confirm,
                    on_remove_cancel,
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
    /// compared by value, so MOVING an open removal prompt is two row renders
    /// (the row that loses it and the row that gains it) and a refusal
    /// landing on one host is one — not a fleet-wide repaint. A field that
    /// drops out of what memoization compares, or a group that starts
    /// invalidating rows it does not describe, moves these per-row counts.
    /// Exercising `HostRowControls` and `HostRowActivity` in the same virtual
    /// DOM is what makes the test say which of the two broke.
    #[test]
    fn only_the_host_rows_whose_state_changed_rerender() {
        // Both facts live OUTSIDE the virtual DOM — the test moves them
        // between renders the way `HostsPanel`'s own signals move between
        // its renders — and the app re-derives every row's state from them
        // on each parent render, so memoization alone decides which rows
        // actually run.
        std::thread_local! {
            static CONFIRMING: std::cell::Cell<HostId> = const { std::cell::Cell::new(1) };
            static REFUSED: std::cell::Cell<Option<HostId>> =
                const { std::cell::Cell::new(None) };
        }

        fn app() -> Element {
            let destination_draft = use_signal(String::new);
            let on_retry = use_callback(|_: HostId| {});
            let on_adopt = use_callback(|_: (HostId, String)| {});
            let on_edit_start = use_callback(|_: (HostId, String)| {});
            let on_edit_submit = use_callback(|_: (HostId, String)| {});
            let on_edit_cancel = use_callback(|_: ()| {});
            let on_remove_start = use_callback(|_: HostId| {});
            let on_remove_confirm = use_callback(|_: HostId| {});
            let on_remove_cancel = use_callback(|_: ()| {});
            let on_provisioning = use_callback(|_: (HostId, ProvisioningOperation)| {});
            let on_menu_toggle = use_callback(|_: HostId| {});
            let confirming = CONFIRMING.with(std::cell::Cell::get);
            let refused = REFUSED.with(std::cell::Cell::get);
            rsx! {
                for id in [1_i64, 2, 3] {
                    HostRow {
                        key: "{id}",
                        host: row_specimen(id),
                        local_setup: false,
                        controls: HostRowControls {
                            confirming_remove: confirming == id,
                            editing: false,
                            menu_open: false,
                        },
                        activity: HostRowActivity {
                            busy: false,
                            error: (refused == Some(id))
                                .then(|| "the helm refused this verb".to_string()),
                            warning: None,
                        },
                        details_open: false,
                        provisioning_menu: ProvisioningMenuState::default(),
                        provisioning_section: dioxus::core::VNode::empty(),
                        destination_draft,
                        on_retry,
                        on_adopt,
                        on_edit_start,
                        on_edit_submit,
                        on_edit_cancel,
                        on_remove_start,
                        on_remove_confirm,
                        on_remove_cancel,
                        on_provisioning,
                        on_menu_toggle,
                    }
                }
            }
        }

        CONFIRMING.with(|confirming| confirming.set(1));
        REFUSED.with(|refused| refused.set(None));
        HOST_ROW_RENDERS.with(|renders| renders.borrow_mut().clear());
        let mut dom = VirtualDom::new(app);
        dom.rebuild_to_vec();
        assert_eq!(
            host_row_renders(),
            vec![(1, 1), (2, 1), (3, 1)],
            "the initial build renders every row once"
        );

        // `HostRowControls`: the removal prompt is at most one row's at a
        // time, so moving it touches the row that had it and the row that
        // gets it — and only those.
        CONFIRMING.with(|confirming| confirming.set(2));
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            host_row_renders(),
            vec![(1, 2), (2, 2), (3, 1)],
            "moving the removal prompt must rerender the row that lost it and the row that \
             gained it, and nothing else"
        );

        // `HostRowActivity`: a refusal is per-host, so it costs exactly the
        // one row that has to show it.
        REFUSED.with(|refused| refused.set(Some(3)));
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            host_row_renders(),
            vec![(1, 2), (2, 2), (3, 2)],
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
    #[test]
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
}
