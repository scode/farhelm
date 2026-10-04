//! The one precondition a session create may carry: which CONNECTION the
//! caller prepared it against.
//!
//! ## Why only the create carries it
//!
//! This module once held checks for other host-scoped mutations too; they
//! went with the routes they guarded. The session CREATE kept its guard on
//! purpose, because a create is an ACTION and the failure it closes is a
//! silent success on the wrong machine:
//!
//! A create names its host by REGISTRY ID, and a registry id outlives the
//! install it points at — a retarget or an adoption in another tab replaces
//! what answers on the id without the id changing, so the action can succeed
//! on the WRONG installation.
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
//! (`crate::http_error`), so the machine-readable part travels in a header,
//! `farhelm_proto::http::PRECONDITION_HEADER`, which `http_error` sets when it
//! finds an [`IncarnationStale`] in the error. It used to be a marker appended
//! to the body, but a body can carry a remote supervisor's text verbatim, and
//! a supervisor that ended its message with the marker could make the client
//! discard its intent and re-read. A conflict without the header is one of
//! the other kinds (a host that is not connected, a supervisor's own refusal)
//! and is not retried by re-reading. A value that is not a number never
//! reaches here at all — axum's extractor rejects it as a 400, which is the
//! right split: unparseable is a client bug, stale is a world that moved.

use crate::manager::SessionClaim;

/// The incarnation-precondition refusal: the request named a connection the
/// host is no longer on.
///
/// Its own type so `http_error` can recognize it and add the precondition
/// header, and so `error_kind` can classify it as `Conflict` (which the agent
/// relay relies on) without the message having to carry anything a client
/// parses. `Display` is the sentence a user is shown.
#[derive(Debug, thiserror::Error)]
#[error(
    "host {host} is not the connection this request was prepared against (it named connection \
     {expected}, and this host is now on connection {current}): a retarget, an adoption, or a \
     reconnection has replaced what answers on that host, and a launch prepared for the \
     previous install may mean something else entirely here — so nothing was changed. Re-read \
     the host and try again"
)]
pub(crate) struct IncarnationStale {
    host: crate::store::HostId,
    expected: u64,
    current: u64,
}

/// This request's own validation failure, quoted as context on a different
/// outcome that takes precedence.
///
/// A keyed fresh create whose local validation fails asks the supervisor for
/// the recorded outcome of that key, and when the supervisor refuses, its
/// refusal is what the client must see, with the local failure quoted after
/// it. Quoting is text, so this keeps the one fact the client acts on:
/// whether that local failure was a stale connection.
#[derive(Debug, thiserror::Error)]
#[error("current request validation also failed: {text}")]
pub(crate) struct AlsoFailedValidation {
    text: String,
    stale: bool,
}

impl AlsoFailedValidation {
    pub(crate) fn of(local: &anyhow::Error) -> Self {
        AlsoFailedValidation {
            text: format!("{local:#}"),
            stale: crate::find_cause::<IncarnationStale>(local).is_some(),
        }
    }
}

/// Whether the helm refused this request because its connection went stale,
/// either directly or as a quoted local failure. `http_error` sets the
/// precondition header from this and from nothing else.
pub(crate) fn is_stale(e: &anyhow::Error) -> bool {
    crate::find_cause::<IncarnationStale>(e).is_some()
        || crate::find_cause::<AlsoFailedValidation>(e).is_some_and(|quoted| quoted.stale)
}

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
    Err(anyhow::Error::new(IncarnationStale {
        host: claim.host,
        expected,
        current: claim.incarnation,
    }))
}
