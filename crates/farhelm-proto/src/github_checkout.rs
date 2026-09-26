//! Pure value types, validators, and naming helpers for owned GitHub
//! checkouts (Design A of the owned-github-checkouts blueprint).
//!
//! Choosing `gh:owner/repo` in the launch composer will create a fresh
//! checkout on the selected host and start the selected agent there. This
//! module is the shared vocabulary for that feature: the validated repo
//! identity, the deterministic directory naming, and the payload shapes the
//! helm and supervisor will exchange. Everything here is pure — no I/O, no
//! process spawning, no database, no shell text. That is the point: the
//! naming allocator on the host independently re-checks collision with
//! `mkdir`, and the clone argv is built by the supervisor's launch shim, so
//! this module's only job is to make it impossible to talk about a checkout
//! that failed validation.
//!
//! That last sentence is the module's security posture, worth stating
//! exactly: [`parse_github_repo`] is the only way to obtain a
//! [`GithubRepo`] that is guaranteed well-formed, and [`GithubRepo::clone_url`]
//! formats purely from validated fields. A rejected input therefore cannot
//! reach a URL or a directory name — the "A1" oracle the tests assert.
//! The struct's fields are public (other units in the stack construct and
//! inspect them directly), so a caller that builds a `GithubRepo` by hand
//! bypasses every guarantee in this module; the parse is the boundary.

use serde::{Deserialize, Serialize};

/// Longest GitHub owner (user or organization) name accepted, bytes.
///
/// GitHub's own limit is 39 characters; keeping the same number means a
/// legal-looking identifier is never refused here and then accepted by
/// GitHub itself. Enforced on the raw input before lowercasing.
pub const MAX_OWNER_LEN: usize = 39;

/// Longest repository name accepted, bytes.
///
/// GitHub's documented repository-name limit is 100 bytes; the value
/// matches so an identifier valid here is never silently wrong upstream.
pub const MAX_REPO_NAME_LEN: usize = 100;

/// Longest generated checkout directory component accepted, bytes.
///
/// A refusal, not a silent truncation: every consumer of a basename
/// (wire field, REST path, filesystem entry) reasons about the whole
/// name, and a truncated name would be a different checkout than the
/// preview showed. 200 bytes fits every filesystem Farhelm supports with
/// room for the archive suffix appended later by the supervisor's
/// last-reference archival. The title's own 64 KiB create-field limit is
/// enforced elsewhere; it is not re-implemented here.
pub const MAX_BASENAME_BYTES: usize = 200;

/// Bound the canonical client identity accepted by lookup-only reconciliation.
/// This transport guard keeps lookup work finite. A new create has the tighter
/// shared 64 KiB allowance, which also charges its serialized checkout snapshot
/// and launch fields; this lookup cap does not enlarge that create allowance.
pub const MAX_CLIENT_IDENTITY_BYTES: usize = 512 * 1024;

/// A validated GitHub repository identity, canonical lowercase.
///
/// The fields are lowercase by construction when the value comes from
/// [`parse_github_repo`]: GitHub treats owner and repo names
/// case-insensitively, and Farhelm uses the lowercased form for identity,
/// the generated clone URL, and the generated directory path, so mixed
/// case in a request can never produce two identities for one repository.
///
/// The fields are deliberately public — later units embed this type in
/// wire payloads and reconstruct it from stored rows — so a hand-built
/// instance bypasses validation. The invariant "lowercase, validated"
/// holds only for values that entered through [`parse_github_repo`];
/// deserialize paths that accept these types from a peer must re-validate
/// rather than trust the fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubRepo {
    /// Repository owner, lowercase, 1–39 ASCII alphanumeric/hyphen
    /// characters, starting and ending alphanumeric, no consecutive
    /// hyphens.
    pub owner: String,
    /// Repository name, lowercase, 1–100 ASCII alphanumeric, underscore,
    /// dot, or hyphen characters, excluding exactly `.` and `..`.
    pub name: String,
}

impl GithubRepo {
    /// The HTTPS clone URL for this repository, formatted purely from the
    /// validated fields: `https://github.com/{owner}/{name}.git`.
    ///
    /// This is the ONLY URL construction this feature permits. Nothing in
    /// this module shells out or builds a shell string — argv construction
    /// is the launch shim's job in a later unit — and because the fields
    /// are validation-checked on the way in, the output can only ever be
    /// characters from `[a-z0-9.-]` between two literal separators. The
    /// guaranteed shape is what lets a later unit pass the URL to
    /// `git clone` as a bare argv element without quoting.
    pub fn clone_url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.owner, self.name)
    }
}

/// Why [`parse_github_repo`] refused an identifier.
///
/// The variants exist so callers (the REST edge answering the browser, the
/// supervisor re-validating on arrival) can give an actionable answer for
/// each failure class without parsing error prose. Characters rejected by
/// the per-component alphabets — whitespace, control characters, `%`
/// escapes, `?`/`#`, backslash, `@` — land in [`RepoError::InvalidOwner`]
/// or [`RepoError::InvalidRepo`]; that includes the branch-suffix form
/// `owner/repo@branch`, which is refused because `@` is outside both
/// alphabets rather than by a dedicated branch-aware check.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoError {
    /// The whole input was empty.
    #[error("repository identifier is empty; enter owner/repo")]
    Empty,
    /// The input had no `/`, so it cannot be an owner/repo pair.
    #[error("missing '/' separator; enter owner/repo, e.g. acme/bar")]
    MissingSeparator,
    /// The input had more than one `/`. This also catches URLs
    /// (`https://github.com/a/b`) — they are just strings with several
    /// slashes, not something to strip or repair.
    #[error("more than one '/' in the identifier; enter exactly owner/repo")]
    ExtraSeparator,
    /// The owner component failed GitHub's rules: empty, over
    /// [`MAX_OWNER_LEN`] is reported as [`RepoError::TooLong`] instead, but
    /// otherwise it must be 1–39 ASCII alphanumeric/hyphen characters,
    /// starting and ending alphanumeric, with no consecutive hyphens.
    #[error(
        "invalid repository owner; use 1-39 letters, digits or hyphens, starting and ending with a letter or digit"
    )]
    InvalidOwner,
    /// The repository-name component failed the rules: empty, exactly `.`
    /// or `..` (path traversal), or containing a character outside ASCII
    /// alphanumeric, underscore, dot, and hyphen.
    #[error("invalid repository name; use 1-100 letters, digits, underscores, dots or hyphens")]
    InvalidRepo,
    /// A component exceeded its length limit: owner over
    /// [`MAX_OWNER_LEN`] or name over [`MAX_REPO_NAME_LEN`].
    #[error("repository identifier is too long")]
    TooLong,
}

/// Parse a whole `owner/repo` identifier — no `gh:` prefix; the UI's
/// composer strips its search label before calling this.
///
/// Accepts exactly `owner/repo`, one slash, both components matching the
/// GitHub name rules (see [`RepoError`] for the class list and
/// [`MAX_OWNER_LEN`]/[`MAX_REPO_NAME_LEN`] for the limits). Both components
/// are lowercased: identity, generated URL, and generated path are all
/// canonical lowercase.
///
/// What is deliberately NOT done:
///
/// - **No `.git` suffix is stripped.** A repo literally named `foo.git`
///   is a legal-looking identifier and is accepted as given, lowercased
///   — it will simply fail to clone later if the repository does not
///   exist under that name. Stripping would silently turn a typo into a
///   different repository.
/// - **No URL repair, no credential/branch peeling.** URL syntax, query
///   and fragment characters, percent-escapes, backslashes, whitespace,
///   control characters, a second `/`, and anything after `@` are all
///   outside the accepted alphabets and are refused, never normalized.
///
/// On refusal the caller cannot build a clone URL or a checkout path:
/// [`GithubRepo`] is only guaranteed well-formed through this parse, which
/// is the whole reason it exists.
pub fn parse_github_repo(input: &str) -> Result<GithubRepo, RepoError> {
    if input.is_empty() {
        return Err(RepoError::Empty);
    }
    let mut parts = input.split('/');
    // `split` always yields at least one element; `next` on the first is
    // therefore safe and `nth(1)` is the second component, if any.
    let owner = parts.next().expect("split yields at least one part");
    let Some(name) = parts.next() else {
        return Err(RepoError::MissingSeparator);
    };
    if parts.next().is_some() {
        return Err(RepoError::ExtraSeparator);
    }
    validate_owner(owner)?;
    validate_repo_name(name)?;
    Ok(GithubRepo {
        owner: owner.to_ascii_lowercase(),
        name: name.to_ascii_lowercase(),
    })
}

/// Owner rule: 1–39 ASCII alphanumeric/hyphen characters, starting and
/// ending alphanumeric, no consecutive hyphens.
fn validate_owner(owner: &str) -> Result<(), RepoError> {
    if owner.is_empty() {
        return Err(RepoError::InvalidOwner);
    }
    if owner.len() > MAX_OWNER_LEN {
        return Err(RepoError::TooLong);
    }
    let valid = owner
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && owner.as_bytes()[0].is_ascii_alphanumeric()
        && owner.as_bytes()[owner.len() - 1].is_ascii_alphanumeric()
        && !owner.contains("--");
    if valid {
        Ok(())
    } else {
        Err(RepoError::InvalidOwner)
    }
}

/// Repository-name rule: 1–100 ASCII alphanumeric, underscore, dot, or
/// hyphen characters, excluding exactly `.` and `..`.
fn validate_repo_name(name: &str) -> Result<(), RepoError> {
    if name.is_empty() {
        return Err(RepoError::InvalidRepo);
    }
    if name.len() > MAX_REPO_NAME_LEN {
        return Err(RepoError::TooLong);
    }
    if name == "." || name == ".." {
        return Err(RepoError::InvalidRepo);
    }
    let valid = name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(RepoError::InvalidRepo)
    }
}

/// A proposed checkout directory basename together with the session title
/// that goes with it.
///
/// The pair exists because the two serve different readers: `basename`
/// names the directory the supervisor will `mkdir`, while `display_title`
/// is the user-visible session title, retained verbatim when the user
/// typed one. They are equal only on the numbered path (no title supplied),
/// where the generated `repo-N` IS the title — see
/// [`checkout_basename`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutName {
    /// The directory name to allocate under the configured root, e.g.
    /// `bar-fix-parser`. Never occupied per the lookup handed to
    /// [`checkout_basename`], and never over [`MAX_BASENAME_BYTES`] bytes.
    pub basename: String,
    /// The session title to record: the trimmed original display title
    /// when one was supplied, otherwise the generated basename itself.
    pub display_title: String,
}

/// Why [`checkout_basename`] refused to propose a name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// The title slugified to nothing (e.g. `!!!`), or a title consisting
    /// of only the repo-name prefix (`Bar-` for repo `bar`) slugified to
    /// nothing after the prefix was peeled. There is no honest directory
    /// name to invent from that input, and inventing a filler one would
    /// misrepresent the user's choice.
    #[error("the name contains no usable characters; use letters, digits or hyphens")]
    EmptySlug,
    /// The generated directory component exceeds [`MAX_BASENAME_BYTES`]
    /// bytes. Deliberately a refusal rather than a silent truncation: a
    /// truncated name is a different directory than the preview showed.
    #[error("generated checkout name is over {MAX_BASENAME_BYTES} bytes; the name must be shorter")]
    TooLong,
    /// An explicitly titled basename is already occupied per the caller's
    /// lookup. Explicit names are never suffixed — the caller's filesystem
    /// allocator reports the conflict, including for an empty directory, a
    /// symlink, or a dangling symlink, and the user chooses a new name.
    #[error("a directory with that name already exists")]
    Occupied,
}

/// Propose a deterministic checkout directory basename for `repo`.
///
/// Pure: the collision check stays with the caller via the `occupied_names`
/// lookup, and the filesystem allocator later re-checks atomically with
/// `mkdir` — this function never touches the filesystem and, given an
/// occupied-name lookup, never returns an occupied name.
///
/// Two paths, keyed on the optional session title (a title that is empty
/// after trimming is treated as absent):
///
/// - **No title:** the lowest positive unused `repo-N`, where `repo` is
///   the repository NAME component and unused means not reported occupied.
///   The session title becomes that basename.
/// - **A title:** the original printable title is retained in
///   [`CheckoutName::display_title`], but the directory is slugged. ASCII
///   lowercase; maximal runs of characters outside `[a-z0-9]` become one
///   hyphen; surrounding hyphens are trimmed; an empty slug is refused. If
///   the title already begins with the canonical repo name plus `-`
///   (ASCII case-insensitive), only the remainder is slugged and the repo
///   prefix is prepended ONCE, which also avoids doubling the prefix when
///   the repo name itself contains dots or underscores. Otherwise
///   `repo-` is prepended to the whole-title slug. An explicit basename
///   that comes back occupied is refused ([`NameError::Occupied`]), never
///   suffixed.
///
/// Worked examples for repo `acme/bar` (from the design, pinned by tests):
///
/// - no title, `bar-1` and `bar-3` occupied → `bar-2`, display `bar-2`
/// - title `Fix parser` → path `bar-fix-parser`, display `Fix parser`
/// - title `bar-fix` → `bar-fix` (prefix not doubled)
/// - title `!!!` → refused (`NameError::EmptySlug`)
///
/// Any composed component over [`MAX_BASENAME_BYTES`] bytes is refused
/// with [`NameError::TooLong`]. The numeric loop reads `occupied_names`
/// until it finds a gap, so a caller whose lookup reports every possible
/// name occupied (a pathology no real scan can produce) would loop
/// forever; the caller's scan is bounded, this function is not.
pub fn checkout_basename(
    repo: &GithubRepo,
    title: Option<&str>,
    occupied_names: &dyn Fn(&str) -> bool,
) -> Result<CheckoutName, NameError> {
    match title.map(str::trim) {
        None | Some("") => {
            let mut n = 1u64;
            loop {
                let candidate = format!("{}-{}", repo.name, n);
                if !occupied_names(&candidate) {
                    return Ok(CheckoutName {
                        display_title: candidate.clone(),
                        basename: candidate,
                    });
                }
                n += 1;
            }
        }
        Some(title) => {
            let prefix = format!("{}-", repo.name);
            // `to_ascii_lowercase` preserves byte length and the prefix is
            // ASCII, so slicing the ORIGINAL at the prefix length is on a
            // char boundary whenever the case-insensitive match succeeded.
            let remainder = if title.to_ascii_lowercase().starts_with(&prefix) {
                &title[prefix.len()..]
            } else {
                title
            };
            let slug = slugify(remainder);
            if slug.is_empty() {
                return Err(NameError::EmptySlug);
            }
            let basename = format!("{}-{}", repo.name, slug);
            if basename.len() > MAX_BASENAME_BYTES {
                return Err(NameError::TooLong);
            }
            if occupied_names(&basename) {
                return Err(NameError::Occupied);
            }
            Ok(CheckoutName {
                basename,
                display_title: title.to_string(),
            })
        }
    }
}

/// Slug a title for directory use: ASCII lowercase, each maximal run of
/// characters outside `[a-z0-9]` collapsed to one hyphen, surrounding
/// hyphens trimmed. Returns `""` when nothing usable remains — the caller
/// turns that into [`NameError::EmptySlug`].
///
/// The pending-separator flag is only flushed after the slug has some
/// content, which is what trims LEADING hyphens: `!!!Fix` starts in a run
/// before any slug character exists, and emitting a separator there would
/// propose `bar--fix` (and check occupancy for a name the design does not
/// allow). Trailing runs never enter `out` because nothing flushes them.
fn slugify(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut pending_run = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_run && !out.is_empty() {
                out.push('-');
            }
            pending_run = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_run = true;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Wire payload shapes
//
// These travel helm↔supervisor (and mirror into REST request bodies) in
// later units of the stack; they are defined here so the names and field
// shapes settle once. Like the rest of this crate's payload types they
// derive only `Debug/Clone/PartialEq/Eq/Serialize/Deserialize`, decode
// missing `Option` fields as `None`, and do not use `deny_unknown_fields`
// — see `PROTOCOL_VERSION`'s additive-discipline notes for why wire types
// here stay decode-tolerant.

/// The full intent for one owned fresh checkout, as the helm hands it to
/// the supervisor: which repository to clone, and the preview binding the
/// create must still verify on the target host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubCheckoutIntent {
    /// The validated repository to clone.
    pub repo: GithubRepo,
    /// The preview binding the browser was shown and the user launched
    /// with; the supervisor re-verifies it before allocating.
    pub preview: CheckoutPreviewBinding,
}

/// The preview binding a fresh-checkout create carries: the exact
/// directory the browser showed, bound to the helm configuration revision
/// it was computed under.
///
/// This is a precondition, not a reservation — nothing on the filesystem
/// is reserved by showing it. The supervisor re-resolves the root, applies
/// the basename, and re-checks collision atomically with `mkdir`; a race
/// is a conflict that refreshes the preview, never a duplicate allocated
/// elsewhere. `config_revision` lets the helm verify the settings the
/// preview was computed against have not changed since.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutPreviewBinding {
    /// The canonical absolute root the checkout would be created under.
    pub canonical_root: String,
    /// The proposed directory basename under that root.
    pub basename: String,
    /// The absolute session working directory that would result
    /// (`canonical_root/basename`), as the browser displayed it.
    pub cwd: String,
    /// The helm checkout-configuration revision the preview was computed
    /// under. Opaque to the client: compared, never interpreted.
    pub config_revision: i64,
}

/// A create whose fresh-checkout destination has been fully resolved:
/// repository, where it goes, and what runs after the clone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedGithubCheckout {
    /// Canonical encoding of the original client-controlled launch request,
    /// captured before profile/structured/configuration resolution. Replays
    /// compare this identity before using the durable resolved snapshot.
    /// Only the authenticated helm constructs it; it is not a credential.
    pub client_identity: String,
    /// The validated repository to clone.
    pub repo: GithubRepo,
    /// The canonical absolute root directory, re-resolved on the target
    /// host at admission time. May differ from the preview's if the
    /// configured root changed; the supervisor refuses that mismatch
    /// rather than cloning somewhere the user did not see.
    pub root: String,
    /// The resolved post-clone command snapshot, if one is configured.
    /// Carried by the helm (the only holder of configuration); the
    /// supervisor never derives it itself.
    pub post_clone: Option<String>,
    /// The preview binding this resolution came from.
    pub preview: CheckoutPreviewBinding,
}

/// One managed checkout as tracked on the supervisor side: its identity,
/// its repository, and where it came from.
///
/// Field names mirror [`crate::SessionInfo`]'s conventions (`id`,
/// canonical path, origin session) so the registry row and the session
/// snapshot read the same way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingCopyInfo {
    /// The supervisor-minted registry identity for this checkout.
    pub id: String,
    /// The repository this checkout was cloned from.
    pub repo: GithubRepo,
    /// The canonical absolute path of the checkout directory.
    pub canonical_path: String,
    /// The id of the session whose create performed the clone — provenance,
    /// not a lifetime dependency; later sessions attach to the checkout as
    /// members without changing this field.
    pub origin_session_id: String,
}

/// The REST-fresh create request shape for a GitHub checkout, mirroring
/// what the browser sends: the repository text exactly as typed, the optional
/// title, and the preview the user accepted.
///
/// `repo` is deliberately the RAW `owner/repo` text, not a validated
/// struct: parsing happens at the helm (and again at the supervisor) so
/// the refusal can name the failure class against what the user actually
/// typed. Nothing travels further on the wire without passing
/// [`parse_github_repo`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubCheckoutRequest {
    /// Raw `owner/repo` text as typed by the user.
    pub repo: String,
    /// Optional session title; absence means the naming allocator picks
    /// the lowest free `repo-N` and uses that as the title too.
    pub title: Option<String>,
    /// The exact preview accepted by the user. Absence is decoded so the
    /// helm can give an actionable refusal, never synthesize a destination.
    pub preview: Option<AcceptedGithubPreview>,
}

/// A displayed destination bound to both a connection and its installation.
/// Connection counters may restart with the helm; the installation identity
/// must still match before a retry can query an old permanent reservation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedGithubPreview {
    /// The exact destination and configuration revision shown before launch.
    #[serde(flatten)]
    pub binding: CheckoutPreviewBinding,
    /// Registry host id, checked against the route's current claim.
    pub host: String,
    /// Connection token required for an unknown create, but not a known retry.
    pub incarnation: u64,
    /// Stable supervisor installation identity, required even for known retries.
    pub installation_identity: String,
}

/// The preview request the helm sends (and the REST layer mirrors) before
/// a fresh checkout is offered for launch: which host, which repository,
/// and the claim context the answer must be validated against.
///
/// `host`/`expected_incarnation` mirror the optional-create request fields
/// on the helm's existing create body (`CreateReq::host` and
/// `CreateReq::expected_incarnation`): the claim is optional, absent means
/// no claim, and the helm refuses a stale incarnation before anything is
/// resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubPreviewRequest {
    /// Which registered host to preview on; `None` means the reserved
    /// local host, matching the create request's default.
    pub host: Option<String>,
    /// Which connection the caller prepared this request against — an
    /// opaque `HostView::incarnation` token, compared never interpreted.
    pub expected_incarnation: Option<u64>,
    /// Raw `owner/repo` text as typed; parsed and refused here, never
    /// normalized into something else.
    pub repo: String,
    /// Optional session title used for deterministic naming.
    pub title: Option<String>,
    /// The checkout root the HELM resolved from its own configuration
    /// (global value with the host's override applied) — stored UNEXPANDED;
    /// only this supervisor expands `~`, against its own captured home.
    /// `None` decodes as absent for frames from a peer that has not learned
    /// the field; a preview without a root cannot name a directory and is
    /// refused with a message saying what is missing.
    pub root: Option<String>,
    /// The helm checkout-configuration revision the root was resolved
    /// under, echoed back so the create can present a binding against the
    /// same revision. `None` decodes as absent like `root`.
    pub config_revision: Option<i64>,
}

/// The host/incarnation pair a client reads back from a host-scoped
/// response and echoes on the follow-up request, mirroring the
/// `HostView` `(id, incarnation)` pair the helm's REST surface already
/// serves — the same pair an `expected_incarnation` precondition is
/// checked against. There is no dedicated REST struct for this today (the
/// pair travels inside `HostView`), so this proto-local shape names it
/// once for the preview payload instead of leaving the fields loose on
/// [`GithubPreviewResponse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimContext {
    /// The host id the preview was resolved against.
    pub host: String,
    /// That host's connection token as of the preview — opaque,
    /// monotonic, never reused; compared, never interpreted.
    pub incarnation: u64,
}

/// The preview reply the helm returns for a fresh-checkout request: the
/// exact proposed path plus the claim context it is valid against.
///
/// Deliberately carries no hook configuration: the post-clone command is
/// helm-side configuration and is not handed to the browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubPreviewResponse {
    /// The canonical absolute root the checkout would be created under.
    pub canonical_root: String,
    /// The proposed directory basename under that root.
    pub basename: String,
    /// The absolute session working directory that would result.
    pub cwd: String,
    /// The helm checkout-configuration revision this preview was computed
    /// under; the create must present a binding against the same revision.
    pub config_revision: i64,
    /// Which host connection this preview is bound to.
    pub claim_context: ClaimContext,
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_github_repo -------------------------------------------------

    /// Mixed case is accepted and canonicalized to lowercase, and the
    /// clone URL is formatted purely from the validated fields.
    #[test]
    fn valid_identifier_is_lowercased_and_yields_clone_url() {
        let repo = parse_github_repo("Acme/Bar.Rocks_1-x").expect("valid identifier");
        assert_eq!(repo.owner, "acme");
        assert_eq!(repo.name, "bar.rocks_1-x");
        assert_eq!(
            repo.clone_url(),
            "https://github.com/acme/bar.rocks_1-x.git"
        );
    }

    /// A `.git` suffix is part of the name, not transport syntax to strip:
    /// the identifier is treated as given, lowercased.
    #[test]
    fn git_suffix_is_not_stripped() {
        let repo = parse_github_repo("acme/bar.git").expect("valid identifier");
        assert_eq!(repo.name, "bar.git");
        assert_eq!(repo.clone_url(), "https://github.com/acme/bar.git.git");
    }

    /// Length edges: 1 and 39 for the owner, 1 and 100 for the name
    /// accepted; one past each refused as `TooLong`.
    #[test]
    fn length_edges() {
        parse_github_repo("a/b").expect("1-char owner and repo");
        let owner39 = "a".repeat(39);
        parse_github_repo(&format!("{owner39}/b")).expect("39-char owner");
        let repo100 = "b".repeat(100);
        parse_github_repo(&format!("a/{repo100}")).expect("100-char repo");

        let err = parse_github_repo(&format!("{}a/b", "a".repeat(39))).expect_err("40-char owner");
        assert_eq!(err, RepoError::TooLong);
        let err = parse_github_repo(&format!("a/{}b", "b".repeat(100))).expect_err("101-char repo");
        assert_eq!(err, RepoError::TooLong);
    }

    /// Every malformed class the design names, with the error variant the
    /// caller should be able to distinguish.
    #[test]
    fn malformed_classes() {
        let cases: &[(&str, RepoError)] = &[
            ("", RepoError::Empty),
            ("acmebar", RepoError::MissingSeparator),
            ("a\\b", RepoError::MissingSeparator),
            ("a/b/c", RepoError::ExtraSeparator),
            ("https://github.com/a/b", RepoError::ExtraSeparator),
            ("/b", RepoError::InvalidOwner),
            ("a--b/c", RepoError::InvalidOwner),
            ("-a/b", RepoError::InvalidOwner),
            ("a-/b", RepoError::InvalidOwner),
            ("å/b", RepoError::InvalidOwner),
            ("a b/c", RepoError::InvalidOwner),
            ("a/", RepoError::InvalidRepo),
            ("a/.", RepoError::InvalidRepo),
            ("a/..", RepoError::InvalidRepo),
            ("a/b%20c", RepoError::InvalidRepo),
            ("a/b c", RepoError::InvalidRepo),
            ("a/b\tc", RepoError::InvalidRepo),
            ("a/b\u{7}c", RepoError::InvalidRepo),
            ("a/b\\c", RepoError::InvalidRepo),
            ("a/b?x=1", RepoError::InvalidRepo),
            ("a/b#frag", RepoError::InvalidRepo),
            ("a/b@main", RepoError::InvalidRepo),
        ];
        for (input, expected) in cases {
            assert_eq!(
                parse_github_repo(input),
                Err(expected.clone()),
                "input {input:?}"
            );
        }
    }

    /// The A1 oracle: a refused identifier can never reach a clone URL.
    /// There is no path from a parse failure to a URL — the Result is Err
    /// for every malformed class, and only a valid parse produces a
    /// well-formed `https://github.com/…` address.
    #[test]
    fn refused_identifiers_yield_no_url() {
        let malformed = [
            "",
            "acmebar",
            "a/b/c",
            "https://github.com/a/b",
            "/b",
            "a/",
            "a/..",
            "a/b%20c",
            "a/b c",
            "a/b@main",
            "a/b?x",
            "a/b#f",
            "a\\b",
            "a/b\u{1f}",
            "-a/b",
            "a-/b",
        ];
        for input in malformed {
            let parsed = parse_github_repo(input);
            assert!(parsed.is_err(), "input {input:?} must be refused");
        }
        let repo = parse_github_repo("acme/bar").expect("valid");
        let url = repo.clone_url();
        assert!(url.starts_with("https://github.com/"));
        assert!(url.ends_with(".git"));
        assert!(!url.contains(' '));
    }

    // -- checkout_basename -------------------------------------------------

    /// Without a title the lowest positive unused `repo-N` is proposed and
    /// doubles as the session title. Repo `acme/bar` with `bar-1` and
    /// `bar-3` occupied proposes `bar-2`.
    #[test]
    fn numbered_name_fills_lowest_gap() {
        let repo = parse_github_repo("acme/bar").expect("valid");
        let occupied = |name: &str| name == "bar-1" || name == "bar-3";
        let name = checkout_basename(&repo, None, &occupied).expect("proposal");
        assert_eq!(name.basename, "bar-2");
        assert_eq!(name.display_title, "bar-2");
    }

    /// An empty title behaves exactly like an absent one, and a
    /// whitespace-only title is treated as absent too.
    #[test]
    fn empty_title_behaves_like_absent() {
        let repo = parse_github_repo("acme/bar").expect("valid");
        for title in [None, Some(""), Some("   ")] {
            let name = checkout_basename(&repo, title, &|_| false).expect("proposal");
            assert_eq!(name.basename, "bar-1");
            assert_eq!(name.display_title, "bar-1");
        }
    }

    /// The design's four named examples for repo `acme/bar`.
    #[test]
    fn titled_names_follow_the_naming_rule() {
        let repo = parse_github_repo("acme/bar").expect("valid");

        // Whole-title slug with repo prefix prepended; display title kept.
        let name = checkout_basename(&repo, Some("Fix parser"), &|_| false).expect("proposal");
        assert_eq!(name.basename, "bar-fix-parser");
        assert_eq!(name.display_title, "Fix parser");

        // Title already beginning with the repo name plus `-` (ASCII
        // case-insensitive): the prefix is NOT doubled.
        let name = checkout_basename(&repo, Some("bar-fix"), &|_| false).expect("proposal");
        assert_eq!(name.basename, "bar-fix");
        assert_eq!(name.display_title, "bar-fix");

        let name = checkout_basename(&repo, Some("Bar-Fix"), &|_| false).expect("proposal");
        assert_eq!(name.basename, "bar-fix");

        // Pure punctuation: nothing usable remains.
        assert_eq!(
            checkout_basename(&repo, Some("!!!"), &|_| false),
            Err(NameError::EmptySlug)
        );
    }

    /// Leading punctuation must TRIM, not become a doubled hyphen: the
    /// reviewer's finding — `!!!Fix parser` proposed `bar--fix-parser` and
    /// `bar-!!!Fix` proposed `bar--fix`, checking occupancy against names
    /// the design does not define. Surrounding hyphens are trimmed, so
    /// only one separator joins the repo prefix to the slug.
    #[test]
    fn leading_punctuation_trims_to_a_single_hyphen() {
        let repo = parse_github_repo("acme/bar").expect("valid");

        let name = checkout_basename(&repo, Some("!!!Fix parser"), &|_| false).expect("proposal");
        assert_eq!(name.basename, "bar-fix-parser");
        assert_eq!(name.display_title, "!!!Fix parser");

        let name = checkout_basename(&repo, Some("bar-!!!Fix"), &|_| false).expect("proposal");
        assert_eq!(name.basename, "bar-fix");

        // Trailing punctuation is equally trimmed.
        let name = checkout_basename(&repo, Some("Fix parser!!!"), &|_| false).expect("proposal");
        assert_eq!(name.basename, "bar-fix-parser");
    }

    /// A title consisting of only the repo prefix slugifies to nothing and
    /// is refused, rather than proposing the bare repo name.
    #[test]
    fn prefix_only_title_is_refused() {
        let repo = parse_github_repo("acme/bar").expect("valid");
        assert_eq!(
            checkout_basename(&repo, Some("Bar-"), &|_| false),
            Err(NameError::EmptySlug)
        );
    }

    /// The prefix-matching shortcut also works when the repo name contains
    /// dots or underscores — that is why the prefix is matched, not
    /// prepended blindly.
    #[test]
    fn dotted_repo_name_avoids_doubled_prefix() {
        let repo = parse_github_repo("acme/my.repo_2").expect("valid");
        let name = checkout_basename(&repo, Some("My.repo_2-fix"), &|_| false).expect("proposal");
        assert_eq!(name.basename, "my.repo_2-fix");
        let name = checkout_basename(&repo, Some("Fix parser"), &|_| false).expect("proposal");
        assert_eq!(name.basename, "my.repo_2-fix-parser");
    }

    /// Nothing over 200 bytes is proposed: boundary accepted at exactly
    /// 200, refused one byte over, with no silent truncation.
    #[test]
    fn over_200_byte_component_is_refused() {
        let repo = parse_github_repo("acme/bar").expect("valid");
        // "bar-" (4) + 196 slug bytes = exactly 200.
        let at_limit = "a".repeat(196);
        let one_over = "a".repeat(197);
        let name = checkout_basename(&repo, Some(at_limit.as_str()), &|_| false).expect("proposal");
        assert_eq!(name.basename.len(), 200);
        let err =
            checkout_basename(&repo, Some(one_over.as_str()), &|_| false).expect_err("201 bytes");
        assert!(matches!(err, NameError::TooLong));
    }

    /// An explicitly titled basename that the caller reports occupied is
    /// refused, never suffixed — the filesystem allocator's mkdir is what
    /// reports the conflict for real, including for an empty directory,
    /// a symlink, or a dangling symlink.
    #[test]
    fn explicit_occupied_name_is_refused_not_suffixed() {
        let repo = parse_github_repo("acme/bar").expect("valid");
        let err = checkout_basename(&repo, Some("existing"), &|n| n == "bar-existing")
            .expect_err("occupied");
        assert_eq!(err, NameError::Occupied);
    }

    // -- wire payload shapes -----------------------------------------------

    /// The payload types round-trip through JSON with stable field names,
    /// and absent `Option` fields stay absent on decode (serde's built-in
    /// Option handling — the same tolerance the create request relies on).
    #[test]
    fn payloads_round_trip() {
        let repo = parse_github_repo("acme/bar").expect("valid");
        let binding = CheckoutPreviewBinding {
            canonical_root: "/srv/checkouts".to_string(),
            basename: "bar-1".to_string(),
            cwd: "/srv/checkouts/bar-1".to_string(),
            config_revision: 7,
        };
        let intent = GithubCheckoutIntent {
            repo: repo.clone(),
            preview: binding.clone(),
        };
        let json = serde_json::to_value(&intent).expect("serialize intent");
        assert_eq!(json["repo"]["owner"], serde_json::json!("acme"));
        assert_eq!(json["preview"]["config_revision"], serde_json::json!(7));
        let decoded: GithubCheckoutIntent = serde_json::from_value(json).expect("decode intent");
        assert_eq!(decoded, intent);

        let request = GithubCheckoutRequest {
            repo: "acme/bar".to_string(),
            title: None,
            preview: None,
        };
        let json = serde_json::to_value(&request).expect("serialize request");
        let decoded: GithubCheckoutRequest = serde_json::from_value(json).expect("decode");
        assert_eq!(decoded, request);

        let preview_request = GithubPreviewRequest {
            host: Some("h-1".to_string()),
            expected_incarnation: Some(3),
            repo: "acme/bar".to_string(),
            title: Some("Fix parser".to_string()),
            root: Some("~/work".to_string()),
            config_revision: Some(4),
        };
        let decoded: GithubPreviewRequest =
            serde_json::from_value(serde_json::to_value(&preview_request).expect("serialize"))
                .expect("decode");
        assert_eq!(decoded, preview_request);

        let response = GithubPreviewResponse {
            canonical_root: "/srv/checkouts".to_string(),
            basename: "bar-fix-parser".to_string(),
            cwd: "/srv/checkouts/bar-fix-parser".to_string(),
            config_revision: 7,
            claim_context: ClaimContext {
                host: "h-1".to_string(),
                incarnation: 3,
            },
        };
        let decoded: GithubPreviewResponse =
            serde_json::from_value(serde_json::to_value(&response).expect("serialize"))
                .expect("decode");
        assert_eq!(decoded, response);

        let resolved = ResolvedGithubCheckout {
            client_identity: "fixture-request".into(),
            repo: repo.clone(),
            root: "/srv/checkouts".to_string(),
            post_clone: Some("npm install".to_string()),
            preview: binding.clone(),
        };
        let decoded: ResolvedGithubCheckout =
            serde_json::from_value(serde_json::to_value(&resolved).expect("serialize"))
                .expect("decode");
        assert_eq!(decoded, resolved);

        let info = WorkingCopyInfo {
            id: "wc-1".to_string(),
            repo: repo.clone(),
            canonical_path: "/srv/checkouts/bar-1".to_string(),
            origin_session_id: "sess-1".to_string(),
        };
        let decoded: WorkingCopyInfo =
            serde_json::from_value(serde_json::to_value(&info).expect("serialize"))
                .expect("decode");
        assert_eq!(decoded, info);
    }
}
