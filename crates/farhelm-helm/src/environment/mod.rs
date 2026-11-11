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
//! - [`recipes::Recipe`] is the shared catalog definition. Its wire base decides
//!   supported providers exhaustively. Tensorlake keeps its existing snapshot
//!   state, with Podman artifacts keyed by place host in the same state_json.
//!   Shared state methods and the build registry's place projection decide
//!   resource presence and freshness exhaustively over [`Place`]. These are
//!   local facts; a projection never probes every place or exposes install paths.
//! - [`EnvironmentProvider`] owns I/O and the interpretation of provider
//!   observations. Tensorlake's adapter lives in [`crate::tensorlake::hosts`]
//!   and its bounded lifecycle polling in [`crate::tensorlake::lifecycle`].
//!   [`podman`] owns rootless commands, image jobs and its Containers HTTP
//!   adapter; the concrete HTTP handle is the same instance in the registry.
//! - [`crate::tensorlake::build::RecipeBuilds`] admits or joins both providers'
//!   work. Its recipe-wide deletion guard closes admission before providers
//!   cancel/wait and clean resources; state remains until cleanup is confirmed.
//!   Prepared artifacts carry source exclusion through create's first Running.
//! - [`EnvironmentPreparer`] is the one route/home installer, implemented in
//!   [`crate::provisioning::snapshot_preparer`]. Tensorlake's wrapper supplies
//!   its gateway access and logind setup; Podman supplies observed image login.
//!   Both use the ordinary payloads, installer actions and checkpoint guard.
//! - [`lifecycle`] dispatches explicit operations. Tensorlake retains its landed
//!   key-before-hold ordering in its lifecycle adapter. Whole-resource removal uses
//!   the same provider dispatch and the row-specific [`Destroyed`] proof.
//! - [`crate::manager`] owns the pure status gate and connection lifetime.
//!   It never translates vendor states or decides policy by provider identity.
//!   A bare Stopped answer authorizes one actor-owned start episode, gated by
//!   its binding's current place connection. Status itself never starts work.
//!   The connected hook's leases span acquisition through serving and end
//!   before a connection hold acknowledges parking.
//! - [`crate::access`] owns the common execution route and command lowering
//!   used by supervisor transport, provisioning and credential delivery.
//!   Registry access is resolved afresh for each operation, so a place host's
//!   editable destination cannot become a stale container route.
//!
//! [`crate::provisioning::service::ProvisioningService`] owns the shared launch
//! task: pending row, progress, private artifact, durable admission, credentials,
//! attachment and ordinary session creation. ProviderHandle's exhaustive
//! registration/admission methods preserve each provider's durable facts.
//! Public wording and marks remain the UI's exhaustive provider home.

pub(crate) mod lifecycle;
pub(crate) mod podman;
pub(crate) mod podman_probe;
pub(crate) mod recipes;

use crate::{
    client::SupervisorClient,
    feed::FleetEvents,
    store::{HostId, HostRow, TensorlakeBinding},
};
use farhelm_proto::{Place, ProviderKind};
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
/// A Podman handle keeps its place ID, so later access resolves the place's
/// current endpoint rather than retaining the destination observed at create.
#[derive(Clone, Debug)]
pub(crate) enum ProviderHandle {
    Tensorlake(TensorlakeBinding),
    Podman(crate::store::PodmanBinding),
}

#[warn(clippy::wildcard_enum_match_arm)]
impl ProviderHandle {
    /// Credentials and connected relays share the current catalog selection.
    /// This name does not select a newer image for create recovery.
    pub(crate) fn recipe(&self) -> &str {
        match self {
            Self::Tensorlake(binding) => &binding.recipe,
            Self::Podman(binding) => &binding.recipe,
        }
    }

    /// A bounded display name and reserved lookup coordinate, never authority
    /// to delete a resource without the provider's ownership confirmation.
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Tensorlake(binding) => &binding.sandbox,
            Self::Podman(binding) => &binding.name,
        }
    }

    /// Register the launch's cleanup owner before preparation or creation.
    /// Host-backed images have no observed account yet; their provider records
    /// it atomically with the selected install facts before sending run.
    pub(crate) async fn register_session(
        store: &crate::store::HelmStore,
        name: String,
        recipe: String,
        place: Place,
        size: farhelm_proto::recipes::RecipeSize,
    ) -> anyhow::Result<HostId> {
        match place {
            Place::TensorlakeAccount => {
                store
                    .register_session_sandbox(crate::store::NewTensorlakeHost {
                        destination: crate::tensorlake::ssh::destination(&name),
                        entry: Default::default(),
                        sandbox: name,
                        recipe,
                        resources: size,
                    })
                    .await
            }
            Place::PodmanHost(host) => {
                store
                    .register_pending_session_container(name, recipe, host)
                    .await
            }
        }
    }

    /// Recovery must admit the installation selected by this resource's own
    /// create, even when the catalog has since published another image.
    pub(crate) async fn prepared_install(
        &self,
        store: &crate::store::HelmStore,
        host: HostId,
    ) -> anyhow::Result<Option<recipes::PreparedInstallFacts>> {
        match self {
            Self::Tensorlake(_) => store.tensorlake_prepared_install(host).await,
            Self::Podman(_) => store.podman_prepared_install(host).await,
        }
    }

    /// The setup owner calls this only after validating its saved prepared
    /// facts and the copied-state boundary. Supervisor identity still comes
    /// from hello; these paths only open the first-dial admission gate.
    pub(crate) async fn admit_install(
        &self,
        store: &crate::store::HelmStore,
        row: &HostRow,
        farhelm: &str,
        state_dir: &str,
    ) -> anyhow::Result<bool> {
        match self {
            Self::Tensorlake(_) => {
                store
                    .admit_tensorlake_install(row.id, farhelm, state_dir)
                    .await
            }
            Self::Podman(_) => {
                store
                    .admit_podman_install(
                        crate::store::PodmanInstallGuard::of(row)?,
                        farhelm,
                        state_dir,
                    )
                    .await
            }
        }
    }

    /// Rollback of a registration before any provider work owns no resource.
    /// This explicit exception must never be used after create was attempted.
    pub(crate) fn never_created(&self, host: HostId) -> Destroyed {
        match self {
            Self::Tensorlake(_) => Destroyed::Tensorlake(
                crate::tensorlake::hosts::SandboxDeleted::never_created(host),
            ),
            Self::Podman(_) => Destroyed::Podman(podman::ContainerDeleted::never_created(host)),
        }
    }

    /// Only providers with a host-backed place wait for another actor before
    /// an automatic stopped-resource start. Account providers have no gate.
    pub(crate) fn place_host(&self) -> Option<HostId> {
        match self {
            Self::Tensorlake(_) => None,
            Self::Podman(binding) => Some(binding.place_host),
        }
    }

    /// Suspend attribution is presentation metadata, never a dial authority.
    pub(crate) fn suspended_at(&self) -> Option<i64> {
        match self {
            Self::Tensorlake(binding) => binding.suspended_at,
            Self::Podman(_) => None,
        }
    }

    /// Provisioning cannot wake suspended processes as a side effect.
    /// Keep the remedy with the binding: Tensorlake users resume a sandbox,
    /// while a paused container needs unpausing before exec can answer.
    pub(crate) fn suspended_provisioning_refusal(&self) -> &'static str {
        match self {
            Self::Tensorlake(_) => {
                "this host's Tensorlake sandbox is suspended, so Farhelm cannot reach it; resume the sandbox first, then try again"
            }
            Self::Podman(_) => {
                "this host's container is paused, so Farhelm cannot reach it; resume the container first, then try again"
            }
        }
    }

    /// Every new handle must name its provider explicitly.
    pub(crate) fn provider(&self) -> ProviderKind {
        match self {
            Self::Tensorlake(_) => ProviderKind::Tensorlake,
            Self::Podman(_) => ProviderKind::Podman,
        }
    }

    /// Copied-state admission must finish before a resource's first dial.
    /// This is the existing durable install-coordinate guard's remedy.
    pub(crate) fn initial_setup_refusal(&self) -> &'static str {
        match self {
            Self::Tensorlake(_) => {
                "initial Tensorlake setup has not passed the supervisor state check; run setup again to continue"
            }
            Self::Podman(_) => "initial container setup has not passed the supervisor state check",
        }
    }
}

/// An answer from the provider, including states this build cannot use.
/// Failed reads use [`StatusUnanswered`] so unknown answers cannot acquire the
/// dial-anyway policy reserved for an independent status channel's failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentStatus {
    Running,
    /// Authorize one automatic start while the place actor is connected.
    /// The actor owns the attempt/failure episode; status is observation only.
    Stopped,
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

/// Cached place availability. Reading this never fans out network probes.
pub(crate) struct EnvironmentPlace {
    pub(crate) place: Place,
    pub(crate) reason: Option<String>,
}

/// Provider-owned selection retains source exclusion until create first
/// observes Running. Private inner fields prevent fabricating that guard.
pub(crate) enum PreparedArtifact {
    Tensorlake(crate::tensorlake::hosts::PreparedSandbox),
    Podman(podman::PreparedContainer),
}

/// Progress sinks reveal preparation only when work is actually required.
/// Named text is bounded by the provider before reaching the operation view.
#[async_trait::async_trait]
pub(crate) trait PreparationProgress: Send + Sync {
    async fn needed(&self);
    async fn step(&self, text: String);
}

/// The single ordinary installer used by both providers' checkpoint work.
/// Providers establish the machine's login manager/home before calling it;
/// success verifies stopped enablement, whole-state absence and machine-id.
#[async_trait::async_trait]
pub(crate) trait EnvironmentPreparer: Send + Sync {
    async fn prepare(
        &self,
        route: &crate::access::RemoteRoute,
        home: std::path::PathBuf,
    ) -> anyhow::Result<recipes::PreparedInstallFacts>;
}

/// Creation may recover an existing resource. Its install coordinates are
/// read from the saved create record, never from the currently selected image.
pub(crate) struct CreatedEnvironment {
    pub(crate) created: bool,
}

/// Provider discovery retains canonical identity separately from its name.
pub(crate) struct OwnedEnvironment {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) place: Place,
    pub(crate) role: String,
}

/// The I/O seam used by actors, lifecycle operations and confirmed removal.
/// Scripted implementations can control observations without changing the
/// static policy selected by the binding's provider.
#[async_trait::async_trait]
pub(crate) trait EnvironmentProvider: Send + Sync {
    /// Admit/join explicit Settings work, then answer while the shared
    /// registry/feed retains progress. Launch preparation never calls this.
    async fn rebuild_recipe(
        &self,
        name: &str,
        place: Place,
        events: Arc<FleetEvents>,
    ) -> anyhow::Result<()>;

    /// Cleanup follows shared recipe-wide deletion admission. Each provider
    /// owns enumeration, cancellation and confirmed cleanup at all its places.
    async fn clean_recipe(
        &self,
        name: &str,
        deletion: &crate::tensorlake::build::DeleteGuard,
    ) -> anyhow::Result<()>;
    /// Read cached place availability and local account state only. Opening a
    /// launcher must not synchronously fan out probes to every ordinary host.
    async fn places(&self) -> anyhow::Result<Vec<EnvironmentPlace>>;

    /// Session launches may prepare a built Tensorlake source but may never
    /// start a billed software build. Podman builds lazily at its place.
    async fn ensure_prepared(
        &self,
        name: &str,
        recipe: &recipes::Recipe,
        place: Place,
        events: Arc<FleetEvents>,
        progress: &dyn PreparationProgress,
    ) -> anyhow::Result<PreparedArtifact>;

    /// A missing artifact permits recorded-create recovery only. The row
    /// and its reserved name exist before any create request is sent.
    async fn create(
        &self,
        binding: &EnvironmentBinding,
        prepared: Option<PreparedArtifact>,
        progress: &dyn PreparationProgress,
    ) -> anyhow::Result<CreatedEnvironment>;

    /// Confirm this helm's ownership marks at the selected place. Incomplete
    /// session rows must remain distinguishable from actual hosted sessions.
    async fn list_owned(&self, place: Place) -> anyhow::Result<Vec<OwnedEnvironment>>;
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
        manager: &crate::manager::ConnectionManager,
        binding: &EnvironmentBinding,
    ) -> anyhow::Result<()>;

    /// Confirm running and refresh a connection after a real resume. Repeated
    /// Resume of a running, connected resource preserves its existing viewers.
    async fn resume(
        &self,
        manager: &crate::manager::ConnectionManager,
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
        row: &HostRow,
        client: Arc<SupervisorClient>,
        events: Arc<FleetEvents>,
    ) -> ConnectedResources;
}

/// Connection-scoped credentials and recipe relays. Dropping a tunnel lease
/// also retires its claiming client, even when another owner keeps an Arc.
#[derive(Default)]
pub(crate) struct ConnectedResources {
    pub(crate) _tunnels: Option<crate::tensorlake::tunnels::TunnelLease>,
    pub(crate) _credentials: Option<crate::provisioning::credential_delivery::ConnectionLease>,
}

/// Explicit provider slots keep dispatch exhaustive. Production installs both
/// real services once; isolated ordinary-host tests may omit unavailable I/O.
/// Missing slots report an unanswered read rather than successful provider work.
#[derive(Clone, Default)]
pub(crate) struct Providers {
    tensorlake: Option<Arc<dyn EnvironmentProvider>>,
    podman: Option<Arc<dyn EnvironmentProvider>>,
    #[cfg(test)]
    tensorlake_test: Option<Arc<TestProvider>>,
}

impl Providers {
    /// Try every installed provider under the one deletion guard; a failure
    /// retains its resources while independent places can finish cleanup.
    pub(crate) async fn clean_recipe(
        &self,
        name: &str,
        deletion: &crate::tensorlake::build::DeleteGuard,
    ) -> anyhow::Result<()> {
        #[cfg(not(test))]
        let Self { tensorlake, podman } = self;
        #[cfg(test)]
        let Self {
            tensorlake,
            podman,
            tensorlake_test: _,
        } = self;
        let mut first = None;
        for provider in [tensorlake, podman].into_iter().flatten() {
            if let Err(error) = provider.clean_recipe(name, deletion).await {
                first.get_or_insert(error);
            }
        }
        first.map_or(Ok(()), Err)
    }
    /// Aggregate cached availability only. Explicit field destructuring makes
    /// a new slot require a decision here as well as in ordinary dispatch.
    pub(crate) async fn places(&self) -> anyhow::Result<Vec<EnvironmentPlace>> {
        #[cfg(not(test))]
        let Self { tensorlake, podman } = self;
        #[cfg(test)]
        let Self {
            tensorlake,
            podman,
            tensorlake_test: _,
        } = self;
        let mut places = Vec::new();
        for provider in [tensorlake, podman].into_iter().flatten() {
            places.extend(provider.places().await?);
        }
        Ok(places)
    }
    /// Production installs both real services once before actors start.
    pub(crate) fn new(
        tensorlake: Arc<dyn EnvironmentProvider>,
        podman: Arc<dyn EnvironmentProvider>,
    ) -> Self {
        Self {
            tensorlake: Some(tensorlake),
            podman: Some(podman),
            #[cfg(test)]
            tensorlake_test: None,
        }
    }
    /// Install the current concrete provider before any host actor starts.
    #[cfg(test)]
    pub(crate) fn with_tensorlake(tensorlake: Arc<dyn EnvironmentProvider>) -> Self {
        Self::test_tensorlake(None, tensorlake)
    }

    /// Fixtures keep scripted actor observations while the real provider
    /// owns lifecycle/create/cleanup. Replacing an injected service updates
    /// that same registry adapter, never a second AppState registry.
    #[cfg(test)]
    pub(crate) fn with_scripted_tensorlake(
        observations: Arc<dyn EnvironmentProvider>,
        actual: Arc<dyn EnvironmentProvider>,
    ) -> Self {
        Self::test_tensorlake(Some(observations), actual)
    }

    #[cfg(test)]
    fn test_tensorlake(
        observations: Option<Arc<dyn EnvironmentProvider>>,
        actual: Arc<dyn EnvironmentProvider>,
    ) -> Self {
        let adapter = Arc::new(TestProvider {
            observations,
            actual: std::sync::Mutex::new(actual),
        });
        Self {
            tensorlake: Some(adapter.clone()),
            podman: None,
            tensorlake_test: Some(adapter),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_test_tensorlake(&self, actual: Arc<dyn EnvironmentProvider>) {
        let adapter = self
            .tensorlake_test
            .as_ref()
            .expect("fixture installed its provider adapter before starting actors");
        *adapter.actual.lock().unwrap() = actual;
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

/// Only test fixtures can replace service seams after their manager starts.
/// Scripted observations remain independent of real lifecycle ownership.
#[cfg(test)]
struct TestProvider {
    observations: Option<Arc<dyn EnvironmentProvider>>,
    actual: std::sync::Mutex<Arc<dyn EnvironmentProvider>>,
}

#[cfg(test)]
impl TestProvider {
    fn actual(&self) -> Arc<dyn EnvironmentProvider> {
        self.actual.lock().unwrap().clone()
    }
    fn observations(&self) -> Arc<dyn EnvironmentProvider> {
        self.observations.clone().unwrap_or_else(|| self.actual())
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl EnvironmentProvider for TestProvider {
    async fn places(&self) -> anyhow::Result<Vec<EnvironmentPlace>> {
        self.actual().places().await
    }
    async fn rebuild_recipe(
        &self,
        name: &str,
        place: Place,
        events: Arc<FleetEvents>,
    ) -> anyhow::Result<()> {
        self.actual().rebuild_recipe(name, place, events).await
    }
    async fn clean_recipe(
        &self,
        name: &str,
        deletion: &crate::tensorlake::build::DeleteGuard,
    ) -> anyhow::Result<()> {
        self.actual().clean_recipe(name, deletion).await
    }
    async fn ensure_prepared(
        &self,
        name: &str,
        recipe: &recipes::Recipe,
        place: Place,
        events: Arc<FleetEvents>,
        progress: &dyn PreparationProgress,
    ) -> anyhow::Result<PreparedArtifact> {
        self.actual()
            .ensure_prepared(name, recipe, place, events, progress)
            .await
    }
    async fn create(
        &self,
        binding: &EnvironmentBinding,
        artifact: Option<PreparedArtifact>,
        progress: &dyn PreparationProgress,
    ) -> anyhow::Result<CreatedEnvironment> {
        self.actual().create(binding, artifact, progress).await
    }
    async fn list_owned(&self, place: Place) -> anyhow::Result<Vec<OwnedEnvironment>> {
        self.actual().list_owned(place).await
    }
    async fn status(
        &self,
        binding: &EnvironmentBinding,
    ) -> Result<EnvironmentStatus, StatusUnanswered> {
        self.observations().status(binding).await
    }
    async fn suspend(
        &self,
        manager: &crate::manager::ConnectionManager,
        binding: &EnvironmentBinding,
    ) -> anyhow::Result<()> {
        self.actual().suspend(manager, binding).await
    }
    async fn resume(
        &self,
        manager: &crate::manager::ConnectionManager,
        binding: &EnvironmentBinding,
    ) -> anyhow::Result<()> {
        self.actual().resume(manager, binding).await
    }
    async fn destroy(&self, binding: &EnvironmentBinding) -> anyhow::Result<Destroyed> {
        self.actual().destroy(binding).await
    }
    async fn connected(
        &self,
        row: &HostRow,
        client: Arc<SupervisorClient>,
        events: Arc<FleetEvents>,
    ) -> ConnectedResources {
        self.observations().connected(row, client, events).await
    }
}

/// Provider-owned evidence permits forgetting exactly one resource row.
/// Each provider keeps its successful-cleanup constructor private. Wrapping its
/// opaque proof preserves that boundary when callers use the shared I/O seam.
#[derive(Debug)]
pub(crate) enum Destroyed {
    Tensorlake(crate::tensorlake::hosts::SandboxDeleted),
    Podman(podman::ContainerDeleted),
}

impl Destroyed {
    /// The store rejects a proof issued for another registry row.
    pub(crate) fn host(&self) -> HostId {
        match self {
            Self::Tensorlake(proof) => proof.host(),
            Self::Podman(proof) => proof.host(),
        }
    }
}
