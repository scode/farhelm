//! Record names, the directories they map to, and which slot may push which record.
//!
//! The names are the whole vocabulary the instructions use (`plan/<slug>`, `executor`, `landing`, `round/<n>`); the
//! `farhelm/` prefix and the slot's own directory under `executors/` are the tool's business. Keeping `executor`
//! free of a slot name is what lets one instruction text serve every executor slot.
//!
//! Write permission follows ownership: a record has exactly one writer at a time, and the sync rules in the `clone`
//! module rely on that (a local edit that collides with another slot's change is a stale leftover and loses, except
//! in a record being pushed, where the push refuses). An executor slot writes the plan it holds and its own log; the
//! landing slot writes its log and its rounds.

use std::fmt;

use super::Failure;

/// The slot name that makes a slot the landing slot; every other slot is an executor slot.
pub const LANDING_SLOT: &str = "landing";

/// A syntactically valid record name. Whether the current slot may push it is a separate check (`check_push`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordName {
    /// `plan/<slug>`: one plan's log and notes, written by whichever executor slot holds the plan.
    Plan(String),
    /// `executor`: the calling executor slot's own log.
    Executor,
    /// `landing`: the landing monitor's log.
    Landing,
    /// `round/<n>`: one landing round's text files.
    Round(String),
}

impl RecordName {
    /// Parses a name from the command line. A malformed name is a usage error (exit 2), found before any config is
    /// read.
    pub fn parse(text: &str) -> Result<Self, Failure> {
        let parsed = match text.split_once('/') {
            None if text == "executor" => Some(Self::Executor),
            None if text == "landing" => Some(Self::Landing),
            Some(("plan", slug)) if is_identifier(slug) => Some(Self::Plan(slug.to_string())),
            Some(("round", n)) if is_round_number(n) => Some(Self::Round(n.to_string())),
            _ => None,
        };
        parsed.ok_or_else(|| {
            Failure::usage(format!(
                "`{text}` is not a record name (expected plan/<slug>, executor, landing or round/<n>, where <slug> \
                 matches [a-z0-9][a-z0-9-]* and <n> is a positive integer without leading zeros)"
            ))
        })
    }

    /// The record's directory relative to the clone's root, for the given slot.
    ///
    /// Fails for `executor` from the landing slot: there is no executor log for it to name, and silently mapping it to
    /// `farhelm/executors/landing/` would give the monitor a log nobody reads.
    pub fn directory(&self, slot: &str) -> Result<String, Failure> {
        Ok(match self {
            Self::Plan(slug) => format!("farhelm/plans/{slug}"),
            Self::Executor if slot == LANDING_SLOT => {
                return Err(Failure::failed(
                    "`executor` names an executor slot's own log, and this is the landing slot",
                ));
            }
            Self::Executor => format!("farhelm/executors/{slot}"),
            Self::Landing => "farhelm/landing".to_string(),
            Self::Round(n) => format!("farhelm/rounds/{n}"),
        })
    }

    /// Refuses a push of a record this slot does not own (exit 1).
    pub fn check_push(&self, slot: &str) -> Result<(), Failure> {
        let landing = slot == LANDING_SLOT;
        let allowed = match self {
            Self::Plan(_) | Self::Executor => !landing,
            Self::Landing | Self::Round(_) => landing,
        };
        if allowed {
            Ok(())
        } else {
            let owner = if landing {
                "executor slots"
            } else {
                "the landing slot"
            };
            Err(Failure::failed(format!(
                "slot `{slot}` may not push `{self}`: only {owner} push it"
            )))
        }
    }
}

impl fmt::Display for RecordName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(slug) => write!(f, "plan/{slug}"),
            Self::Executor => f.write_str("executor"),
            Self::Landing => f.write_str("landing"),
            Self::Round(n) => write!(f, "round/{n}"),
        }
    }
}

/// `[a-z0-9][a-z0-9-]*`: the shape of plan slugs and slot names. It is also what makes them safe as single path
/// components and as git pathspecs (no `/`, `.`, glob characters or leading dash).
pub fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some('a'..='z' | '0'..='9'))
        && chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-'))
}

/// A positive decimal integer without leading zeros, so each round has exactly one spelling (`7`, never `07`).
fn is_round_number(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some('1'..='9')) && chars.all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four name shapes parse, and their directories carry the `farhelm/` prefix callers never write. This is the
    /// mapping every instruction and the landing script rely on, so a change here moves everyone's records.
    #[test]
    fn names_map_to_their_directories() {
        let cases = [
            (
                "plan/fix-the-thing",
                "exec-1",
                "farhelm/plans/fix-the-thing",
            ),
            ("executor", "exec-3", "farhelm/executors/exec-3"),
            ("landing", "landing", "farhelm/landing"),
            ("round/12", "landing", "farhelm/rounds/12"),
        ];
        for (text, slot, directory) in cases {
            let name = RecordName::parse(text).unwrap();
            assert_eq!(name.to_string(), text);
            assert_eq!(name.directory(slot).unwrap(), directory, "{text}");
        }
    }

    /// Malformed names are usage errors (exit 2), including the near misses that would otherwise produce a second
    /// spelling of the same record (`round/07`) or escape the record's directory (`plan/../x`).
    #[test]
    fn malformed_names_are_usage_errors() {
        for text in [
            "",
            "plan",
            "plan/",
            "plan/Upper",
            "plan/-lead",
            "plan/a/b",
            "plan/../x",
            "plan/a.b",
            "round/0",
            "round/07",
            "round/-1",
            "round/1x",
            "rounds/1",
            "executor/exec-1",
            "landing/x",
            "Landing",
        ] {
            let err = RecordName::parse(text).unwrap_err();
            assert_eq!(err.code, 2, "{text:?}");
        }
    }

    /// Push permissions follow ownership: executor slots push plans and their own log, the landing slot pushes its log
    /// and rounds, and each is refused the other's records with exit 1.
    #[test]
    fn push_permissions_follow_ownership() {
        let plan = RecordName::parse("plan/x").unwrap();
        let round = RecordName::parse("round/3").unwrap();
        assert!(plan.check_push("exec-1").is_ok());
        assert!(RecordName::Executor.check_push("exec-1").is_ok());
        assert!(RecordName::Landing.check_push("landing").is_ok());
        assert!(round.check_push("landing").is_ok());
        for (name, slot) in [
            (&plan, "landing"),
            (&RecordName::Executor, "landing"),
            (&RecordName::Landing, "exec-1"),
            (&round, "exec-2"),
        ] {
            assert_eq!(
                name.check_push(slot).unwrap_err().code,
                1,
                "{name} from {slot}"
            );
        }
    }

    /// `executor` has no meaning for the landing slot, so even reading it is refused rather than inventing
    /// `farhelm/executors/landing/`.
    #[test]
    fn executor_from_landing_is_refused() {
        assert_eq!(
            RecordName::Executor.directory("landing").unwrap_err().code,
            1
        );
    }
}
