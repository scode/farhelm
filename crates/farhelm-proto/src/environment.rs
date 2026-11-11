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

/// The durable location of a provider's artifacts and environments.
/// Host IDs use the helm registry's i64 representation. Keeping the provider
/// inside the place prevents a target from pairing a host with another provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "host", rename_all = "snake_case")]
pub enum Place {
    #[serde(rename = "tensorlake_account")]
    TensorlakeAccount,
    #[serde(rename = "podman_host")]
    PodmanHost(i64),
}

#[warn(clippy::wildcard_enum_match_arm)]
impl Place {
    /// Accepted container launches update their last-used place independently
    /// of whether they carry a reusable structured agent selection.
    pub fn container_host(self) -> Option<i64> {
        match self {
            Self::TensorlakeAccount => None,
            Self::PodmanHost(host) => Some(host),
        }
    }
    /// Provider identity follows the place, including when its host is absent.
    /// Resolving or validating the current host remains the helm's responsibility.
    pub fn provider(self) -> ProviderKind {
        match self {
            Self::TensorlakeAccount => ProviderKind::Tensorlake,
            Self::PodmanHost(_) => ProviderKind::Podman,
        }
    }
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

/// Captured fresh-machine intent, independent of the installation it creates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LaunchTarget {
    Environment { recipe: String, place: Place },
}

impl LaunchTarget {
    /// The selected catalog name remains explicit even when it was removed.
    pub fn recipe(&self) -> &str {
        match self {
            Self::Environment { recipe, .. } => recipe,
        }
    }

    /// The selected place outlives the disposable environment created there.
    pub fn place(&self) -> Place {
        match self {
            Self::Environment { place, .. } => *place,
        }
    }

    /// Provider selection follows the captured place.
    pub fn provider(&self) -> ProviderKind {
        self.place().provider()
    }
}
