//! Process authority shared by report integrations, independent of vendor records.
//!
//! Each corridor chooses the emitter and ancestors allowed to correct it. This
//! policy compares only identities; vendor admission still validates the report
//! and decides what accepting or correcting it means for the conversation.

use serde::{Deserialize, Serialize};

use crate::procs::{ChainLink, ProcessIdentity};

/// One launch's admitted emitter and the ancestors allowed to correct it.
/// A conversation survives relaunch while process authority does not, so the pin
/// is stored separately. PID plus kernel start token fences PID reuse. Keep this
/// JSON shape compatible with the pins already stored for Codex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReportPin {
    pub(crate) process: ProcessIdentity,
    pub(crate) ancestors: Vec<ProcessIdentity>,
}

/// The candidate's authority, before vendor-specific conversation validation.
/// Correction lets a recorded ancestor replace a mistaken child selection;
/// acceptance still obeys the vendor's ordinary binding constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PinDecision {
    Accept,
    Correct,
    Refuse,
}

impl ReportPin {
    /// Prepare the pin to write after the caller has admitted a report.
    ///
    /// The first report from an emitter defines which ancestors may correct it.
    /// A later snapshot can end at a different anchor, so preserve the stored
    /// bytes when the same emitter reports again.
    pub(crate) fn replacement_json(
        previous: Option<(&str, &Self)>,
        emitter: ProcessIdentity,
        ancestors: Vec<ProcessIdentity>,
    ) -> serde_json::Result<String> {
        match previous {
            Some((stored, pin)) if pin.process == emitter => Ok(stored.to_owned()),
            _ => serde_json::to_string(&Self {
                process: emitter,
                ancestors,
            }),
        }
    }

    /// Release a dead pin only for a report outside its recorded process chain.
    /// Recorded ancestors take precedence even after the pinned child exits.
    /// The complete incoming chain fences descendants across exec, which can
    /// change an image without changing PID/start. The smaller correction list
    /// cannot establish that a report is outside that chain.
    ///
    /// Admission supplies liveness only after it has reliable process evidence;
    /// an unavailable read must never be passed here as proof of death.
    pub(crate) fn judge(
        &self,
        emitter: ProcessIdentity,
        alive: bool,
        chain: &[ChainLink],
    ) -> PinDecision {
        if self.process == emitter {
            PinDecision::Accept
        } else if self.ancestors.contains(&emitter) {
            PinDecision::Correct
        } else if !alive
            && !chain
                .iter()
                .any(|link| link.pid == self.process.pid && link.start == self.process.start)
        {
            PinDecision::Accept
        } else {
            PinDecision::Refuse
        }
    }
}
