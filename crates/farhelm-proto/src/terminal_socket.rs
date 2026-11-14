//! The control messages on a terminal socket: the text frames beside the binary grid messages, in each direction.
//!
//! A terminal socket (the helm's `/api/sessions/{id}/term` routes, one per attached terminal) carries the grid stream
//! as binary frames and a small control vocabulary as JSON text frames, each `{"type": ..., ...}`. Both ends used to
//! spell that vocabulary separately: the helm with string constants and its own parse enum, the GUI's helm client with
//! its own notice enum and literal JSON. Two spellings of one wire format drift without anything noticing, so the
//! helm and the client now serialize and parse these two enums.
//!
//! Unknown messages are ignored in both directions, never fatal: an older end must keep working against a newer one
//! that added a message. On the GUI side that is [`TerminalServerMessage::Unknown`]; on the helm side a frame that
//! fails to parse is dropped.

use serde::{Deserialize, Serialize};

use crate::DetachCode;

/// What the GUI sends the helm on a terminal socket as text.
///
/// There is no flow control here: the helm paces the supervisor from its own queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalClientMessage {
    /// The terminal's new size, in cells.
    Resize { cols: u16, rows: u16 },
    /// A liveness probe for this socket. The helm answers it itself with [`TerminalServerMessage::Pong`] and forwards
    /// it nowhere: the question is whether this socket still carries anything, which only this end can answer.
    Ping,
}

/// What the helm sends the GUI on a terminal socket as text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalServerMessage {
    /// The answer to [`TerminalClientMessage::Ping`].
    Pong,
    /// The attach's catch-up boundary: every binary message before it was replay, every one after it is live. It rides
    /// the same queue as the grid messages, so its position is exact. Presentation only; nothing decides session or
    /// lifecycle behavior from it.
    ReplayComplete,
    /// The attachment ended. `code` is what a client decides from; `reason` is only ever shown. Followed by a close.
    Detached {
        reason: String,
        /// Always present from this helm. Optional on the reading side so a notice from a helm that predates detach
        /// codes still parses (and counts as [`DetachCode::Other`] to whoever decides from it).
        #[serde(default)]
        code: Option<DetachCode>,
    },
    /// A message type this build does not know, from a newer helm. Ignored. Never sent: it does not serialize, so
    /// [`TerminalServerMessage::to_text`] refuses it rather than emitting a bogus `{"type":"unknown"}`.
    #[serde(other, skip_serializing)]
    Unknown,
}

impl TerminalClientMessage {
    /// This message as the JSON text of its control frame. Serializing these variants cannot fail (two numbers or
    /// nothing), so the empty string, were it ever returned, would be ignored by the helm as an unparseable message.
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

impl TerminalServerMessage {
    /// This message as the JSON text of its control frame. The sendable variants hold only strings and a unit-only
    /// code enum, so serializing them cannot fail; [`TerminalServerMessage::Unknown`] is not sendable and yields the
    /// empty string, which every client ignores as an unparseable control message.
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire format is the contract both ends already speak, so every message must serialize to, and parse back
    /// from, the exact JSON the helm and the GUI client exchanged before these types existed.
    #[farhelm_testtrace::test]
    fn every_message_keeps_its_wire_spelling() {
        let client = [
            (
                TerminalClientMessage::Resize {
                    cols: 101,
                    rows: 43,
                },
                r#"{"type":"resize","cols":101,"rows":43}"#,
            ),
            (TerminalClientMessage::Ping, r#"{"type":"ping"}"#),
        ];
        for (message, wire) in client {
            assert_eq!(serde_json::to_string(&message).unwrap(), wire);
            assert_eq!(
                serde_json::from_str::<TerminalClientMessage>(wire).unwrap(),
                message
            );
        }
        let server = [
            (TerminalServerMessage::Pong, r#"{"type":"pong"}"#),
            (
                TerminalServerMessage::ReplayComplete,
                r#"{"type":"replay_complete"}"#,
            ),
            (
                TerminalServerMessage::Detached {
                    reason: "another client attached".into(),
                    code: Some(DetachCode::TakenOver),
                },
                r#"{"type":"detached","reason":"another client attached","code":"taken_over"}"#,
            ),
        ];
        for (message, wire) in server {
            assert_eq!(serde_json::to_string(&message).unwrap(), wire);
            assert_eq!(
                serde_json::from_str::<TerminalServerMessage>(wire).unwrap(),
                message
            );
        }
    }

    /// An older client must keep working against a newer helm: an unknown notice parses as `Unknown` (and is ignored),
    /// and a detach notice from a helm that predates codes still parses, with no code.
    #[farhelm_testtrace::test]
    fn unknown_and_codeless_notices_still_parse() {
        assert_eq!(
            serde_json::from_str::<TerminalServerMessage>(r#"{"type":"someday","x":1}"#).unwrap(),
            TerminalServerMessage::Unknown
        );
        assert_eq!(
            serde_json::from_str::<TerminalServerMessage>(r#"{"type":"detached","reason":"gone"}"#)
                .unwrap(),
            TerminalServerMessage::Detached {
                reason: "gone".into(),
                code: None
            }
        );
    }
}
