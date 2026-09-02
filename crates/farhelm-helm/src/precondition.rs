//! The one precondition a session create may carry: which CONNECTION the
//! caller prepared it against.
//!
//! ## Why this survived the profile-default simplification
//!
//! This module once held two checks for every host-scoped mutation: which
//! install (`expected_incarnation`) and which definition (`expected_definition`
//! on profile edits). Both were removed from the profile routes along with
//! the install-bound remembered default — profile edits are last-write-wins
//! now (SPEC.md, Concepts / Agent profile), and a default is only a suggestion
//! in a dropdown. The session CREATE kept its guard on purpose, because a
//! create is an ACTION and the failure it closes is a silent success on the
//! wrong machine:
//!
//! A create names its host by REGISTRY ID, and a registry id outlives the
//! install it points at — a retarget or an adoption in another tab replaces
//! what answers on the id without the id changing. The selected profile is
//! helm-wide now, but the action can still succeed on the WRONG installation.
//! The client checks before it sends; the window it cannot close is between
//! its own check and the helm's routing, so the check travels WITH the
//! request. The still-open TODO entry on the HostId-reuse create-default
//! window relies on this check by name.
//!
//! ## Optional, always
//!
//! Absent means "no expectation", and the create behaves exactly as it did
//! before this guard existed: `curl`, scripts, the CLI, and older UI builds
//! have no incarnation to name, and a mandatory precondition would make the
//! API unusable by hand for the sake of a race those callers are not in.
//!
//! ## The refusal, and what a client branches on
//!
//! A 409, because it is "the world moved, ask again" rather than anything
//! wrong with the request. Error bodies in this API are PROSE shown verbatim
//! (`crate::http_error`), so the machine-readable part is a stable marker
//! appended to the sentence — [`INCARNATION_MARKER`]. A client branches on the
//! marker and may strip a trailing bracketed one before display; a conflict
//! carrying NO marker is one of the other kinds (a host that is not connected,
//! a supervisor's own refusal) and is not retried by re-reading. A value that
//! is not a number never reaches here at all — axum's extractor rejects it as
//! a 400, which is the right split: unparseable is a client bug, stale is a
//! world that moved.

use crate::manager::SessionClaim;
use farhelm_proto::ErrorKind;

/// The marker an incarnation-precondition refusal ends with.
///
/// Stable across releases: it is API, not diagnostics. Bracketed and trailing
/// so the prose in front of it stays the sentence a user is shown.
pub(crate) const INCARNATION_MARKER: &str = "[farhelm:precondition/incarnation]";

/// Refuse unless `expected` names the connection `claim` was taken on.
///
/// `None` is not a wildcard so much as an absence of any claim about the
/// world — see this module's docs on why that stays supported.
///
/// Compared against the CLAIM rather than against a fresh status read, and
/// that is the whole point: the claim is the connection this request will
/// actually be performed on, so a comparison against anything else could pass
/// while the create goes somewhere third.
pub(crate) fn incarnation_holds(claim: &SessionClaim, expected: Option<u64>) -> anyhow::Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if expected == claim.incarnation {
        return Ok(());
    }
    Err(anyhow::Error::new(crate::SupervisorError {
        kind: ErrorKind::Conflict,
        message: format!(
            "host {} is not the connection this request was prepared against (it named connection \
             {expected}, and this host is now on connection {}): a retarget, an adoption, or a \
             reconnection has replaced what answers on that host, and profile ids from the previous \
             install can resolve here to something else entirely — so nothing was changed. Re-read \
             the host and try again {INCARNATION_MARKER}",
            claim.host, claim.incarnation
        ),
    }))
}
