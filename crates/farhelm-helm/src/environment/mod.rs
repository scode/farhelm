//! Disposable machines share one connection actor and one lifecycle owner.
//!
//! The supervisor inside an environment remains an ordinary supervisor. A new
//! provider supplies the machine operations around it through
//! [`EnvironmentProvider`], and joins the explicit slots in [`Providers`].
//! Provider decisions have a few named homes so adding a provider exposes each
//! choice to the compiler.
//!
//! - [`farhelm_proto::ProviderKind`] owns durable identity and static facts,
//!   including the policy for a status channel that could not answer.
//! - [`ProviderHandle`] owns the provider-specific binding. The store decodes
//!   it through [`HostRow::environment_binding`], preserving existing columns.
//! - [`EnvironmentProvider`] owns I/O and the interpretation of provider
//!   observations. Tensorlake's adapter lives in [`crate::tensorlake::hosts`]
//!   and its bounded lifecycle polling in [`crate::tensorlake::lifecycle`].
//! - [`lifecycle`] dispatches explicit operations. Tensorlake retains its landed
//!   key-before-hold ordering in its lifecycle adapter. Whole-resource removal uses
//!   the same provider dispatch and the row-specific [`Destroyed`] proof.
//! - [`crate::manager`] owns the pure status gate and connection lifetime.
//!   It never translates vendor states or decides policy by provider identity.
//!   The connected hook's leases span acquisition through serving and end
//!   before a connection hold acknowledges parking.
//!
//! The initial adapter covers the operations existing actors and lifecycle
//! callers consume. Places, prepared selection, create and owned-resource
//! listing join this same trait when their shared recipe/storage consumers
//! arrive. Prepared selection must retain the build engine's source-use guard
//! through create's first Running observation, as current Tensorlake creation
//! does. Account settings and the underlying build engines stay provider-owned.

pub(crate) mod lifecycle;

use crate::{
    client::SupervisorClient,
    feed::FleetEvents,
    store::{HostId, HostRow, TensorlakeBinding},
};
use farhelm_proto::ProviderKind;
use std::sync::Arc;

/// The resource coordinates recorded by this helm, apart from install identity.
/// Handles retain each provider's ownership evidence, including an unanswered
/// create. A missing name lookup alone cannot erase that evidence.
#[derive(Clone, Debug)]
pub(crate) struct EnvironmentBinding {
    pub(crate) host: HostId,
    pub(crate) handle: ProviderHandle,
}

impl EnvironmentBinding {
    /// The handle is the single authority for provider identity.
    pub(crate) fn provider(&self) -> ProviderKind {
        self.handle.provider()
    }
}

/// Provider-owned coordinates, decoded from the current host representation.
/// Podman's handle joins this enum with the storage change that can construct it.
#[derive(Clone, Debug)]
pub(crate) enum ProviderHandle {
    Tensorlake(TensorlakeBinding),
}

#[warn(clippy::wildcard_enum_match_arm)]
impl ProviderHandle {
    /// Every new handle must name its provider explicitly.
    pub(crate) fn provider(&self) -> ProviderKind {
        match self {
            Self::Tensorlake(_) => ProviderKind::Tensorlake,
        }
    }

    /// Copied-state admission must finish before a resource's first dial.
    /// This is the existing durable install-coordinate guard's remedy.
    pub(crate) fn initial_setup_refusal(&self) -> &'static str {
        match self {
            Self::Tensorlake(_) => {
                "initial Tensorlake setup has not passed the supervisor state check; run setup again to continue"
            }
        }
    }
}

/// An answer from the provider, including states this build cannot use.
/// Failed reads use [`StatusUnanswered`] so unknown answers cannot acquire the
/// dial-anyway policy reserved for an independent status channel's failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentStatus {
    Running,
    /// The provider is changing state. The provider supplies the retry wording.
    Changing(String),
    /// Suspended resources require explicit Resume. The provider retains any
    /// diagnosis of a failed resume, including its fallback when none was given.
    Suspended {
        resume_failed: Option<String>,
    },
    /// Definitively missing, with the provider's deletion remedy.
    Missing(String),
    /// The provider answered with a state this build cannot safely use.
    Unknown(String),
}

/// No status answer was obtained. The complete provider-written sentence is
/// suitable as a refusal or as context after a failed independent dial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StatusUnanswered(pub(crate) String);

/// The I/O seam used by actors, lifecycle operations and confirmed removal.
/// Scripted implementations can control observations without changing the
/// static policy selected by the binding's provider.
#[async_trait::async_trait]
pub(crate) trait EnvironmentProvider: Send + Sync {
    /// Read status once per attempt, inside the actor's bounded attempt budget.
    /// `Ok(Unknown)` is an answer and must refuse dialing. `Err` means the
    /// provider could not answer and invokes its static unanswered-status policy.
    async fn status(
        &self,
        binding: &EnvironmentBinding,
    ) -> Result<EnvironmentStatus, StatusUnanswered>;

    /// Confirm suspension under the ordinary provisioning lock and actor hold.
    /// The existing context keeps validation ahead of connection withdrawal.
    async fn suspend(
        &self,
        state: &crate::AppState,
        binding: &EnvironmentBinding,
    ) -> anyhow::Result<()>;

    /// Confirm running and refresh a connection after a real resume. Repeated
    /// Resume of a running, connected resource preserves its existing viewers.
    async fn resume(
        &self,
        state: &crate::AppState,
        binding: &EnvironmentBinding,
    ) -> anyhow::Result<()>;

    /// Confirm destruction of recorded resources and provider leftovers before
    /// permitting the host row to be forgotten. Failure retains ownership.
    async fn destroy(&self, binding: &EnvironmentBinding) -> anyhow::Result<Destroyed>;

    /// Acquire resources for this exact connection before it becomes routable.
    /// Cancellation must drop partially acquired leases. Their lifetime covers
    /// the hook-to-Connected publication gap and ends before a hold parks.
    async fn connected(
        &self,
        _row: &HostRow,
        _client: Arc<SupervisorClient>,
        _events: Arc<FleetEvents>,
    ) -> ConnectedResources {
        ConnectedResources::default()
    }
}

/// Connection-scoped credentials and recipe relays. Dropping a tunnel lease
/// also retires its claiming client, even when another owner keeps an Arc.
#[derive(Default)]
pub(crate) struct ConnectedResources {
    pub(crate) _tunnels: Option<crate::tensorlake::tunnels::TunnelLease>,
    pub(crate) _credentials: Option<crate::provisioning::credential_delivery::ConnectionLease>,
}

/// Explicit provider slots keep dispatch exhaustive. Empty slots are useful
/// for ordinary-host managers and for staged provider assembly. They report an
/// unanswered read instead of manufacturing resource state or successful work.
#[derive(Clone, Default)]
pub(crate) struct Providers {
    tensorlake: Option<Arc<dyn EnvironmentProvider>>,
    podman: Option<Arc<dyn EnvironmentProvider>>,
}

impl Providers {
    /// Install the current concrete provider before any host actor starts.
    pub(crate) fn with_tensorlake(tensorlake: Arc<dyn EnvironmentProvider>) -> Self {
        Self {
            tensorlake: Some(tensorlake),
            podman: None,
        }
    }

    /// Resolve every provider explicitly. Absence follows the same status
    /// policy as another unanswered read, while mutation callers return it.
    pub(crate) fn get(
        &self,
        kind: ProviderKind,
    ) -> Result<&dyn EnvironmentProvider, StatusUnanswered> {
        match kind {
            ProviderKind::Tensorlake => self.tensorlake.as_deref().ok_or_else(|| {
                StatusUnanswered("Farhelm could not ask Tensorlake whether the sandbox is running: this helm has no Tensorlake account to ask".into())
            }),
            ProviderKind::Podman => self.podman.as_deref().ok_or_else(|| {
                StatusUnanswered("Farhelm could not ask Podman whether the container is running: this helm has no Podman provider".into())
            }),
        }
    }
}

/// Provider-owned evidence permits forgetting exactly one resource row.
/// Each provider keeps its successful-cleanup constructor private. Wrapping its
/// opaque proof preserves that boundary when callers use the shared I/O seam.
#[derive(Debug)]
pub(crate) enum Destroyed {
    Tensorlake(crate::tensorlake::hosts::SandboxDeleted),
}

impl Destroyed {
    /// The store rejects a proof issued for another registry row.
    pub(crate) fn host(&self) -> HostId {
        match self {
            Self::Tensorlake(proof) => proof.host(),
        }
    }
}
