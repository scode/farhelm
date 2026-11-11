//! Provider identity and static connection policy shared by the helm and UI.
//!
//! Observations belong to the provider implementation. Decisions that hold for
//! every observation of one provider belong here, with exhaustive matches so a
//! new provider must choose its behavior before it compiles.

use serde::{Deserialize, Serialize};

/// A provider of disposable machines running an ordinary Farhelm supervisor.
/// The spelling is durable vocabulary for environment bindings and UI choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Tensorlake,
    Podman,
}

/// Whether a failed status read leaves the connection path worth trying.
/// This applies only when the provider could not answer. A successful answer
/// with an unknown state always withholds the dial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhenStatusUnanswered {
    /// Status uses an independent path. Try the connection, retaining the
    /// provider's explanation if that connection also fails.
    DialAnyway,
    /// Status and connection share a path. Let the normal retry ladder retry
    /// the status read before attempting another connection.
    RetryLater,
}

#[warn(clippy::wildcard_enum_match_arm)]
impl ProviderKind {
    /// An unavailable status channel must not take running Tensorlake hosts
    /// offline. Podman's status and exec both depend on the place-host route.
    pub fn when_status_unanswered(self) -> WhenStatusUnanswered {
        match self {
            Self::Tensorlake => WhenStatusUnanswered::DialAnyway,
            Self::Podman => WhenStatusUnanswered::RetryLater,
        }
    }
}
