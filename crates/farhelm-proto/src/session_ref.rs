//! Session references at the helm boundary bind an opaque supervisor ID to one host entry.
//!
//! Supervisors keep using their own bare IDs. The helm supplies the entry UUID, so a peer
//! cannot select another entry by returning an ID that merely looks like a full reference.
//! Parsing belongs at client ingress; formatting a supervisor reply always uses `new`.

use std::{fmt, str::FromStr};

/// The helm admits a supervisor's 36-byte UUID with four bytes of format margin.
/// This is deliberately separate from the unchanged wire relay's larger target allowance.
pub const MAX_BARE_SESSION_ID_BYTES: usize = 40;

/// A canonical entry UUID, its separator, and the largest admitted bare ID.
pub const MAX_FULL_SESSION_ID_BYTES: usize = 36 + 1 + MAX_BARE_SESSION_ID_BYTES;

/// An entry-bound session identity; neither component can be replaced after validation.
///
/// The separator is always byte 36. The bare suffix is opaque and may contain colons,
/// including text resembling another full reference. Existence and authorization are
/// separate checks: a valid reference can name a deleted entry or a missing session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionRef {
    entry_uuid: String,
    session_id: String,
}

/// A malformed boundary reference, with a static reason safe to show without echoing its input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct SessionRefError(&'static str);

impl SessionRef {
    /// Qualify a supervisor-reported ID with the entry that supplied it.
    ///
    /// The suffix is never parsed as an existing full reference. In particular, a host's
    /// choice of suffix cannot replace the helm-assigned prefix.
    pub fn new(entry_uuid: &str, session_id: &str) -> Result<Self, SessionRefError> {
        if !is_entry_uuid(entry_uuid) {
            return Err(SessionRefError(
                "the host entry UUID is not canonical lowercase UUID text",
            ));
        }
        if let Some(problem) = bare_session_id_problem(session_id) {
            return Err(SessionRefError(problem));
        }
        Ok(Self {
            entry_uuid: entry_uuid.to_owned(),
            session_id: session_id.to_owned(),
        })
    }

    /// The helm-assigned entry identity, for an entry-scoped registry lookup.
    pub fn entry_uuid(&self) -> &str {
        &self.entry_uuid
    }

    /// The exact ID to send to the resolved entry's supervisor, without the helm prefix.
    pub fn bare_id(&self) -> &str {
        &self.session_id
    }
}

impl FromStr for SessionRef {
    type Err = SessionRefError;

    /// Refuse bare IDs and malformed prefixes without guessing an owner fleet-wide.
    /// Agent callers apply their exact-own-bare-ID exception before invoking this parser.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let prefix = value.get(..36).ok_or(SessionRefError(
            "a full session ID must start with a host entry UUID and a colon",
        ))?;
        if value.as_bytes().get(36) != Some(&b':') {
            return Err(SessionRefError(
                "a full session ID needs a colon after its host entry UUID",
            ));
        }
        Self::new(prefix, &value[37..])
    }
}

impl fmt::Display for SessionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.entry_uuid, self.session_id)
    }
}

/// Validate the fixed ASCII prefix without adding UUID randomness support to browser builds.
/// UUID version and variant do not participate in routing; canonical spelling does.
pub fn is_entry_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, byte)| match i {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte),
        })
}

/// The helm's bare-ID admission rule, shared by its ingress and full-reference parser.
/// Opaque text remains allowed within the small bound; control characters and URL dot
/// segments cannot safely identify a session in CLI output or a browser path.
pub fn bare_session_id_problem(value: &str) -> Option<&'static str> {
    if value.is_empty() {
        Some("the session id is empty")
    } else if value.len() > MAX_BARE_SESSION_ID_BYTES {
        Some("the session id is past the helm ingress cap")
    } else if value.chars().any(char::is_control) {
        Some("the session id contains control characters")
    } else if value == "." || value == ".." {
        Some("the session id is a dot segment")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY: &str = "0f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d";

    /// A colon in the opaque suffix must not change the entry chosen by the prefix.
    #[farhelm_testtrace::test]
    fn a_reference_round_trips_its_exact_entry_and_opaque_suffix() {
        let looks_qualified = format!("{ENTRY}:id");
        for bare in [
            "session",
            "other:session",
            "a/%:雪",
            " a ",
            looks_qualified.as_str(),
        ] {
            let reference = SessionRef::new(ENTRY, bare).unwrap();
            let parsed: SessionRef = reference.to_string().parse().unwrap();
            assert_eq!(parsed, reference);
            assert_eq!(parsed.entry_uuid(), ENTRY);
            assert_eq!(parsed.bare_id(), bare);
        }
    }

    /// Fixed byte offsets must reject malformed UTF-8 boundaries without panicking,
    /// and noncanonical spellings must not introduce aliases for one host entry.
    #[farhelm_testtrace::test]
    fn malformed_and_noncanonical_prefixes_are_refused() {
        for value in [
            "bare-session".to_string(),
            format!("{}:session", ENTRY.to_uppercase()),
            format!("{};session", ENTRY),
            format!("{}:session", ENTRY.replace('-', "_")),
            format!("{}雪:session", "a".repeat(35)),
            format!("{}:session", "a".repeat(36)),
        ] {
            assert!(value.parse::<SessionRef>().is_err(), "{value:?}");
        }
    }

    /// Limits count bytes, so multibyte suffixes cannot consume more relay or URL
    /// space than an ASCII suffix at the same accepted boundary.
    #[farhelm_testtrace::test]
    fn the_small_bare_and_full_limits_are_inclusive() {
        for bare in ["x".repeat(40), "é".repeat(20)] {
            let reference = SessionRef::new(ENTRY, &bare).unwrap();
            assert_eq!(reference.to_string().len(), MAX_FULL_SESSION_ID_BYTES);
            assert!(reference.to_string().parse::<SessionRef>().is_ok());
        }
        for bare in ["x".repeat(41), "é".repeat(21)] {
            assert!(SessionRef::new(ENTRY, &bare).is_err());
            assert!(format!("{ENTRY}:{bare}").parse::<SessionRef>().is_err());
        }
    }

    /// Qualification must refuse IDs that existing helm ingress cannot address safely.
    #[farhelm_testtrace::test]
    fn empty_control_and_dot_suffixes_are_refused() {
        for bare in ["", ".", "..", "a\nb", "a\u{1b}b"] {
            assert!(SessionRef::new(ENTRY, bare).is_err(), "{bare:?}");
            assert!(format!("{ENTRY}:{bare}").parse::<SessionRef>().is_err());
        }
    }
}
