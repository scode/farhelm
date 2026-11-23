//! A private copy of a vendor's configuration for agents whose Farhelm hooks are global files.
//!
//! Grok and Antigravity (every kind for which `AgentKind::offers_persistent_hooks` is true) discover Farhelm's
//! conversation hooks from files in the user's own configuration, `~/.grok/hooks/farhelm.json` and
//! `~/.gemini/config/plugins/farhelm/`. A drift case must run Farhelm's real setup for them, since that setup is part
//! of what users get, but it must never run it against the maintainer's real files. Those files name the `farhelm`
//! that wrote them, here the build under test, and every session of that agent on the machine runs them. A drift
//! run that rewrote them would point the maintainer's installed Farhelm, and plain runs of the agent, at a test
//! binary that later disappears, and the installed Farhelm would rewrite them back at its next consented launch.
//!
//! So each such case gets a mirror of the vendor's real directory: every entry is a symlink to the real one, except
//! the paths Farhelm's setup writes and the directories above them, which are real private directories. The vendor
//! keeps its login, sessions and settings; Farhelm's real setup writes its files into the mirror through the
//! supervisor's `user_home`/`grok_home` seams; the agent reads the mirror through `HOME` or `GROK_HOME` in the
//! supervisor's `launch_env`, which also applies on Restart. The planned paths come from the supervisor itself
//! (`persistent_hook_paths_for_test`), so a vendor's hook paths are stated only in its own module.
//!
//! One gap needs care: a vendor that saves a file by renaming a new copy over it replaces the mirror's symlink and
//! leaves the real file as it was. A refreshed login token would then be lost with the mirror. [`PrivateConfig::finish`]
//! copies such a replaced file back when the real one has not changed meanwhile, and reports it otherwise. Entries
//! the vendor creates inside the private directories themselves are dropped with the mirror.
//!
//! Adding a kind with persistent hooks needs nothing here as long as its paths lie under the home or Grok's home;
//! the guard test below fails for one that does not, and for one without a drift profile.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context as _, bail, ensure};
use farhelm_proto::AgentKind;
use farhelm_supervisor::service::SupervisorSeams;

/// A directory the supervisor resolves hook paths against, and the agent variable that redirects the agent to it.
///
/// Exhaustive on purpose: a vendor whose hook files live under a third directory needs a supervisor seam for it
/// first, and then a variant here, or its real configuration would be the one written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Root {
    /// The user's home: `user_home` in the supervisor, `HOME` for the agent.
    Home,
    /// Grok's configuration root: `grok_home` in the supervisor, `GROK_HOME` for the agent.
    GrokHome,
}

impl Root {
    const ALL: [Root; 2] = [Root::Home, Root::GrokHome];

    /// The variable the agent reads this directory from.
    fn variable(self) -> &'static str {
        match self {
            Root::Home => "HOME",
            Root::GrokHome => "GROK_HOME",
        }
    }

    /// Where the maintainer's real directory is, resolved the way the supervisor resolves it. Reading the test
    /// process's environment is fine; it is never changed.
    fn real(self) -> anyhow::Result<PathBuf> {
        let home = || {
            std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(PathBuf::from)
                .context("HOME is not set")
        };
        match self {
            Root::Home => home(),
            Root::GrokHome => match std::env::var_os("GROK_HOME").filter(|root| !root.is_empty()) {
                Some(root) => Ok(PathBuf::from(root)),
                None => Ok(home()?.join(".grok")),
            },
        }
    }

    /// The scratch directory a planning run resolves this root to.
    fn planning_dir(self, scratch: &Path) -> PathBuf {
        match self {
            Root::Home => scratch.join("plan-home"),
            Root::GrokHome => scratch.join("plan-grok-home"),
        }
    }
}

/// Which root a planned hook path belongs to, and the path relative to it. A path under neither is refused: the
/// mirror cannot stand in for a directory it does not know, and writing it anyway would reach the real one.
pub(super) fn classify(path: &Path, scratch: &Path) -> anyhow::Result<(Root, PathBuf)> {
    for root in Root::ALL {
        if let Ok(relative) = path.strip_prefix(root.planning_dir(scratch)) {
            ensure!(
                !relative.as_os_str().is_empty()
                    && relative
                        .components()
                        .all(|component| matches!(component, Component::Normal(_))),
                "planned hook path {} is not a plain path below its root",
                path.display()
            );
            return Ok((root, relative.to_path_buf()));
        }
    }
    bail!(
        "Farhelm's hook setup plans {}, which is under neither the home nor Grok's home; the drift test cannot keep \
         it private (see drift/private_config.rs)",
        path.display()
    )
}

/// A regular file the mirror links to, as it was when the mirror was built: enough to tell whether the maintainer's
/// copy changed while the case ran.
#[derive(Debug, PartialEq, Eq)]
struct Stamp {
    inode: u64,
    len: u64,
    modified_nanos: i128,
}

impl Stamp {
    fn of(path: &Path) -> Option<Self> {
        let metadata = fs::metadata(path).ok().filter(fs::Metadata::is_file)?;
        Some(Self {
            inode: metadata.ino(),
            len: metadata.len(),
            modified_nanos: i128::from(metadata.mtime()) * 1_000_000_000
                + i128::from(metadata.mtime_nsec()),
        })
    }
}

/// One root's mirror: the private directory, the hook files kept out of it, and the links it holds.
#[derive(Debug)]
struct Mirror {
    root: Root,
    real: PathBuf,
    private: PathBuf,
    /// Hook files relative to the root, with the real file's bytes before the case (`None` when absent).
    leaves: Vec<(PathBuf, Option<Vec<u8>>)>,
    /// Every symlink the mirror made, relative to the root, with the stamp of a regular-file target.
    links: Vec<(PathBuf, Option<Stamp>)>,
}

impl Mirror {
    /// Build `private` as a mirror of `real` with `leaves` left out. Every directory above a leaf is a real private
    /// directory, so the setup's writes cannot pass through a link into the real tree; every other entry in those
    /// directories is a symlink to the real entry. A real directory that does not exist yet mirrors as empty.
    fn build(root: Root, real: &Path, private: &Path, leaves: &[PathBuf]) -> anyhow::Result<Self> {
        let leaf_set: BTreeSet<&Path> = leaves.iter().map(PathBuf::as_path).collect();
        let mut dirs = BTreeSet::from([PathBuf::new()]);
        for leaf in leaves {
            dirs.extend(leaf.ancestors().skip(1).map(Path::to_path_buf));
        }
        let mut links = Vec::new();
        // BTreeSet order puts every directory after its parent.
        for dir in &dirs {
            fs::create_dir(private.join(dir))
                .with_context(|| format!("creating {}", private.join(dir).display()))?;
            let source = real.join(dir);
            let entries = match fs::read_dir(&source) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(error).with_context(|| format!("reading {}", source.display()));
                }
            };
            for entry in entries {
                let relative = dir.join(entry?.file_name());
                if dirs.contains(&relative) || leaf_set.contains(relative.as_path()) {
                    continue;
                }
                std::os::unix::fs::symlink(real.join(&relative), private.join(&relative))
                    .with_context(|| format!("linking {}", private.join(&relative).display()))?;
                links.push((relative.clone(), Stamp::of(&real.join(&relative))));
            }
        }
        let leaves = leaves
            .iter()
            .map(|leaf| (leaf.clone(), fs::read(real.join(leaf)).ok()))
            .collect();
        Ok(Self {
            root,
            real: real.to_path_buf(),
            private: private.to_path_buf(),
            leaves,
            links,
        })
    }

    /// The maintainer's hook files must be exactly as they were. Checked after setup and again at the end.
    fn real_leaves_unchanged(&self) -> anyhow::Result<()> {
        for (leaf, before) in &self.leaves {
            let now = fs::read(self.real.join(leaf)).ok();
            ensure!(
                now == *before,
                "the real {} changed while a drift case ran",
                self.real.join(leaf).display()
            );
        }
        Ok(())
    }

    /// Farhelm's setup wrote every planned file into the mirror as a regular file of its own.
    fn private_leaves_installed(&self) -> anyhow::Result<()> {
        for (leaf, _) in &self.leaves {
            let path = self.private.join(leaf);
            let metadata = fs::symlink_metadata(&path)
                .with_context(|| format!("Farhelm's setup did not write {}", path.display()))?;
            ensure!(
                metadata.is_file(),
                "{} is not a regular file",
                path.display()
            );
        }
        Ok(())
    }

    /// Copy back regular files the vendor replaced in the mirror, when the real file is unchanged since the mirror
    /// was built. Returns a note for each replaced file it could not copy back.
    fn copy_back(&self) -> anyhow::Result<Vec<String>> {
        let mut notes = Vec::new();
        for (relative, stamp) in &self.links {
            let private = self.private.join(relative);
            let Ok(metadata) = fs::symlink_metadata(&private) else {
                continue; // The vendor removed the link (a lock file, say); the real entry is as it was.
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }
            let real = self.real.join(relative);
            if stamp.is_none() || Stamp::of(&real) != *stamp {
                notes.push(format!(
                    "{} was rewritten during the case, but the real {} changed too; the case's copy was dropped",
                    relative.display(),
                    real.display()
                ));
                continue;
            }
            // Resolve the maintainer's own symlinks (dotfiles, say) so the rename replaces the file, not the link.
            let target =
                fs::canonicalize(&real).with_context(|| format!("resolving {}", real.display()))?;
            let staged = target.with_file_name(format!(
                ".{}.farhelm-drift-{}",
                target
                    .file_name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_default(),
                std::process::id()
            ));
            fs::copy(&private, &staged).with_context(|| format!("staging {}", staged.display()))?;
            fs::set_permissions(&staged, fs::metadata(&target)?.permissions())?;
            fs::rename(&staged, &target)
                .with_context(|| format!("replacing {}", target.display()))?;
        }
        Ok(notes)
    }
}

/// The private configuration of one drift case, for an agent whose Farhelm hooks are global files.
#[derive(Debug)]
pub(super) struct PrivateConfig {
    mirrors: Vec<Mirror>,
}

impl PrivateConfig {
    /// `None` for an agent without persistent hooks. Otherwise plans the setup's paths in scratch directories under
    /// `scratch` and builds one mirror per root those paths use, also under `scratch`.
    pub(super) async fn prepare(kind: AgentKind, scratch: &Path) -> anyhow::Result<Option<Self>> {
        if !kind.offers_persistent_hooks() {
            return Ok(None);
        }
        let mut by_root = std::collections::BTreeMap::<Root, Vec<PathBuf>>::new();
        for path in plan(kind, scratch).await? {
            let (root, relative) = classify(&path, scratch)?;
            by_root.entry(root).or_default().push(relative);
        }
        ensure!(
            !by_root.is_empty(),
            "Farhelm's setup plans no hook files for {kind:?}"
        );
        let mut mirrors = Vec::new();
        for (root, leaves) in by_root {
            let private = scratch.join(format!("private-{}", root.variable().to_ascii_lowercase()));
            mirrors.push(Mirror::build(root, &root.real()?, &private, &leaves)?);
        }
        Ok(Some(Self { mirrors }))
    }

    /// Point the supervisor's setup and the agent's environment at the mirrors.
    pub(super) fn apply(&self, seams: &mut SupervisorSeams) {
        for mirror in &self.mirrors {
            match mirror.root {
                Root::Home => seams.user_home = Some(mirror.private.clone()),
                Root::GrokHome => seams.grok_home = Some(mirror.private.clone()),
            }
            seams.launch_env.push((
                mirror.root.variable().into(),
                mirror.private.to_string_lossy().into_owned(),
            ));
        }
    }

    /// The fixture premise once the session exists: Farhelm's setup wrote every planned file into a mirror, and none
    /// of the maintainer's real hook files changed.
    pub(super) fn check_installed(&self) -> anyhow::Result<()> {
        for mirror in &self.mirrors {
            mirror.private_leaves_installed()?;
            mirror.real_leaves_unchanged()?;
        }
        Ok(())
    }

    /// Run after the case, on every path: re-check the real hook files and copy back what the vendor rewrote. The
    /// notes name rewritten files that could not be copied back.
    pub(super) fn finish(&self) -> anyhow::Result<Vec<String>> {
        let mut notes = Vec::new();
        for mirror in &self.mirrors {
            notes.extend(mirror.copy_back()?);
            mirror.real_leaves_unchanged()?;
        }
        Ok(notes)
    }
}

/// Ask the supervisor which files its setup writes for `kind`, with both roots in fresh scratch directories.
async fn plan(kind: AgentKind, scratch: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let (home, grok_home) = (
        Root::Home.planning_dir(scratch),
        Root::GrokHome.planning_dir(scratch),
    );
    fs::create_dir_all(&home)?;
    fs::create_dir_all(&grok_home)?;
    farhelm_supervisor::agent_kind::persistent_hook_paths_for_test(kind, &home, &grok_home).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::LaunchHarness;

    /// The guard for future agents. A kind with global hook files must get a drift profile and plan only paths a
    /// mirror can stand in for; otherwise a drift run would either skip the setup users get or write the
    /// maintainer's real configuration. Kinds without persistent hooks must get no mirror at all.
    #[farhelm_testtrace::test]
    async fn every_agent_with_global_hook_files_runs_its_setup_in_a_private_copy() {
        let scratch = farhelm_teststate::tempdir().unwrap();
        let mut covered = 0;
        for harness in LaunchHarness::ALL {
            let kind = harness.agent_kind();
            let case = scratch.path().join(format!("{harness:?}"));
            fs::create_dir(&case).unwrap();
            if !kind.offers_persistent_hooks() {
                assert!(
                    plan(kind, &case).await.is_err(),
                    "{harness:?} plans hooks it does not offer"
                );
                continue;
            }
            assert!(
                super::super::profile(*harness).is_some(),
                "{harness:?} has global hook files but no drift profile"
            );
            let planned = plan(kind, &case).await.unwrap();
            assert!(!planned.is_empty(), "{harness:?} plans no hook files");
            for path in planned {
                classify(&path, &case).unwrap_or_else(|error| panic!("{harness:?}: {error:#}"));
            }
            covered += 1;
        }
        assert!(
            covered >= 2,
            "Grok and Antigravity are expected to have global hook files"
        );
    }

    /// A path outside both roots cannot be mirrored, and must be refused rather than written for real.
    #[farhelm_testtrace::test]
    fn a_hook_path_outside_both_roots_is_refused() {
        let scratch = Path::new("/scratch");
        let (root, relative) = classify(
            Path::new("/scratch/plan-home/.gemini/config/x.json"),
            scratch,
        )
        .unwrap();
        assert_eq!(
            (root, relative),
            (Root::Home, PathBuf::from(".gemini/config/x.json"))
        );
        assert!(classify(Path::new("/etc/vendor/hooks.json"), scratch).is_err());
        assert!(classify(Path::new("/scratch/plan-home"), scratch).is_err());
        assert!(classify(Path::new("/scratch/plan-home/../escape.json"), scratch).is_err());
    }

    /// A real vendor tree with a sibling file, a sibling directory, an existing Farhelm hook file and the user's
    /// own hook file beside it.
    fn real_tree(real: &Path) {
        fs::create_dir_all(real.join("config/plugins/farhelm")).unwrap();
        fs::create_dir_all(real.join("sessions")).unwrap();
        fs::write(real.join("auth.json"), "token-1").unwrap();
        fs::write(real.join("config/hooks.json"), "user hooks").unwrap();
        fs::write(
            real.join("config/plugins/farhelm/hooks.json"),
            "installed farhelm",
        )
        .unwrap();
    }

    /// The trust boundary itself: ancestors of a hook file are private directories, everything else links to the
    /// real entry, the real hook file is neither linked nor touched when the private one is written, and the
    /// premise checks see both.
    #[farhelm_testtrace::test]
    fn the_mirror_keeps_hook_files_private_and_links_everything_else() {
        let scratch = farhelm_teststate::tempdir().unwrap();
        let (real, private) = (scratch.path().join("real"), scratch.path().join("private"));
        real_tree(&real);
        let leaf = PathBuf::from("config/plugins/farhelm/hooks.json");
        let mirror =
            Mirror::build(Root::Home, &real, &private, std::slice::from_ref(&leaf)).unwrap();

        for dir in ["", "config", "config/plugins", "config/plugins/farhelm"] {
            let metadata = fs::symlink_metadata(private.join(dir)).unwrap();
            assert!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "{dir:?} must be private"
            );
        }
        for linked in ["auth.json", "sessions", "config/hooks.json"] {
            assert_eq!(
                fs::read_link(private.join(linked)).unwrap(),
                real.join(linked)
            );
        }
        assert!(fs::symlink_metadata(private.join(&leaf)).is_err());
        assert!(mirror.private_leaves_installed().is_err());

        fs::write(private.join(&leaf), "this build's farhelm").unwrap();
        mirror.private_leaves_installed().unwrap();
        mirror.real_leaves_unchanged().unwrap();
        assert_eq!(
            fs::read_to_string(real.join(&leaf)).unwrap(),
            "installed farhelm"
        );

        fs::write(real.join(&leaf), "changed").unwrap();
        assert!(mirror.real_leaves_unchanged().is_err());
    }

    /// A vendor that saves by rename replaces the link; the new file must reach the real tree unless the real file
    /// also changed, in which case the newer real file wins and the loss is reported.
    #[farhelm_testtrace::test]
    fn a_file_the_vendor_replaced_is_copied_back_only_over_an_unchanged_original() {
        let scratch = farhelm_teststate::tempdir().unwrap();
        let (real, private) = (scratch.path().join("real"), scratch.path().join("private"));
        real_tree(&real);
        fs::write(real.join("state.json"), "state-1").unwrap();
        let leaf = PathBuf::from("config/plugins/farhelm/hooks.json");
        let mirror = Mirror::build(Root::GrokHome, &real, &private, &[leaf]).unwrap();

        for (name, content) in [("auth.json", "token-2"), ("state.json", "state-2")] {
            let staged = private.join(format!("{name}.new"));
            fs::write(&staged, content).unwrap();
            fs::rename(&staged, private.join(name)).unwrap();
        }
        // The maintainer's own session rewrote state.json meanwhile.
        fs::remove_file(real.join("state.json")).unwrap();
        fs::write(real.join("state.json"), "state-from-elsewhere").unwrap();

        let notes = mirror.copy_back().unwrap();
        assert_eq!(
            fs::read_to_string(real.join("auth.json")).unwrap(),
            "token-2"
        );
        assert_eq!(
            fs::read_to_string(real.join("state.json")).unwrap(),
            "state-from-elsewhere"
        );
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("state.json"));
        // Dropping the mirror removes links, never what they point at.
        fs::remove_dir_all(&private).unwrap();
        assert_eq!(
            fs::read_to_string(real.join("config/hooks.json")).unwrap(),
            "user hooks"
        );
    }

    /// A vendor directory that does not exist yet mirrors as an empty private tree with the ancestors in place.
    #[farhelm_testtrace::test]
    fn a_missing_real_directory_mirrors_as_empty() {
        let scratch = farhelm_teststate::tempdir().unwrap();
        let (real, private) = (
            scratch.path().join("absent"),
            scratch.path().join("private"),
        );
        let leaf = PathBuf::from("hooks/farhelm.json");
        let mirror =
            Mirror::build(Root::GrokHome, &real, &private, std::slice::from_ref(&leaf)).unwrap();
        assert!(private.join("hooks").is_dir());
        assert!(mirror.links.is_empty());
        mirror.real_leaves_unchanged().unwrap();
    }
}
