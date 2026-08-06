//! Optional preconditions a client can attach to a request, so one prepared
//! against a given state cannot execute — or be answered — in another.
//!
//! ## What this closes, and why the client cannot close it alone
//!
//! Every host-scoped mutation names a host by REGISTRY ID, and a registry id
//! outlives the install it points at: a retarget or an adoption in another tab
//! (or another client entirely) replaces what answers on that id, and the id
//! itself never changes. A browser that read a catalog, opened a dialog, and
//! posted an edit therefore describes a moment that may already be over, and
//! the helm — which sees only the id — cannot tell.
//!
//! That would be merely stale if the request then failed. It does not fail:
//! profile ids are minted per supervisor and every fresh supervisor seeds the
//! same STARTER profiles, so an edit or a profile-backed create aimed at
//! `starter-claude` RESOLVES on the successor install, against a profile the
//! user never opened. The failure mode is a silent success on the wrong
//! machine.
//!
//! A client can check before it sends, and the UI does; the window it cannot
//! close is between its own check and the helm's routing. So the check has to
//! travel WITH the request, which is what these preconditions are.
//!
//! A READ can want the same guarantee, for a different reason: it changes
//! nothing on the far side, but a client that FILES the answer under (host,
//! connection) — as the UI's catalog storage does — records the successor's
//! catalog under the predecessor's key when the two diverge mid-request, and
//! everything it later does with that cache is about the wrong install. So
//! `GET /api/hosts/{id}/profiles` takes one too.
//!
//! ## Optional, always
//!
//! Absent means "no expectation", and the request behaves exactly as it did
//! before this module existed. That is deliberate rather than transitional:
//! `curl`, scripts, the CLI, and older UI builds are all legitimate callers
//! that have no incarnation to name, and a mandatory precondition would make
//! the API unusable by hand for the sake of a race those callers are not in.
//!
//! ## Two expectations, checked at two different layers
//!
//! - [`incarnation_holds`] is about WHICH INSTALL. It compares the caller's
//!   expectation against the connection token the routing decision was made
//!   on ([`crate::manager::SessionClaim::incarnation`]), which is minted
//!   afresh whenever a host's client changes — including when it goes away.
//! - [`definition_holds`] is about WHICH DEFINITION, and exists because a
//!   profile update replaces the whole definition: two editors open on one
//!   profile otherwise resolve by last-write-wins, and the loser's change
//!   disappears with nothing anywhere reporting that it did.
//!
//! ## How a precondition relates to the claim revalidation already here
//!
//! They are two halves of one chain and neither is sufficient alone.
//!
//! A precondition binds the CALLER'S expectation to the claim the mutation is
//! about to be performed under: after [`incarnation_holds`] passes, "the
//! connection the caller meant" and "the connection this request routed to"
//! are the same connection. The claim revalidation that already guards every
//! mutation (`crate::profiles::still_the_same_connection`, and the manager's
//! own `claim_is_current` under each host's cache lock) then binds that claim
//! to the moment the result is reported or recorded. Composed, they say: this
//! mutation ran on the install the caller named, and that install is still
//! what the id means now.
//!
//! Which is why a precondition is checked at every point the claim is taken —
//! before routing, and again under the per-host mutation lock, since a request
//! can queue there for as long as the edit ahead of it takes.
//!
//! And why the LAST check is a different function. By the time a result is
//! ready, what the claim revalidation notices is that the world moved, not
//! that any number disagreed; [`connection_moved`] is how that discovery is
//! reported to a caller that guarded, in the vocabulary it guarded in.
//!
//! ## What no server-side check can close
//!
//! The moment after the reply leaves. A connection replaced while a 200 is in
//! flight produces an answer that was true when it was written and describes
//! an install that is already gone — for a read, a catalog filed under a key
//! that no longer applies; for a mutation, a success reported for a host that
//! has since moved on. The client's own activation gate is what covers it:
//! whatever it keyed the answer by, checked again when it USES what it kept,
//! plus the invalidation feed telling it to re-read the moment a connection
//! changes. Preconditions narrow the window to the reply's own flight time;
//! they cannot remove it, and a client that believed otherwise would be
//! trusting a guarantee nothing offers.
//!
//! ## A malformed precondition is a 400, not a 409
//!
//! `expected_incarnation` is a `u64` in a JSON body (or a query parameter on
//! the delete), so a value that is not a number never reaches these checks at
//! all — axum's own extractor rejects it before the handler runs, with its own
//! status and its own prose. That split is the right one: a value this API
//! cannot parse is a client bug, while a value it can parse and finds stale is
//! a world that moved. Only the second is worth a marker, and only the second
//! is worth retrying after a re-read.
//!
//! ## The refusal, and what a client branches on
//!
//! Both are 409s, because both are "the world moved, ask again" rather than
//! anything wrong with the request. Error bodies in this API are PROSE shown
//! verbatim (`crate::http_error`), so the machine-readable part is a stable
//! marker appended to the end of the sentence — [`INCARNATION_MARKER`] and
//! [`DEFINITION_MARKER`]. A client branches on the marker and may strip a
//! trailing bracketed one before display; a conflict carrying NO marker is one
//! of the other kinds (a host that is not connected, an ambiguous session
//! owner, a supervisor's own refusal) and is not retried by re-reading.

use crate::manager::SessionClaim;
use crate::store::HostId;
use farhelm_proto::{AgentKind, ErrorKind, Profile};

/// The marker an incarnation-precondition refusal ends with.
///
/// Stable across releases: it is API, not diagnostics. Bracketed and trailing
/// so the prose in front of it stays the sentence a user is shown.
pub(crate) const INCARNATION_MARKER: &str = "[farhelm:precondition/incarnation]";

/// The marker a definition-precondition refusal ends with. See
/// [`INCARNATION_MARKER`].
pub(crate) const DEFINITION_MARKER: &str = "[farhelm:precondition/definition]";

/// Refuse unless `expected` names the connection `claim` was taken on.
///
/// `None` is not a wildcard so much as an absence of any claim about the
/// world — see this module's docs on why that stays supported.
///
/// Compared against the CLAIM rather than against a fresh status read, and
/// that is the whole point: the claim is the connection this request will
/// actually be performed on, so a comparison against anything else could pass
/// while the mutation goes somewhere third. Every call site therefore checks
/// wherever it takes a claim, not once at the top.
pub(crate) fn incarnation_holds(claim: &SessionClaim, expected: Option<u64>) -> anyhow::Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if expected == claim.incarnation {
        return Ok(());
    }
    Err(conflict(format!(
        "host {} is not the connection this request was prepared against (it named connection \
         {expected}, and this host is now on connection {}): a retarget, an adoption, or a \
         reconnection has replaced what answers on that host, and profile ids from the previous \
         install can resolve here to something else entirely — so nothing was changed. Re-read the \
         host and try again {INCARNATION_MARKER}",
        claim.host, claim.incarnation
    )))
}

/// Refuse unless `expected` describes the definition currently stored.
///
/// `stored` is the profile as the catalog holds it right now, read under the
/// per-host mutation lock; `None` means the catalog has no such profile, which
/// fails any expectation — an editor seeded from a definition cannot be
/// rewriting one that is gone.
///
/// The comparison is on [`definition_fingerprint`], which is deliberately the
/// same four fields the identical-edit suppression compares (see
/// [`crate::profiles::update_profile`]). Those two must agree: a submission
/// the suppression calls "identical to what is stored" is exactly a submission
/// whose fingerprint equals the stored one, and if the two notions could
/// disagree an edit could be both suppressed as a no-op and refused as stale.
pub(crate) fn definition_holds(
    host: HostId,
    profile_id: &str,
    stored: Option<&Profile>,
    expected: Option<&str>,
) -> anyhow::Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let current = stored.map(definition_fingerprint);
    if current.as_deref() == Some(expected) {
        return Ok(());
    }
    let what = match stored {
        None => format!("profile {profile_id} no longer exists on host {host}"),
        Some(_) => format!(
            "profile {profile_id} on host {host} has been changed since this editor was opened"
        ),
    };
    Err(conflict(format!(
        "{what}, and this update would have replaced the whole definition — so it was refused \
         rather than silently reverting somebody else's change. Re-read the profile and reapply \
         {DEFINITION_MARKER}"
    )))
}

/// The refusal a GUARDED request gets when the connection it named was still
/// current at routing but has moved by the time there is something to report.
///
/// The same marker as [`incarnation_holds`], because it is the same fact
/// arriving later: the connection the caller asserted is not the one this
/// answer describes. It exists as its own constructor because that later
/// discovery is made against a CLAIM rather than against a number — the
/// generic revalidation (`crate::profiles::still_the_same_connection`) is what
/// notices, and its refusal is unmarked, which would drop a guarded caller
/// into the branch it wrote for "the host is busy" instead of the one it wrote
/// for "the world moved".
///
/// `current` is what the host is on NOW, or `None` for a host that has since
/// gone away entirely — carried only so the caller's log or toast can say what
/// it was overtaken by.
pub(crate) fn connection_moved(host: HostId, expected: u64, current: Option<u64>) -> anyhow::Error {
    let now = match current {
        Some(current) => format!("connection {current}"),
        None => "no connection at all".to_string(),
    };
    conflict(format!(
        "host {host} left connection {expected} while this request was being answered (it is on \
         {now} now), so what came back describes an install that is no longer what this host \
         means — nothing is being reported. Re-read the host and try again {INCARNATION_MARKER}"
    ))
}

/// The refusal a definition precondition gets when the catalog it would be
/// compared against could not be read.
///
/// Same marker as a mismatch, because the caller's recovery is the same: read
/// the profile again and reapply. What it must NOT do is fall through to the
/// edit — an update whose precondition could not be evaluated is an update
/// reported as guarded while nothing was compared, and a client that asked for
/// the guarantee would then believe it. (The suppression on the same path does
/// fall through, deliberately: forwarding an unverified edit is what this
/// endpoint has always done, and the caller asserted nothing.)
pub(crate) fn definition_unverifiable(
    host: HostId,
    profile_id: &str,
    error: &anyhow::Error,
) -> anyhow::Error {
    conflict(format!(
        "host {host}'s profile catalog could not be read ({error:#}), so the expected_definition \
         this update to profile {profile_id} carries could not be checked and nothing was \
         forwarded — an unchecked precondition is worse than a refused one. Try again \
         {DEFINITION_MARKER}"
    ))
}

/// The canonical encoding of a profile DEFINITION — what `expected_definition`
/// carries.
///
/// ## Not a hash, deliberately
///
/// The value has to be produced by a browser and compared by this process, and
/// the two share no hash primitive whose exact bytes are worth betting a
/// precondition on. A digest would also make a COLLISION into a silent pass,
/// which is precisely the failure this precondition exists to prevent. So the
/// encoding IS the fingerprint: reproducible from the definition in any
/// language, comparable by string equality, and bounded by the definition's own
/// size (which the supervisor caps per field).
///
/// Clients need not build it at all — `GET /api/hosts/{id}/profiles` serves the
/// fingerprint of every profile it returns
/// ([`crate::profiles::ProfilesView::definitions`]), and echoing that value
/// back is the intended use.
///
/// ## The encoding
///
/// Length-prefixed per field, in BYTES of UTF-8, in a fixed order — the same
/// idiom (and the same reasoning) as `crate::store::SessionFilter::fingerprint`:
/// no combination of user text can spell another definition's encoding, which a
/// delimiter-joined form would allow the moment somebody names a profile
/// `x;i5:claude`.
///
/// ```text
/// n<len>:<name>;i<len>:<invocation>;k<len>:<agent kind>;
/// then either  r-;                       (no resume template)
/// or           r<count>;<len>:<arg>;...  (one length-prefixed arg each)
/// ```
///
/// The profile ID is NOT in it. The id travels in the URL and is what selects
/// the row being compared, so including it would only let a caller assert
/// something the request already states.
pub(crate) fn definition_fingerprint(profile: &Profile) -> String {
    fn field(out: &mut String, tag: char, value: &str) {
        out.push_str(&format!("{tag}{}:{value};", value.len()));
    }
    let mut out = String::new();
    field(&mut out, 'n', &profile.name);
    field(&mut out, 'i', &profile.invocation);
    field(&mut out, 'k', agent_kind_key(profile.agent_kind));
    match &profile.resume_template {
        // An ABSENT template and an empty one are different definitions —
        // absence means "derive one per kind" — so they must encode
        // differently. See `Profile::resume_template`.
        None => out.push_str("r-;"),
        Some(args) => {
            out.push_str(&format!("r{};", args.len()));
            for arg in args {
                field(&mut out, 'a', arg);
            }
        }
    }
    out
}

/// The wire word for an agent kind — the same spelling `AgentKind`'s
/// `snake_case` serde produces, so a fingerprint built from a JSON profile a
/// client already holds needs no translation.
///
/// Matched exhaustively rather than serialized through serde: a new kind must
/// be a compile error here, since a wrong word would make every guarded update
/// on that kind refuse forever.
fn agent_kind_key(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Claude => "claude",
        AgentKind::Codex => "codex",
        AgentKind::Generic => "generic",
    }
}

/// A typed 409 whose body is `message` verbatim — see this module's docs on
/// the marker convention, and `crate::http_error` for the mapping.
fn conflict(message: String) -> anyhow::Error {
    anyhow::Error::new(crate::SupervisorError {
        kind: ErrorKind::Conflict,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, invocation: &str, template: Option<Vec<&str>>) -> Profile {
        Profile {
            id: "p-1".to_string(),
            name: name.to_string(),
            invocation: invocation.to_string(),
            agent_kind: AgentKind::Claude,
            resume_template: template
                .map(|args| args.into_iter().map(str::to_string).collect::<Vec<_>>()),
        }
    }

    /// The fingerprint agrees with the identical-edit suppression's own notion
    /// of "the same definition", in both directions.
    ///
    /// Spec: two definitions have equal fingerprints exactly when the
    /// suppression would call them identical.
    ///
    /// This is the coupling the precondition rests on. If a definition could
    /// be "identical" to the suppression and different to this, an edit would
    /// be suppressed as a no-op AND refused as stale by the same request; if
    /// it could be different to the suppression and identical here, a stale
    /// editor would be waved through to overwrite the change it never saw —
    /// which is the whole thing being prevented.
    ///
    /// The adjacent-field cases are here because the encoding is what stands
    /// between user text and a forged fingerprint: a profile may be NAMED
    /// anything, including the encoding of another profile.
    #[test]
    fn a_definition_fingerprint_agrees_with_the_suppressions_equality() {
        let cases = [
            profile("Claude Code", "claude", None),
            profile("Claude Code", "claude", Some(vec![])),
            profile("Claude Code", "claude", Some(vec!["claude", "--resume"])),
            profile("Claude Code", "claude", Some(vec!["claude --resume"])),
            // Adjacent fields must not be able to borrow each other's text.
            profile("ab", "c", None),
            profile("a", "bc", None),
            // A name that spells out what a delimiter-joined encoding would
            // emit for a different definition.
            profile("x;i6:claude;", "claude", None),
            profile("x", "claude", None),
        ];
        for left in &cases {
            for right in &cases {
                assert_eq!(
                    definition_fingerprint(left) == definition_fingerprint(right),
                    left == right,
                    "fingerprint equality must track definition equality: {left:?} vs {right:?}"
                );
            }
        }

        // And the id is deliberately outside the fingerprint: it is the URL's,
        // not the body's, so it cannot be part of what an editor asserts.
        let mut renamed_id = profile("Claude Code", "claude", None);
        renamed_id.id = "p-2".to_string();
        assert_eq!(
            definition_fingerprint(&renamed_id),
            definition_fingerprint(&profile("Claude Code", "claude", None))
        );
    }

    /// An absent expectation is not an expectation: both checks pass, so
    /// every pre-existing caller keeps working.
    ///
    /// Worth pinning explicitly rather than trusting the `Option` to read
    /// obviously — the opposite default (absent means "must be unchanged")
    /// would break `curl`, the CLI, and every older client at once, which is
    /// the kind of regression a compiler cannot catch.
    #[test]
    fn an_absent_precondition_is_no_precondition() {
        let claim = SessionClaim {
            host: 3,
            incarnation: 12,
            identity: None,
        };
        assert!(incarnation_holds(&claim, None).is_ok());
        assert!(definition_holds(3, "p-1", None, None).is_ok());
        assert!(definition_holds(3, "p-1", Some(&profile("a", "b", None)), None).is_ok());
    }

    /// Each refusal is a 409 carrying its OWN marker, and never the other's.
    ///
    /// Spec: an incarnation mismatch and a definition mismatch are both
    /// `ErrorKind::Conflict`, and each body ends with exactly one of the two
    /// markers.
    ///
    /// The markers are what a client branches on — "re-read the host" versus
    /// "re-read the profile and reapply your change" are different recoveries,
    /// and every OTHER 409 this API produces (a host that is not connected, a
    /// supervisor's own refusal) means "not now" rather than either. Pinned on
    /// the exact strings because they are API: changing one silently turns a
    /// client's branch into a dead branch.
    #[test]
    fn each_precondition_refusal_carries_its_own_marker() {
        let claim = SessionClaim {
            host: 3,
            incarnation: 12,
            identity: None,
        };
        let stale = incarnation_holds(&claim, Some(11)).expect_err("11 is not 12");
        let refusal = stale
            .downcast_ref::<crate::SupervisorError>()
            .expect("typed, so the REST edge can map it");
        assert_eq!(refusal.kind, ErrorKind::Conflict);
        assert!(
            refusal.message.ends_with(INCARNATION_MARKER),
            "the marker is a trailing token, so a client can strip it for display: {}",
            refusal.message
        );
        assert!(!refusal.message.contains(DEFINITION_MARKER));

        let current = profile("Claude Code", "claude", None);
        let stale = definition_holds(3, "p-1", Some(&current), Some("n1:x;i1:y;k6:claude;r-;"))
            .expect_err("that is not this definition");
        let refusal = stale
            .downcast_ref::<crate::SupervisorError>()
            .expect("typed, so the REST edge can map it");
        assert_eq!(refusal.kind, ErrorKind::Conflict);
        assert!(refusal.message.ends_with(DEFINITION_MARKER));
        assert!(!refusal.message.contains(INCARNATION_MARKER));

        // A profile that has been DELETED under the editor fails the same
        // precondition, and says so rather than reporting a mismatch against
        // a definition that is not there.
        let gone = definition_holds(3, "p-1", None, Some(&definition_fingerprint(&current)))
            .expect_err("a deleted profile satisfies no expectation");
        let refusal = gone
            .downcast_ref::<crate::SupervisorError>()
            .expect("typed");
        assert!(refusal.message.contains("no longer exists"));
        assert!(refusal.message.ends_with(DEFINITION_MARKER));

        // And the expectation that HOLDS produces no refusal at all.
        assert!(incarnation_holds(&claim, Some(12)).is_ok());
        assert!(
            definition_holds(
                3,
                "p-1",
                Some(&current),
                Some(&definition_fingerprint(&current))
            )
            .is_ok()
        );
    }
}
