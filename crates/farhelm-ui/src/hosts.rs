//! The hosts surface (PLAN_M6.md item 6): `HostsPanel` — one row per
//! registered host with its connection state always visible — plus the
//! renderer-free wording this UI derives from a `HostPhase`, and the
//! read-state model both this panel and the stale session view are drawn
//! from.
//!
//! ## Why the state chip is the whole point
//!
//! SPEC.md: "Per-host connection state is always visible." Not behind a
//! menu, not only when something is wrong, and not summarized into
//! "online/offline" — because the eight phases the helm distinguishes call
//! for different responses, and collapsing them would leave a user with a
//! red dot and no idea whether to wait (`connecting`), do nothing
//! (`unreachable-reprobing` re-probes forever), upgrade a binary
//! (`version-skew`), make a decision (`identity-mismatch`), or press retry
//! (`retired`). Each row therefore carries the phase, the evidence behind
//! it, and — where one exists — the sentence to act on.
//!
//! The chip's TEXT is the helm's own phase vocabulary verbatim, not a
//! friendlier synonym. The same strings appear in the helm's logs and inside
//! every refusal a non-connected host produces ("host 3 is
//! unreachable-reprobing, so this operation is refused…"), and a user
//! comparing an error against this panel has to find the same word in both.
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

use dioxus::prelude::*;

use crate::api::{Commit, add_host, adopt_host, remove_host, retry_host, set_host_destination};
use crate::ops::OpLock;
use crate::{ApiBase, Host, HostId, HostKind, HostPhase, RefreshHealth};

// ---------------------------------------------------------------------
// Displaying peer-supplied text
// ---------------------------------------------------------------------

/// Whether this character must be shown as an escape rather than rendered.
///
/// Three groups, all invisible or layout-affecting and none of them
/// legitimate inside an identity, a build string, or an ssh destination:
///
/// - The C0/C1 control ranges and DEL — including the line and paragraph
///   separators, which would break a value across lines mid-sentence.
/// - The bidirectional formatting controls. These are the spoofing vector:
///   an RLO inside an identity reverses the text that FOLLOWS it, so
///   "recorded X, reported Y" can be made to read with the two swapped
///   while the underlying values are unchanged.
/// - The zero-width and joining characters, which let two different
///   identities render identically.
///
/// Enumerated rather than derived from Unicode's general categories, because
/// this crate carries no Unicode tables and pulling one in for a display
/// rule would be a large dependency for a small list. That makes the set a
/// KNOWN-BAD list rather than a proof: a future format character outside it
/// would render as itself. The direction-isolated element each value sits in
/// (see [`DetailPart`]) is the second, category-free layer that bounds the
/// damage either way.
fn must_escape(ch: char) -> bool {
    ch.is_control()
        || matches!(ch,
            '\u{061C}'                    // Arabic letter mark
            | '\u{200E}' | '\u{200F}'     // LTR/RTL marks
            | '\u{202A}'..='\u{202E}'     // embeddings and overrides
            | '\u{2066}'..='\u{2069}'     // isolates
            | '\u{00AD}'                  // soft hyphen
            | '\u{200B}'..='\u{200D}'     // zero-width space/non-joiner/joiner
            | '\u{2060}'                  // word joiner
            | '\u{180E}'                  // Mongolian vowel separator
            | '\u{FEFF}'                  // zero-width no-break space / BOM
            | '\u{2028}' | '\u{2029}'     // line and paragraph separators
        )
}

/// One peer-supplied value as it should be SHOWN.
///
/// Escapes everything [`must_escape`] rejects, and gives the two degenerate
/// values an unambiguous form of their own. That last part is not cosmetic:
/// an identity that renders as nothing at all appears in the adopt button as
/// `adopt ` and in the mismatch evidence as a gap, so a user is asked to
/// approve something they cannot see. Saying `(empty)` is the difference
/// between an odd-looking prompt and an invisible one.
///
/// The result is for DISPLAY only. Everything that has to match on the far
/// end — the adopt request's `reported`, above all — uses the raw value.
pub(crate) fn display_peer(raw: &str) -> String {
    if raw.is_empty() {
        return "(empty)".to_string();
    }
    let mut shown = String::with_capacity(raw.len());
    // Whether anything the user can actually SEE survived. An escape counts
    // (it prints as `<U+202E>`); an ordinary space does not.
    let mut anything_visible = false;
    for ch in raw.chars() {
        if must_escape(ch) {
            shown.push_str(&format!("<U+{:04X}>", ch as u32));
            anything_visible = true;
        } else {
            anything_visible |= !ch.is_whitespace();
            shown.push(ch);
        }
    }
    if anything_visible {
        shown
    } else {
        format!("(whitespace only: {} characters)", raw.chars().count())
    }
}

/// One run of a rendered detail: either this UI's own words, or a value that
/// came from somewhere else.
///
/// The split exists so that a peer value can never lay out the sentence
/// AROUND it. Each `Peer` run is rendered into its own element carrying
/// `dir="ltr"` and CSS `unicode-bidi: isolate`, which bounds directional
/// resolution to that element — so "recorded as install X; now reports Y"
/// keeps X and Y on the sides the labels put them on, whatever they contain.
/// Concatenating everything into one string and interpolating it, which is
/// what this replaced, gives that guarantee up: the escaping in
/// [`display_peer`] would still hold, but only for the control characters it
/// knows about, and strong-RTL letters need no control character at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DetailPart {
    /// This UI's own wording — trusted, and never escaped.
    Text(String),
    /// A value from the helm, a supervisor, or the registry. Escaped and
    /// isolated when rendered; carried raw here.
    Peer(String),
}

impl DetailPart {
    /// Our own words.
    fn text(words: impl Into<String>) -> DetailPart {
        DetailPart::Text(words.into())
    }

    /// Someone else's.
    fn peer(value: impl Into<String>) -> DetailPart {
        DetailPart::Peer(value.into())
    }
}

/// A detail as one flat string, for tests.
///
/// Deliberately not used for rendering: flattening throws away exactly the
/// per-value isolation [`DetailPart`] exists to provide, so a renderer that
/// reached for this would be undoing the guarantee while looking tidier.
#[cfg(test)]
fn detail_text(parts: &[DetailPart]) -> String {
    parts
        .iter()
        .map(|part| match part {
            DetailPart::Text(text) => text.clone(),
            DetailPart::Peer(value) => display_peer(value),
        })
        .collect()
}

/// Render a detail: our words as plain runs, every peer value isolated in
/// its own fixed-direction element.
///
/// `dir="ltr"` AND the stylesheet's `unicode-bidi: isolate` together are
/// what make the isolation real — the attribute sets the base direction, the
/// property stops the run participating in the surrounding paragraph's
/// bidirectional resolution at all.
#[component]
pub(crate) fn PeerLine(class: String, parts: Vec<DetailPart>) -> Element {
    rsx! {
        div { class: "{class}",
            for (index , part) in parts.iter().enumerate() {
                match part {
                    DetailPart::Text(text) => rsx! {
                        span { key: "{index}", "{text}" }
                    },
                    DetailPart::Peer(value) => rsx! {
                        span {
                            key: "{index}",
                            class: "peer-value",
                            dir: "ltr",
                            "{display_peer(value)}"
                        }
                    },
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// The phase vocabulary
// ---------------------------------------------------------------------

/// The chip's text for one phase: the helm's own stable label.
///
/// Kept as a total match rather than a derived string so that a new phase
/// forces a deliberate decision here — the alternative, deriving the label
/// from the wire tag, would silently show `Unrecognized` as an empty chip.
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

/// The CSS modifier the chip carries, grouping the eight phases by what a
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

/// The evidence behind a phase — the diagnosis, never the remedy (that is
/// [`state_remedy`]'s job).
///
/// Every branch renders values the helm supplied rather than restating the
/// phase in longer words: both versions on a skew, both identities on a
/// mismatch, the transport's own text when a host will not answer. A chip
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
        // The one unreachable host whose remedy is a command on the machine
        // the user is already sitting at. Provisioning is M7's, so this is a
        // manual path and deliberately never an offer to install anything.
        //
        // The `--state-dir` caveat is load-bearing rather than pedantic: a
        // helm started with `--state-dir` reaches its local supervisor over
        // the socket in THAT directory, so a bare `farhelm supervisor run`
        // would start a supervisor this helm never talks to — and the user
        // would be left staring at an unreachable row after doing exactly
        // what it told them. The frozen `/api/hosts` contract carries no
        // state directory, so this is as exact as this PR can honestly be;
        // a contract-borne remediation is a later extension.
        HostPhase::Unreachable { cause, .. } if cause == "local-supervisor-not-running" => {
            Some(vec![DetailPart::text(
                "no supervisor is running on this machine — start one with `farhelm supervisor \
                 run`, passing the same `--state-dir` this helm was started with if it has one",
            )])
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
/// one dropped request blanks every chip on the surface SPEC.md requires to
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
                phase = phase_label(&host.state),
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

/// The hosts panel: every registered host, its state, and the management
/// verbs SPEC.md's host management consists of.
///
/// ## Where the data and the in-flight state come from
///
/// `hosts` is `ListView`'s signal, not this component's: the create dialog's
/// host selector reads the same list, and two polls of `/api/hosts` for one
/// page would be a second cadence nobody chose. `on_changed` is how a
/// mutation asks for an immediate refetch instead of waiting out the poll —
/// there is nothing honest to paint optimistically, because every host verb
/// changes state this side cannot predict (an add's chip is whatever the
/// connection finds; a retarget's is a fresh active-retry window).
///
/// `ops` is the page's single live-operation token (`ops::OpLock`), and it
/// is what actually excludes: every verb below claims it synchronously at
/// handler entry and releases it when the request completes. `busy_host` is
/// `ListView`'s too but is only for DRAWING — which row shows itself busy —
/// because a token is a page-wide fact and a row needs to know whether it is
/// the row. See the `ops` module for why a `blocked` prop, which this
/// replaced, could not do the job: a prop is a render-time value, and two
/// clicks inside one frame both read the page as idle.
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
#[component]
pub(crate) fn HostsPanel(
    hosts: Signal<HostsRead>,
    mut ops: OpLock,
    mut busy_host: Signal<Option<HostId>>,
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
    // An add that committed with an unreadable reply, which has no row to
    // sit on — see the form's `on_added`.
    let mut add_warning = use_signal(|| None::<String>);

    // One shared shape for the five mutations: claim the page's operation
    // token, clear this host's stale lines, run the request, then ask for a
    // refetch or record what came back — and release the token whatever
    // happened. Written once because the only thing that differs between
    // them is the request, which is why it arrives already built as a boxed
    // future rather than as five near-identical copies of this bookkeeping.
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
        if !ops.claim() {
            return false;
        }
        busy_host.set(Some(host));
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
            busy_host.set(None);
            // Released on every path. A leaked token leaves the whole page
            // inert with nothing on screen to explain why, which is a far
            // worse failure than any of the outcomes above.
            ops.release();
        });
        true
    };

    let retry_base = base.clone();
    let on_retry = move |host: HostId| {
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
    // Cosmetic, not the guard — every handler below claims the token for
    // itself (see the `ops` module).
    let busy = ops.busy();
    rsx! {
        section { class: "hosts-panel",
            div { class: "hosts-panel-header",
                span { class: "hosts-panel-title", "hosts" }
                button {
                    r#type: "button",
                    class: "btn add-host-button",
                    // This control UNMOUNTS the add form, so it must not act
                    // while anything is in flight: dropping the component
                    // mid-request strands the response with nothing left to
                    // act on it. The token is read synchronously in the
                    // handler for the same reason every other guard here is
                    // — the attribute is one render behind.
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
            if adding() {
                AddHostForm {
                    ops,
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
            if let Some(list) = read.hosts() {
                div { class: "host-list",
                    for host in list.iter().cloned() {
                        HostRow {
                            key: "{host.id}",
                            busy: *busy_host.read() == Some(host.id) || busy,
                            confirming_remove: *confirming_remove.read() == Some(host.id),
                            editing: *editing.read() == Some(host.id),
                            error: errors.read().get(&host.id).cloned(),
                            warning: warnings.read().get(&host.id).cloned(),
                            destination_draft,
                            on_retry: on_retry.clone(),
                            on_adopt: on_adopt.clone(),
                            on_edit_start: move |(id, destination): (HostId, String)| {
                                if ops.busy_now() {
                                    return;
                                }
                                confirming_remove.set(None);
                                destination_draft.set(destination);
                                editing.set(Some(id));
                            },
                            on_edit_submit: on_edit_submit.clone(),
                            on_edit_cancel: move |_| editing.set(None),
                            on_remove_start: move |id: HostId| {
                                if ops.busy_now() {
                                    return;
                                }
                                editing.set(None);
                                confirming_remove.set(Some(id));
                            },
                            on_remove_confirm: on_remove_confirm.clone(),
                            on_remove_cancel: move |_| confirming_remove.set(None),
                            host,
                        }
                    }
                }
            }
        }
    }
}

/// One host's row: name, state chip, the evidence, the remedy, and whichever
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
/// handles, on the wrapper rather than on the chip so a test can find a row
/// and then assert about anything inside it.
#[component]
fn HostRow(
    host: Host,
    busy: bool,
    confirming_remove: bool,
    editing: bool,
    error: Option<String>,
    warning: Option<String>,
    destination_draft: Signal<String>,
    on_retry: EventHandler<HostId>,
    on_adopt: EventHandler<(HostId, String)>,
    on_edit_start: EventHandler<(HostId, String)>,
    on_edit_submit: EventHandler<(HostId, String)>,
    on_edit_cancel: EventHandler<()>,
    on_remove_start: EventHandler<HostId>,
    on_remove_confirm: EventHandler<HostId>,
    on_remove_cancel: EventHandler<()>,
) -> Element {
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

    rsx! {
        div {
            class: "host-row",
            "data-host-id": "{id}",
            "data-host-phase": "{phase_label(&host.state)}",
            "data-host-kind": "{kind_attribute}",
            div { class: "host-row-main",
                span { class: "host-name peer-value", dir: "ltr", "{shown_name}" }
                span { class: "host-chip {phase_class(&host.state)}", "{phase_label(&host.state)}" }
                if confirming_remove {
                    // Consequence first and never truncated, then the host
                    // it is about — the reading order the delete prompt
                    // established, for the reason recorded there: a long
                    // name must not be able to clip the sentence that says
                    // what the button does.
                    span { class: "confirm-consequence",
                        "forgetting a host leaves its supervisor and sessions running; re-adding \
                         the destination finds them again:"
                    }
                    span { class: "confirm-title peer-value", dir: "ltr", "\"{shown_name}\"" }
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
                        // Focus lands on the way OUT of the destructive
                        // action, through the plain HTML attribute rather
                        // than a fallible `set_focus` whose discarded
                        // `Result` could drop the safety behavior silently.
                        autofocus: true,
                        onclick: move |_| on_remove_cancel.call(()),
                        "cancel"
                    }
                } else if editing {
                    HostDestinationForm {
                        draft: destination_draft,
                        busy,
                        on_submit: move |destination| on_edit_submit.call((id, destination)),
                        on_cancel: move |_| on_edit_cancel.call(()),
                    }
                } else {
                    if let (Some(reported), Some(label)) = (adopt_identity, adopt_label) {
                        button {
                            r#type: "button",
                            class: "btn host-adopt",
                            disabled: busy,
                            onclick: move |_| on_adopt.call((id, reported.clone())),
                            // Its own isolated run inside the button, so an
                            // identity cannot rearrange the verb around it
                            // and make "adopt X" read as something else.
                            span { class: "peer-value", dir: "ltr", "{label}" }
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn host-retry",
                        disabled: busy,
                        onclick: move |_| on_retry.call(id),
                        "retry"
                    }
                    if manageable {
                        button {
                            r#type: "button",
                            class: "btn host-edit",
                            disabled: busy,
                            onclick: move |_| on_edit_start.call(edit_start.clone()),
                            "edit destination"
                        }
                        button {
                            r#type: "button",
                            class: "btn host-remove",
                            disabled: busy,
                            onclick: move |_| on_remove_start.call(id),
                            "remove"
                        }
                    }
                }
            }
            PeerLine { class: "host-detail", parts: detail }
            if let Some(remedy) = remedy {
                PeerLine { class: "host-remedy", parts: remedy }
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

/// The add-host form: an ssh destination, plus the two optional fields that
/// describe the remote INSTALL rather than its address.
///
/// Registering ALWAYS succeeds for a usable destination, whether or not
/// anything answers there — so this form's success is not a claim that the
/// host is up, and the panel's new chip is where "there was nothing there"
/// appears. That is deliberate helm behavior (it is what makes a registry
/// meaningful for a machine that is switched off), and it is why this form
/// closes on success rather than waiting for a connection that may never
/// come.
///
/// The two optional fields are exposed rather than hidden behind a default
/// because they are the difference between a working entry and a permanently
/// unreachable one whenever farhelm is not on the remote's `PATH` or its
/// supervisor serves a non-default state directory — the case the e2e
/// harness itself is built on.
///
/// `ops` is the page's live-operation token, claimed HERE at submit rather
/// than reflected from a prop: this form previously took no gate at all, so
/// its POST could start beside a create or a removal that had already begun
/// — and a `blocked` prop would not have fixed it, since a prop is one
/// render behind (see the `ops` module).
///
/// A 2xx whose body will not decode is a SUCCESS here: the host was
/// registered, so the form closes and the panel refreshes, and the unread
/// reply becomes a warning on the new row rather than an error on a form
/// that would otherwise invite the user to add the same host twice.
#[component]
fn AddHostForm(mut ops: OpLock, on_added: EventHandler<Option<String>>) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut ssh = use_signal(String::new);
    let mut remote_farhelm = use_signal(String::new);
    let mut remote_state_dir = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let busy = ops.busy();

    rsx! {
        form {
            class: "add-host-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                // The claim IS the guard, and it happens synchronously here:
                // the rerender that disables these controls is not
                // synchronous with the event that queued this submit, and a
                // second add would either duplicate the destination (409) or
                // register a second entry for one machine.
                if !ops.claim() {
                    return;
                }
                error.set(None);
                let base = base.clone();
                let (destination, farhelm, state_dir) = (ssh(), remote_farhelm(), remote_state_dir());
                spawn(async move {
                    match add_host(&base, &destination, &farhelm, &state_dir).await {
                        Ok(Commit::Confirmed) => on_added.call(None),
                        Ok(Commit::Unvalidated(warning)) => on_added.call(Some(warning)),
                        Err(e) => error.set(Some(e)),
                    }
                    // Released on every path, unlike the create form's,
                    // which navigates away: there is no navigation on
                    // success here, so a token left held would strand the
                    // whole page inert.
                    ops.release();
                });
            },
            // Same total opt-out of browser text mangling the create form's
            // command fields carry, for the same reason: all three of these
            // become part of a command line, and a "corrected" one dials or
            // execs something the user did not type.
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
                class: "btn add-host-submit",
                disabled: busy,
                "add"
            }
            if let Some(err) = error.read().clone() {
                div { class: "create-session-error add-host-error", "{err}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The chip must show the helm's OWN phase vocabulary, because a user
    /// comparing a refused operation ("host 3 is unreachable-reprobing, so
    /// this operation is refused…") against this panel has to find the same
    /// word in both. A friendlier synonym here would break that match with
    /// nothing to indicate it had.
    ///
    /// Exhaustive over the whole taxonomy rather than a sample: a label is a
    /// one-line match arm, and the failure it guards against — a new phase
    /// borrowing another's word — is invisible unless every phase is listed.
    #[test]
    fn every_phase_is_chipped_with_the_helms_own_label() {
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

    /// Peer-supplied text must not be able to lay out the sentence around
    /// it. The escaping half is asserted here; the isolation half is
    /// structural (each value is its own `Peer` run — see the next test).
    ///
    /// Directional overrides are the concrete attack: an RLO inside an
    /// identity reverses everything after it, so "recorded A, reported B"
    /// can be made to read as its own opposite while the values are
    /// untouched. Zero-width characters are the quieter one: two identities
    /// that differ only by a zero-width joiner render identically.
    #[test]
    fn directional_and_invisible_characters_are_escaped_rather_than_rendered() {
        for hostile in [
            "\u{202E}reversed",
            "id\u{200B}with-zero-width",
            "id\u{2066}isolated",
            "id\u{FEFF}bom",
            "id\u{0007}bell",
            "id\u{2028}line-separator",
        ] {
            let shown = display_peer(hostile);
            for ch in hostile.chars().filter(|ch| must_escape(*ch)) {
                assert!(
                    !shown.contains(ch),
                    "{hostile:?} must not render {ch:?} literally: {shown}"
                );
                assert!(
                    shown.contains(&format!("<U+{:04X}>", ch as u32)),
                    "{hostile:?} must show {ch:?} as an escape: {shown}"
                );
            }
        }
        // Ordinary text is untouched: escaping that mangled real identities
        // would make the panel unreadable for the 99% case.
        assert_eq!(
            display_peer("d1444cac-789f-4264-879c-38010250630a"),
            "d1444cac-789f-4264-879c-38010250630a"
        );
        // Strong-RTL letters carry no control character and are legitimate
        // text, so they pass through — the per-value isolation is what
        // bounds THEIR reordering, not the escaping.
        assert_eq!(display_peer("שלום"), "שלום");
    }

    /// An identity that is empty, or nothing but whitespace, must render as
    /// something a person can see and name.
    ///
    /// The failure this prevents is an adopt button reading `adopt ` and a
    /// mismatch whose two sides are a gap and a gap: the user is asked to
    /// approve something invisible, which is worse than an ugly label.
    #[test]
    fn empty_and_blank_identities_get_an_unambiguous_display_form() {
        assert_eq!(display_peer(""), "(empty)");
        assert_eq!(display_peer("   "), "(whitespace only: 3 characters)");
        assert_eq!(
            display_peer("\u{00A0}"),
            "(whitespace only: 1 characters)",
            "a non-breaking space is still invisible, and is still whitespace"
        );
        // A value whose only content is an escape is visible BECAUSE of the
        // escape, so it renders as itself rather than as the blank form.
        assert_eq!(display_peer("\u{202E}"), "<U+202E>");
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

    /// The manual-start hint belongs to exactly one cause, and it must offer
    /// a COMMAND rather than an installation: provisioning is M7's, and
    /// PLAN_M6.md is explicit that a registered destination with no running
    /// supervisor gets a manual path and never an offer to install.
    ///
    /// The `--state-dir` caveat is asserted because without it the hint is
    /// actively wrong for a helm started with one: the user would start a
    /// supervisor on the default state directory, which that helm never
    /// dials, and the row would stay exactly as it was.
    #[test]
    fn only_the_local_supervisor_cause_gets_the_manual_start_hint() {
        let hint = detail_text(
            &state_remedy(&HostPhase::Unreachable {
                cause: "local-supervisor-not-running".to_string(),
                last_error: "no such file or directory".to_string(),
            })
            .expect("the one unreachable cause with a remedy"),
        );
        assert!(
            hint.contains("farhelm supervisor run"),
            "the hint has to name the command: {hint}"
        );
        assert!(
            hint.contains("--state-dir"),
            "a helm with its own state dir needs the supervisor pointed at it: {hint}"
        );
        assert!(
            !hint.contains("install"),
            "provisioning is M7's; this must not offer to set anything up: {hint}"
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
    /// the snapshot (so the panel can keep drawing chips) while still
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
            notice.contains("version-skew"),
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
}
