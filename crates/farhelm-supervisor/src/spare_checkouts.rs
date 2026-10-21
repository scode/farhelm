//! Recovery ownership for ready checkouts beneath a reserved checkout root.
//!
//! The supervisor remembers each outstanding owner namespace, not every build
//! and claim transition. A verified namespace owns only this repository's strict
//! private names; that covers a build killed before it could record its own
//! identity. A ready slot has separate recorded identity and is never adopted.
//! Takes need none of these records: their atomic rename preserves identity,
//! and their session/generation name lets startup protect surviving clone panes.

use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::fs;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};
use farhelm_proto::github_checkout::GithubRepo;
use serde::{Deserialize, Serialize};

use crate::checkout_trash::{open_child_dir, remove_opened_tree};
use crate::working_copies::{DirectoryEvidence, SPARE_DIR_NAME, rename_at_noreplace};

/// A corrupt private record must not allocate without bound. Exhaustion is a
/// retained failure, never a truncated collection that forgets old locations.
const MAX_RECORD_BYTES: u64 = 16 * 1024 * 1024;
/// Bound both names retained in memory and directory-iterator work. Like the
/// working-copy occupancy scan, this budget includes the call proving EOF.
const MAX_NAMESPACE_ENTRIES: usize = 100_000;

/// Startup's already-observed launch evidence, kept separate from disk work.
/// A failed tmux observation protects all claims; a per-launch uncertainty
/// protects that exact generation rather than borrowing another run's pane.
#[derive(Default)]
pub(crate) struct LiveClaims {
    pub(crate) uncertain: bool,
    pub(crate) protected: HashSet<(String, i64)>,
}

impl LiveClaims {
    /// Unknown observations retain claims; positive liveness is fenced to the
    /// launch generation encoded in this exact private directory name.
    fn protects(&self, session: &str, generation: i64) -> bool {
        self.uncertain || self.protected.contains(&(session.to_owned(), generation))
    }
}

/// A record contains ownership evidence only. Old locations remain until
/// every generated object for this repository there has durably gone.
#[derive(Serialize, Deserialize)]
struct SpareRecord {
    locations: Vec<Namespace>,
}

/// The owner directory is the smallest shared recovery anchor. The ready
/// identity is recorded before publication, so an absent slot can still have
/// a private build or claim that startup must find in this same namespace.
#[derive(Clone, Serialize, Deserialize)]
struct Namespace {
    path: PathBuf,
    identity: DirectoryEvidence,
    ready: Option<DirectoryEvidence>,
}

/// Private names carry the lifecycle fact cleanup actually consumes. No
/// persistent build status or branch/remote information duplicates them.
#[derive(Debug, PartialEq, Eq)]
enum PrivateName {
    Building,
    Claimed { session: String, generation: i64 },
    Trash,
}

/// Accept only canonical UUIDs and generations produced by Farhelm. A prefix
/// match alone must never turn arbitrary user content into removable debris.
/// The colon cannot occur in a validated repository name, so a generated
/// private object cannot collide with another repository's ready slot.
fn private_name(repo: &str, name: &str) -> Option<PrivateName> {
    let suffix = name.strip_prefix(&format!(".{repo}:"))?;
    for (prefix, kind) in [
        ("building-", PrivateName::Building),
        ("trash-", PrivateName::Trash),
    ] {
        if let Some(id) = suffix.strip_prefix(prefix) {
            return canonical_uuid(id).then_some(kind);
        }
    }
    let claimed = suffix.strip_prefix("claimed-")?;
    let (session, generation) = claimed.rsplit_once('-')?;
    let parsed = generation.parse::<i64>().ok()?;
    (canonical_uuid(session) && parsed >= 0 && parsed.to_string() == generation).then(|| {
        PrivateName::Claimed {
            session: session.to_owned(),
            generation: parsed,
        }
    })
}

/// UUID parsing alone accepts several aliases; recovery accepts the one
/// spelling the private-name constructors use, without case or format aliases.
fn canonical_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}

/// Use the same strict lifecycle alphabet for taking and startup recovery.
/// A legacy or malformed launch cannot claim a folder cleanup cannot protect.
pub(crate) fn claim_name(repo: &str, session: &str, generation: i64) -> Option<CString> {
    let name = format!(".{repo}:claimed-{session}-{generation}");
    if matches!(private_name(repo, &name), Some(PrivateName::Claimed { .. })) {
        CString::new(name).ok()
    } else {
        None
    }
}

/// Finish spare recovery while the caller holds this repository's existing
/// nonblocking cache flock. A take never takes that lock; its live claim is
/// protected by the separately collected launch snapshot instead.
pub(crate) fn sweep(
    cache: &Path,
    repo: &GithubRepo,
    live: &LiveClaims,
    evict: bool,
) -> anyhow::Result<()> {
    sweep_with_namespace_sync(cache, repo, live, evict, &mut |namespace| {
        namespace.sync_all()
    })
}

/// Inject only the durability barrier, keeping tests on the real rename,
/// confined removal and record-publication path. A failed namespace sync must
/// retain the original evidence even when directory removal already completed.
fn sweep_with_namespace_sync(
    cache: &Path,
    repo: &GithubRepo,
    live: &LiveClaims,
    evict: bool,
    sync_namespace: &mut impl FnMut(&fs::File) -> std::io::Result<()>,
) -> anyhow::Result<()> {
    let record_path = cache.with_extension("spare.json");
    let file = match fs::File::open(&record_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        bail!("spare ownership record exceeds the {MAX_RECORD_BYTES}-byte limit; folders retained");
    }
    let record: SpareRecord =
        serde_json::from_slice(&bytes).context("reading spare ownership record")?;
    let mut retained = Vec::new();
    let mut failures = Vec::new();
    for original in record.locations {
        let mut location = original.clone();
        match sweep_namespace(&mut location, repo, live, evict, sync_namespace) {
            Ok(false) => {}
            Ok(true) => retained.push(location),
            Err(error) => {
                failures.push(format!("{}: {error:#}", location.path.display()));
                // A later cleanup or durability failure may follow ready
                // removal. Keep the old identity until its deletion is durable.
                retained.push(original);
            }
        }
    }
    // Persist successful per-location progress even if another location failed.
    // Losing one old-root cleanup must not forget its still-live claim.
    if retained.is_empty() {
        fs::remove_file(&record_path)?;
        fs::File::open(record_path.parent().context("record parent missing")?)?.sync_all()?;
    } else {
        crate::files::write_durable_sync(
            &record_path,
            &serde_json::to_vec(&SpareRecord {
                locations: retained,
            })?,
            &crate::files::RealFs,
        )?;
    }
    if !failures.is_empty() {
        bail!(
            "spare cleanup retained uncertain folders: {}",
            failures.join("; ")
        );
    }
    Ok(())
}

/// Open the accepted checkout root, then confine every reserved child lookup
/// below it. A checkout root may itself be on a separate mounted filesystem;
/// NO_XDEV starts here, just as it does for archived checkout removal.
/// Missing reserved containers are positive absence: this cleanup never removes
/// them, so no mutation of their entries awaits our namespace durability barrier.
fn open_namespace(location: &Namespace, repo: &GithubRepo) -> anyhow::Result<Option<fs::File>> {
    let owner = location
        .path
        .file_name()
        .context("namespace owner missing")?;
    let reserved = location.path.parent().context("namespace parent missing")?;
    let root = reserved.parent().context("checkout root missing")?;
    if !location.path.is_absolute()
        || location
            .path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        || owner != std::ffi::OsStr::new(&repo.owner)
        || reserved.file_name() != Some(std::ffi::OsStr::new(SPARE_DIR_NAME))
    {
        bail!("spare record is outside its reserved repository namespace");
    }
    let root = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)?;
    let opened = (|| {
        let reserved = open_child_dir(&root, &CString::new(SPARE_DIR_NAME)?, true)?;
        open_child_dir(&reserved, &CString::new(repo.owner.as_str())?, true)
    })();
    let namespace = match opened {
        Ok(namespace) => namespace,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    location
        .identity
        .verify(&namespace)
        .context("spare namespace was replaced")?;
    Ok(Some(namespace))
}

/// Cleanup recognizes only this repository's ready slot and generated names.
/// Other repositories and unknown files in the shared owner directory stay
/// untouched, and cannot by themselves keep this repository's record alive.
fn sweep_namespace(
    location: &mut Namespace,
    repo: &GithubRepo,
    live: &LiveClaims,
    evict: bool,
    sync_namespace: &mut impl FnMut(&fs::File) -> std::io::Result<()>,
) -> anyhow::Result<bool> {
    let Some(namespace) = open_namespace(location, repo)? else {
        return Ok(false);
    };
    let names = namespace_names(&namespace, MAX_NAMESPACE_ENTRIES)?;
    for name in names {
        let Ok(text) = name.to_str() else { continue };
        if text == repo.name {
            let expected = location
                .ready
                .context("ready spare has no recorded identity")?;
            let directory = open_child_dir(&namespace, &name, true)?;
            expected
                .verify(&directory)
                .context("ready spare was replaced")?;
            if evict {
                discard_child(&namespace, &name, expected, &repo.name)?;
                location.ready = None;
            }
        } else if let Some(kind) = private_name(&repo.name, text) {
            if let PrivateName::Claimed {
                session,
                generation,
            } = &kind
                && live.protects(session, *generation)
            {
                continue;
            }
            // Dedicated namespace ownership covers even a mkdir whose process
            // died before recording a per-tree identity. Verify the anchor
            // first, then capture the actual child before parking and removing.
            let directory = open_child_dir(&namespace, &name, true)?;
            let expected = DirectoryEvidence::capture(&directory)?;
            discard_child(&namespace, &name, expected, &repo.name)?;
        }
    }
    // Re-read after removal: directory iteration during mutation can skip an
    // entry, and absence of the ready slot alone never proves a root empty.
    let remains = namespace_names(&namespace, MAX_NAMESPACE_ENTRIES)?
        .iter()
        .any(|name| {
            name.to_str()
                .is_ok_and(|name| name == repo.name || private_name(&repo.name, name).is_some())
        });
    // Checkout roots and private records may live on different filesystems.
    // Sync the namespace before a record forgets a ready identity or location,
    // including an absence-only retry after a prior failed barrier.
    sync_namespace(&namespace)
        .context("syncing spare namespace before retiring ownership evidence")?;
    Ok(remains)
}

/// Collect child components from an opened namespace, without reopening its
/// pathname or following anything it contains. A read failure keeps the record.
fn namespace_names(namespace: &fs::File, cap: usize) -> anyhow::Result<Vec<CString>> {
    let mut names = Vec::new();
    let mut entries = rustix::fs::Dir::read_from(namespace)?;
    for _ in 0..cap {
        let Some(entry) = entries.next() else {
            return Ok(names);
        };
        let entry = entry?;
        let name = entry.file_name();
        if name.to_bytes() != b"." && name.to_bytes() != b".." {
            names.push(name.to_owned());
        }
    }
    bail!(
        "spare namespace scan reached its {cap}-entry limit without proving completion; record retained"
    )
}

/// Rename before recursively removing, then verify the opened moved object.
/// An ordinary replacement never inherits a previous object's authority. A
/// failure leaves a strict trash name that the next sweep can safely retry.
fn discard_child(
    namespace: &fs::File,
    name: &CStr,
    expected: DirectoryEvidence,
    repo: &str,
) -> anyhow::Result<()> {
    let selected = open_child_dir(namespace, name, true)?;
    expected.verify(&selected)?;
    let trash = CString::new(format!(".{repo}:trash-{}", uuid::Uuid::new_v4()))?;
    rename_at_noreplace(namespace.as_raw_fd(), name, namespace.as_raw_fd(), &trash)?;
    let moved = open_child_dir(namespace, &trash, true)?;
    expected.verify(&moved)?;
    remove_opened_tree(namespace, &moved, &trash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    /// Keep cache locks, private records, checkout roots and all retained
    /// content in one owned temporary tree. No child or test environment changes.
    struct Fixture {
        _directory: tempfile::TempDir,
        state: PathBuf,
        cache: PathBuf,
        repo: GithubRepo,
        location: Namespace,
    }

    impl Fixture {
        /// Backdate the real cache lock so eviction uses the production clock
        /// and flock path, not a parallel test-only retention implementation.
        fn new(age_days: u64) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            let state = root.join("state");
            let repo = farhelm_proto::github_checkout::parse_github_repo("example/repo").unwrap();
            let cache = crate::repo_cache::path(&state, &repo);
            let lock = crate::repo_cache::acquire(&cache).unwrap();
            fs::create_dir(&cache).unwrap();
            fs::write(cache.join("objects"), "cache objects").unwrap();
            let modified = SystemTime::now() - Duration::from_secs(age_days * 24 * 60 * 60);
            fs::File::open(cache.with_extension("lock"))
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(modified))
                .unwrap();
            assert_eq!(lock.modified().unwrap(), modified);
            drop(lock);
            let path = root
                .join("checkouts")
                .join(SPARE_DIR_NAME)
                .join(&repo.owner);
            fs::create_dir_all(&path).unwrap();
            let identity = DirectoryEvidence::capture(&fs::File::open(&path).unwrap()).unwrap();
            let location = Namespace {
                path,
                identity,
                ready: None,
            };
            Self {
                _directory: directory,
                state,
                cache,
                repo,
                location,
            }
        }

        /// Establish the ready identity before testing its retention or removal.
        /// The marker distinguishes replacement content from the original spare.
        fn ready(&mut self) -> PathBuf {
            let path = self.location.path.join(&self.repo.name);
            fs::create_dir(&path).unwrap();
            fs::write(path.join("marker"), "ready spare").unwrap();
            self.location.ready =
                Some(DirectoryEvidence::capture(&fs::File::open(&path).unwrap()).unwrap());
            path
        }

        /// A partial builder may have no per-tree record. Only the verified
        /// namespace and this strict generated name authorize its recovery.
        fn building(&self) -> PathBuf {
            let path = self
                .location
                .path
                .join(format!(".repo:building-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            fs::write(path.join("partial"), "partial clone").unwrap();
            path
        }

        /// Claim names bind cleanup to one accepted launch, not a session's
        /// newer generation. A marker makes unintended recursive removal visible.
        fn claim(&self, session: &str, generation: i64) -> PathBuf {
            let path = self
                .location
                .path
                .join(format!(".repo:claimed-{session}-{generation}"));
            fs::create_dir(&path).unwrap();
            fs::write(path.join("partial"), "claimed clone").unwrap();
            path
        }

        /// Publish fixtures in the actual private record format. Production
        /// initialization belongs to the later builder unit and is not added here.
        fn remember(&self, locations: Vec<Namespace>) {
            fs::write(
                self.cache.with_extension("spare.json"),
                serde_json::to_vec(&SpareRecord { locations }).unwrap(),
            )
            .unwrap();
        }

        /// Read back retained recovery evidence after the actual sweep wrote it.
        /// Tests must distinguish a retained location from content left untracked.
        fn record(&self) -> SpareRecord {
            serde_json::from_slice(&fs::read(self.cache.with_extension("spare.json")).unwrap())
                .unwrap()
        }
    }

    /// Recovery accepts the exact generated grammar, never a convenient
    /// substring, UUID alias, negative generation or another repository's name.
    #[farhelm_testtrace::test]
    fn private_names_are_strict_and_repository_scoped() {
        let session = uuid::Uuid::new_v4().to_string();
        assert_eq!(
            private_name("repo", &format!(".repo:building-{session}")),
            Some(PrivateName::Building)
        );
        assert_eq!(
            private_name("repo", &format!(".repo:trash-{session}")),
            Some(PrivateName::Trash)
        );
        assert_eq!(
            private_name("repo", &format!(".repo:claimed-{session}-7")),
            Some(PrivateName::Claimed {
                session: session.clone(),
                generation: 7
            })
        );
        for name in [
            format!(".other:building-{session}"),
            format!("prefix.repo:building-{session}"),
            format!(".repo:building-{}", session.replace('-', "")),
            format!(".repo:claimed-{session}-07"),
            format!(".repo:claimed-{session}--1"),
            format!(".repo:trash-{session}-extra"),
            format!(".repo.building-{session}"),
        ] {
            assert_eq!(private_name("repo", &name), None, "{name}");
        }
    }

    /// Repository names can look exactly like the old private grammar. Their
    /// ready slots must survive another repository's eviction even though both
    /// records share one owner namespace and the other repository's cache is locked.
    #[farhelm_testtrace::test]
    fn generated_names_preserve_other_repository_ready_slots() {
        let id = uuid::Uuid::new_v4();
        for old_basename in [
            format!(".repo.building-{id}"),
            format!(".repo.trash-{id}"),
            format!(".repo.claimed-{id}-7"),
        ] {
            let mut fixture = Fixture::new(31);
            let ready = fixture.ready();
            let building = fixture.building();
            let claim = fixture.claim(&uuid::Uuid::new_v4().to_string(), 7);
            fixture.remember(vec![fixture.location.clone()]);
            let other_repo = farhelm_proto::github_checkout::parse_github_repo(&format!(
                "example/{old_basename}"
            ))
            .expect("the real repository parser accepts the old generated-looking basename");
            assert_eq!(other_repo.name, old_basename);
            assert!(
                farhelm_proto::github_checkout::parse_github_repo(&format!(
                    "example/.repo:building-{id}"
                ),)
                .is_err(),
                "the new private delimiter must be outside the repository alphabet"
            );
            let other_cache = crate::repo_cache::path(&fixture.state, &other_repo);
            let other_lock = crate::repo_cache::acquire(&other_cache).unwrap();
            let contender =
                crate::launch::PreparationLock::open(&other_cache.with_extension("lock")).unwrap();
            assert!(
                !contender.try_acquire().unwrap(),
                "the other repository is independently in use"
            );
            let other_ready = fixture.location.path.join(&other_repo.name);
            fs::create_dir(&other_ready).unwrap();
            fs::write(other_ready.join("marker"), "other repository ready").unwrap();
            let other_identity =
                DirectoryEvidence::capture(&fs::File::open(&other_ready).unwrap()).unwrap();
            let other_record = other_cache.with_extension("spare.json");
            let mut other_location = fixture.location.clone();
            other_location.ready = Some(other_identity);
            let other_bytes = serde_json::to_vec(&SpareRecord {
                locations: vec![other_location],
            })
            .unwrap();
            fs::write(&other_record, &other_bytes).unwrap();
            assert!(ready.exists() && building.exists() && claim.exists());
            assert_eq!(
                fs::read_to_string(other_ready.join("marker")).unwrap(),
                "other repository ready"
            );
            other_identity
                .verify(&fs::File::open(&other_ready).unwrap())
                .unwrap();

            crate::repo_cache::sweep(&fixture.state, &LiveClaims::default()).unwrap();

            assert!(!ready.exists() && !building.exists() && !claim.exists());
            assert!(!fixture.cache.exists());
            assert!(!fixture.cache.with_extension("spare.json").exists());
            assert_eq!(
                fs::read_to_string(other_ready.join("marker")).unwrap(),
                "other repository ready"
            );
            other_identity
                .verify(&fs::File::open(&other_ready).unwrap())
                .unwrap();
            assert_eq!(fs::read(&other_record).unwrap(), other_bytes);
            drop(other_lock);
        }
    }

    /// Deletion can succeed while its directory fsync fails. Both that first
    /// failure and an absence-only retry must retain the original ready identity
    /// and location until the checkout filesystem confirms the mutation durable.
    #[farhelm_testtrace::test]
    fn namespace_sync_failure_retains_original_evidence_until_durable_retry() {
        let mut fixture = Fixture::new(31);
        let ready = fixture.ready();
        fixture.remember(vec![fixture.location.clone()]);
        let record_path = fixture.cache.with_extension("spare.json");
        let original = fs::read(&record_path).unwrap();
        assert_eq!(
            fs::read_to_string(ready.join("marker")).unwrap(),
            "ready spare"
        );
        let mut failed_barriers = 0;
        for attempt in 0..2 {
            if attempt == 1 {
                assert!(
                    !ready.exists(),
                    "the second attempt must be an absence-only retry"
                );
                assert!(
                    namespace_names(&fs::File::open(&fixture.location.path).unwrap(), 16)
                        .unwrap()
                        .is_empty()
                );
            }
            let result = sweep_with_namespace_sync(
                &fixture.cache,
                &fixture.repo,
                &LiveClaims::default(),
                true,
                &mut |namespace| {
                    fixture.location.identity.verify(namespace).unwrap();
                    assert!(
                        !ready.exists(),
                        "removal must precede the actual namespace barrier"
                    );
                    failed_barriers += 1;
                    Err(std::io::Error::other("injected namespace fsync failure"))
                },
            );
            assert!(result.is_err());
            assert_eq!(failed_barriers, attempt + 1);
            assert!(!ready.exists());
            assert_eq!(fs::read(&record_path).unwrap(), original);
            let retained = fixture.record();
            assert_eq!(retained.locations.len(), 1);
            assert_eq!(retained.locations[0].path, fixture.location.path);
            assert_eq!(retained.locations[0].ready, fixture.location.ready);
        }
        let mut successful_barriers = 0;
        sweep_with_namespace_sync(
            &fixture.cache,
            &fixture.repo,
            &LiveClaims::default(),
            true,
            &mut |namespace| {
                fixture.location.identity.verify(namespace).unwrap();
                successful_barriers += 1;
                namespace.sync_all()
            },
        )
        .unwrap();
        assert_eq!(successful_barriers, 1);
        assert!(!ready.exists());
        assert!(!record_path.exists());
    }

    /// A namespace replacement must not transfer the old record's authority,
    /// even when its replacement contains perfectly generated cleanup names.
    #[farhelm_testtrace::test]
    fn replaced_namespace_and_invalid_record_paths_are_retained() {
        let fixture = Fixture::new(31);
        let building = fixture.building();
        fixture.remember(vec![fixture.location.clone()]);
        let parked = fixture.location.path.with_file_name("parked");
        fs::rename(&fixture.location.path, &parked).unwrap();
        fs::create_dir(&fixture.location.path).unwrap();
        let replacement = fixture.building();
        assert!(sweep(&fixture.cache, &fixture.repo, &LiveClaims::default(), true).is_err());
        assert_eq!(
            fs::read_to_string(replacement.join("partial")).unwrap(),
            "partial clone"
        );
        assert!(
            parked
                .join(building.file_name().unwrap())
                .join("partial")
                .exists()
        );
        assert_eq!(fixture.record().locations.len(), 1);

        let mut invalid = fixture.location.clone();
        invalid.path = parked;
        fixture.remember(vec![invalid]);
        assert!(sweep(&fixture.cache, &fixture.repo, &LiveClaims::default(), true).is_err());
        assert!(replacement.join("partial").exists());
    }

    /// Namespace ownership does not adopt a replacement ready slot. Its own
    /// recorded identity is required independently of private-leftover recovery.
    #[farhelm_testtrace::test]
    fn replaced_ready_spare_is_not_adopted() {
        let mut fixture = Fixture::new(31);
        let ready = fixture.ready();
        fixture.remember(vec![fixture.location.clone()]);
        fs::rename(&ready, fixture.location.path.join("saved-original")).unwrap();
        fs::create_dir(&ready).unwrap();
        fs::write(ready.join("marker"), "replacement").unwrap();
        assert!(sweep(&fixture.cache, &fixture.repo, &LiveClaims::default(), true).is_err());
        assert_eq!(
            fs::read_to_string(ready.join("marker")).unwrap(),
            "replacement"
        );
        assert_eq!(fixture.record().locations[0].ready, fixture.location.ready);
    }

    /// Following a reserved-container symlink would select the same recorded
    /// owner inode while escaping confinement. The lookup itself must refuse it.
    #[farhelm_testtrace::test]
    fn reserved_ancestor_symlink_is_refused_even_with_matching_namespace_identity() {
        let fixture = Fixture::new(31);
        let building = fixture.building();
        fixture.remember(vec![fixture.location.clone()]);
        let container = fixture.location.path.parent().unwrap();
        let parked = container.with_file_name("parked-spares");
        fs::rename(container, &parked).unwrap();
        std::os::unix::fs::symlink(&parked, container).unwrap();
        assert_eq!(
            DirectoryEvidence::capture(&fs::File::open(&fixture.location.path).unwrap()).unwrap(),
            fixture.location.identity
        );
        assert!(sweep(&fixture.cache, &fixture.repo, &LiveClaims::default(), true).is_err());
        assert!(building.join("partial").exists());
        assert_eq!(fixture.record().locations.len(), 1);
    }

    /// A mount-like child-open refusal stops the actual shared recursive
    /// remover. Never substitute a pathname delete after confinement fails.
    #[farhelm_testtrace::test]
    fn confined_spare_removal_keeps_mount_contents() {
        let fixture = Fixture::new(31);
        let name = CString::new(format!(".repo:trash-{}", uuid::Uuid::new_v4())).unwrap();
        let trash = fixture.location.path.join(name.to_str().unwrap());
        fs::create_dir(&trash).unwrap();
        fs::create_dir(trash.join("mounted")).unwrap();
        fs::write(trash.join("mounted/precious"), "outside mount").unwrap();
        let parent = fs::File::open(&fixture.location.path).unwrap();
        let directory = open_child_dir(&parent, &name, true).unwrap();
        let mut refused = false;
        let result = crate::checkout_trash::remove_opened_tree_with(
            &parent,
            &directory,
            &name,
            &mut |parent, name| {
                if name.to_bytes() == b"mounted" {
                    refused = true;
                    Err(std::io::Error::from_raw_os_error(libc::EXDEV))
                } else {
                    open_child_dir(parent, name, true)
                }
            },
        );
        assert!(
            refused,
            "the real traversal must reach the injected mount boundary"
        );
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(trash.join("mounted/precious")).unwrap(),
            "outside mount"
        );
    }

    /// Ready spares follow the cache's real thirty-day age; private building
    /// debris is removed on either startup, with no separate retention clock.
    #[farhelm_testtrace::test]
    fn cache_eviction_and_spare_retention_share_the_same_observation() {
        for age in [1, 31] {
            let mut fixture = Fixture::new(age);
            let ready = fixture.ready();
            let building = fixture.building();
            fixture.remember(vec![fixture.location.clone()]);
            assert!(ready.join("marker").exists() && building.join("partial").exists());
            crate::repo_cache::sweep(&fixture.state, &LiveClaims::default()).unwrap();
            assert!(!building.exists());
            assert_eq!(ready.exists(), age == 1);
            assert_eq!(fixture.cache.exists(), age == 1);
            assert_eq!(
                fixture.cache.with_extension("spare.json").exists(),
                age == 1
            );
            assert!(fixture.cache.with_extension("lock").exists());
        }
    }

    /// A surviving clone's existing cache flock excludes both eviction and
    /// spare cleanup. Once released, the same sweep can recover both trees.
    #[farhelm_testtrace::test]
    fn held_cache_lock_skips_all_spare_cleanup() {
        let mut fixture = Fixture::new(31);
        let ready = fixture.ready();
        let building = fixture.building();
        fixture.remember(vec![fixture.location.clone()]);
        let lock = crate::repo_cache::acquire(&fixture.cache).unwrap();
        let contender =
            crate::launch::PreparationLock::open(&fixture.cache.with_extension("lock")).unwrap();
        assert!(!contender.try_acquire().unwrap());
        crate::repo_cache::sweep(&fixture.state, &LiveClaims::default()).unwrap();
        assert!(ready.join("marker").exists() && building.join("partial").exists());
        assert!(fixture.cache.exists());
        drop(lock);
        crate::repo_cache::sweep(&fixture.state, &LiveClaims::default()).unwrap();
        assert!(!ready.exists() && !building.exists() && !fixture.cache.exists());
    }

    /// Exact live launch evidence protects its claim through cache eviction.
    /// A different generation is not live merely because its session is.
    #[farhelm_testtrace::test]
    fn live_claim_survives_but_dead_generation_is_cleaned() {
        let fixture = Fixture::new(31);
        let session = uuid::Uuid::new_v4().to_string();
        let live_claim = fixture.claim(&session, 7);
        let dead_claim = fixture.claim(&session, 6);
        fixture.remember(vec![fixture.location.clone()]);
        let mut live = LiveClaims::default();
        live.protected.insert((session, 7));
        crate::repo_cache::sweep(&fixture.state, &live).unwrap();
        assert!(live_claim.join("partial").exists());
        assert!(!dead_claim.exists());
        assert!(!fixture.cache.exists());
        assert_eq!(fixture.record().locations.len(), 1);
        crate::repo_cache::sweep(&fixture.state, &LiveClaims::default()).unwrap();
        assert!(!live_claim.exists());
        assert!(!fixture.cache.with_extension("spare.json").exists());
    }

    /// Failed liveness observation retains every possible claim, while a
    /// generated orphan builder still has no live-clone protection to inherit.
    #[farhelm_testtrace::test]
    fn uncertain_liveness_retains_claims_without_retaining_orphan_builds() {
        let fixture = Fixture::new(1);
        let claim = fixture.claim(&uuid::Uuid::new_v4().to_string(), 1);
        let building = fixture.building();
        fixture.remember(vec![fixture.location.clone()]);
        let live = LiveClaims {
            uncertain: true,
            ..Default::default()
        };
        crate::repo_cache::sweep(&fixture.state, &live).unwrap();
        assert!(claim.join("partial").exists());
        assert!(!building.exists());
        assert_eq!(fixture.record().locations.len(), 1);
    }

    /// A new checkout root must not overwrite the only pointer to an old
    /// live claim. Retire the old location only after its generated objects go.
    #[farhelm_testtrace::test]
    fn old_root_is_remembered_until_its_live_claim_is_gone() {
        let fixture = Fixture::new(1);
        let session = uuid::Uuid::new_v4().to_string();
        let claim = fixture.claim(&session, 2);
        let root = fixture.location.path.ancestors().nth(3).unwrap();
        let new_path = root
            .join("new-checkouts")
            .join(SPARE_DIR_NAME)
            .join(&fixture.repo.owner);
        fs::create_dir_all(new_path.join("repo")).unwrap();
        fs::write(new_path.join("repo/marker"), "new ready").unwrap();
        let new_location = Namespace {
            identity: DirectoryEvidence::capture(&fs::File::open(&new_path).unwrap()).unwrap(),
            ready: Some(
                DirectoryEvidence::capture(&fs::File::open(new_path.join("repo")).unwrap())
                    .unwrap(),
            ),
            path: new_path.clone(),
        };
        fixture.remember(vec![fixture.location.clone(), new_location]);
        let mut live = LiveClaims::default();
        live.protected.insert((session, 2));
        crate::repo_cache::sweep(&fixture.state, &live).unwrap();
        assert!(claim.exists());
        assert_eq!(fixture.record().locations.len(), 2);
        crate::repo_cache::sweep(&fixture.state, &LiveClaims::default()).unwrap();
        assert!(!claim.exists());
        assert_eq!(fixture.record().locations.len(), 1);
        assert_eq!(fixture.record().locations[0].path, new_path);
        assert_eq!(
            fs::read_to_string(new_path.join("repo/marker")).unwrap(),
            "new ready"
        );
    }

    /// Missing ready content is not an empty namespace: unrecorded generated
    /// builders are recovered, while near-matching and other-repo names remain.
    #[farhelm_testtrace::test]
    fn generated_orphans_are_cleaned_without_adopting_unknown_names() {
        let fixture = Fixture::new(1);
        let building = fixture.building();
        let unknown = fixture.location.path.join(".repo:building-not-a-uuid");
        let other = fixture
            .location
            .path
            .join(format!(".other:building-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&unknown).unwrap();
        fs::create_dir(&other).unwrap();
        fixture.remember(vec![fixture.location.clone()]);
        assert!(!fixture.location.path.join("repo").exists());
        crate::repo_cache::sweep(&fixture.state, &LiveClaims::default()).unwrap();
        assert!(!building.exists());
        assert!(unknown.exists() && other.exists());
        assert!(!fixture.cache.with_extension("spare.json").exists());
    }

    /// Resource refusal retains ownership and content rather than parsing a
    /// truncated record or treating an incomplete namespace as empty.
    #[farhelm_testtrace::test]
    fn oversized_record_and_incomplete_namespace_scan_fail_closed() {
        let fixture = Fixture::new(1);
        let building = fixture.building();
        fixture.remember(vec![fixture.location.clone()]);
        let record_path = fixture.cache.with_extension("spare.json");
        fs::OpenOptions::new()
            .write(true)
            .open(&record_path)
            .unwrap()
            .set_len(MAX_RECORD_BYTES + 1)
            .unwrap();
        assert_eq!(
            fs::metadata(&record_path).unwrap().len(),
            MAX_RECORD_BYTES + 1
        );
        assert!(sweep(&fixture.cache, &fixture.repo, &LiveClaims::default(), false).is_err());
        assert_eq!(
            fs::metadata(&record_path).unwrap().len(),
            MAX_RECORD_BYTES + 1
        );
        assert_eq!(
            fs::read_to_string(building.join("partial")).unwrap(),
            "partial clone"
        );
        let namespace = fs::File::open(&fixture.location.path).unwrap();
        assert!(namespace_names(&namespace, 1).is_err());
        assert_eq!(
            namespace_names(&namespace, 16).unwrap(),
            vec![CString::new(building.file_name().unwrap().to_str().unwrap()).unwrap()]
        );
        assert!(building.join("partial").exists());
    }
}
