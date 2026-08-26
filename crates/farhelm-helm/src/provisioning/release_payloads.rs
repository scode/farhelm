//! The verified download payload source: how a release-shaped helm gets the
//! host binaries it pushes over SSH (D2, D3).
//!
//! This module is the entire trust boundary of download-based provisioning.
//! Everything below it — the extractor in [`super::payloads`], the SSH
//! executor — treats the file it is handed as authentic, so the only place
//! that decides whether a downloaded byte is allowed to become a host binary
//! is here. The chain is:
//!
//! 1. a `SHA256SUMS` signed with the key CI holds, verified against
//!    [`MINISIGN_PUBKEY`] compiled into this binary;
//! 2. the signature's TRUSTED COMMENT, which must read exactly
//!    `farhelm v{version}` for this build's own version;
//! 3. a per-asset SHA-256 computed over the bytes as they arrive off the
//!    socket.
//!
//! Step 2 is easy to mistake for decoration and is not. The signature alone
//! authenticates only the CONTENTS of `SHA256SUMS`, and those contents name
//! no version — the version appears only in the URL and the cache path, both
//! of which the server chooses. Without the trusted comment, whoever serves
//! the release URL (a compromised mirror or CDN, not necessarily the key
//! holder) can replay an older release's perfectly valid manifest, signature
//! and same-named assets at the new version's URL and downgrade every host
//! the helm provisions. The trusted comment is inside minisign's global
//! signature, so binding the version there costs one string compare and
//! closes that hole.
//!
//! ONE tag convention, everywhere: a release tag is `vX.Y.Z` — it already
//! carries the leading `v` — and the trusted comment is `farhelm ` followed
//! by that tag. **The Step 5 `sign-sums` job MUST therefore sign with
//! `minisign -S -t "farhelm $TAG" -s key -m SHA256SUMS`**, with no second
//! `v` of its own; `farhelm v$TAG` would render `farhelm vv1.2.3` and
//! produce a release every shipped helm refuses.
//! [`required_trusted_comment`] is the consumer half of that contract and
//! `signing_and_verification_agree_on_the_tag_convention` pins the two
//! together.
//!
//! A failure anywhere in the chain refuses. Nothing is ever "used anyway" or
//! repaired.
//!
//! The cache is the second concern. Provisioning happens interactively from
//! the hosts panel and re-runs are routine, so a second "add host" must not
//! re-download tens of megabytes — but a cache is also the obvious place for
//! a half-written file from a killed process to survive. Hence the layout
//! below: everything THIS module writes goes to a FIXED `.part` sibling and
//! is renamed, so a crash leaves only names it already knows how to clean
//! up. That is not the whole story, though: the shared extractor it calls
//! into ([`extract_single_member`], owned by the directory source) stages
//! through randomly named `tempfile` siblings, so [`housekeeping`] also
//! sweeps `.tmp*` leftovers out of a generation directory on first use.

use super::assets;
use super::payloads::{PayloadSource, copy_executable, extract_single_member};
use super::plan::{PayloadArch, PayloadKind};
use anyhow::{Context as _, anyhow, bail};
use async_trait::async_trait;
use futures_util::StreamExt as _;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};
use tracing::info;
use url::Url;

/// The public half of the minisign key CI signs every release's
/// `SHA256SUMS` with; pairs with the `MINISIGN_SECRET_KEY` repository
/// secret (D3).
///
/// Published `SHA256SUMS` files are never re-signed or replaced — that is
/// repository policy, enforced by the `sign-sums` job refusing to overwrite
/// an existing one — so a helm carrying this key keeps verifying every
/// release that was ever signed with it, including releases cut long after
/// that helm was built.
///
/// Rotation is cheap TODAY because of D2: a helm only ever downloads the
/// release matching its own version, and that release is signed by whatever
/// key the same commit compiled in here. So a rotation is one PR that swaps
/// this constant and `farhelm-release.pub` together with the repository
/// secret, and the next release simply uses the new key; nothing already in
/// the field looks at it. The ordering problem only appears with a
/// cross-version download — an auto-updater verifying the NEXT release with
/// the CURRENT key — which nothing does yet. Whoever adds one must first
/// ship a release carrying the new key (signed with the old), and only then
/// sign with the new; see SPEC_impl.md's "Release signing key" for the
/// recipe and that constraint.
///
/// The key is only half the contract. Signing must ALSO pass
/// `-t "farhelm $TAG"` — the tag already begins with `v`, so there is no
/// second one — putting the version in the signed trusted comment. See this
/// module's header for why a signature without it is replayable across
/// versions. `farhelm-release.pub` sits beside this file as the committed
/// oracle for this constant.
pub const MINISIGN_PUBKEY: &str = "RWSNQaVU+WXJm29s7DRqwrHGbzMgOJck6kLPfVU4Gvk1uCnwgdlzp/U/";

/// The release THIS build asks for. A helm downloads the payloads matching
/// its OWN version and no other, which is what keeps a provisioned host
/// running exactly what the helm that provisioned it expects (D2).
///
/// `ReleasePayloadSource` does not read this constant itself — it takes the
/// version it expects as a constructor argument instead, and this is the
/// only value the ONE production call site
/// (`payloads::production_payloads_with_key`) ever passes. That split
/// exists because of a real incident: the fixtures under
/// `tests/fixtures/release/` are signed once, permanently, for version
/// `0.0.3` (see that directory's `README.md`), and the first real release
/// tag bumped the workspace version away from `0.0.3` and broke every test
/// in this module — they were all reading this same constant to know what
/// version to expect, so the bump and the fixtures disagreed. Tests inject
/// `test_support::FIXTURE_VERSION` instead, and `production_wiring_...` (in
/// `provisioning.rs`'s test module) is the oracle that production really
/// does pass this constant and nothing else.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Size ceiling for `SHA256SUMS` and its signature. Both are a few hundred
/// bytes by construction (six lines, one signature), so anything larger is
/// not a checksum file at all — a login page, an error document, a wrong
/// URL. The cap exists so a hostile or misconfigured server cannot make the
/// helm buffer an unbounded response while it is still deciding whether to
/// trust anything at that URL; the ASSETS themselves are streamed and have
/// no cap, because by then their expected hash is already known.
const SUMS_MAX_BYTES: usize = 64 * 1024;

/// The exact trusted comment a release's signature must carry: `farhelm `
/// followed by `version`'s release TAG, which is `v` + the version.
///
/// Rendered rather than stored so it cannot drift from the version the
/// caller supplies, and taking that version as a parameter (rather than
/// reading a module constant) so a bump automatically stops accepting the
/// previous release's signature IN PRODUCTION, without also demanding the
/// test fixtures be re-signed — see [`VERSION`]'s docstring for the
/// incident that made this a parameter. The producing half of this contract
/// is `sign-sums`' `-t "farhelm $TAG"`; see the module header, and the
/// parity test that pins them together.
fn required_trusted_comment(version: &str) -> String {
    format!("farhelm {}", release_tag(version))
}

/// The git tag one release is published under (`vX.Y.Z`, D15).
///
/// Split out from [`required_trusted_comment`] so the tag convention has one
/// definition that a test can compare against the signing command's `$TAG`,
/// rather than a `v` hidden inside a format string that reads correctly
/// whether or not the signer adds one of their own.
fn release_tag(version: &str) -> String {
    format!("v{version}")
}

/// The HTTP seam every request goes through.
///
/// It exists for one reason that static typing cannot express otherwise:
/// [`ReleasePayloadSource::get`] promises EXACTLY one retry, and only for
/// connect failures. `reqwest::Error` cannot be constructed outside reqwest,
/// so without this seam a test could only produce connect failures by
/// pointing at a port it hopes stays closed — which proves nothing about the
/// attempt COUNT and races anything else on the machine for that port.
///
/// Production is [`HttpTransport`]; the failure classification lives there,
/// so everything above this trait reasons about
/// [`TransportError::Connect`] rather than about reqwest internals.
#[async_trait]
pub(super) trait Transport: std::fmt::Debug + Send + Sync {
    async fn get(&self, url: Url) -> Result<reqwest::Response, TransportError>;
}

/// A request that never produced a response, split by whether retrying it
/// could plausibly help.
pub(super) enum TransportError {
    /// The connection was never established (refused, DNS, TLS handshake).
    /// This is the one failure mode routinely transient enough to retry.
    Connect(anyhow::Error),
    /// Anything else: a mid-body stall, a protocol error, a timeout while
    /// data was already flowing. Retrying would re-pay a large download for
    /// a failure that is not usually transient.
    Other(anyhow::Error),
}

impl TransportError {
    fn into_inner(self) -> anyhow::Error {
        match self {
            Self::Connect(error) | Self::Other(error) => error,
        }
    }
}

/// The real transport: one `reqwest::Client`, with reqwest's own connect
/// classification deciding what counts as retryable.
#[derive(Debug)]
pub(super) struct HttpTransport {
    client: reqwest::Client,
}

impl HttpTransport {
    pub(super) fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn get(&self, url: Url) -> Result<reqwest::Response, TransportError> {
        self.client.get(url).send().await.map_err(|error| {
            let connect = error.is_connect();
            let error = transport_error(error);
            if connect {
                TransportError::Connect(error)
            } else {
                TransportError::Other(error)
            }
        })
    }
}

/// Wrap a `reqwest::Error` for rendering, with its URL stripped.
///
/// `reqwest::Error`'s `Display` includes the URL of the request that failed,
/// and after a redirect that is no longer the base URL an operator supplied
/// — it is wherever the release server sent us, which for an object store or
/// a mirror is routinely a signed URL carrying a token in its query string.
/// These errors are rendered into provisioning progress that reaches the
/// browser, so a redirect target must not ride along. Nothing is lost: every
/// message built from these already names the validated base URL separately.
fn transport_error(error: reqwest::Error) -> anyhow::Error {
    anyhow::Error::new(error.without_url())
}

/// Downloads, verifies, caches, and unpacks release payloads for a
/// release-shaped helm (D13) or an explicit `--release-base-url`.
///
/// One instance serves the whole process. State it holds is exactly the
/// state that must not be duplicated: the once-per-process `SHA256SUMS`
/// fetch, the once-per-process cache housekeeping, and one lock per asset
/// name so two concurrent "add host" flows asking for the same binary
/// produce one download rather than two writers racing over the same cache
/// files.
pub(super) struct ReleasePayloadSource {
    /// Where assets are fetched from, guaranteed to end in `/` so
    /// [`Url::join`] appends an asset name instead of replacing the last
    /// path segment.
    base_url: Url,
    /// The shared parent of every generation directory
    /// (`<state_dir>/payloads`), kept so housekeeping can prune the
    /// generations this build has outgrown.
    cache_root: PathBuf,
    /// This source's private generation directory (see [`Self::new`]).
    cache_dir: PathBuf,
    /// The release version this source will accept — the version its
    /// signed manifest must be signed for, the version its cache directory
    /// is keyed by, and the version named in every refusal it renders.
    ///
    /// A constructor argument rather than a read of [`VERSION`] so this
    /// type has no built-in opinion about `CARGO_PKG_VERSION`: production
    /// passes [`VERSION`] (the one call site is
    /// `payloads::production_payloads_with_key`), and tests pass
    /// `test_support::FIXTURE_VERSION`, the version the committed fixtures
    /// are permanently signed for. See [`VERSION`]'s docstring for why that
    /// split exists.
    version: String,
    /// The minisign public key `SHA256SUMS.minisig` must verify against.
    /// Injected rather than read from [`MINISIGN_PUBKEY`] directly so tests
    /// can drive the real verification path with a throwaway test key
    /// instead of needing the production secret key to exist.
    pubkey: &'static str,
    transport: Arc<dyn Transport>,
    /// The verified checksum file, fetched at most once per process
    /// (successes only — a failed fetch leaves this empty so the next "add
    /// host" retries rather than inheriting a transient network failure for
    /// the lifetime of the helm).
    sums: OnceCell<ReleaseSums>,
    /// Runs [`Self::housekeeping`] exactly once, before this source's first
    /// download. Awaited rather than fire-and-forget so cleanup can never
    /// race a download it would otherwise delete out from under.
    housekept: OnceCell<()>,
    /// One mutex per published asset name. `std::sync::Mutex` guards only
    /// the map lookup (never held across an await); the value it hands out
    /// is the async mutex actually held for the duration of a download.
    asset_locks: std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

/// The parsed, signature-verified contents of one release's `SHA256SUMS`:
/// published asset name to lowercase hex SHA-256.
///
/// The RAW bytes are kept beside the parsed entries, and they are not
/// redundant: they are the only authenticated copy of the control files this
/// process still holds once they have been written to disk, so a cached copy
/// that later fails verification can be repaired from memory instead of
/// condemning every subsequent request in this process to a full re-download
/// (see [`ReleasePayloadSource::cached_controls_verified`]). Both are a few
/// hundred bytes; holding them costs nothing worth measuring.
struct ReleaseSums {
    entries: BTreeMap<String, String>,
    sums_bytes: Vec<u8>,
    signature_bytes: Vec<u8>,
}

/// Names the type and the two fields that identify WHICH release source
/// this is, deliberately omitting the checksum map and the lock table —
/// they are large, uninteresting in a log line, and the reason a derived
/// `Debug` would be worse than this one. The leading type name is also the
/// hook `production_payloads`' selection tests match on.
impl std::fmt::Debug for ReleasePayloadSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReleasePayloadSource")
            .field("base_url", &self.base_url.as_str())
            .field("cache_dir", &self.cache_dir)
            .finish_non_exhaustive()
    }
}

impl ReleasePayloadSource {
    /// Build a source reading `base_url` into a generation directory of its
    /// own below `cache_root` (which is `<state_dir>/payloads`).
    ///
    /// The directory is `v{version}-<first 12 hex of sha256(base_url)>`.
    /// Both halves matter: the version because a helm only ever wants its
    /// own release's payloads and an upgraded helm must not inherit the old
    /// one's binaries, and the URL hash because a test or mirror pointed at
    /// some other server must be physically unable to poison — or be
    /// confused by — the cache the real GitHub release wrote.
    ///
    /// `base_url` is normalised to end in `/`, so an operator passing
    /// `--release-base-url https://example.invalid/farhelm/v1` gets asset
    /// URLs under that path rather than beside it. Queries, fragments and
    /// embedded credentials are rejected earlier, at the flag
    /// (`HelmArgs::release_base_url`), because a base URL carrying them
    /// cannot survive [`Url::join`] intact and would silently address a
    /// different endpoint than every error message names.
    ///
    /// `version` is the release this source will accept (see the field's
    /// docstring); production passes [`VERSION`], tests pass
    /// `test_support::FIXTURE_VERSION`.
    pub(super) fn new(
        base_url: Url,
        cache_root: PathBuf,
        version: impl Into<String>,
        pubkey: &'static str,
        client: reqwest::Client,
    ) -> Self {
        Self::with_transport(
            base_url,
            cache_root,
            version,
            pubkey,
            Arc::new(HttpTransport::new(client)),
        )
    }

    /// [`Self::new`] with the HTTP seam supplied — the constructor the retry
    /// policy tests use. Production always goes through [`Self::new`].
    pub(super) fn with_transport(
        base_url: Url,
        cache_root: PathBuf,
        version: impl Into<String>,
        pubkey: &'static str,
        transport: Arc<dyn Transport>,
    ) -> Self {
        let version = version.into();
        let base_url = normalise_base_url(base_url);
        let mut hasher = Sha256::new();
        hasher.update(base_url.as_str().as_bytes());
        let digest = hex(&hasher.finalize());
        let cache_dir = cache_root.join(format!("v{version}-{}", &digest[..12]));
        Self {
            base_url,
            cache_root,
            cache_dir,
            version,
            pubkey,
            transport,
            sums: OnceCell::new(),
            housekept: OnceCell::new(),
            asset_locks: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// The async mutex for one asset name, created on first request.
    ///
    /// Per asset rather than one global lock so provisioning a cold host —
    /// which needs both a `farhelm` and a `tmux` — downloads them at the
    /// same time; in-process rather than a lock file because a helm is one
    /// process per state directory (plan Step 3), so there is no second
    /// writer to coordinate with.
    fn asset_lock(&self, asset: &str) -> Arc<Mutex<()>> {
        let mut locks = self
            .asset_locks
            .lock()
            .expect("the asset lock table is never held across a panic");
        Arc::clone(locks.entry(asset.to_string()).or_default())
    }

    /// Run [`housekeeping`] once for this source, before its first download.
    ///
    /// Ordering is the whole point: housekeeping deletes stale `.part` and
    /// temporary files by NAME PATTERN, which is only safe while nothing in
    /// this process is writing one. Gating it behind a `OnceCell` that every
    /// `path()` awaits means the second concurrent caller waits for the
    /// first's cleanup instead of racing it.
    async fn housekeep(&self) -> anyhow::Result<()> {
        self.housekept
            .get_or_try_init(|| async {
                let cache_root = self.cache_root.clone();
                let cache_dir = self.cache_dir.clone();
                let version = self.version.clone();
                tokio::task::spawn_blocking(move || housekeeping(&cache_root, &cache_dir, &version))
                    .await
                    .context("the payload cache housekeeping task panicked")?
            })
            .await?;
        Ok(())
    }

    /// The verified checksum file for this release, fetched once per
    /// process.
    ///
    /// Deliberately network-backed even when a previous run left a valid
    /// `SHA256SUMS` in the cache: a cache MISS means this process is about
    /// to trust bytes it has not seen before, and a release that was still
    /// publishing when an earlier run cached its checksum file would
    /// otherwise be believed forever. Cache HITS never come through here —
    /// they re-verify the cached copy from disk and make no request at all.
    ///
    /// `get_or_try_init` rather than `get_or_init` so a failed fetch leaves
    /// the cell EMPTY: a helm that lost its network for one "add host" must
    /// not be poisoned for the rest of its uptime.
    async fn sums(&self) -> anyhow::Result<&ReleaseSums> {
        self.sums
            .get_or_try_init(|| async { self.fetch_sums().await })
            .await
    }

    /// Fetch `SHA256SUMS` and `SHA256SUMS.minisig`, verify signature and
    /// version, parse the entries, and mirror both files into the cache so a
    /// later process can satisfy a cache hit without the network.
    async fn fetch_sums(&self) -> anyhow::Result<ReleaseSums> {
        let sums_bytes = self.fetch_capped("SHA256SUMS").await?;
        let signature_bytes = self.fetch_capped("SHA256SUMS.minisig").await?;
        verify_sums(self.pubkey, &self.version, &sums_bytes, &signature_bytes)?;

        let text = std::str::from_utf8(&sums_bytes)
            .map_err(|_| malformed_sums_refusal(&self.version, &self.base_url))?;
        let entries = parse_sums(text)
            .ok_or_else(|| malformed_sums_refusal(&self.version, &self.base_url))?;

        // Published only after verification, so the cache can never hold a
        // checksum file that was not signed for this version. Both writes
        // and their fsyncs are one blocking transaction: `sync_all` can
        // stall for a noticeable time on an unhealthy filesystem, and a
        // stalled tokio worker delays every unrelated request the helm is
        // serving.
        let cache_dir = self.cache_dir.clone();
        let published_sums = sums_bytes.clone();
        let published_signature = signature_bytes.clone();
        tokio::task::spawn_blocking(move || {
            publish_controls(&cache_dir, &published_sums, &published_signature)
        })
        .await
        .context("the checksum publication task panicked")??;
        Ok(ReleaseSums {
            entries,
            sums_bytes,
            signature_bytes,
        })
    }

    /// Fetch one small control file (`SHA256SUMS` or its signature),
    /// refusing anything past [`SUMS_MAX_BYTES`] before it is copied rather
    /// than after.
    ///
    /// The cap is checked against the PROSPECTIVE length, and an oversized
    /// `Content-Length` is refused before the body is read at all: this is
    /// the one fetch made before anything about the server has been
    /// authenticated, so the memory boundary has to hold against a server
    /// that answers with one enormous chunk. The streamed check stays
    /// authoritative because `Content-Length` is a claim, not a fact.
    ///
    /// A 404 on EITHER file reports D17's one message: a release that is
    /// still publishing has some of its assets and not others, and a single
    /// request cannot tell that apart from a release that does not exist,
    /// so both consumers of this URL (here and `install.sh`) say the same
    /// thing rather than guessing.
    async fn fetch_capped(&self, name: &str) -> anyhow::Result<Vec<u8>> {
        let version = &self.version;
        let response = self.get(name).await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            bail!(
                "no SHA256SUMS for v{version} at {} (HTTP 404): the release is not published or \
                 is still publishing; retry in a few minutes, or pass --payload-dir",
                self.base_url
            );
        }
        if !response.status().is_success() {
            bail!(
                "fetching {name} for v{version} from {} failed: HTTP {}",
                self.base_url,
                response.status()
            );
        }
        if response.content_length().is_some_and(|length| {
            length > u64::try_from(SUMS_MAX_BYTES).expect("64 KiB fits in u64")
        }) {
            return Err(self.oversized_refusal(name));
        }

        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| self.unreachable(transport_error(error)))?;
            if !append_capped(&mut body, &chunk, SUMS_MAX_BYTES) {
                return Err(self.oversized_refusal(name));
            }
        }
        Ok(body)
    }

    /// Issue one GET for `name` under the base URL, retrying exactly once
    /// on a connect failure.
    ///
    /// Only connect errors are retried, and only once: a refused or dropped
    /// TCP connect is the failure mode that is routinely transient (a
    /// captive portal coming up, a CDN node cycling), while a mid-body
    /// stall or an HTTP status is either slow-and-still-running or a real
    /// answer, and retrying those would double a large download for no
    /// gain. There is no overall deadline for the same reason: a host
    /// binary is tens of megabytes and a slow link is not an error.
    ///
    /// "Exactly one" is only true because the client disables reqwest's own
    /// protocol-nack retries (`release_client`); otherwise this seam would
    /// be one of several places attempts are made.
    async fn get(&self, name: &str) -> anyhow::Result<reqwest::Response> {
        let url = self
            .base_url
            .join(name)
            .with_context(|| format!("building the download URL for {name}"))?;
        let outcome = match self.transport.get(url.clone()).await {
            Ok(response) => Ok(response),
            Err(TransportError::Connect(_)) => self.transport.get(url).await,
            Err(other) => Err(other),
        };
        outcome.map_err(|error| self.unreachable(error.into_inner()))
    }

    /// The one message for "this machine could not talk to the release
    /// server", covering connect failures, TLS failures, and mid-body
    /// transport errors alike — from the operator's seat those are one
    /// problem with one fix, and the underlying error is appended for the
    /// detail.
    fn unreachable(&self, error: anyhow::Error) -> anyhow::Error {
        let version = &self.version;
        anyhow!(
            "add host needs farhelm's v{version} release assets from {}, and this machine could \
             not reach it: {error:#}",
            self.base_url
        )
    }

    /// The refusal for a control file that exceeds [`SUMS_MAX_BYTES`].
    fn oversized_refusal(&self, name: &str) -> anyhow::Error {
        let version = &self.version;
        anyhow!(
            "refusing v{version} assets: {name} at {} is larger than 64 KiB and cannot be a \
             checksum file",
            self.base_url
        )
    }

    /// Whether the cache already holds a usable extracted binary for
    /// `asset`, cleaning up after itself when it does not.
    ///
    /// Threat model, so the guarantee is not overstated: farhelm's state
    /// directory is trusted against the account that owns it, exactly as the
    /// rest of helm state is. The marker beside `<asset>.bin` is a
    /// SELF-CONSISTENCY check, not provenance — it is computed locally from
    /// the extracted binary and is not bound to the signed manifest, so
    /// anyone who can write helm state could replace both. What it does
    /// catch is the thing that actually happens: a helm killed mid-write, a
    /// truncated file, a filesystem that lost a block. The cached signature
    /// is re-verified alongside it (cheap, once per asset request) so a
    /// process restart still re-establishes that the manifest it is trusting
    /// was signed for THIS version — that check is about staleness and
    /// version replay, not about hostile local writes.
    ///
    /// Any inconsistency — no marker, unreadable marker, unreadable binary,
    /// hash mismatch — is treated as a partially written cache entry rather
    /// than as an error: the entry is discarded so the caller refetches.
    /// That is the recovery path for a helm killed mid-download, and making
    /// it an error instead would turn one bad file into a permanently broken
    /// "add host" that only a manual `rm -rf` could clear.
    async fn cached(&self, asset: &str) -> anyhow::Result<bool> {
        if !self.cached_controls_verified().await? {
            return Ok(false);
        }
        let cache_dir = self.cache_dir.clone();
        let probed = asset.to_string();
        // One blocking task for every filesystem read the hit decision needs.
        // The marker read and the hash were previously on the async worker,
        // and hashing a release binary is tens of megabytes of I/O — enough
        // to stall unrelated requests the helm is serving.
        //
        // Only a panic is an error here. An I/O failure — permissions, ACLs,
        // a damaged file, something that is not a regular file at all — means
        // the entry is unusable, which is a MISS: propagating it would turn
        // one bad file into an "add host" that can never succeed again.
        let hit = tokio::task::spawn_blocking(move || cached_binary_matches(&cache_dir, &probed))
            .await
            .context("the cache verification task panicked")?;
        if hit {
            return Ok(true);
        }
        info!(
            asset,
            "discarding an incomplete or corrupt cached provisioning payload"
        );
        self.discard(asset).await?;
        Ok(false)
    }

    /// Whether the cached control files verify — repairing them from
    /// authenticated memory first, if this process already has them.
    ///
    /// The repair is what keeps one corrupted byte on disk from costing a
    /// full asset re-download on EVERY later request in this process: the
    /// miss path would happily reuse the in-memory manifest, download the
    /// asset again, and leave the corrupt control files exactly as it found
    /// them, so the next lookup missed again for the same reason. Rewriting
    /// them from [`ReleaseSums`]'s retained raw bytes is safe because those
    /// bytes are the ones that already passed [`verify_sums`] — republishing
    /// cannot install anything the network path would have refused.
    ///
    /// Returns false, not an error, when verification fails and there is
    /// nothing to repair from: the only consequence is that the caller takes
    /// the miss path, which fetches and verifies from the network anyway and
    /// renders the precise refusal there.
    async fn cached_controls_verified(&self) -> anyhow::Result<bool> {
        let cache_dir = self.cache_dir.clone();
        let pubkey = self.pubkey;
        let version = self.version.clone();
        let verified =
            tokio::task::spawn_blocking(move || cached_sums_verify(&cache_dir, pubkey, &version))
                .await
                .context("the cached checksum verification task panicked")?;
        if verified {
            return Ok(true);
        }
        let Some(sums) = self.sums.get() else {
            return Ok(false);
        };
        let cache_dir = self.cache_dir.clone();
        let sums_bytes = sums.sums_bytes.clone();
        let signature_bytes = sums.signature_bytes.clone();
        tokio::task::spawn_blocking(move || {
            publish_controls(&cache_dir, &sums_bytes, &signature_bytes)
        })
        .await
        .context("the checksum repair task panicked")??;
        info!(
            cache_dir = %self.cache_dir.display(),
            "repaired the cached release manifest from this process's verified copy"
        );
        Ok(true)
    }

    /// Remove every cache file belonging to one asset — the download, its
    /// `.part` stagings, the extracted binary, and the marker — so the next
    /// attempt starts from nothing. Absent files are not an error; a
    /// genuinely undeletable one is, since silently continuing would leave
    /// the next attempt reading the same corrupt bytes.
    ///
    /// The suffix list is the complete set of FIXED per-asset names this
    /// source writes (see [`ASSET_SUFFIXES`]), which is what makes recovery a
    /// matter of deleting known paths rather than guessing.
    async fn discard(&self, asset: &str) -> anyhow::Result<()> {
        let cache_dir = self.cache_dir.clone();
        let asset = asset.to_string();
        tokio::task::spawn_blocking(move || discard_asset(&cache_dir, &asset))
            .await
            .context("the cache cleanup task panicked")?
    }

    /// Stream one asset to `<asset>.part`, hashing as it arrives, and rename
    /// it into place only once the hash matches `expected`.
    ///
    /// Hashing during the stream rather than re-reading the finished file is
    /// what makes "the bytes that were checked" and "the bytes that were
    /// written" the same bytes by construction, and it keeps a
    /// tens-of-megabytes archive out of memory. Nothing is ever renamed to
    /// its final name before the comparison passes, so a mismatch cannot
    /// leave something that a later run mistakes for a verified download.
    async fn download_verified(&self, asset: &str, expected: &str) -> anyhow::Result<PathBuf> {
        let version = &self.version;
        let response = self.get(asset).await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            bail!("release v{version} has no asset named {asset} (HTTP 404)");
        }
        if !response.status().is_success() {
            bail!(
                "fetching {asset} for v{version} from {} failed: HTTP {}",
                self.base_url,
                response.status()
            );
        }

        let part = self.cache_dir.join(format!("{asset}.part"));
        let mut file = tokio::fs::File::create(&part)
            .await
            .with_context(|| format!("creating {}", part.display()))?;
        let mut hasher = Sha256::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            // The body stream fails with the FINAL request URL attached (a
            // redirect target, possibly a signed object-store URL) — strip
            // it exactly as the transport path does.
            let chunk = chunk.map_err(|error| self.unreachable(transport_error(error)))?;
            hasher.update(&chunk);
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
                .await
                .with_context(|| format!("writing {}", part.display()))?;
        }
        tokio::io::AsyncWriteExt::flush(&mut file)
            .await
            .with_context(|| format!("writing {}", part.display()))?;
        file.sync_all()
            .await
            .with_context(|| format!("writing {}", part.display()))?;
        drop(file);

        let actual = hex(&hasher.finalize());
        if actual != expected {
            // The partial download is removed rather than kept for
            // inspection: it is unverified content sitting in helm state,
            // and the operator's next step is a retry, not a forensic read.
            return Err(checksum_refusal(
                asset,
                &actual,
                expected,
                remove_if_present(&part),
            ));
        }
        let downloaded = self.cache_dir.join(asset);
        std::fs::rename(&part, &downloaded)
            .with_context(|| format!("installing {}", downloaded.display()))?;
        Ok(downloaded)
    }
}

#[async_trait]
impl PayloadSource for ReleasePayloadSource {
    async fn path(&self, payload: PayloadKind, arch: PayloadArch) -> anyhow::Result<PathBuf> {
        // One match for the whole payload-format decision: the published
        // name, and the archive member to extract — `None` for the one kind
        // D5 ships unarchived. Deriving both here keeps the name and the
        // shape from drifting apart in separate matches.
        let (asset, member) = match payload {
            PayloadKind::Farhelm => {
                let archive = assets::farhelm_archive_for(arch);
                (assets::archive_name(archive), Some(archive.member))
            }
            PayloadKind::Tmux => (assets::tmux_name(arch).to_string(), None),
        };
        self.housekeep().await?;

        // Held across every await below, so a concurrent request for the
        // same asset waits and then observes the cache hit the first one
        // produced instead of downloading a second copy over the first.
        let lock = self.asset_lock(&asset);
        let _guard = lock.lock().await;

        let binary = self.cache_dir.join(format!("{asset}.bin"));
        if self.cached(&asset).await? {
            return Ok(binary);
        }

        std::fs::create_dir_all(&self.cache_dir)
            .with_context(|| format!("creating {}", self.cache_dir.display()))?;
        let sums = self.sums().await?;
        let version = &self.version;
        let expected = sums
            .entries
            .get(&asset)
            .ok_or_else(|| anyhow!("SHA256SUMS for v{version} has no entry for {asset}"))?;
        let downloaded = self.download_verified(&asset, expected).await?;

        // Everything from here is blocking work on whole-binary-sized files
        // — extraction, hashing, two fsyncs — so the entire publication runs
        // as one transaction off the async workers, with the asset mutex
        // still held across it.
        let staged = self.cache_dir.join(format!("{asset}.bin.part"));
        let marker = self.cache_dir.join(format!("{asset}.bin.sha256"));
        let published = binary.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            match member {
                Some(member) => extract_single_member(&downloaded, member, &staged)?,
                // tmux ships unarchived (D5), so "extraction" is a verbatim
                // copy that only has to add the executable bit; going
                // through `.bin` anyway keeps one cache shape for both
                // payload kinds.
                None => copy_executable(&downloaded, &staged)?,
            }
            let digest = sha256_file(&staged)?;
            // Marker first, then the rename: a crash between the two leaves
            // a marker with no `.bin`, which the cache check reads as a
            // miss. The reverse order would leave a `.bin` with no marker,
            // which is the same miss but one that has already published a
            // file under its final name.
            publish_bytes(&marker, format!("{digest}\n").as_bytes())?;
            std::fs::rename(&staged, &published)
                .with_context(|| format!("installing {}", published.display()))
        })
        .await
        .context("the payload extraction task panicked")??;
        Ok(binary)
    }
}

/// Every FIXED filename suffix this source writes for one asset, relative to
/// the asset's published name. [`discard_asset`] works from this list, so
/// adding a fixed-name cache file without adding it here is the one way to
/// create an orphan it cannot clean up.
///
/// Complete for fixed per-asset names, and only for those. The shared
/// extractor this module calls into stages through randomly named `tempfile`
/// siblings that no list can enumerate — [`housekeeping`] sweeps those by
/// their `.tmp*` prefix instead, on first use, when nothing in the process is
/// writing one.
const ASSET_SUFFIXES: [&str; 6] = [
    "",
    ".part",
    ".bin",
    ".bin.part",
    ".bin.sha256",
    ".bin.sha256.part",
];

/// Longest a marker file may be before it is refused unread: a 64-character
/// digest, a newline, and slack for a stray carriage return. A cache entry
/// whose marker is larger is not a marker, so there is no reason to allocate
/// it in order to find that out.
const MARKER_MAX_BYTES: u64 = 128;

/// Verify a `SHA256SUMS` against `pubkey`: the signature, then the version
/// bound into its trusted comment.
///
/// One function because the download path and the cached-copy re-check must
/// apply IDENTICAL rules — a cache that accepted a signature the network
/// path would refuse would be a way to keep using a downgraded manifest
/// across restarts.
///
/// Every way authenticity can fail — not UTF-8, not a minisign signature, a
/// legacy (non-prehashed) signature, a good signature from the wrong key —
/// collapses into one refusal on purpose. Telling them apart would only tell
/// an attacker which guess was closer. The VERSION mismatch is separate,
/// because that one is an operator-actionable fact about a real, correctly
/// signed release.
fn verify_sums(pubkey: &str, version: &str, sums: &[u8], signature: &[u8]) -> anyhow::Result<()> {
    let key = minisign_verify::PublicKey::from_base64(pubkey).map_err(|error| {
        anyhow!("farhelm's built-in minisign public key is not decodable: {error}")
    })?;
    let signature = std::str::from_utf8(signature)
        .ok()
        .and_then(|text| minisign_verify::Signature::decode(text).ok())
        .ok_or_else(|| signature_refusal(version))?;
    key.verify(sums, &signature, false)
        .map_err(|_| signature_refusal(version))?;

    let comment = signature.trusted_comment();
    if comment != required_trusted_comment(version) {
        bail!(
            "refusing v{version} assets: SHA256SUMS.minisig was signed for {comment}, not this \
             version"
        );
    }
    Ok(())
}

/// The refusal for a `SHA256SUMS` that does not verify against the key this
/// binary carries. See [`verify_sums`] for why the three distinguishable
/// causes deliberately produce one message.
fn signature_refusal(version: &str) -> anyhow::Error {
    anyhow!(
        "refusing v{version} assets: SHA256SUMS.minisig does not verify with farhelm's built-in \
         key"
    )
}

/// The refusal for a `SHA256SUMS` whose signature checks out but whose body
/// is not `sha256sum` output. Not one of the plan's enumerated texts because
/// it cannot happen against a release CI produced — it exists so a
/// hand-edited or truncated file fails loudly instead of parsing to an empty
/// entry set that then reports every asset as missing.
fn malformed_sums_refusal(version: &str, base_url: &Url) -> anyhow::Error {
    anyhow!("refusing v{version} assets: SHA256SUMS at {base_url} is not in sha256sum format")
}

/// Prune what this source has outgrown, once per process, before its first
/// download.
///
/// Two kinds of debris accumulate. GENERATION directories: every version and
/// base URL gets its own, each holding a release archive plus its extracted
/// binary, and an upgraded helm would otherwise keep every previous
/// version's copies forever. Only directories matching the exact generation
/// name shape are considered, only those whose version prefix differs from
/// this build's, and only real directories — a symlink planted under
/// `payloads/` is left strictly alone rather than followed. Same-version
/// generations are KEPT: those belong to other base URLs a running helm may
/// still be using.
///
/// STAGING files inside this source's own generation: `.part` files, and the
/// randomly named temporaries the shared extractor stages through. Both are
/// removed by name pattern, which is safe only because this runs before the
/// process writes any of its own — see [`ReleasePayloadSource::housekeep`].
///
/// Filesystem failures OTHER than "it is not there" are propagated with the
/// path rather than skipped. Silently swallowing them (a permission fault, a
/// failing disk) would disable cleanup indefinitely while provisioning
/// carried on and the cache grew without bound — with nothing anywhere to
/// say why.
///
/// `current_version` is the calling source's injected `version` field, not
/// a read of the module-level [`VERSION`] constant: `cache_dir` is itself
/// keyed by that same injected version, so a generation belonging to it
/// must be judged "current" by the same yardstick, or a test source built
/// with `FIXTURE_VERSION` would find its own generation misclassified as
/// foreign the moment the crate's real version diverges from the
/// fixtures'.
fn housekeeping(cache_root: &Path, cache_dir: &Path, current_version: &str) -> anyhow::Result<()> {
    let current = cache_dir.file_name();
    let entries = match std::fs::read_dir(cache_root) {
        Ok(entries) => entries,
        // Nothing cached yet is the overwhelmingly common case on a fresh
        // helm, and is not a condition worth reporting.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", cache_root.display()));
        }
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("reading {}", cache_root.display()))?;
        if Some(entry.file_name().as_os_str()) == current {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !is_stale_generation(&name, current_version) {
            continue;
        }
        // `symlink_metadata`, not `metadata`: a symlink named like a
        // generation must never make this walk into whatever it points at.
        // A vanished entry is a benign race with another cleanup; anything
        // else is a real fault worth surfacing.
        let metadata = match std::fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspecting {}", entry.path().display()));
            }
        };
        if !metadata.is_dir() {
            continue;
        }
        std::fs::remove_dir_all(entry.path())
            .with_context(|| format!("removing stale {}", entry.path().display()))?;
        info!(generation = %name, "removed a payload cache generation from an older farhelm");
    }

    let entries = match std::fs::read_dir(cache_dir) {
        Ok(entries) => entries,
        // This source's own generation not existing yet is the normal cold
        // start; anything else means the directory is there and unreadable.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", cache_dir.display()));
        }
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("reading {}", cache_dir.display()))?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !name.ends_with(".part") && !name.starts_with(".tmp") {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspecting {}", entry.path().display()));
            }
        };
        if !metadata.is_file() {
            continue;
        }
        remove_if_present(&entry.path())?;
        info!(file = %name, "removed an abandoned payload cache staging file");
    }
    Ok(())
}

/// Whether `name` is a generation directory belonging to a DIFFERENT farhelm
/// version — the only entries under `payloads/` this build may delete.
///
/// The shape is `v<version>-<12 lowercase hex>`, and the split is on the
/// LAST 13 characters rather than on the last hyphen. That distinction is the
/// whole point: a SemVer prerelease contains hyphens of its own, so splitting
/// on the last one made `v1.2.3-rc.1-0123456789ab` unparseable — its
/// generations were never pruned — while accepting `v-0123456789ab`, whose
/// version is the empty string, as a directory to delete recursively.
///
/// Parsing the remainder as real SemVer rather than string-comparing it also
/// means an equal version written differently cannot be mistaken for a
/// foreign one. Anything that does not parse is left alone: this predicate
/// guards a recursive delete inside the helm's state directory, so every name
/// it is unsure about must survive.
///
/// `current_version` is supplied by the caller rather than read from a
/// module constant, so this predicate has no built-in notion of "this
/// build's version" — see [`housekeeping`] for why that matters.
fn is_stale_generation(name: &str, current_version: &str) -> bool {
    let Some(split) = name.len().checked_sub(12) else {
        return false;
    };
    let (version, digest) = name.split_at(split);
    if !digest
        .chars()
        .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase() && c.is_ascii_hexdigit())
    {
        return false;
    }
    let Some(version) = version.strip_suffix('-').and_then(|v| v.strip_prefix('v')) else {
        return false;
    };
    let (Ok(parsed), Ok(current)) = (
        semver::Version::parse(version),
        semver::Version::parse(current_version),
    ) else {
        return false;
    };
    parsed != current
}

/// Guarantee a trailing `/` so [`Url::join`] treats the base as a directory.
/// Without it, joining `farhelm-x86_64…tar.gz` onto `…/download/v1.2.3`
/// silently produces `…/download/farhelm-x86_64…tar.gz`.
fn normalise_base_url(mut base_url: Url) -> Url {
    if !base_url.path().ends_with('/') {
        let path = format!("{}/", base_url.path());
        base_url.set_path(&path);
    }
    base_url
}

/// Parse `sha256sum` output into name → lowercase hex digest, returning
/// `None` for anything that is not that format.
///
/// Strict on purpose (64 hex characters, then a name): the alternative —
/// skipping lines it does not understand — would turn a truncated or
/// HTML-wrapped file into a valid-looking checksum set with entries
/// missing, reported downstream as "no entry for {asset}", which points the
/// operator at the wrong problem. Accepts both `sha256sum` output modes,
/// since the binary-mode `*` prefix is part of that format.
fn parse_sums(text: &str) -> Option<BTreeMap<String, String>> {
    let mut entries = BTreeMap::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let (digest, name) = line.split_once(char::is_whitespace)?;
        if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let name = name.trim_start().trim_start_matches('*').trim();
        if name.is_empty() {
            return None;
        }
        entries.insert(name.to_string(), digest.to_ascii_lowercase());
    }
    if entries.is_empty() {
        return None;
    }
    Some(entries)
}

/// SHA-256 of a file on disk as lowercase hex. Synchronous and streaming —
/// callers run it through `spawn_blocking`, and it must not read a
/// whole binary into memory.
fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file =
        std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).with_context(|| format!("hashing {}", path.display()))?;
    Ok(hex(&hasher.finalize()))
}

/// Write `contents` to `path` through a FIXED `<path>.part` sibling, so a
/// reader never observes a half-written control file — and so a crash leaves
/// a file this module already knows the name of.
///
/// Deliberately not `NamedTempFile`: a random temporary name that outlives
/// its process (SIGKILL, power loss) is unrecognisable to the next run and
/// accumulates forever. Synchronous, including the fsync; callers run it
/// through `spawn_blocking`.
fn publish_bytes(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
    let staging = path.with_file_name(format!(
        "{}.part",
        path.file_name()
            .and_then(|name| name.to_str())
            .expect("cache control files always have a UTF-8 name")
    ));
    let mut file = std::fs::File::create(&staging)
        .with_context(|| format!("creating {}", staging.display()))?;
    std::io::Write::write_all(&mut file, contents)
        .with_context(|| format!("writing {}", staging.display()))?;
    file.sync_all()
        .with_context(|| format!("writing {}", staging.display()))?;
    drop(file);
    std::fs::rename(&staging, path).with_context(|| format!("installing {}", path.display()))?;
    Ok(())
}

/// Delete `path`, treating "already gone" as success.
///
/// A DIRECTORY sitting where a cache file belongs is debris the same way a
/// truncated file is — a botched manual copy, a restore, an interrupted
/// tool — and it is removed for the same reason: refusing would wedge every
/// future "add host" on a state directory the operator has no reason to
/// suspect. Every other failure is returned, because a file this module
/// could not remove is a file the next attempt will read.
fn remove_if_present(path: &Path) -> anyhow::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) if path.is_dir() => {}
        Err(error) => {
            return Err(error).with_context(|| format!("removing {}", path.display()));
        }
    }
    std::fs::remove_dir_all(path).with_context(|| format!("removing {}", path.display()))
}

/// Append `chunk` to `body` unless doing so would cross `limit`, answering
/// whether it was appended.
///
/// A function rather than three lines inline because it is the memory
/// boundary the control-file fetch relies on BEFORE anything about the
/// server is authenticated, and the property that matters — the oversized
/// bytes are never copied at all — is invisible from the outside. Checked
/// against the prospective length with saturating arithmetic, so a
/// preposterous chunk length cannot wrap past the comparison.
fn append_capped(body: &mut Vec<u8>, chunk: &[u8], limit: usize) -> bool {
    if body.len().saturating_add(chunk.len()) > limit {
        return false;
    }
    body.extend_from_slice(chunk);
    true
}

/// The refusal for an asset whose bytes do not hash to their signed entry,
/// folding in a failure to remove the rejected download.
///
/// Both halves matter to whoever reads it: "these bytes were wrong" and "and
/// they are still sitting in your state directory" are separate things to
/// act on. Split out from the download path because the cleanup-failure
/// branch cannot be reached from a test through the filesystem — the staging
/// file has to be creatable for the download to get that far — so this is
/// where that combination is pinned.
fn checksum_refusal(
    asset: &str,
    actual: &str,
    expected: &str,
    cleanup: anyhow::Result<()>,
) -> anyhow::Error {
    let refusal =
        anyhow!("refusing {asset}: SHA-256 {actual} does not match SHA256SUMS ({expected})");
    match cleanup {
        Ok(()) => refusal,
        Err(error) => error.context(format!("{refusal:#}")),
    }
}

/// Publish both verified control files into `cache_dir`, creating it first.
///
/// One function so the initial fetch and the later repair write the pair the
/// same way; synchronous, including two fsyncs, so callers run it through
/// `spawn_blocking`.
fn publish_controls(cache_dir: &Path, sums: &[u8], signature: &[u8]) -> anyhow::Result<()> {
    std::fs::create_dir_all(cache_dir)
        .with_context(|| format!("creating {}", cache_dir.display()))?;
    publish_bytes(&cache_dir.join("SHA256SUMS"), sums)?;
    publish_bytes(&cache_dir.join("SHA256SUMS.minisig"), signature)
}

/// Re-verify the cached `SHA256SUMS` — signature AND version — against
/// `pubkey` and `version`.
///
/// Returns false, never an error, for every failure including "not cached
/// yet": the only consequence is that the caller takes the miss path, which
/// fetches and verifies from the network anyway and renders the precise
/// refusal there.
///
/// Both files are read through [`read_capped`], so a restored or corrupted
/// cache cannot make this allocate more than the network path would have
/// accepted in the first place. Synchronous; callers use `spawn_blocking`.
fn cached_sums_verify(cache_dir: &Path, pubkey: &str, version: &str) -> bool {
    let limit = u64::try_from(SUMS_MAX_BYTES).expect("64 KiB fits in u64");
    let Some(sums) = read_capped(&cache_dir.join("SHA256SUMS"), limit) else {
        return false;
    };
    let Some(signature) = read_capped(&cache_dir.join("SHA256SUMS.minisig"), limit) else {
        return false;
    };
    verify_sums(pubkey, version, &sums, &signature).is_ok()
}

/// Whether `<asset>.bin` in `cache_dir` is a regular file whose SHA-256
/// matches the marker beside it.
///
/// The `symlink_metadata` check is not a formality. Hashing streams whatever
/// the path opens to, so a FIFO left in the cache blocks forever waiting for
/// a writer, and a symlink to `/dev/zero` hashes without end — either one
/// pins a blocking worker and leaves "add host" hanging with no error to
/// report. Requiring an ordinary file, without following symlinks, turns both
/// into an ordinary discardable miss. Synchronous; callers use
/// `spawn_blocking`.
fn cached_binary_matches(cache_dir: &Path, asset: &str) -> bool {
    let binary = cache_dir.join(format!("{asset}.bin"));
    if !std::fs::symlink_metadata(&binary).is_ok_and(|metadata| metadata.is_file()) {
        return false;
    }
    let Some(recorded) = read_capped(
        &cache_dir.join(format!("{asset}.bin.sha256")),
        MARKER_MAX_BYTES,
    ) else {
        return false;
    };
    let Ok(recorded) = std::str::from_utf8(&recorded) else {
        return false;
    };
    sha256_file(&binary).is_ok_and(|actual| recorded.trim() == actual)
}

/// Read a cache file that is known to be small, refusing before allocating.
///
/// `None` for every reason a cache entry could be unusable — absent, not a
/// regular file, a symlink, larger than `limit` — because all of them mean
/// the same thing to the caller: a miss. The size is taken from
/// `symlink_metadata` rather than after reading, so an oversized or restored
/// cache file cannot be pulled into memory just to be rejected.
fn read_capped(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > limit {
        return None;
    }
    std::fs::read(path).ok()
}

/// Remove every fixed-name cache file belonging to one asset. Synchronous;
/// callers use `spawn_blocking`. See [`ASSET_SUFFIXES`].
fn discard_asset(cache_dir: &Path, asset: &str) -> anyhow::Result<()> {
    for suffix in ASSET_SUFFIXES {
        remove_if_present(&cache_dir.join(format!("{asset}{suffix}")))?;
    }
    Ok(())
}

/// Lowercase hex for a digest. Hand-rolled rather than pulling in a hex
/// crate for the one place farhelm renders bytes as hex.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// The loopback release fixture, shared by this module's own tests and by
/// `provisioning.rs`'s end-to-end provisioning test.
///
/// It lives here rather than in either test module because both need the
/// same server, and a second copy would be a second thing to keep in step
/// with the committed fixture layout.
#[cfg(test)]
pub(super) mod test_support {
    use super::*;
    use axum::extract::State;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// The miniature release served to every test — six assets, a real
    /// `SHA256SUMS`, and a real minisign signature carrying this version in
    /// its trusted comment. See that directory's `README.md`.
    pub(in crate::provisioning) const FIXTURE_DIR: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/release");

    /// The version every committed fixture is signed for, PERMANENTLY —
    /// re-signing on every release bump is exactly the friction this
    /// module's tests must not impose (see [`VERSION`]'s docstring for the
    /// incident). Every test that builds a `ReleasePayloadSource`, or that
    /// asserts a rendered `v{version}` string, uses this constant instead of
    /// [`VERSION`] so the suite stays correct regardless of what the
    /// workspace version happens to be.
    ///
    /// `variants/other-version/` is signed for `v0.0.2` and must NEVER be
    /// changed to match this constant: it is the downgrade-replay fixture,
    /// and its entire point is to disagree with `FIXTURE_VERSION`.
    pub(in crate::provisioning) const FIXTURE_VERSION: &str = "0.0.3";

    /// The throwaway key the fixtures are signed with, read out of the
    /// committed `.pub` file rather than pasted here, so regenerating the
    /// fixture key pair needs no code change.
    ///
    /// This is emphatically NOT [`MINISIGN_PUBKEY`]: the production key's
    /// secret half exists only as a repository secret, so a test that used
    /// it could not sign anything and would have to stub out the one check
    /// this whole module exists to perform.
    pub(in crate::provisioning) fn test_pubkey() -> &'static str {
        const KEY_FILE: &str = include_str!("../../tests/fixtures/release/test-key.pub");
        KEY_FILE
            .lines()
            .nth(1)
            .expect("a minisign public key file is a comment line then the key line")
            .trim()
    }

    pub(in crate::provisioning) fn fixture_bytes(relative: &str) -> Vec<u8> {
        std::fs::read(Path::new(FIXTURE_DIR).join(relative))
            .unwrap_or_else(|error| panic!("reading fixture {relative}: {error}"))
    }

    /// The exact bytes every fixture "binary" holds, reconstructed from the
    /// rule the fixture README states rather than copied. A drifted fixture
    /// therefore fails a byte comparison instead of quietly comparing
    /// whatever it happens to contain against itself.
    pub(in crate::provisioning) fn expected_member(package: &str, target: &str) -> Vec<u8> {
        format!("#!/bin/sh\necho \"farhelm fixture: {package} {target}\"\n").into_bytes()
    }

    /// A client with production's exact settings plus `no_proxy()`.
    ///
    /// The `no_proxy()` is not cosmetic: reqwest reads `HTTP_PROXY` and
    /// friends from the environment, so on a developer machine behind a
    /// proxy a `http://127.0.0.1:…` fixture URL would open its socket to
    /// that proxy — quietly breaking this repository's rule that no test
    /// talks to anything but loopback. Everything else is shared with
    /// production so the tests cannot drift from the shipped timeouts,
    /// redirect policy, or retry policy.
    pub(in crate::provisioning) fn test_client() -> reqwest::Client {
        super::super::payloads::release_client_builder()
            .no_proxy()
            .build()
            .expect("building the fixture HTTP client")
    }

    /// How the fixture server should answer one path instead of serving the
    /// file of that name.
    ///
    /// Overrides live in the SERVER, never in the fixture directory: a test
    /// that mutated committed files would leave the checkout dirty on
    /// failure and would race every other test sharing the directory.
    #[derive(Clone)]
    pub(in crate::provisioning) enum Override {
        /// Answer 404, for "the release has no such asset" and "the release
        /// is not published yet".
        NotFound,
        /// Answer 200 with these bytes — tampered assets, an alternate
        /// signed `SHA256SUMS` from `variants/`, or a body too large to be a
        /// checksum file.
        Body(Vec<u8>),
        /// Answer 307 elsewhere, the way GitHub redirects release assets to
        /// its object store. The target is a path on this same loopback
        /// server, or an absolute loopback URL for the cases that need the
        /// redirected request to land somewhere the fixture does not serve.
        Redirect(String),
        /// Answer HTTP 500 while the counter is positive, then serve the
        /// real file — a transient failure that must not be remembered.
        FailFirst(Arc<AtomicUsize>),
        /// Announce arrival on `arrived`, then wait for `gate` before
        /// serving the real file. Lets a test prove two requests are in
        /// flight at once without sleeping.
        Hold {
            arrived: Arc<tokio::sync::Notify>,
            gate: Arc<tokio::sync::Notify>,
        },
        /// Stream `prefix`, then wait for `gate` before streaming `suffix`,
        /// recording in `released` whether the suffix was ever sent. Serves
        /// a CHUNKED body, so no `Content-Length` is announced.
        Gated {
            prefix: Vec<u8>,
            suffix: Vec<u8>,
            gate: Arc<tokio::sync::Notify>,
            released: Arc<AtomicBool>,
        },
    }

    #[derive(Clone)]
    struct FixtureState {
        requests: Arc<std::sync::Mutex<Vec<String>>>,
        overrides: Arc<HashMap<String, Override>>,
    }

    /// A loopback stand-in for a GitHub release, aborted when dropped.
    ///
    /// It counts requests because several of this module's contracts are
    /// about requests NOT happening — a cache hit makes none, two concurrent
    /// callers make one download between them — and a request log is the
    /// only way to observe that from outside.
    pub(in crate::provisioning) struct FixtureRelease {
        pub(in crate::provisioning) base_url: Url,
        requests: Arc<std::sync::Mutex<Vec<String>>>,
        server: tokio::task::JoinHandle<()>,
    }

    impl Drop for FixtureRelease {
        fn drop(&mut self) {
            self.server.abort();
        }
    }

    impl FixtureRelease {
        pub(in crate::provisioning) async fn start(overrides: Vec<(&str, Override)>) -> Self {
            let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
            let state = FixtureState {
                requests: Arc::clone(&requests),
                overrides: Arc::new(
                    overrides
                        .into_iter()
                        .map(|(name, response)| (name.to_string(), response))
                        .collect(),
                ),
            };
            let app = axum::Router::new().fallback(serve).with_state(state);
            // Port 0 on loopback only: no test in this repository may open a
            // socket to anything but 127.0.0.1.
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            Self {
                base_url: Url::parse(&format!("http://{address}/")).unwrap(),
                requests,
                server,
            }
        }

        pub(in crate::provisioning) fn requests(&self) -> Vec<String> {
            self.requests.lock().unwrap().clone()
        }

        /// A source pointed at this release, caching under `cache_root`,
        /// expecting [`FIXTURE_VERSION`], verifying with the fixture key and
        /// using the test client.
        pub(in crate::provisioning) fn source(&self, cache_root: &Path) -> ReleasePayloadSource {
            ReleasePayloadSource::new(
                self.base_url.clone(),
                cache_root.to_path_buf(),
                FIXTURE_VERSION,
                test_pubkey(),
                test_client(),
            )
        }
    }

    /// Serve one fixture file, an override, or 404.
    ///
    /// Names containing a path separator are refused rather than resolved:
    /// the release layout is flat, and a fixture server that walked out of
    /// its own directory would be a strange thing to leave lying around even
    /// in tests.
    async fn serve(
        State(state): State<FixtureState>,
        uri: axum::http::Uri,
    ) -> axum::response::Response {
        use axum::response::IntoResponse as _;

        let name = uri.path().trim_start_matches('/').to_string();
        state.requests.lock().unwrap().push(name.clone());
        if name.is_empty() || name.contains('/') {
            return axum::http::StatusCode::NOT_FOUND.into_response();
        }
        match state.overrides.get(&name).cloned() {
            Some(Override::NotFound) => axum::http::StatusCode::NOT_FOUND.into_response(),
            Some(Override::Body(body)) => body.into_response(),
            Some(Override::Redirect(target)) => {
                axum::response::Redirect::temporary(&target).into_response()
            }
            Some(Override::FailFirst(remaining)) => {
                let failed = remaining
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                        left.checked_sub(1)
                    })
                    .is_ok();
                if failed {
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
                } else {
                    serve_file(&name)
                }
            }
            Some(Override::Hold { arrived, gate }) => {
                arrived.notify_one();
                gate.notified().await;
                serve_file(&name)
            }
            Some(Override::Gated {
                prefix,
                suffix,
                gate,
                released,
            }) => gated_response(prefix, suffix, gate, released),
            None => serve_file(&name),
        }
    }

    fn serve_file(name: &str) -> axum::response::Response {
        use axum::response::IntoResponse as _;

        match std::fs::read(Path::new(FIXTURE_DIR).join(name)) {
            Ok(bytes) => bytes.into_response(),
            Err(_) => axum::http::StatusCode::NOT_FOUND.into_response(),
        }
    }

    /// A loopback listener that accepts every connection and immediately
    /// closes it, aborted when dropped.
    ///
    /// This is how a REAL transport failure is produced without giving up the
    /// port. The obvious alternative — bind an ephemeral port, drop the
    /// listener, then connect to it — hands the port back to the OS for the
    /// window between the two, so another process on the machine can claim it
    /// and the test ends up talking to something unrelated, or waiting out
    /// the client's 60-second read timeout. Holding the port and hanging up
    /// on the request is deterministic and cannot be squatted.
    pub(in crate::provisioning) struct ClosingListener {
        pub(in crate::provisioning) address: std::net::SocketAddr,
        listener: tokio::task::JoinHandle<()>,
    }

    impl Drop for ClosingListener {
        fn drop(&mut self) {
            self.listener.abort();
        }
    }

    impl ClosingListener {
        pub(in crate::provisioning) async fn start() -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let task = tokio::spawn(async move {
                while let Ok((stream, _)) = listener.accept().await {
                    drop(stream);
                }
            });
            Self {
                address,
                listener: task,
            }
        }

        pub(in crate::provisioning) fn base_url(&self) -> Url {
            Url::parse(&format!("http://{}/", self.address)).unwrap()
        }
    }

    /// A chunked response whose tail is withheld until `gate` fires.
    ///
    /// The point is what it can PROVE: a reader that refuses partway through
    /// never releases the gate, so `released` staying false is evidence the
    /// client stopped reading rather than merely rejecting a body it had
    /// already swallowed whole.
    fn gated_response(
        prefix: Vec<u8>,
        suffix: Vec<u8>,
        gate: Arc<tokio::sync::Notify>,
        released: Arc<AtomicBool>,
    ) -> axum::response::Response {
        let (sender, receiver) = tokio::sync::mpsc::channel::<std::io::Result<Vec<u8>>>(2);
        tokio::spawn(async move {
            if sender.send(Ok(prefix)).await.is_err() {
                return;
            }
            gate.notified().await;
            released.store(true, Ordering::SeqCst);
            let _ = sender.send(Ok(suffix)).await;
        });
        let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
            receiver.recv().await.map(|item| (item, receiver))
        });
        axum::response::Response::new(axum::body::Body::from_stream(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    /// The generation directory `source` caches into, derived the same way
    /// production does so a test can plant or inspect cache files.
    fn generation(cache_root: &Path, base_url: &Url) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(normalise_base_url(base_url.clone()).as_str().as_bytes());
        cache_root.join(format!(
            "v{FIXTURE_VERSION}-{}",
            &hex(&hasher.finalize())[..12]
        ))
    }

    /// A transport that answers from a fixed script and counts attempts.
    ///
    /// The only way to prove "exactly one retry, connect failures only"
    /// without a real network: `reqwest::Error` cannot be constructed
    /// outside reqwest, and a closed port proves nothing about how many
    /// times it was dialled.
    #[derive(Debug)]
    struct ScriptedTransport {
        attempts: Arc<AtomicUsize>,
        script: std::sync::Mutex<std::collections::VecDeque<Scripted>>,
    }

    #[derive(Debug)]
    enum Scripted {
        Connect,
        Other,
        Ok(Vec<u8>),
    }

    impl ScriptedTransport {
        fn new(script: Vec<Scripted>) -> (Arc<Self>, Arc<AtomicUsize>) {
            let attempts = Arc::new(AtomicUsize::new(0));
            let transport = Arc::new(Self {
                attempts: Arc::clone(&attempts),
                script: std::sync::Mutex::new(script.into()),
            });
            (transport, attempts)
        }
    }

    #[async_trait]
    impl Transport for ScriptedTransport {
        async fn get(&self, _url: Url) -> Result<reqwest::Response, TransportError> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            match self.script.lock().unwrap().pop_front() {
                Some(Scripted::Connect) => {
                    Err(TransportError::Connect(anyhow!("scripted connect failure")))
                }
                Some(Scripted::Other) => {
                    Err(TransportError::Other(anyhow!("scripted stream failure")))
                }
                Some(Scripted::Ok(body)) => Ok(reqwest::Response::from(
                    axum::http::Response::builder()
                        .status(200)
                        .body(body)
                        .unwrap(),
                )),
                None => Err(TransportError::Other(anyhow!("the script ran out"))),
            }
        }
    }

    fn scripted_source(
        cache_root: &Path,
        script: Vec<Scripted>,
    ) -> (ReleasePayloadSource, Arc<AtomicUsize>) {
        let (transport, attempts) = ScriptedTransport::new(script);
        let source = ReleasePayloadSource::with_transport(
            Url::parse("http://127.0.0.1:9/").unwrap(),
            cache_root.to_path_buf(),
            FIXTURE_VERSION,
            test_pubkey(),
            transport,
        );
        (source, attempts)
    }

    /// Spec: every payload kind and architecture the provisioner can ask for
    /// is downloaded, checksum-verified, unpacked to a single executable
    /// binary, and returned — a `farhelm` archive through the extractor, a
    /// bare `tmux` through the verbatim copy.
    ///
    /// This is the whole reason the module exists, and it runs against real
    /// signed bytes over a real socket: a stubbed verifier here would leave
    /// the one security-relevant path in provisioning untested.
    #[tokio::test]
    async fn downloads_verifies_and_extracts_every_payload_kind_and_arch() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache = tempfile::tempdir().unwrap();
        let source = release.source(cache.path());

        for (arch, target) in [
            (PayloadArch::X86_64, "x86_64-unknown-linux-musl"),
            (PayloadArch::Aarch64, "aarch64-unknown-linux-musl"),
        ] {
            let farhelm = source.path(PayloadKind::Farhelm, arch).await.unwrap();
            assert_eq!(
                std::fs::read(&farhelm).unwrap(),
                expected_member("farhelm", target)
            );
            assert_eq!(mode(&farhelm), 0o755, "a host payload must be executable");

            let tmux = source.path(PayloadKind::Tmux, arch).await.unwrap();
            assert_eq!(
                std::fs::read(&tmux).unwrap(),
                expected_member("tmux", target)
            );
            assert_eq!(mode(&tmux), 0o755, "a host payload must be executable");
        }

        // `SHA256SUMS` and its signature are fetched once for the whole
        // process, not once per asset: four assets, six requests.
        let requests = release.requests();
        assert_eq!(
            requests
                .iter()
                .filter(|name| name.starts_with("SHA256SUMS"))
                .count(),
            2,
            "the checksum file is fetched once per process: {requests:?}"
        );
        assert_eq!(requests.len(), 6, "{requests:?}");
    }

    /// Spec: the committed fixture set really is the six-asset release the
    /// rest of this module's tests assume — the manifest names match
    /// [`assets::sums_members`] exactly, every listed digest matches the
    /// file on disk, and all four archives hold exactly one REGULAR member
    /// at dist's nesting with the documented bytes.
    ///
    /// Without this the macOS and desktop archives are never opened by any
    /// test, so they could rot, drift, or be silently truncated while the
    /// suite stayed green and the comments went on calling this a complete
    /// release.
    #[test]
    fn the_committed_fixture_release_is_internally_consistent() {
        let sums_bytes = fixture_bytes("SHA256SUMS");
        verify_sums(
            test_pubkey(),
            FIXTURE_VERSION,
            &sums_bytes,
            &fixture_bytes("SHA256SUMS.minisig"),
        )
        .expect("the committed fixture manifest must verify with the committed test key");
        let entries = parse_sums(std::str::from_utf8(&sums_bytes).unwrap()).unwrap();

        assert_eq!(
            entries.keys().cloned().collect::<Vec<_>>(),
            assets::sums_members(),
            "the fixture manifest must list exactly the names a release publishes"
        );
        for (name, expected) in &entries {
            let mut hasher = Sha256::new();
            hasher.update(fixture_bytes(name));
            assert_eq!(&hex(&hasher.finalize()), expected, "digest drift in {name}");
        }

        for archive in &assets::RELEASE_ARCHIVES {
            let name = assets::archive_name(archive);
            let file = std::fs::File::open(Path::new(FIXTURE_DIR).join(&name)).unwrap();
            let mut reader = tar::Archive::new(flate2::read::GzDecoder::new(file));
            let members: Vec<_> = reader
                .entries()
                .unwrap()
                .map(|entry| {
                    let mut entry = entry.unwrap();
                    let path = entry.path().unwrap().to_string_lossy().into_owned();
                    let is_file = entry.header().entry_type().is_file();
                    let mut bytes = Vec::new();
                    std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
                    (path, is_file, bytes)
                })
                .collect();
            assert_eq!(members.len(), 1, "{name} must hold exactly one member");
            let (path, is_file, bytes) = &members[0];
            assert_eq!(
                path,
                &format!("{}-{}/{}", archive.package, archive.target, archive.member)
            );
            assert!(is_file, "{name}'s member must be a regular file");
            assert_eq!(bytes, &expected_member(archive.package, archive.target));
        }
    }

    /// Spec: a payload already in the cache is returned without touching the
    /// network at all — not even to re-fetch the checksum file — which is
    /// what keeps a second "add host" from re-downloading tens of megabytes.
    ///
    /// Uses a FRESH source over the same cache directory, because the
    /// interesting case is a restarted helm rather than a second call
    /// through one process's in-memory state.
    #[tokio::test]
    async fn a_cached_payload_is_reused_without_any_request() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache = tempfile::tempdir().unwrap();
        let first = release
            .source(cache.path())
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap();
        let after_warmup = release.requests().len();

        let second = release
            .source(cache.path())
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap();

        assert_eq!(first, second);
        let requests = release.requests();
        assert_eq!(
            requests.len(),
            after_warmup,
            "a cache hit must perform no request: {requests:?}"
        );
    }

    /// Spec: a cached `SHA256SUMS` that no longer verifies is not a cache
    /// hit, however healthy the extracted binary and its marker look.
    ///
    /// What the re-check buys, stated exactly (F3, review round 2): it
    /// re-establishes across a restart that the manifest this process is
    /// about to trust really was signed by the built-in key, FOR THIS
    /// VERSION. It says nothing about the cached `.bin`, whose marker is
    /// generated locally and detects only accidental corruption under the
    /// standing assumption that the state directory is trusted against its
    /// own account. Without this test, deleting the re-check would leave
    /// every cache test green while a process restart silently stopped
    /// re-checking the signature and the version binding.
    #[tokio::test]
    async fn a_cached_payload_whose_signature_no_longer_verifies_is_refetched() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache = tempfile::tempdir().unwrap();
        let binary = release
            .source(cache.path())
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();
        let after_warmup = release.requests().len();

        let cached_signature = generation(cache.path(), &release.base_url).join("SHA256SUMS");
        let mut tampered = std::fs::read(&cached_signature).unwrap();
        tampered[0] = if tampered[0] == b'a' { b'b' } else { b'a' };
        std::fs::write(&cached_signature, &tampered).unwrap();

        let recovered = release
            .source(cache.path())
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();
        assert_eq!(recovered, binary);
        assert_eq!(
            std::fs::read(&recovered).unwrap(),
            expected_member("tmux", "x86_64-unknown-linux-musl")
        );
        let requests = release.requests();
        assert_eq!(
            requests.len(),
            after_warmup + 3,
            "both control files and the asset must be fetched again: {requests:?}"
        );
    }

    /// Spec: two callers racing for the same asset produce ONE download.
    ///
    /// This is the per-asset mutex's reason to exist: without it both
    /// callers would write the same `.part` file concurrently, and the
    /// loser's rename would publish bytes the winner had already replaced.
    #[tokio::test]
    async fn concurrent_requests_for_one_asset_download_it_once() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache = tempfile::tempdir().unwrap();
        let source = release.source(cache.path());

        let (first, second) = tokio::join!(
            source.path(PayloadKind::Tmux, PayloadArch::X86_64),
            source.path(PayloadKind::Tmux, PayloadArch::X86_64),
        );

        assert_eq!(first.unwrap(), second.unwrap());
        let asset = assets::tmux_name(PayloadArch::X86_64);
        let requests = release.requests();
        let downloads = requests.iter().filter(|name| *name == asset).count();
        assert_eq!(downloads, 1, "{requests:?}");
    }

    /// Spec: requests for DIFFERENT assets overlap — the locks are per
    /// asset, not one lock over the source.
    ///
    /// A single global mutex would pass the same-asset test above while
    /// serialising a cold host's `farhelm` and `tmux` downloads, which is
    /// exactly the latency this design set out to avoid. Proven by holding
    /// both responses open at the server and requiring both requests to have
    /// ARRIVED before either is released — no sleeps, no timing guesses.
    #[tokio::test(flavor = "multi_thread")]
    async fn requests_for_different_assets_overlap() {
        let farhelm_asset = assets::archive_name(assets::farhelm_archive_for(PayloadArch::X86_64));
        let tmux_asset = assets::tmux_name(PayloadArch::X86_64);
        let gates: Vec<_> = (0..2)
            .map(|_| {
                (
                    Arc::new(tokio::sync::Notify::new()),
                    Arc::new(tokio::sync::Notify::new()),
                )
            })
            .collect();
        let release = FixtureRelease::start(vec![
            (
                farhelm_asset.as_str(),
                Override::Hold {
                    arrived: Arc::clone(&gates[0].0),
                    gate: Arc::clone(&gates[0].1),
                },
            ),
            (
                tmux_asset,
                Override::Hold {
                    arrived: Arc::clone(&gates[1].0),
                    gate: Arc::clone(&gates[1].1),
                },
            ),
        ])
        .await;
        let cache = tempfile::tempdir().unwrap();
        let source = Arc::new(release.source(cache.path()));

        let farhelm = tokio::spawn({
            let source = Arc::clone(&source);
            async move { source.path(PayloadKind::Farhelm, PayloadArch::X86_64).await }
        });
        let tmux = tokio::spawn({
            let source = Arc::clone(&source);
            async move { source.path(PayloadKind::Tmux, PayloadArch::X86_64).await }
        });

        // Both must be in flight at the server before either is answered.
        // A source that serialised its assets would never reach the second
        // arrival, and this times out into a failure rather than hanging.
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            gates[0].0.notified().await;
            gates[1].0.notified().await;
        })
        .await
        .expect("both asset requests must reach the server before either is answered");

        gates[0].1.notify_one();
        gates[1].1.notify_one();
        assert_eq!(
            std::fs::read(farhelm.await.unwrap().unwrap()).unwrap(),
            expected_member("farhelm", "x86_64-unknown-linux-musl")
        );
        assert_eq!(
            std::fs::read(tmux.await.unwrap().unwrap()).unwrap(),
            expected_member("tmux", "x86_64-unknown-linux-musl")
        );
    }

    /// Spec: a connect failure is retried exactly once, and the retry's
    /// result is used.
    ///
    /// Attempt COUNT is the contract, and it is invisible from outside
    /// without the transport seam: a test against a closed port cannot tell
    /// one attempt from three.
    #[tokio::test]
    async fn a_connect_failure_is_retried_once_and_then_succeeds() {
        let cache = tempfile::tempdir().unwrap();
        let asset = assets::tmux_name(PayloadArch::X86_64);
        let (source, attempts) = scripted_source(
            cache.path(),
            vec![
                Scripted::Connect,
                Scripted::Ok(fixture_bytes("SHA256SUMS")),
                Scripted::Ok(fixture_bytes("SHA256SUMS.minisig")),
                Scripted::Ok(fixture_bytes(asset)),
            ],
        );

        let binary = source
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(binary).unwrap(),
            expected_member("tmux", "x86_64-unknown-linux-musl")
        );
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            4,
            "three fetches, one of them retried once"
        );
    }

    /// Spec: the retry is ONE retry, not a loop — a second connect failure
    /// gives up — and a non-connect failure is not retried at all.
    ///
    /// Retrying a mid-body failure would re-pay a multi-megabyte download
    /// for a failure that is rarely transient, and an unbounded retry loop
    /// would turn an outage into a hammering client.
    #[tokio::test]
    async fn retries_stop_after_one_attempt_and_never_start_for_other_failures() {
        for (script, expected_attempts, label) in [
            (vec![Scripted::Connect, Scripted::Connect], 2, "connect"),
            (vec![Scripted::Other], 1, "non-connect"),
        ] {
            let cache = tempfile::tempdir().unwrap();
            let (source, attempts) = scripted_source(cache.path(), script);
            let error = source
                .path(PayloadKind::Tmux, PayloadArch::X86_64)
                .await
                .unwrap_err();
            assert!(
                format!("{error:#}").starts_with("add host needs farhelm's"),
                "{label}: {error:#}"
            );
            assert_eq!(
                attempts.load(Ordering::SeqCst),
                expected_attempts,
                "{label}"
            );
        }
    }

    /// Spec: a release server this machine cannot reach names the URL and
    /// the transport failure, so the operator can tell "no network" apart
    /// from "the release is missing" without reading logs.
    ///
    /// Deliberately a REAL socket failure rather than the scripted
    /// transport, because the rendered text carries reqwest's own error chain
    /// and nothing else proves that reads well. Attempt COUNTS are the
    /// scripted seam's job, not this one's.
    ///
    /// The failure comes from a listener that accepts and hangs up rather
    /// than from a port deliberately left closed (F11, review round 2): the
    /// earlier version bound an ephemeral port, dropped the listener, and
    /// then connected, which hands the port back to the OS for exactly the
    /// window in which anything else on the machine could claim it — turning
    /// the test into a request to an unrelated service or a wait for the
    /// 60-second read timeout. Holding the port removes the race entirely.
    #[tokio::test]
    async fn an_unreachable_release_server_names_the_base_url() {
        let dead = ClosingListener::start().await;
        let base_url = dead.base_url();
        let cache = tempfile::tempdir().unwrap();
        let source = ReleasePayloadSource::new(
            base_url.clone(),
            cache.path().to_path_buf(),
            FIXTURE_VERSION,
            test_pubkey(),
            test_client(),
        );

        let error = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            source.path(PayloadKind::Farhelm, PayloadArch::X86_64),
        )
        .await
        .expect("a hung-up connection must fail promptly, not wait out a timeout")
        .unwrap_err();
        let expected = format!(
            "add host needs farhelm's v{FIXTURE_VERSION} release assets from {base_url}, and this \
             machine could not reach it: "
        );
        assert!(
            format!("{error:#}").starts_with(&expected),
            "expected a message starting {expected:?}, got {error:#}"
        );
    }

    /// Spec: a 404 on `SHA256SUMS` reports D17's single message — a release
    /// that does not exist and a release still uploading its assets look
    /// identical to one request, so this deliberately does not guess which
    /// one it is.
    #[tokio::test]
    async fn a_missing_sha256sums_reports_the_not_published_or_still_publishing_message() {
        let release = FixtureRelease::start(vec![("SHA256SUMS", Override::NotFound)]).await;
        let cache = tempfile::tempdir().unwrap();
        let error = release
            .source(cache.path())
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            format!(
                "no SHA256SUMS for v{FIXTURE_VERSION} at {} (HTTP 404): the release is not \
                 published or is still publishing; retry in a few minutes, or pass --payload-dir",
                release.base_url
            )
        );
    }

    /// Spec: a failed checksum fetch is NOT remembered — the next "add host"
    /// tries again and succeeds.
    ///
    /// `OnceCell::get_or_try_init` is what makes that true, and nothing else
    /// in the suite would notice if it became `get_or_init`: every other
    /// refusal test stops after one call, so a helm poisoned for the rest of
    /// its uptime by one transient 500 would look perfectly healthy here.
    #[tokio::test]
    async fn a_failed_checksum_fetch_is_retried_by_the_next_request() {
        let release = FixtureRelease::start(vec![(
            "SHA256SUMS",
            Override::FailFirst(Arc::new(AtomicUsize::new(1))),
        )])
        .await;
        let cache = tempfile::tempdir().unwrap();
        let source = release.source(cache.path());

        let error = source
            .path(PayloadKind::Tmux, PayloadArch::Aarch64)
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("HTTP 500"), "{error:#}");

        let binary = source
            .path(PayloadKind::Tmux, PayloadArch::Aarch64)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(binary).unwrap(),
            expected_member("tmux", "aarch64-unknown-linux-musl")
        );
        let requests = release.requests();
        assert_eq!(
            requests.iter().filter(|name| *name == "SHA256SUMS").count(),
            2,
            "the second call must fetch the checksum file again: {requests:?}"
        );
    }

    /// Spec: a `SHA256SUMS` whose signature does not verify is refused
    /// outright — the single check that stands between "some server served
    /// bytes" and "these are farhelm's release binaries".
    #[tokio::test]
    async fn a_sha256sums_that_does_not_verify_is_refused() {
        // One flipped hex digit: still well-formed sha256sum output, so the
        // refusal can only come from the signature check.
        let mut tampered = fixture_bytes("SHA256SUMS");
        tampered[0] = if tampered[0] == b'a' { b'b' } else { b'a' };
        let release = FixtureRelease::start(vec![("SHA256SUMS", Override::Body(tampered))]).await;
        let cache = tempfile::tempdir().unwrap();

        let error = release
            .source(cache.path())
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            format!(
                "refusing v{FIXTURE_VERSION} assets: SHA256SUMS.minisig does not verify with \
                 farhelm's built-in key"
            )
        );
    }

    /// Spec: a signature in minisign's legacy, non-prehashed format is
    /// refused like any other bad signature.
    ///
    /// `verify(.., allow_legacy = false)` is a deliberate narrowing of the
    /// accepted trust policy, and every other fixture uses the modern
    /// format — so without a committed legacy signature, flipping that flag
    /// to `true` would broaden what farhelm accepts with the suite still
    /// green.
    #[tokio::test]
    async fn a_legacy_format_signature_is_refused() {
        let release = FixtureRelease::start(vec![(
            "SHA256SUMS.minisig",
            Override::Body(fixture_bytes(
                "variants/legacy-signature/SHA256SUMS.minisig",
            )),
        )])
        .await;
        let cache = tempfile::tempdir().unwrap();

        let error = release
            .source(cache.path())
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            format!(
                "refusing v{FIXTURE_VERSION} assets: SHA256SUMS.minisig does not verify with \
                 farhelm's built-in key"
            )
        );
    }

    /// Spec: a correctly signed `SHA256SUMS` for ANOTHER version is refused,
    /// naming the version it was actually signed for.
    ///
    /// This is the downgrade attack the trusted comment exists to stop: the
    /// served manifest, its signature, and its assets are all authentic —
    /// they are simply an older release's, replayed at this version's URL by
    /// whoever controls that URL. Nothing in the signed BYTES distinguishes
    /// them, so if this test ever passes for the wrong reason the version
    /// binding has been lost.
    #[tokio::test]
    async fn a_sha256sums_signed_for_another_version_is_refused() {
        let release = FixtureRelease::start(vec![
            (
                "SHA256SUMS",
                Override::Body(fixture_bytes("variants/other-version/SHA256SUMS")),
            ),
            (
                "SHA256SUMS.minisig",
                Override::Body(fixture_bytes("variants/other-version/SHA256SUMS.minisig")),
            ),
        ])
        .await;
        let cache = tempfile::tempdir().unwrap();

        let error = release
            .source(cache.path())
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            format!(
                "refusing v{FIXTURE_VERSION} assets: SHA256SUMS.minisig was signed for farhelm \
                 v0.0.2, not this version"
            )
        );
    }

    /// Spec: a cached manifest signed for another version does not survive a
    /// restart as a cache hit.
    ///
    /// The cached-copy re-check applies the SAME rule as the download path.
    /// If it only checked the signature, a helm that cached a replayed
    /// manifest once would keep serving its binaries from cache forever,
    /// with the network check that would have caught it never running again.
    #[tokio::test]
    async fn a_cached_manifest_from_another_version_is_not_a_cache_hit() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache = tempfile::tempdir().unwrap();
        release
            .source(cache.path())
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();

        let generation = generation(cache.path(), &release.base_url);
        std::fs::write(
            generation.join("SHA256SUMS"),
            fixture_bytes("variants/other-version/SHA256SUMS"),
        )
        .unwrap();
        std::fs::write(
            generation.join("SHA256SUMS.minisig"),
            fixture_bytes("variants/other-version/SHA256SUMS.minisig"),
        )
        .unwrap();
        let after_warmup = release.requests().len();

        release
            .source(cache.path())
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();
        let requests = release.requests();
        assert!(
            requests.len() > after_warmup,
            "a wrong-version cached manifest must force a refetch: {requests:?}"
        );
    }

    /// Spec: an asset whose bytes do not hash to its signed entry is refused,
    /// naming both digests, and the rejected bytes do not survive under the
    /// asset's own name. Covers the case the signature alone cannot: a
    /// genuine, correctly signed checksum file with a substituted asset
    /// behind it.
    #[tokio::test]
    async fn an_asset_whose_bytes_do_not_match_sha256sums_is_refused() {
        let asset = assets::archive_name(assets::farhelm_archive_for(PayloadArch::X86_64));
        let tampered = b"not the release archive".to_vec();
        let release =
            FixtureRelease::start(vec![(asset.as_str(), Override::Body(tampered.clone()))]).await;
        let cache = tempfile::tempdir().unwrap();

        let error = release
            .source(cache.path())
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        let mut hasher = Sha256::new();
        hasher.update(&tampered);
        let actual = hex(&hasher.finalize());
        let expected = parse_sums(std::str::from_utf8(&fixture_bytes("SHA256SUMS")).unwrap())
            .unwrap()
            .remove(&asset)
            .unwrap();
        assert_eq!(
            format!("{error:#}"),
            format!("refusing {asset}: SHA-256 {actual} does not match SHA256SUMS ({expected})")
        );
        let generation = generation(cache.path(), &release.base_url);
        assert!(
            !generation.join(&asset).exists() && !generation.join(format!("{asset}.part")).exists(),
            "unverified bytes must not survive, staged or published"
        );
    }

    /// Spec: an asset the signed `SHA256SUMS` says nothing about is refused
    /// rather than downloaded unverified — the case a release that shipped
    /// an incomplete checksum file would produce.
    ///
    /// Driven by `variants/without-tmux`, a separately signed checksum file,
    /// because serving an edited one would fail the signature check first
    /// and never reach this path.
    #[tokio::test]
    async fn an_asset_absent_from_sha256sums_is_refused() {
        let release = FixtureRelease::start(vec![
            (
                "SHA256SUMS",
                Override::Body(fixture_bytes("variants/without-tmux/SHA256SUMS")),
            ),
            (
                "SHA256SUMS.minisig",
                Override::Body(fixture_bytes("variants/without-tmux/SHA256SUMS.minisig")),
            ),
        ])
        .await;
        let cache = tempfile::tempdir().unwrap();

        let error = release
            .source(cache.path())
            .path(PayloadKind::Tmux, PayloadArch::Aarch64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            format!(
                "SHA256SUMS for v{FIXTURE_VERSION} has no entry for {}",
                assets::tmux_name(PayloadArch::Aarch64)
            )
        );
    }

    /// Spec: a listed-but-absent asset names the asset, not the release —
    /// distinct from D17's "no SHA256SUMS" message, because a signed
    /// checksum file with a missing asset behind it is a broken release, not
    /// an unpublished one.
    #[tokio::test]
    async fn a_missing_release_asset_names_the_asset() {
        let asset = assets::archive_name(assets::farhelm_archive_for(PayloadArch::Aarch64));
        let release = FixtureRelease::start(vec![(asset.as_str(), Override::NotFound)]).await;
        let cache = tempfile::tempdir().unwrap();

        let error = release
            .source(cache.path())
            .path(PayloadKind::Farhelm, PayloadArch::Aarch64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            format!("release v{FIXTURE_VERSION} has no asset named {asset} (HTTP 404)")
        );
    }

    /// Spec: an asset served as a redirect is followed, and the FINAL body
    /// is what gets checksum-verified and extracted.
    ///
    /// Production depends on this: GitHub answers every release asset URL
    /// with a redirect to its object store, so a client built without
    /// redirect following would fail every real download while passing every
    /// direct-response test in this file.
    #[tokio::test]
    async fn a_redirected_asset_is_followed_and_still_verified() {
        let asset = assets::tmux_name(PayloadArch::Aarch64);
        let release = FixtureRelease::start(vec![
            (asset, Override::Redirect("/object-store-copy".to_string())),
            ("object-store-copy", Override::Body(fixture_bytes(asset))),
        ])
        .await;
        let cache = tempfile::tempdir().unwrap();

        let binary = release
            .source(cache.path())
            .path(PayloadKind::Tmux, PayloadArch::Aarch64)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(binary).unwrap(),
            expected_member("tmux", "aarch64-unknown-linux-musl")
        );
        assert!(
            release
                .requests()
                .iter()
                .any(|name| name == "object-store-copy"),
            "the redirect target must actually have been fetched"
        );
    }

    /// Spec: an archive carrying more than one member named `farhelm` is
    /// refused instead of one being picked — a dist archive with two would
    /// mean the release build itself went wrong, and guessing which binary
    /// to install on somebody's host is not a recovery.
    ///
    /// Needs `variants/two-member`'s own signed `SHA256SUMS`: an archive
    /// substituted without one would be rejected for its checksum long
    /// before anything opened it.
    #[tokio::test]
    async fn an_archive_without_exactly_one_matching_member_is_refused() {
        let asset = assets::archive_name(assets::farhelm_archive_for(PayloadArch::X86_64));
        let release = FixtureRelease::start(vec![
            (
                "SHA256SUMS",
                Override::Body(fixture_bytes("variants/two-member/SHA256SUMS")),
            ),
            (
                "SHA256SUMS.minisig",
                Override::Body(fixture_bytes("variants/two-member/SHA256SUMS.minisig")),
            ),
            (
                asset.as_str(),
                Override::Body(fixture_bytes(
                    "variants/two-member/farhelm-x86_64-unknown-linux-musl.tar.gz",
                )),
            ),
        ])
        .await;
        let cache = tempfile::tempdir().unwrap();

        let error = release
            .source(cache.path())
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            format!("{asset} contains 2 members named farhelm; expected exactly one")
        );
    }

    /// Spec: a `SHA256SUMS` whose announced `Content-Length` exceeds the cap
    /// is refused before its body is read.
    ///
    /// This is the cheap half of the memory boundary: the fixture server
    /// buffers this body, so axum announces its length and the refusal comes
    /// from the header check. The streaming check below is what covers a
    /// server that announces nothing.
    #[tokio::test]
    async fn an_oversized_sha256sums_is_refused_from_its_content_length() {
        let release = FixtureRelease::start(vec![(
            "SHA256SUMS",
            Override::Body(vec![b'a'; SUMS_MAX_BYTES + 1]),
        )])
        .await;
        let cache = tempfile::tempdir().unwrap();

        let error = release
            .source(cache.path())
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            format!(
                "refusing v{FIXTURE_VERSION} assets: SHA256SUMS at {} is larger than 64 KiB and \
                 cannot be a checksum file",
                release.base_url
            )
        );
    }

    /// Spec: a chunked `SHA256SUMS` is refused as soon as the cap is
    /// crossed, WITHOUT reading the rest.
    ///
    /// The cap exists to bound memory before anything about the server has
    /// been authenticated, so "refuses eventually" is not the property that
    /// matters — an implementation that buffered a gigabyte and checked at
    /// the end would satisfy that. The server holds the tail behind a gate
    /// this test never opens: `released` staying false is the evidence that
    /// farhelm stopped reading.
    ///
    /// The whole call is under a timeout for exactly that reason: a
    /// regression to read-until-EOF would block on the withheld tail
    /// forever, and a hanging test reports nothing useful. Expiry is a
    /// failure, not a skip.
    #[tokio::test]
    async fn an_oversized_chunked_sha256sums_stops_reading_at_the_cap() {
        let gate = Arc::new(tokio::sync::Notify::new());
        let released = Arc::new(AtomicBool::new(false));
        let release = FixtureRelease::start(vec![(
            "SHA256SUMS",
            Override::Gated {
                prefix: vec![b'a'; SUMS_MAX_BYTES + 1],
                suffix: vec![b'b'; 1024],
                gate: Arc::clone(&gate),
                released: Arc::clone(&released),
            },
        )])
        .await;
        let cache = tempfile::tempdir().unwrap();

        let source = release.source(cache.path());
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            source.path(PayloadKind::Farhelm, PayloadArch::X86_64),
        )
        .await
        .expect("the cap must be enforced mid-stream, not after end-of-file")
        .unwrap_err();
        assert!(
            format!("{error:#}").contains("is larger than 64 KiB"),
            "{error:#}"
        );
        assert!(
            !released.load(Ordering::SeqCst),
            "the refusal must come before the withheld tail is requested"
        );
    }

    /// Spec: `append_capped` refuses an oversized chunk WITHOUT copying it.
    ///
    /// The end-to-end test above can only observe that a refusal happened;
    /// this observes the property the cap actually exists for — the bytes
    /// never enter the buffer — which an implementation that appended first
    /// and checked afterwards would fail while still passing every network
    /// test in this file.
    #[test]
    fn append_capped_refuses_a_chunk_before_copying_it() {
        let mut body = Vec::new();
        assert!(append_capped(&mut body, b"under", 16));
        assert_eq!(body, b"under");

        assert!(
            !append_capped(&mut body, &[b'x'; 64], 16),
            "a chunk that would cross the cap must be refused"
        );
        assert_eq!(
            body, b"under",
            "the refused chunk must not have been copied"
        );

        // Exactly at the limit is allowed; one past it is not.
        let mut body = Vec::new();
        assert!(append_capped(&mut body, &[b'x'; 16], 16));
        assert_eq!(body.len(), 16);
        let mut body = Vec::new();
        assert!(!append_capped(&mut body, &[b'x'; 17], 16));
        assert!(body.is_empty());
    }

    /// Spec: a cache entry whose marker no longer matches its binary is
    /// discarded and refetched rather than trusted or reported as an error.
    ///
    /// This is the recovery path for a helm killed mid-extraction, which is
    /// otherwise a permanently broken "add host" that only a manual `rm -rf`
    /// of helm state would clear.
    #[tokio::test]
    async fn a_corrupt_extraction_marker_is_discarded_and_the_asset_refetched() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache = tempfile::tempdir().unwrap();
        let source = release.source(cache.path());
        let binary = source
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap();
        let after_warmup = release.requests().len();

        let asset = assets::archive_name(assets::farhelm_archive_for(PayloadArch::X86_64));
        std::fs::write(
            generation(cache.path(), &release.base_url).join(format!("{asset}.bin.sha256")),
            "not a digest\n",
        )
        .unwrap();

        let recovered = source
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap();
        assert_eq!(recovered, binary);
        assert_eq!(
            std::fs::read(&recovered).unwrap(),
            expected_member("farhelm", "x86_64-unknown-linux-musl")
        );
        // Exactly one more request: the asset itself. The checksum file is
        // still held from the first call, so recovery does not re-fetch it.
        let requests = release.requests();
        assert_eq!(requests.len(), after_warmup + 1, "{requests:?}");
    }

    /// Spec: a cached binary that cannot be hashed is an invalid entry, not
    /// a permanent failure — it is discarded and refetched.
    ///
    /// Planting a DIRECTORY where `<asset>.bin` belongs reproduces this
    /// deterministically (opening it succeeds, reading it does not) and
    /// stands in for the real-world versions: a file whose permissions
    /// changed, an ACL, a damaged filesystem. Propagating the I/O error
    /// instead would leave "add host" broken until somebody deleted helm
    /// state by hand.
    #[tokio::test]
    async fn an_unhashable_cached_binary_is_discarded_and_refetched() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache = tempfile::tempdir().unwrap();
        let source = release.source(cache.path());
        let binary = source
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();

        std::fs::remove_file(&binary).unwrap();
        std::fs::create_dir(&binary).unwrap();
        std::fs::write(binary.join("planted"), b"debris").unwrap();

        let recovered = source
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();
        assert_eq!(recovered, binary);
        assert_eq!(
            std::fs::read(&recovered).unwrap(),
            expected_member("tmux", "x86_64-unknown-linux-musl")
        );
    }

    /// Spec: staging files abandoned by an earlier process are removed on
    /// first use, and only staging files are.
    ///
    /// A `SIGKILL` runs no destructor, so whatever was mid-write survives.
    /// Because every staging name this source writes is FIXED, recovery is a
    /// matter of deleting known patterns rather than guessing at random
    /// temporary names — which is exactly why the cache does not use random
    /// ones. The published files beside them must be left alone.
    #[tokio::test]
    async fn abandoned_staging_files_are_removed_on_first_use() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache = tempfile::tempdir().unwrap();
        let generation = generation(cache.path(), &release.base_url);
        std::fs::create_dir_all(&generation).unwrap();
        let asset = assets::tmux_name(PayloadArch::X86_64);
        let orphans = [
            generation.join(format!("{asset}.part")),
            generation.join(format!("{asset}.bin.part")),
            generation.join(".tmpAbCdEf"),
        ];
        for orphan in &orphans {
            std::fs::write(orphan, b"abandoned").unwrap();
        }
        let bystander = generation.join("unrelated-file");
        std::fs::write(&bystander, b"keep me").unwrap();

        release
            .source(cache.path())
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();

        for orphan in &orphans {
            assert!(
                !orphan.exists(),
                "{} must have been cleaned up",
                orphan.display()
            );
        }
        assert!(
            bystander.is_file(),
            "housekeeping must not touch files it does not own"
        );
    }

    /// Spec: generation directories belonging to OTHER farhelm versions are
    /// removed on first use; same-version generations and anything that is
    /// not a generation directory are left alone.
    ///
    /// Each generation holds a release archive plus its extracted binary, so
    /// without this an upgraded helm keeps every version it has ever run.
    /// The bounds matter as much as the cleanup: same-version siblings
    /// belong to other base URLs a running helm may still be using, and the
    /// name-shape check is what keeps this from becoming an arbitrary
    /// recursive delete inside helm state.
    #[tokio::test]
    async fn payload_cache_generations_from_older_versions_are_pruned() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache = tempfile::tempdir().unwrap();
        let plant = |name: &str| {
            let directory = cache.path().join(name);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("payload.bin"), b"old").unwrap();
            directory
        };
        let stale = plant("v0.0.1-0123456789ab");
        let stale_prerelease = plant("v0.0.1-rc.1-0123456789ab");
        let same_version_other_url = plant(&format!("v{FIXTURE_VERSION}-ffffffffffff"));
        let not_a_generation = plant("scratch");
        // A symlink WEARING a generation name, pointing at a directory that
        // must survive. Following it would delete somebody else's tree; the
        // `symlink_metadata` check is the only thing standing in the way.
        let outside = cache.path().join("not-a-generation-at-all");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("precious"), b"keep").unwrap();
        let disguised = cache.path().join("v0.0.2-abcdefabcdef");
        std::os::unix::fs::symlink(&outside, &disguised).unwrap();

        release
            .source(cache.path())
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();

        assert!(!stale.exists(), "an older version's cache must be pruned");
        assert!(
            !stale_prerelease.exists(),
            "a prerelease generation is an older version too"
        );
        assert!(
            same_version_other_url.is_dir(),
            "another base URL's current-version cache must survive"
        );
        assert!(
            not_a_generation.is_dir(),
            "housekeeping must only touch directories it recognises"
        );
        assert!(
            outside.join("precious").is_file(),
            "a symlink named like a generation must never be followed"
        );
        assert!(
            std::fs::symlink_metadata(&disguised).is_ok(),
            "the symlink itself is left alone rather than unlinked"
        );
    }

    /// Spec: the cache directory is `v{version}-<12 hex of sha256(base_url)>`
    /// below the payload root.
    ///
    /// Both halves are load-bearing: the version so an upgraded helm never
    /// serves the previous release's binaries out of a shared cache, and the
    /// URL hash so a fixture server, a mirror, or a prerelease URL cannot
    /// read or poison what the real GitHub release wrote.
    #[tokio::test]
    async fn the_cache_directory_is_keyed_by_version_and_base_url() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache_root = tempfile::tempdir().unwrap();
        release
            .source(cache_root.path())
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();

        let mut hasher = Sha256::new();
        hasher.update(release.base_url.as_str().as_bytes());
        let digest = hex(&hasher.finalize());
        let expected = format!("v{FIXTURE_VERSION}-{}", &digest[..12]);

        let entries: Vec<String> = std::fs::read_dir(cache_root.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec![expected]);
    }

    /// Spec: a base URL without a trailing slash still addresses assets
    /// BELOW it, not beside it.
    ///
    /// `--release-base-url` is typed by hand, and `Url::join` would
    /// otherwise silently replace the last path segment — turning
    /// `…/download/v1.2.3` into `…/download/farhelm-….tar.gz` and fetching
    /// from the wrong place with no error to explain it.
    #[test]
    fn a_base_url_without_a_trailing_slash_is_normalised() {
        let normalised =
            normalise_base_url(Url::parse("https://example.invalid/releases/v1.2.3").unwrap());
        assert_eq!(
            normalised.join("farhelm.tar.gz").unwrap().as_str(),
            "https://example.invalid/releases/v1.2.3/farhelm.tar.gz"
        );
    }

    /// Spec: `parse_sums` accepts real `sha256sum` output in both its modes
    /// and rejects anything else outright.
    ///
    /// Rejecting rather than skipping unparsable lines is the point: a
    /// truncated or HTML-wrapped file that parsed to a partial entry set
    /// would surface downstream as "no entry for {asset}", pointing the
    /// operator at a missing asset instead of at a broken download.
    #[test]
    fn parse_sums_accepts_sha256sum_output_and_rejects_everything_else() {
        let digest = "0".repeat(64);
        let parsed = parse_sums(&format!(
            "{digest}  farhelm-x86_64-unknown-linux-musl.tar.gz\n\
             {digest} *tmux-x86_64-unknown-linux-musl\n"
        ))
        .unwrap();
        assert_eq!(
            parsed.keys().collect::<Vec<_>>(),
            vec![
                "farhelm-x86_64-unknown-linux-musl.tar.gz",
                "tmux-x86_64-unknown-linux-musl"
            ]
        );

        assert!(parse_sums("").is_none(), "an empty file is not a sums file");
        assert!(
            parse_sums("<html><body>404</body></html>\n").is_none(),
            "an error page is not a sums file"
        );
        assert!(
            parse_sums(&format!("{digest}\n")).is_none(),
            "a digest with no filename is not a sums file"
        );
        assert!(
            parse_sums("cafe  farhelm.tar.gz\n").is_none(),
            "a truncated digest is not a sums file"
        );
    }

    /// Spec: only `v<other SemVer version>-<12 lowercase hex>` names are
    /// eligible for pruning — PRERELEASES of other versions included, and
    /// nothing whose version half is absent or unparseable.
    ///
    /// Worth a unit test of its own because this predicate is the sole guard
    /// on a recursive delete inside the helm's state directory: every name it
    /// wrongly accepts is a directory somebody loses, and every name it
    /// wrongly rejects is a cache generation that leaks forever.
    ///
    /// Both failure directions are real regressions, not hypotheticals
    /// (F5, review round 2). Splitting on the LAST hyphen — which reads
    /// correctly until you remember SemVer prereleases contain hyphens —
    /// made `v0.0.1-rc.1-<digest>` unparseable so `-rc.N` generations were
    /// never pruned, while happily accepting `v-<digest>`, whose version is
    /// the empty string, as something to delete recursively.
    #[test]
    fn only_foreign_version_generation_names_are_prunable() {
        // A fixed stand-in for "current", deliberately NOT the crate's own
        // `VERSION`: this predicate takes its notion of current from the
        // caller now (see its docstring), precisely so a unit test like
        // this one cannot collide with whatever the real workspace version
        // happens to be. It very nearly did: proving the fixture-version
        // fix means bumping the workspace to `9.9.9`, and this test used to
        // hardcode `9.9.9` below as an example of "some OTHER version" —
        // reading `VERSION` for "current" would have made that literal
        // equal to current instead of foreign the moment the proof ran.
        let current = "5.5.5";
        for stale in [
            "v0.0.1-0123456789ab",
            "v9.9.9-ffffffffffff",
            // A prerelease of another version is still another version.
            "v0.0.1-rc.1-0123456789ab",
            "v9.9.9-alpha.2+build.7-0123456789ab",
            // A prerelease of THIS version is not this version (SemVer says
            // `1.2.3-rc.1 != 1.2.3`), so its generation is prunable too.
            &format!("v{current}-rc.1-0123456789ab"),
        ] {
            assert!(
                is_stale_generation(stale, current),
                "{stale} must be prunable"
            );
        }

        assert!(
            !is_stale_generation(&format!("v{current}-0123456789ab"), current),
            "this build's own generations belong to other base URLs"
        );
        for kept in [
            "payloads",
            "v0.0.1",
            "v0.0.1-0123456789",     // digest too short
            "v0.0.1-0123456789abcd", // digest too long
            "v0.0.1-0123456789AB",   // digests are lowercase
            "0.0.1-0123456789ab",    // no leading v
            "v-0123456789ab",        // empty version
            "v..-0123456789ab",      // not a version at all
            "vnot.a.version-0123456789ab",
            "0123456789ab",
            "..",
            "",
        ] {
            assert!(!is_stale_generation(kept, current), "{kept} must be kept");
        }
    }

    /// Spec: the key compiled into shipped binaries equals the committed
    /// production public key file.
    ///
    /// The real oracle for `MINISIGN_PUBKEY`. A decodability check alone is
    /// nearly worthless here — most single-character substitutions still
    /// parse as a valid minisign key — and every behavioural test in this
    /// file injects the fixture key instead, so a mistyped constant would
    /// otherwise surface only on a real "add host" against a real release.
    /// The maintainer cross-checks the same line against the key held as
    /// `MINISIGN_SECRET_KEY`.
    #[test]
    fn the_built_in_public_key_matches_the_committed_release_key_file() {
        const KEY_FILE: &str = include_str!("farhelm-release.pub");
        assert_eq!(
            KEY_FILE
                .lines()
                .nth(1)
                .expect("a minisign public key file is a comment line then the key line")
                .trim(),
            MINISIGN_PUBKEY
        );
        assert!(
            minisign_verify::PublicKey::from_base64(MINISIGN_PUBKEY).is_ok(),
            "the constant must also decode as a minisign key"
        );
    }

    /// Spec: the producer's signing command and the consumer's expectation
    /// agree on ONE tag convention.
    ///
    /// This is the whole of F1 (review round 2) in one assertion. A release
    /// tag is `vX.Y.Z`, so `sign-sums` renders `farhelm $TAG`; documentation
    /// that said `farhelm v$TAG` would have produced `farhelm vv0.0.3` and a
    /// release every shipped helm refuses — with nothing in the suite to
    /// notice, because both halves were only ever described in prose. Here
    /// the producer's rendering is reconstructed from a realistic tag and
    /// compared against the string verification actually demands.
    #[test]
    fn signing_and_verification_agree_on_the_tag_convention() {
        // Deliberately the real production constant, not `FIXTURE_VERSION`:
        // this test pins the tag convention CI's `sign-sums` job and this
        // module's own verification actually agree on, which is a fact
        // about `VERSION`, not about whatever the fixtures are signed for.
        let tag = release_tag(VERSION);
        assert!(
            tag.starts_with('v') && tag[1..] == *VERSION,
            "a release tag is v + the crate version, not something else: {tag}"
        );
        // What `minisign -S -t "farhelm $TAG"` puts in the trusted comment.
        let produced = format!("farhelm {tag}");
        assert_eq!(produced, required_trusted_comment(VERSION));
        // The mistake this exists to catch, spelled out so it cannot be
        // reintroduced by "fixing" the convention in only one place.
        assert_ne!(
            format!("farhelm v{tag}"),
            required_trusted_comment(VERSION),
            "`farhelm v$TAG` doubles the v and must never be what we document"
        );
    }

    /// Spec: no committed fixture archive carries the generating account's
    /// identity in its tar headers.
    ///
    /// The repository must contain no local-environment detail, and tar
    /// records the creating user and group by NAME unless told otherwise —
    /// inside committed binary data, where no reviewer will see it. The
    /// regeneration recipe's ownership flags are easy to drop; this is what
    /// makes dropping them fail the suite instead of shipping a name.
    #[test]
    fn the_committed_archives_record_no_account_identity() {
        let mut checked = 0;
        for relative in [
            "farhelm-aarch64-apple-darwin.tar.gz",
            "farhelm-aarch64-unknown-linux-musl.tar.gz",
            "farhelm-desktop-aarch64-apple-darwin.tar.gz",
            "farhelm-x86_64-unknown-linux-musl.tar.gz",
            "variants/two-member/farhelm-x86_64-unknown-linux-musl.tar.gz",
        ] {
            let file = std::fs::File::open(Path::new(FIXTURE_DIR).join(relative)).unwrap();
            let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
            for entry in archive.entries().unwrap() {
                let entry = entry.unwrap();
                let header = entry.header();
                assert_eq!(header.uid().unwrap(), 0, "{relative} records a uid");
                assert_eq!(header.gid().unwrap(), 0, "{relative} records a gid");
                // The name fields are what actually leak an account: tar
                // writes them from the generating user unless `--numeric-owner`
                // suppresses them, and then they are absent or empty rather
                // than a placeholder.
                for (what, name) in [
                    ("user", header.username().unwrap()),
                    ("group", header.groupname().unwrap()),
                ] {
                    assert!(
                        name.unwrap_or_default().is_empty(),
                        "{relative} records a {what} name: {name:?}"
                    );
                }
                checked += 1;
            }
        }
        assert!(checked >= 5, "every committed archive must have been read");
    }

    /// Spec: a checksum mismatch renders on its own, and renders TOGETHER
    /// with a failure to delete the rejected bytes.
    ///
    /// Both facts are actionable and neither substitutes for the other: the
    /// download was wrong, and unverified bytes are still in the state
    /// directory. Asserted at the seam rather than end to end because the
    /// cleanup-failure branch is unreachable through the filesystem — the
    /// staging file has to be creatable for a download to reach a checksum
    /// comparison at all, so no planted directory or permission can produce
    /// a mismatch AND a failed removal in one run.
    #[test]
    fn a_checksum_refusal_carries_a_cleanup_failure_when_there_is_one() {
        let clean = checksum_refusal("asset.tar.gz", "aaaa", "bbbb", Ok(()));
        assert_eq!(
            format!("{clean:#}"),
            "refusing asset.tar.gz: SHA-256 aaaa does not match SHA256SUMS (bbbb)"
        );

        let dirty = checksum_refusal(
            "asset.tar.gz",
            "aaaa",
            "bbbb",
            Err(anyhow!(
                "removing /cache/asset.tar.gz.part: permission denied"
            )),
        );
        let rendered = format!("{dirty:#}");
        assert!(
            rendered.contains("does not match SHA256SUMS (bbbb)"),
            "the mismatch must survive: {rendered}"
        );
        assert!(
            rendered.contains("removing /cache/asset.tar.gz.part: permission denied"),
            "the cleanup failure must survive with its path: {rendered}"
        );
    }

    /// Spec: `remove_if_present` reports a removal it could not perform,
    /// naming the path.
    ///
    /// The companion to the seam test above: that one proves the two
    /// messages compose, this one proves a real filesystem refusal actually
    /// produces an error rather than being swallowed. A read-only parent
    /// directory is the portable way to make `unlink` fail as an ordinary
    /// user; root ignores directory permissions, so this asserts nothing
    /// there and says so.
    #[test]
    fn remove_if_present_reports_a_removal_it_could_not_perform() {
        if unsafe { libc::geteuid() } == 0 {
            println!("skipping: root is not subject to directory permissions");
            return;
        }
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().unwrap();
        let locked = root.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        let victim = locked.join("payload.part");
        std::fs::write(&victim, b"unverified").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();

        let error = remove_if_present(&victim).unwrap_err();
        let rendered = format!("{error:#}");

        // Restore write permission so the tempdir can clean itself up.
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            rendered.contains(&victim.display().to_string()),
            "the failure must name the path: {rendered}"
        );
    }

    /// Spec: a corrupted cached control file is REPAIRED from this process's
    /// authenticated copy, so it costs one recovery download and not one per
    /// request forever.
    ///
    /// Without the repair the miss path reuses the in-memory manifest,
    /// re-downloads the asset, and leaves the corrupt file exactly as it
    /// found it — so every later lookup in the same process misses again for
    /// the same reason. The second half of this test, the cache hit with
    /// zero requests, is what a non-repairing implementation fails.
    #[tokio::test]
    async fn a_corrupt_cached_control_file_is_repaired_from_memory() {
        let release = FixtureRelease::start(Vec::new()).await;
        let cache = tempfile::tempdir().unwrap();
        let source = release.source(cache.path());
        // Warm the in-memory manifest with one asset.
        source
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();
        let after_warmup = release.requests().len();

        let cached_sums = generation(cache.path(), &release.base_url).join("SHA256SUMS");
        std::fs::write(&cached_sums, b"not a checksum file at all\n").unwrap();

        // A DIFFERENT asset, so this is a genuine miss for its own reasons:
        // exactly one request, for the asset itself.
        let binary = source
            .path(PayloadKind::Tmux, PayloadArch::Aarch64)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(&binary).unwrap(),
            expected_member("tmux", "aarch64-unknown-linux-musl")
        );
        let requests = release.requests();
        assert_eq!(
            requests.len(),
            after_warmup + 1,
            "recovery must cost one asset download, not a control refetch: {requests:?}"
        );

        // The repair is what makes this a hit.
        let again = source
            .path(PayloadKind::Tmux, PayloadArch::Aarch64)
            .await
            .unwrap();
        assert_eq!(again, binary);
        let requests = release.requests();
        assert_eq!(
            requests.len(),
            after_warmup + 1,
            "the repaired control files must make the next lookup a cache hit: {requests:?}"
        );
    }

    /// Spec: a cached `<asset>.bin` that is not a regular file is a
    /// discardable miss, and hashing never touches it.
    ///
    /// Both plants would hang forever if the file type were not checked
    /// first: opening a FIFO blocks until a writer appears, and hashing a
    /// symlink to `/dev/zero` never reaches end-of-file. Either one pins a
    /// blocking worker and leaves "add host" waiting with nothing to report,
    /// so the timeout here is the assertion — its expiry IS the bug.
    #[tokio::test]
    async fn a_cached_binary_that_is_not_a_regular_file_is_discarded() {
        for plant in ["fifo", "symlink"] {
            let release = FixtureRelease::start(Vec::new()).await;
            let cache = tempfile::tempdir().unwrap();
            let source = release.source(cache.path());
            let binary = source
                .path(PayloadKind::Tmux, PayloadArch::X86_64)
                .await
                .unwrap();

            std::fs::remove_file(&binary).unwrap();
            match plant {
                "fifo" => {
                    let raw =
                        std::ffi::CString::new(binary.as_os_str().as_encoded_bytes()).unwrap();
                    assert_eq!(
                        unsafe { libc::mkfifo(raw.as_ptr(), 0o600) },
                        0,
                        "planting a FIFO must succeed"
                    );
                }
                _ => std::os::unix::fs::symlink("/dev/zero", &binary).unwrap(),
            }

            let recovered = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                source.path(PayloadKind::Tmux, PayloadArch::X86_64),
            )
            .await
            .unwrap_or_else(|_| panic!("{plant}: hashing must never be attempted on it"))
            .unwrap();
            assert_eq!(
                std::fs::read(&recovered).unwrap(),
                expected_member("tmux", "x86_64-unknown-linux-musl"),
                "{plant}: the entry must be discarded and refetched"
            );
        }
    }

    /// Spec: cached control files and markers larger than their bound are a
    /// recoverable miss, refused before they are read.
    ///
    /// The 64 KiB cap the network path enforces has to hold for the cache
    /// too: a restored, corrupted, or hostile-looking cache is exactly as
    /// unauthenticated as a response body, and reading it to find out how big
    /// it is defeats the bound. Recovery, not failure, because a cache entry
    /// is never worth ending an "add host" over.
    #[tokio::test]
    async fn oversized_cached_control_files_and_markers_are_a_miss() {
        let asset = assets::tmux_name(PayloadArch::X86_64);
        for oversized in ["SHA256SUMS", "SHA256SUMS.minisig", "marker"] {
            let release = FixtureRelease::start(Vec::new()).await;
            let cache = tempfile::tempdir().unwrap();
            release
                .source(cache.path())
                .path(PayloadKind::Tmux, PayloadArch::X86_64)
                .await
                .unwrap();
            let after_warmup = release.requests().len();

            let generation = generation(cache.path(), &release.base_url);
            let (path, filler) = match oversized {
                "marker" => (
                    generation.join(format!("{asset}.bin.sha256")),
                    vec![b'0'; usize::try_from(MARKER_MAX_BYTES).unwrap() + 1],
                ),
                name => (generation.join(name), vec![b'x'; SUMS_MAX_BYTES + 1]),
            };
            std::fs::write(&path, &filler).unwrap();

            let recovered = release
                .source(cache.path())
                .path(PayloadKind::Tmux, PayloadArch::X86_64)
                .await
                .unwrap();
            assert_eq!(
                std::fs::read(&recovered).unwrap(),
                expected_member("tmux", "x86_64-unknown-linux-musl"),
                "{oversized}: an oversized cache file must be recovered from"
            );
            assert!(
                release.requests().len() > after_warmup,
                "{oversized}: the oversized entry must not have been treated as a hit"
            );
        }
    }

    /// Spec: housekeeping reports a filesystem fault it cannot work around,
    /// naming the path.
    ///
    /// Silently skipping (the previous behaviour) meant a permission fault
    /// disabled generation cleanup indefinitely while provisioning carried
    /// on and the cache grew without bound — with nothing anywhere saying
    /// why. Root ignores directory permissions, so this asserts nothing
    /// there and says so.
    #[test]
    fn housekeeping_reports_a_filesystem_fault_it_cannot_work_around() {
        if unsafe { libc::geteuid() } == 0 {
            println!("skipping: root is not subject to directory permissions");
            return;
        }
        use std::os::unix::fs::PermissionsExt as _;

        let cache_root = tempfile::tempdir().unwrap();
        let cache_dir = cache_root
            .path()
            .join(format!("v{FIXTURE_VERSION}-0123456789ab"));
        std::fs::create_dir_all(&cache_dir).unwrap();
        // A stale generation whose removal will fail: the parent is
        // read-only, so `remove_dir_all` cannot unlink it.
        std::fs::create_dir(cache_root.path().join("v0.0.1-ffffffffffff")).unwrap();
        std::fs::set_permissions(cache_root.path(), std::fs::Permissions::from_mode(0o500))
            .unwrap();

        let error = housekeeping(cache_root.path(), &cache_dir, FIXTURE_VERSION).unwrap_err();
        let rendered = format!("{error:#}");

        std::fs::set_permissions(cache_root.path(), std::fs::Permissions::from_mode(0o700))
            .unwrap();
        assert!(
            rendered.contains("v0.0.1-ffffffffffff"),
            "the failure must name the generation it could not remove: {rendered}"
        );
    }

    /// Spec: the redirect policy follows a same-scheme hop, refuses an
    /// https → http downgrade, and stops after five hops.
    ///
    /// A redirect target is chosen by the server, so without the downgrade
    /// rule an operator who deliberately typed `https://` could be walked
    /// onto a plaintext connection. The signature check would still refuse
    /// the result, but the request has already gone out by then, and
    /// "refused the answer" is not "did not ask".
    ///
    /// Tested as a pure decision because `reqwest::redirect::Attempt` has no
    /// public constructor: nothing outside reqwest can drive
    /// `Policy::redirect`. The loopback redirect test covers the wiring.
    #[test]
    fn the_redirect_policy_allows_object_stores_and_refuses_downgrades() {
        use super::super::payloads::{RedirectDecision, release_redirect_decision};

        let https =
            Url::parse("https://github.com/scode/farhelm/releases/download/v0.0.3/").unwrap();
        let object_store = Url::parse("https://objects.githubusercontent.com/asset").unwrap();
        let plaintext = Url::parse("http://objects.githubusercontent.com/asset").unwrap();
        let mirror = Url::parse("http://127.0.0.1:8080/release/").unwrap();

        assert_eq!(
            release_redirect_decision(Some(&https), &object_store, 1),
            RedirectDecision::Follow,
            "GitHub's own redirect to its object store must still work"
        );
        assert_eq!(
            release_redirect_decision(Some(&https), &plaintext, 1),
            RedirectDecision::Downgrade
        );
        assert_eq!(
            release_redirect_decision(Some(&mirror), &mirror, 1),
            RedirectDecision::Follow,
            "an http mirror redirecting within http is not a downgrade"
        );
        assert_eq!(
            release_redirect_decision(Some(&https), &object_store, 5),
            RedirectDecision::TooManyHops
        );
        // A loopback or private destination is deliberately NOT refused:
        // `--release-base-url` exists so a helm can be pointed at an internal
        // mirror, and its response still has to hash to a signed entry.
        assert_eq!(
            release_redirect_decision(Some(&mirror), &mirror, 0),
            RedirectDecision::Follow
        );
    }

    /// Spec: a redirect that fails is reported without the redirect TARGET.
    ///
    /// Validating the operator's base URL says nothing about where a server
    /// sends us next, and object stores routinely redirect to signed URLs
    /// with a token in the query. `reqwest::Error`'s own `Display` carries
    /// that URL, and these errors are rendered into provisioning progress
    /// the browser shows — so the URL is stripped before wrapping. The
    /// sentinel here would appear verbatim without that.
    #[tokio::test]
    async fn a_failing_redirect_never_renders_its_target_url() {
        let dead = ClosingListener::start().await;
        let asset = assets::tmux_name(PayloadArch::X86_64);
        let target = format!("{}leak?token=sentinel-token", dead.base_url());
        let release =
            FixtureRelease::start(vec![(asset, Override::Redirect(target.clone()))]).await;
        let cache = tempfile::tempdir().unwrap();

        let source = release.source(cache.path());
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            source.path(PayloadKind::Tmux, PayloadArch::X86_64),
        )
        .await
        .expect("a hung-up redirect target must fail promptly")
        .unwrap_err();

        let rendered = format!("{error:#}");
        assert!(
            rendered.starts_with("add host needs farhelm's"),
            "{rendered}"
        );
        for secret in ["sentinel-token", "leak"] {
            assert!(
                !rendered.contains(secret),
                "the redirect target leaked {secret:?}: {rendered}"
            );
        }
        assert!(
            rendered.contains(release.base_url.as_str()),
            "the validated base URL is still named: {rendered}"
        );
    }
}
