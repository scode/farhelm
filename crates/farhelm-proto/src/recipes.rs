//! Shared recipes describe disposable environments across providers.
//!
//! A recipe selects its software base, packages, agent authentication and
//! outbound tunnels. Its size applies to Tensorlake alone. A base declares
//! which providers can build it, while each provider owns its artifacts.
//! Recipes belong to the helm, with one catalog for every client, and are
//! stored by name. The name never changes
//! after creation because hosts refer to their recipe by it.
//!
//! The types here are the HTTP shapes the helm and the UI share, and the
//! validation both run: the UI checks a draft before sending it so the user
//! sees a refusal without a round trip, and the helm checks again because it
//! is the authority. The checks are deliberately narrower than "whatever
//! Tensorlake accepts" where a value later reaches a command line or a
//! sandbox name: package words, image names and tunnel hosts end up in shell
//! scripts and ssh arguments, and a recipe name ends up verbatim inside the
//! snapshot builder's sandbox name (`farhelm-build-<helm tag>-<recipe>`).
//!
//! Endpoints refer to the write-only credential catalog by name. Neither
//! reads nor saves contain token values; changing the endpoint server does
//! not silently change which named credential the recipe selects.
//!
//! The read shape also reports a recipe's snapshot build: the built
//! snapshot ([`SnapshotView`]) and whether a build runs or how the last one
//! ended ([`BuildView`]). The helm owns that state; no save carries it.
//!
//! What this module does NOT cover: whether Tensorlake accepts a size (the
//! plan's limits are not readable through the API, so Tensorlake's own
//! refusal decides when a sandbox is created).

use serde::{Deserialize, Serialize};

use crate::ProviderKind;

/// The longest recipe name, in bytes.
///
/// Sized so the snapshot builder's sandbox name, `farhelm-build-` (14) plus a
/// 4-character helm tag, a hyphen and the recipe name, stays within
/// Tensorlake's 63-byte name limit with room to spare.
pub const RECIPE_NAME_CAP: usize = 32;

/// The most packages one recipe may install.
pub const PACKAGES_CAP: usize = 500;

/// The longest single package word (name, optional `:arch`, optional
/// `=version`), in bytes.
pub const PACKAGE_WORD_CAP: usize = 128;

/// The most outbound tunnels one recipe may declare.
pub const TUNNELS_CAP: usize = 32;

/// The longest endpoint base URL, in bytes (ntfy's server URL cap).
pub const BASE_URL_CAP: usize = 2048;

/// The longest endpoint token, in bytes (ntfy's token cap).
pub const TOKEN_CAP: usize = 4096;

/// Tensorlake's sandbox size. Whole numbers only: Tensorlake sizes memory and disk in
/// MiB, and Farhelm multiplies these by 1024 when it creates a sandbox.
///
/// The caps in [`check_recipe`] are loose sanity bounds, not plan limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RecipeSize {
    pub cpus: u32,
    pub memory_gb: u32,
    pub disk_gb: u32,
}

/// The starting filesystem Farhelm builds on for every recipe.
///
/// Packages and the official Claude Code and Codex installers run for every
/// base. An image is a named pointer and a snapshot has an ID, so those choices
/// stay distinct rather than guessing from the supplied text. The default also
/// identifies snapshots built before recipes had an explicit base.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecipeBase {
    /// The provider's managed Ubuntu base, shared by Tensorlake and Podman.
    /// Preserve the historical spelling so existing recipes and snapshots load.
    #[default]
    #[serde(rename = "tensorlake_ubuntu")]
    ManagedUbuntu,
    /// A Tensorlake image name.
    #[serde(rename = "image")]
    TensorlakeImage { image: String },
    /// A Tensorlake snapshot ID.
    #[serde(rename = "snapshot")]
    TensorlakeSnapshot { snapshot_id: String },
    /// A Podman image and the login account used inside it. Providers retain
    /// their own bootstrap and readiness checks for user-supplied filesystems.
    #[serde(rename = "container_image")]
    ContainerImage { reference: String, user: String },
}

#[warn(clippy::wildcard_enum_match_arm)]
impl RecipeBase {
    /// Compatibility is a static base/provider fact. Every added provider and
    /// base must make this choice explicitly before launch or build can use it.
    pub fn supports(&self, provider: ProviderKind) -> bool {
        match provider {
            ProviderKind::Tensorlake => match self {
                Self::ManagedUbuntu
                | Self::TensorlakeImage { .. }
                | Self::TensorlakeSnapshot { .. } => true,
                Self::ContainerImage { .. } => false,
            },
            ProviderKind::Podman => match self {
                Self::ManagedUbuntu | Self::ContainerImage { .. } => true,
                Self::TensorlakeImage { .. } | Self::TensorlakeSnapshot { .. } => false,
            },
        }
    }
}

/// One outbound tunnel: programs in the sandbox reach `127.0.0.1:<sandbox_port>`
/// and Farhelm carries the connection to `<host>:<port>` as seen from the
/// helm's machine.
///
/// `host` is a DNS name, an IPv4 literal or a bracketed IPv6 literal, which is
/// exactly the shape an ssh `-R` forward spec can carry without quoting.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Tunnel {
    pub sandbox_port: u16,
    pub host: String,
    pub port: u16,
}

/// An agent endpoint names a credential without exposing its saved value.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct EndpointView {
    pub base_url: String,
    pub token_credential: Option<String>,
}

/// An agent endpoint as a save sends it.
///
/// `token_credential` is a complete selection: None uses no token. The helm
/// validates the referenced entry's existence and endpoint-token kind in the
/// same transaction as the recipe write, so deletion cannot race admission.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointSave {
    pub base_url: String,
    pub token_credential: Option<String>,
}

/// Everything a recipe save sets apart from the name.
///
/// Each agent selects its endpoint or its named subscription login, never
/// both. Neither means explicit None: Claude clears its three managed env
/// keys, while Codex clears only Farhelm's provider selection/token and keeps
/// its auth.json. Values belong to Settings → Credentials; recipes carry names.
///
/// Unknown fields are refused at every level, nested size, base, tunnel and
/// endpoint included: a field a newer client sets and this helm would silently
/// drop is worse than a clear refusal. `deny_unknown_fields` does not reach
/// into nested types, and [`RecipeSize`], [`RecipeBase`] and [`Tunnel`] stay
/// tolerant because reads share them, so decoding goes through the strict
/// save mirror `strict::RecipeSave`.
#[derive(Clone, Deserialize, Serialize)]
#[serde(from = "strict::RecipeSave")]
pub struct RecipeSave {
    pub size: RecipeSize,
    pub base: RecipeBase,
    /// Optional apt package words for any base; saves remove exact duplicates.
    pub packages: Vec<String>,
    pub claude: Option<EndpointSave>,
    pub codex: Option<EndpointSave>,
    /// Claude's None mode clears its three managed authentication keys;
    /// Subscription selects a saved setup-token without changing onboarding.
    pub claude_login_credential: Option<String>,
    /// Subscription and API endpoint selection are mutually exclusive. Both
    /// absent asks Farhelm to clear only its Codex provider selection/token;
    /// the host's auth.json remains Codex's after a successful delivery.
    pub codex_login_credential: Option<String>,
    /// A catalog identity, never the token. None removes Farhelm's managed
    /// token file at the next setup/connection without clearing user gh auth.
    pub github_token_credential: Option<String>,
    pub tunnels: Vec<Tunnel>,
}

/// Write-only decoding shapes for [`RecipeSave`]: the shared types' fields,
/// with unknown fields refused at each level. They exist only to be converted
/// into the shared types, so a field added to one of those must be added here
/// too (the conversion below stops compiling otherwise, since it names every
/// field).
mod strict {
    use serde::Deserialize;

    /// [`super::RecipeSave`]'s strict decoding.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct RecipeSave {
        size: RecipeSize,
        base: RecipeBase,
        packages: Vec<String>,
        claude: Option<super::EndpointSave>,
        codex: Option<super::EndpointSave>,
        claude_login_credential: Option<String>,
        codex_login_credential: Option<String>,
        github_token_credential: Option<String>,
        tunnels: Vec<Tunnel>,
    }

    /// [`super::RecipeSize`], refusing unknown fields.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RecipeSize {
        cpus: u32,
        memory_gb: u32,
        disk_gb: u32,
    }

    /// [`super::RecipeBase`], refusing unknown fields inside each kind.
    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
    enum RecipeBase {
        // Serde's internally tagged unit visitor discards extra fields even
        // with deny_unknown_fields. An empty struct variant enforces the save
        // contract while the shared read enum remains tolerant.
        #[serde(rename = "tensorlake_ubuntu")]
        ManagedUbuntu {},
        #[serde(rename = "image")]
        TensorlakeImage { image: String },
        #[serde(rename = "snapshot")]
        TensorlakeSnapshot { snapshot_id: String },
        #[serde(rename = "container_image")]
        ContainerImage { reference: String, user: String },
    }

    /// [`super::Tunnel`], refusing unknown fields.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Tunnel {
        sandbox_port: u16,
        host: String,
        port: u16,
    }

    impl From<RecipeSave> for super::RecipeSave {
        fn from(save: RecipeSave) -> Self {
            let RecipeSize {
                cpus,
                memory_gb,
                disk_gb,
            } = save.size;
            Self {
                size: super::RecipeSize {
                    cpus,
                    memory_gb,
                    disk_gb,
                },
                base: match save.base {
                    RecipeBase::ManagedUbuntu {} => super::RecipeBase::ManagedUbuntu,
                    RecipeBase::TensorlakeImage { image } => {
                        super::RecipeBase::TensorlakeImage { image }
                    }
                    RecipeBase::TensorlakeSnapshot { snapshot_id } => {
                        super::RecipeBase::TensorlakeSnapshot { snapshot_id }
                    }
                    RecipeBase::ContainerImage { reference, user } => {
                        super::RecipeBase::ContainerImage { reference, user }
                    }
                },
                packages: save.packages,
                claude: save.claude,
                codex: save.codex,
                claude_login_credential: save.claude_login_credential,
                codex_login_credential: save.codex_login_credential,
                github_token_credential: save.github_token_credential,
                tunnels: save
                    .tunnels
                    .into_iter()
                    .map(
                        |Tunnel {
                             sandbox_port,
                             host,
                             port,
                         }| super::Tunnel {
                            sandbox_port,
                            host,
                            port,
                        },
                    )
                    .collect(),
            }
        }
    }
}

/// The body of `POST /api/tensorlake/recipes`: a new recipe and its name.
///
/// The recipe is nested rather than flattened because serde cannot combine
/// `flatten` with `deny_unknown_fields`. `PUT /api/tensorlake/recipes/{name}`
/// takes a bare [`RecipeSave`]; there the URL is the name's only authority.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateRecipe {
    pub name: String,
    pub recipe: RecipeSave,
}

/// One recipe as the helm reports it, without secrets.
///
/// Readers tolerate fields they do not know, so a client loaded before a helm
/// upgrade keeps working when this shape grows. The build fields default
/// when absent for the same reason in the other direction: a reply from a
/// helm without snapshot builds reads as "nothing built, nothing running".
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RecipeView {
    pub name: String,
    pub size: RecipeSize,
    pub base: RecipeBase,
    pub packages: Vec<String>,
    pub claude: Option<EndpointView>,
    pub codex: Option<EndpointView>,
    pub claude_login_credential: Option<String>,
    pub codex_login_credential: Option<String>,
    pub github_token_credential: Option<String>,
    pub tunnels: Vec<Tunnel>,
    /// The built software layer. Base or package edits keep it until an explicit
    /// rebuild records its replacement; preparation adds Farhelm on top of it.
    #[serde(default)]
    pub snapshot: Option<SnapshotView>,
    /// Whether a snapshot build is running, or how the last one ended when
    /// it did not succeed.
    #[serde(default)]
    pub build: BuildView,
    /// The Farhelm installation checkpoint reused by new hosts, when available.
    #[serde(default)]
    pub prepared: Option<PreparedSnapshotView>,
    /// Lazy preparation runs separately from an explicit snapshot build.
    #[serde(default)]
    pub preparation: PrepareView,
    /// A build/preparation sandbox or superseded snapshot created for this
    /// recipe still exists on Tensorlake because deleting it failed. It bills
    /// until the next build or the recipe's deletion removes it.
    #[serde(default)]
    pub cleanup_pending: bool,
}

/// A built snapshot as the list shows it. Times are seconds since the epoch.
///
/// `base`, `packages` and `disk_gb` are what the snapshot was built with, so a client
/// can tell when the recipe has moved on since: a different base or packages mean the
/// snapshot is stale, and a recipe disk smaller than `disk_gb` means hosts
/// cannot start from it at all (a Tensorlake sandbox cannot be smaller than
/// the snapshot it starts from).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SnapshotView {
    pub id: String,
    pub built_at: i64,
    /// Older snapshots were always built on Tensorlake Ubuntu.
    #[serde(default)]
    pub base: RecipeBase,
    pub disk_gb: u32,
    pub packages: Vec<String>,
    /// The `--version` lines the build read from the installed agents.
    pub claude_version: Option<String>,
    pub codex_version: Option<String>,
}

/// The steps of a snapshot build, in order. The UI shows the running one as
/// "step n of [`BuildStep::COUNT`]".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildStep {
    /// Removing what an earlier build left behind.
    Cleaning,
    /// Asking Tensorlake for the build sandbox.
    Creating,
    /// Waiting for the build sandbox to run.
    Starting,
    /// Reaching the build sandbox and checking every base requirement.
    Connecting,
    Packages,
    ClaudeCode,
    Codex,
    /// Preparing the filesystem and taking the snapshot.
    Snapshotting,
    /// Recording the snapshot and deleting the build sandbox.
    Finishing,
}

impl BuildStep {
    /// How many steps a build has.
    pub const COUNT: u8 = 9;

    /// This step's position, from 1 to [`Self::COUNT`].
    pub fn number(self) -> u8 {
        match self {
            BuildStep::Cleaning => 1,
            BuildStep::Creating => 2,
            BuildStep::Starting => 3,
            BuildStep::Connecting => 4,
            BuildStep::Packages => 5,
            BuildStep::ClaudeCode => 6,
            BuildStep::Codex => 7,
            BuildStep::Snapshotting => 8,
            BuildStep::Finishing => 9,
        }
    }

    /// What the step is doing, as a lowercase phrase that completes
    /// "Building snapshot: …" and "Snapshot build failed while …".
    pub fn label(self) -> &'static str {
        match self {
            BuildStep::Cleaning => "cleaning up after the last build",
            BuildStep::Creating => "creating the build sandbox",
            BuildStep::Starting => "waiting for the build sandbox to start",
            BuildStep::Connecting => "connecting and checking the base",
            BuildStep::Packages => "installing packages",
            BuildStep::ClaudeCode => "installing Claude Code",
            BuildStep::Codex => "installing Codex",
            BuildStep::Snapshotting => "taking the snapshot",
            BuildStep::Finishing => "finishing up",
        }
    }
}

/// A recipe's snapshot build status. Times are seconds since the epoch.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BuildView {
    /// No build is running, and the last one (if any) succeeded.
    #[default]
    Idle,
    /// A build is running and has reached `step`.
    Building { step: BuildStep, started_at: i64 },
    /// The last build failed at `step`; `message` says why, for display. The
    /// recipe's earlier snapshot, if it has one, is untouched.
    Failed {
        step: BuildStep,
        message: String,
        at: i64,
    },
    /// A build started and never finished because the helm stopped while it
    /// ran. What it left behind is removed by the next build.
    Interrupted { started_at: i64 },
}

/// A prepared installation's provenance, with freshness decided by the helm.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PreparedSnapshotView {
    pub id: String,
    pub prepared_at: i64,
    pub farhelm_version: String,
    pub source_snapshot: String,
    pub stale_reason: Option<PreparedStaleReason>,
}

/// The only reasons preparation is renewed. Recipe settings alone do not renew it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreparedStaleReason {
    Expired,
    FarhelmUpdated,
    SnapshotRebuilt,
}

/// Preparation's five visible phases; installation includes checkpoint-safety
/// verification, and snapshotting begins only after that verification succeeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepareStep {
    Creating,
    Starting,
    Installing,
    Snapshotting,
    Finishing,
}

impl PrepareStep {
    /// All clients use the same phase count in waiting and failure messages.
    pub const COUNT: u8 = 5;

    /// Stable one-based position for progress and failure messages.
    pub fn number(self) -> u8 {
        match self {
            Self::Creating => 1,
            Self::Starting => 2,
            Self::Installing => 3,
            Self::Snapshotting => 4,
            Self::Finishing => 5,
        }
    }

    /// A phrase completing both preparing and preparation-failed messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::Creating => "creating the preparer sandbox",
            Self::Starting => "starting the preparer sandbox",
            Self::Installing => "installing Farhelm",
            Self::Snapshotting => "taking the prepared snapshot",
            Self::Finishing => "finishing up",
        }
    }
}

/// Preparation outlives its requesting host setup. An interrupted run is retried
/// lazily by the next new host, without an eager renewal or separate button.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PrepareView {
    /// No preparation is running; the prepared record separately describes reuse.
    #[default]
    Idle,
    /// Registry-owned work that concurrent host setups join.
    Preparing { step: PrepareStep, started_at: i64 },
    /// Failure preserves the earlier checkpoint and its actual stale reason.
    Failed {
        step: PrepareStep,
        message: String,
        at: i64,
    },
    /// A durable start without a live task; the next use reconciles its ownership.
    Interrupted { started_at: i64 },
}

/// The body of `GET /api/tensorlake/recipes`, ordered by name.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RecipesView {
    pub recipes: Vec<RecipeView>,
}

/// Refuse a name that is not usable as a new recipe's name.
///
/// The grammar is `^[a-z][a-z0-9-]*$`, not ending in `-`, at most
/// [`RECIPE_NAME_CAP`] bytes. It is narrower than a launch template's on
/// purpose: the name goes verbatim into the builder sandbox's name, so it must
/// already be a valid piece of a Tensorlake name, and mapping it with no
/// sanitizing keeps two recipes from ever sharing a builder (sanitizing would
/// turn "Rust dev" and "rust-dev" into one). It is also URL-safe, so it can be
/// a path segment without encoding, and `.`/`..` are impossible.
pub fn check_recipe_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("enter a recipe name");
    }
    if name.len() > RECIPE_NAME_CAP {
        return Err("a recipe name can be at most 32 characters");
    }
    let bytes = name.as_bytes();
    if !bytes[0].is_ascii_lowercase()
        || !bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        || name.ends_with('-')
    {
        return Err(
            "a recipe name uses lowercase letters, digits and hyphens, starts with a letter and does not end with a hyphen",
        );
    }
    Ok(())
}

/// Split the editor's package text into package words: separated by any ASCII
/// whitespace (spaces, tabs, newlines), in order, exact duplicates dropped.
///
/// This does not validate the words; [`check_recipe`] does.
pub fn parse_packages(text: &str) -> Vec<String> {
    let mut packages: Vec<String> = Vec::new();
    for word in text.split_ascii_whitespace() {
        if !packages.iter().any(|seen| seen == word) {
            packages.push(word.to_string());
        }
    }
    packages
}

/// Whether `word` is an acceptable apt package argument: a Debian package name
/// (lowercase alphanumeric first, then `[a-z0-9+.-]`, at least two
/// characters), an optional `:arch` and an optional `=version`.
///
/// The leading-alphanumeric rule is a security boundary, not style. The build
/// shell-quotes every word, but apt-get still parses a quoted `-oAPT::...` as
/// an option, so nothing may start with `-`.
fn package_word_is_valid(word: &str) -> bool {
    if word.is_empty() || word.len() > PACKAGE_WORD_CAP {
        return false;
    }
    let (rest, version) = match word.split_once('=') {
        Some((rest, version)) => (rest, Some(version)),
        None => (word, None),
    };
    let (name, arch) = match rest.split_once(':') {
        Some((name, arch)) => (name, Some(arch)),
        None => (rest, None),
    };
    let name_ok = name.len() >= 2
        && (name.as_bytes()[0].is_ascii_lowercase() || name.as_bytes()[0].is_ascii_digit())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"+.-".contains(&b));
    let arch_ok = arch.is_none_or(|arch| {
        !arch.is_empty()
            && arch
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    });
    let version_ok = version.is_none_or(|version| {
        !version.is_empty()
            && version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".+~:-".contains(&b))
    });
    name_ok && arch_ok && version_ok
}

/// Whether `host` is a DNS name, an IPv4 literal, or a bracketed IPv6 literal,
/// at most 253 bytes and not starting with `-`.
///
/// DNS labels are 1 to 63 letters, digits and hyphens, not starting or ending
/// with a hyphen; case does not matter. A dotted all-digit name must parse as
/// an IPv4 address, so `999.1.1.1` is refused rather than taken as a name.
pub fn check_tunnel_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 || host.starts_with('-') {
        return false;
    }
    if let Some(inner) = host.strip_prefix('[') {
        return inner
            .strip_suffix(']')
            .is_some_and(|inner| inner.parse::<core::net::Ipv6Addr>().is_ok());
    }
    if host.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return host.parse::<core::net::Ipv4Addr>().is_ok();
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

/// Refuse an agent endpoint base URL Farhelm will not write into a sandbox.
///
/// Accepted: `http` or `https`, a host (as [`check_tunnel_host`] defines it),
/// an optional port from 1 to 65535 and an optional path; visible ASCII only,
/// at most [`BASE_URL_CAP`] bytes. Refused: credentials, a query, a fragment,
/// and anything else (ntfy's server URL rule). The messages are fixed so a
/// refusal cannot echo what was typed.
///
/// This is a structural check without a URL library (the UI's wasm build
/// keeps this crate's dependencies small); the helm also parses the URL
/// properly before storing it.
pub fn check_base_url(url: &str) -> Result<(), &'static str> {
    const SHAPE: &str = "use an http or https base URL without credentials, query or fragment";
    if url.is_empty() {
        return Err("enter a base URL");
    }
    if url.len() > BASE_URL_CAP || !url.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        return Err(SHAPE);
    }
    let lower = url.to_ascii_lowercase();
    let rest = if lower.starts_with("http://") {
        &url["http://".len()..]
    } else if lower.starts_with("https://") {
        &url["https://".len()..]
    } else {
        return Err(SHAPE);
    };
    if rest.contains(['?', '#', '@', '\\']) {
        return Err(SHAPE);
    }
    let authority = rest.split('/').next().unwrap_or_default();
    // A bracketed IPv6 host carries colons of its own, so the port is what
    // follows the closing bracket, not the last colon.
    let (host, port) = if authority.starts_with('[') {
        match authority.find(']') {
            Some(end) => {
                let (host, after) = authority.split_at(end + 1);
                match after.strip_prefix(':') {
                    Some(port) => (host, Some(port)),
                    None if after.is_empty() => (host, None),
                    None => return Err(SHAPE),
                }
            }
            None => return Err(SHAPE),
        }
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    if !check_tunnel_host(host) {
        return Err(SHAPE);
    }
    if let Some(port) = port
        && !(port.len() <= 5
            && port.bytes().all(|b| b.is_ascii_digit())
            && port
                .parse::<u32>()
                .is_ok_and(|port| (1..=65535).contains(&port)))
    {
        return Err(SHAPE);
    }
    Ok(())
}

/// Refuse a replacement token that is not 1 to [`TOKEN_CAP`] visible ASCII
/// bytes. An empty token is not a token; callers treat it as "clear".
pub fn check_token(token: &str) -> Result<(), &'static str> {
    if token.is_empty()
        || token.len() > TOKEN_CAP
        || !token.bytes().all(|b| (0x21..=0x7e).contains(&b))
    {
        return Err("a token can contain at most 4096 visible ASCII characters");
    }
    Ok(())
}

/// Refuse a recipe the helm will not store.
///
/// Checks the size's sanity bounds (1 to 64 CPUs, 1 to 512 GB of memory, 1 to
/// 4096 GB of disk), package words per `package_word_is_valid`,
/// at most [`PACKAGES_CAP`]; an image name of 1 to 200 bytes of
/// `[A-Za-z0-9._/:-]` not starting with `-`; a snapshot ID of 1 to 64
/// lowercase letters and digits, each endpoint's base URL and credential
/// name, and the tunnels (at most [`TUNNELS_CAP`], unique non-zero sandbox
/// ports other than 22, which the sandbox's own sshd holds, non-zero target
/// ports, valid hosts).
/// Container image references and login users retain their literal spelling.
/// Both are required and must fit process arguments, which exclude NUL. The
/// recipe write's existing request-body bound limits their combined size.
/// Podman and image inspection decide reference syntax and account resolution.
///
/// Messages are user-facing and name the offending package, image or tunnel
/// where that helps; none of those values is secret. URL and credential-name messages
/// are fixed. Duplicate packages are not refused here: the helm drops them
/// when it stores the recipe, as [`parse_packages`] does.
pub fn check_recipe(recipe: &RecipeSave) -> Result<(), String> {
    let size = recipe.size;
    if !(1..=64).contains(&size.cpus) {
        return Err("CPUs must be a whole number from 1 to 64".into());
    }
    if !(1..=512).contains(&size.memory_gb) {
        return Err("memory must be a whole number of GB from 1 to 512".into());
    }
    if !(1..=4096).contains(&size.disk_gb) {
        return Err("disk must be a whole number of GB from 1 to 4096".into());
    }
    if recipe.packages.len() > PACKAGES_CAP {
        return Err(format!(
            "a recipe can install at most {PACKAGES_CAP} packages"
        ));
    }
    if let Some(bad) = recipe
        .packages
        .iter()
        .find(|word| !package_word_is_valid(word))
    {
        return Err(format!("{bad:?} is not a valid Ubuntu package name"));
    }
    match &recipe.base {
        RecipeBase::ManagedUbuntu => {}
        RecipeBase::TensorlakeImage { image } => {
            if image.is_empty() {
                return Err("enter an image name".into());
            }
            if image.len() > 200
                || image.starts_with('-')
                || !image
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._/:-".contains(&b))
            {
                return Err(
                    "an image name uses letters, digits and . _ / : -, at most 200 characters"
                        .into(),
                );
            }
        }
        RecipeBase::TensorlakeSnapshot { snapshot_id } => {
            if snapshot_id.is_empty() {
                return Err("enter a snapshot ID".into());
            }
            if snapshot_id.len() > 64
                || !snapshot_id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            {
                return Err("a snapshot ID uses lowercase letters and digits only".into());
            }
        }
        RecipeBase::ContainerImage { reference, user } => {
            if reference.is_empty() {
                return Err("enter a container image reference".into());
            }
            if user.is_empty() {
                return Err("enter a container login user".into());
            }
            if reference.contains('\0') || user.contains('\0') {
                return Err("a container image reference and login user cannot contain NUL".into());
            }
        }
    }
    for (agent, endpoint) in [("Claude Code", &recipe.claude), ("Codex", &recipe.codex)] {
        let Some(endpoint) = endpoint else { continue };
        check_base_url(&endpoint.base_url).map_err(|reason| format!("{agent}: {reason}"))?;
        if let Some(name) = endpoint.token_credential.as_deref() {
            check_recipe_name(name)
                .map_err(|_| format!("{agent}: select a valid endpoint credential name"))?;
        }
    }
    if recipe.claude.is_some() && recipe.claude_login_credential.is_some() {
        return Err("Claude: select either an API endpoint or a subscription login".into());
    }
    if let Some(name) = recipe.claude_login_credential.as_deref() {
        check_recipe_name(name)
            .map_err(|_| "Claude: select a valid login credential name".to_string())?;
    }
    if recipe.codex.is_some() && recipe.codex_login_credential.is_some() {
        return Err("Codex: select either an API endpoint or a subscription login".into());
    }
    if let Some(name) = recipe.codex_login_credential.as_deref() {
        check_recipe_name(name)
            .map_err(|_| "Codex: select a valid login credential name".to_string())?;
    }
    if let Some(name) = recipe.github_token_credential.as_deref() {
        check_recipe_name(name).map_err(|_| "select a valid GitHub credential name".to_string())?;
    }
    if recipe.tunnels.len() > TUNNELS_CAP {
        return Err(format!("a recipe can have at most {TUNNELS_CAP} tunnels"));
    }
    for (index, tunnel) in recipe.tunnels.iter().enumerate() {
        if tunnel.sandbox_port == 0 || tunnel.port == 0 {
            return Err("tunnel ports must be from 1 to 65535".into());
        }
        if tunnel.sandbox_port == 22 {
            return Err(
                "sandbox port 22 is taken by the sandbox's SSH server; pick another port".into(),
            );
        }
        if recipe.tunnels[..index]
            .iter()
            .any(|earlier| earlier.sandbox_port == tunnel.sandbox_port)
        {
            return Err(format!(
                "sandbox port {} is used by two tunnels",
                tunnel.sandbox_port
            ));
        }
        if !check_tunnel_host(&tunnel.host) {
            return Err(format!(
                "{:?} is not a host name, IPv4 address or bracketed IPv6 address",
                tunnel.host
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tensorlake/recipes_tests.rs"]
mod tests;
