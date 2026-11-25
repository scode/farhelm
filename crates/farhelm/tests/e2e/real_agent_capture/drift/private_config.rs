//! Private vendor configuration for tests that exercise Farhelm's on-disk hook setup or Claude workspace trust.
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
//! copies such a replaced file back when the real one has not changed meanwhile and the case's terminal hosts are
//! confirmed ended, and names it in a note otherwise, including when its copy-back fails. Entries the vendor creates
//! inside the private directories themselves are dropped with the mirror.
//!
//! Adding a kind with persistent hooks needs nothing here as long as its paths lie under the home or Grok's home;
//! the guard test below fails for one that does not, and for one without a drift profile.
//!
//! Claude's trust test has an explicit mirror. Farhelm canonicalizes the selected config before editing it, so each
//! of Claude's possible trust files needs its own regular-file copy. Those copies are discarded with the test.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context as _, bail, ensure};
use farhelm_proto::AgentKind;
use farhelm_supervisor::service::SupervisorSeams;

/// A directory the supervisor resolves hook paths against, and the agent variable that redirects the agent to it.
///
/// Exhaustive on purpose: a vendor whose hook files live under another directory needs a supervisor seam for it
/// first, and then a variant here, or its real configuration would be the one written. `ClaudeConfigDir` holds no
/// hook files and has no seam; it only carries the copies of Claude's config for the workspace trust check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Root {
    /// The user's home: `user_home` in the supervisor, `HOME` for the agent.
    Home,
    /// Grok's configuration root: `grok_home` in the supervisor, `GROK_HOME` for the agent.
    GrokHome,
    /// Claude's explicitly selected config directory. The supervisor's launch environment supplies this override.
    ClaudeConfigDir,
}

impl Root {
    const ALL: [Root; 2] = [Root::Home, Root::GrokHome];

    /// The variable the agent reads this directory from.
    fn variable(self) -> &'static str {
        match self {
            Root::Home => "HOME",
            Root::GrokHome => "GROK_HOME",
            Root::ClaudeConfigDir => "CLAUDE_CONFIG_DIR",
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
            Root::ClaudeConfigDir => std::env::var_os(self.variable())
                .filter(|root| !root.is_empty())
                .map(PathBuf::from)
                .context("CLAUDE_CONFIG_DIR is not set"),
        }
    }

    /// The scratch directory a planning run resolves this root to.
    fn planning_dir(self, scratch: &Path) -> PathBuf {
        match self {
            Root::Home => scratch.join("plan-home"),
            Root::GrokHome => scratch.join("plan-grok-home"),
            Root::ClaudeConfigDir => scratch.join("plan-claude-config"),
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
    /// Vendor config files that can be edited after canonicalization. They never enter the copy-back list.
    copied: Vec<(PathBuf, Option<Vec<u8>>)>,
    /// Every symlink the mirror made, relative to the root, with the stamp of a regular-file target.
    links: Vec<(PathBuf, Option<Stamp>)>,
}

impl Mirror {
    /// Build `private` as a mirror of `real` with `leaves` left out. Every directory above a leaf is a real private
    /// directory, so the setup's writes cannot pass through a link into the real tree; every other entry in those
    /// directories is a symlink to the real entry. A real directory that does not exist yet mirrors as empty.
    fn build(root: Root, real: &Path, private: &Path, leaves: &[PathBuf]) -> anyhow::Result<Self> {
        Self::build_with_copies(root, real, private, leaves, &[], &[], &BTreeSet::new())
    }

    /// Build directories above both kinds of protected leaf. Copy a vendor config's bytes into a distinct inode,
    /// even when its real name is a symlink, so the launch's canonicalization stays in the mirror. `fresh` names
    /// entries the vendor writes during the case that must start empty in the mirror and be discarded with it
    /// rather than reach the real tree through a link.
    fn build_with_copies(
        root: Root,
        real: &Path,
        private: &Path,
        leaves: &[PathBuf],
        copies: &[PathBuf],
        fresh: &[PathBuf],
        protected: &BTreeSet<(u64, u64)>,
    ) -> anyhow::Result<Self> {
        let mut leaf_set: BTreeSet<PathBuf> =
            leaves.iter().chain(copies).chain(fresh).cloned().collect();
        for copy in copies {
            // Claude creates this directory to serialize config updates. A real lock held by another
            // session must not block the private launch. Leave the name available in the mirror.
            let mut lock = copy.as_os_str().to_owned();
            lock.push(".lock");
            leaf_set.insert(lock.into());
        }
        let mut dirs = BTreeSet::from([PathBuf::new()]);
        for leaf in &leaf_set {
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
                let source = real.join(&relative);
                let target = private.join(&relative);
                if !protected.is_empty()
                    && regular_file_identity(&source).is_some_and(|file| protected.contains(&file))
                {
                    // Linking an alias would let a vendor write reach a known protected config through another name.
                    fs::copy(&source, &target).with_context(|| {
                        format!("copying vendor config alias into {}", target.display())
                    })?;
                    continue;
                }
                std::os::unix::fs::symlink(&source, &target)
                    .with_context(|| format!("linking {}", target.display()))?;
                links.push((relative, Stamp::of(&source)));
            }
        }
        let leaves = leaves
            .iter()
            .map(|leaf| (leaf.clone(), fs::read(real.join(leaf)).ok()))
            .collect();
        let mut copied = Vec::new();
        for leaf in copies {
            let source = real.join(leaf);
            let before = read_optional(&source)?;
            if before.is_some() {
                let target = private.join(leaf);
                fs::copy(&source, &target)
                    .with_context(|| format!("copying vendor config into {}", target.display()))?;
                ensure!(
                    fs::canonicalize(&target)? == fs::canonicalize(private)?.join(leaf),
                    "private config {} resolves outside its mirror",
                    target.display(),
                );
            }
            copied.push((leaf.clone(), before));
        }
        Ok(Self {
            root,
            real: real.to_path_buf(),
            private: private.to_path_buf(),
            leaves,
            copied,
            links,
        })
    }

    /// The maintainer's hook files must be exactly as they were. Checked after setup and again at the end.
    fn real_leaves_unchanged(&self) -> anyhow::Result<()> {
        for (leaf, before) in &self.leaves {
            let path = self.real.join(leaf);
            ensure!(
                fs::read(&path).ok() == *before,
                "the real {} changed while a drift case ran",
                path.display()
            );
        }
        for (leaf, before) in &self.copied {
            let path = self.real.join(leaf);
            ensure!(
                read_optional(&path)? == *before,
                "the real {} changed while a drift case ran",
                path.display()
            );
        }
        Ok(())
    }

    /// Farhelm's setup wrote every planned file into the mirror as a regular file of its own.
    fn private_leaves_installed(&self) -> anyhow::Result<()> {
        for (leaf, _) in self
            .leaves
            .iter()
            .chain(self.copied.iter().filter(|(_, before)| before.is_some()))
        {
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
    /// was built. Returns a note for each replaced file it could not copy back. A failed copy of one file becomes its
    /// note rather than an early return, so it cannot cost the files after it (a refreshed login among them) or the
    /// caller's hook check.
    fn copy_back(&self, protected: &BTreeSet<(u64, u64)>) -> Vec<String> {
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
            if let Err(error) = Self::replace_real(&private, &real, protected) {
                notes.push(format!(
                    "{} was rewritten during the case, but copying it back failed: {error:#}; the case's copy was \
                     dropped",
                    relative.display()
                ));
            }
        }
        notes
    }

    /// Atomically replace the real file with the vendor's copy from the mirror, keeping the real file's permissions.
    fn replace_real(
        private: &Path,
        real: &Path,
        protected: &BTreeSet<(u64, u64)>,
    ) -> anyhow::Result<()> {
        // Resolve the maintainer's own symlinks (dotfiles, say) so the rename replaces the file, not the link.
        let target =
            fs::canonicalize(real).with_context(|| format!("resolving {}", real.display()))?;
        ensure!(
            !regular_file_identity(&target).is_some_and(|file| protected.contains(&file)),
            "copy-back destination is a protected vendor config"
        );
        let staged = target.with_file_name(format!(
            ".{}.farhelm-drift-{}",
            target
                .file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_default(),
            std::process::id()
        ));
        // `fs::copy` creates the staged file before writing it, so every step from the copy on shares the cleanup:
        // a copy cut short (a full disk, say) must not leave part of a login token beside the real file.
        let copied = fs::copy(private, &staged)
            .with_context(|| format!("staging {}", staged.display()))
            .and_then(|_| {
                let permissions = fs::metadata(&target)
                    .with_context(|| format!("reading {}", target.display()))?
                    .permissions();
                fs::set_permissions(&staged, permissions)
                    .with_context(|| format!("setting permissions on {}", staged.display()))
            })
            .and_then(|()| {
                fs::rename(&staged, &target)
                    .with_context(|| format!("replacing {}", target.display()))
            });
        if copied.is_err() {
            let _ = fs::remove_file(&staged);
        }
        copied
    }

    /// Name every file the vendor replaced in the mirror, for a case whose terminal hosts were not confirmed ended
    /// and whose copies are therefore not copied back. The mirror is deleted with the case, so these notes are the
    /// only record that the vendor's write was lost.
    fn dropped_without_shutdown(&self) -> Vec<String> {
        self.links
            .iter()
            .filter(|(relative, _)| {
                fs::symlink_metadata(self.private.join(relative))
                    .is_ok_and(|metadata| metadata.is_file())
            })
            .map(|(relative, _)| {
                format!(
                    "{} was rewritten during the case, but its terminal hosts were not confirmed ended; the case's \
                     copy was dropped and the real {} was not touched",
                    relative.display(),
                    self.real.join(relative).display()
                )
            })
            .collect()
    }
}

/// An absent config is valid. Other read failures would hide whether a real file changed during the case.
fn read_optional(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("checking vendor config {}", path.display()));
        }
    };
    ensure!(
        metadata.is_file(),
        "{} is not a regular file",
        path.display()
    );
    fs::read(path)
        .map(Some)
        .with_context(|| format!("reading vendor config {}", path.display()))
}

/// Match file aliases across both mirrors, including separate symlinks and hard links to one config.
fn regular_file_identity(path: &Path) -> Option<(u64, u64)> {
    let metadata = fs::metadata(path).ok().filter(fs::Metadata::is_file)?;
    Some((metadata.dev(), metadata.ino()))
}

/// Private vendor files for one drift case. Ordinary cases use this only for persistent hook setup.
#[derive(Debug)]
pub(super) struct PrivateConfig {
    mirrors: Vec<Mirror>,
    /// The Claude trust case replaces the login shell because rc files could undo its path redirects.
    launch_shell: Option<PathBuf>,
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
        Ok(Some(Self {
            mirrors,
            launch_shell: None,
        }))
    }

    /// Opt in separately from hook setup. A typical base launch of Claude uses its ordinary configuration.
    pub(super) fn prepare_claude_trust(scratch: &Path) -> anyhow::Result<Self> {
        let home = Root::Home.real()?;
        let config_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Self::claude_trust_mirrors(scratch, &home, config_dir.as_deref())
    }

    /// Protect the normal, custom OAuth and legacy names so the test uses the production resolver's
    /// precedence. The isolated files keep their original lengths and permissions, including empty read-only
    /// legacy files. An explicit config directory is mirrored separately and overrides the vendor's path.
    fn claude_trust_mirrors(
        scratch: &Path,
        home: &Path,
        config_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        ensure!(
            home.is_absolute(),
            "Claude trust test requires an absolute HOME"
        );
        ensure!(
            config_dir.is_none_or(Path::is_absolute),
            "Claude trust test requires an absolute CLAUDE_CONFIG_DIR"
        );
        let names =
            |legacy: &str| [".claude.json", ".claude-custom-oauth.json", legacy].map(PathBuf::from);
        let home_copies = names(".claude/.config.json");
        let override_copies = names(".config.json");
        let protected = home_copies
            .iter()
            .filter_map(|name| regular_file_identity(&home.join(name)))
            .chain(config_dir.into_iter().flat_map(|config_dir| {
                override_copies
                    .iter()
                    .filter_map(move |name| regular_file_identity(&config_dir.join(name)))
            }))
            .collect();
        let private_home = scratch.join("private-home");
        // Claude snapshots its config into a backups directory beside it, and prunes old snapshots there. A link
        // would put the private config's snapshots, with the test workspace's trust, into the real directory.
        let mut mirrors = vec![Mirror::build_with_copies(
            Root::Home,
            home,
            &private_home,
            &[],
            &home_copies,
            &[PathBuf::from(".claude/backups")],
            &protected,
        )?];
        let private_override = scratch.join("private-claude-config");
        if let Some(config_dir) = config_dir {
            mirrors.push(Mirror::build_with_copies(
                Root::ClaudeConfigDir,
                config_dir,
                &private_override,
                &[],
                &override_copies,
                &[PathBuf::from("backups")],
                &protected,
            )?);
        }
        let shell = claude_trust_shell(
            scratch,
            &private_home,
            config_dir.map(|_| private_override.as_path()),
        );
        Ok(Self {
            mirrors,
            launch_shell: Some(shell),
        })
    }

    /// After the composer opens without a trust answer, show that Farhelm wrote the fresh project's key in a
    /// private config instead of relying on a trust decision the vendor already knew.
    pub(super) fn check_claude_trust(&self, work: &Path) -> anyhow::Result<()> {
        let work = fs::canonicalize(work)?;
        let key = work.to_str().context("Claude workspace is not UTF-8")?;
        for mirror in &self.mirrors {
            for (relative, before) in &mirror.copied {
                if let Some(bytes) = read_optional(&mirror.private.join(relative))?
                    && claude_project_is_trusted(&bytes, key)
                {
                    ensure!(
                        before
                            .as_ref()
                            .is_none_or(|bytes| !claude_project_is_trusted(bytes, key)),
                        "Claude's scratch workspace was already trusted before this test"
                    );
                    return Ok(());
                }
            }
        }
        bail!("Claude's private configuration did not acquire trust for the scratch workspace")
    }

    /// Point the supervisor's setup and the agent's environment at the mirrors.
    pub(super) fn apply(&self, seams: &mut SupervisorSeams) {
        if let Some(shell) = &self.launch_shell {
            seams.launch_shell = Some(shell.to_string_lossy().into_owned());
        }
        for mirror in &self.mirrors {
            match mirror.root {
                Root::Home => seams.user_home = Some(mirror.private.clone()),
                Root::GrokHome => seams.grok_home = Some(mirror.private.clone()),
                Root::ClaudeConfigDir => {}
            }
            seams.launch_env.push((
                mirror.root.variable().into(),
                mirror.private.to_string_lossy().into_owned(),
            ));
        }
    }

    /// The fixture starts with all planned hook paths private and empty. A consented launch either installs every
    /// reporter there or keeps the paths absent when the supervisor's hook setting is off.
    pub(super) fn check_hook_setup(&self, expected: bool) -> anyhow::Result<()> {
        for mirror in &self.mirrors {
            if expected {
                mirror.private_leaves_installed()?;
            } else {
                for (leaf, _) in &mirror.leaves {
                    let path = mirror.private.join(leaf);
                    ensure!(
                        fs::symlink_metadata(&path)
                            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
                        "hook setup with hooks off did not leave {} absent",
                        path.display()
                    );
                }
            }
            mirror.real_leaves_unchanged()?;
        }
        Ok(())
    }

    /// Run after every case: copy back what the vendor replaced, then re-check the real hook files. Copy-back waits
    /// for confirmed host shutdown, so a still-running vendor cannot race it. When shutdown was not confirmed, each
    /// replaced file is named instead of silently dropped with the mirror: it may be a refreshed login whose loss
    /// signs the user out of that vendor. Copy-back runs before the hook check, so a changed hook file does not cost the
    /// vendor's other writes. The check's error keeps its own message first, since the runner's summary shows only the
    /// first line, and carries the notes after it.
    pub(super) fn finish(&self, stopped: bool) -> anyhow::Result<Vec<String>> {
        let mut notes = Vec::new();
        let protected = self
            .mirrors
            .iter()
            .flat_map(|mirror| {
                mirror
                    .copied
                    .iter()
                    .filter_map(|(file, _)| regular_file_identity(&mirror.real.join(file)))
            })
            .collect();
        for mirror in &self.mirrors {
            if stopped {
                notes.extend(mirror.copy_back(&protected));
            } else {
                notes.extend(mirror.dropped_without_shutdown());
            }
        }
        let unchanged = self
            .mirrors
            .iter()
            .try_for_each(|mirror| mirror.real_leaves_unchanged());
        match unchanged {
            Ok(()) => Ok(notes),
            Err(error) if notes.is_empty() => Err(error),
            Err(error) => Err(anyhow::anyhow!("{error:#}\n{}", notes.join("\n"))),
        }
    }
}

/// The real launch shim resolves trust after its login shell. Use an owned shell that pins the mirror paths and
/// starts the shim without sourcing rc files that could restore either original path.
fn claude_trust_shell(scratch: &Path, home: &Path, config_dir: Option<&Path>) -> PathBuf {
    let home = shell_words::quote(home.to_str().expect("private home is UTF-8"));
    let config = config_dir.map_or_else(
        || "unset CLAUDE_CONFIG_DIR".to_owned(),
        |path| {
            format!(
                "export CLAUDE_CONFIG_DIR={}",
                shell_words::quote(path.to_str().expect("private config is UTF-8"))
            )
        },
    );
    crate::host_creates::script(
        &scratch.join("claude-trust-shell"),
        &format!(
            r#"[ "$#" = 4 ] && [ "$1" = -l ] && [ "$2" = -i ] && [ "$3" = -c ] || exit 64
export HOME={home}
{config}
unset ENV BASH_ENV
exec /bin/sh -c "$4""#
        ),
    )
}

/// The one workspace trust field Farhelm sets in Claude's document.
fn claude_project_is_trusted(bytes: &[u8], work: &str) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes).is_ok_and(|document| {
        document["projects"][work]["hasTrustDialogAccepted"].as_bool() == Some(true)
    })
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

    /// A vendor that saves by rename replaces the link. Wait for confirmed shutdown before copying the new file
    /// over its unchanged original, and name every replaced file when shutdown was not confirmed, since a dropped
    /// copy can be a refreshed login. A concurrent change to the real file wins and is reported.
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

        let config = PrivateConfig {
            mirrors: vec![mirror],
            launch_shell: None,
        };
        let unconfirmed = config.finish(false).unwrap();
        assert_eq!(unconfirmed.len(), 2, "{unconfirmed:?}");
        for name in ["auth.json", "state.json"] {
            assert!(
                unconfirmed.iter().any(|note| note.starts_with(name)),
                "{unconfirmed:?}"
            );
        }
        assert_eq!(
            fs::read_to_string(real.join("auth.json")).unwrap(),
            "token-1"
        );
        let notes = config.finish(true).unwrap();
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

    /// A changed real hook file fails the case, but only after the vendor's replaced files were copied back: the
    /// check once ran first and its early return dropped a refreshed login with the mirror. The failure's first line
    /// names the hook file, since the runner's summary shows only that line, and the copy-back notes follow it.
    #[farhelm_testtrace::test]
    fn a_changed_real_hook_file_fails_only_after_the_copy_back() {
        let scratch = farhelm_teststate::tempdir().unwrap();
        let (real, private) = (scratch.path().join("real"), scratch.path().join("private"));
        real_tree(&real);
        fs::write(real.join("state.json"), "state-1").unwrap();
        let leaf = PathBuf::from("config/plugins/farhelm/hooks.json");
        let mirror =
            Mirror::build(Root::GrokHome, &real, &private, std::slice::from_ref(&leaf)).unwrap();

        for (name, content) in [("auth.json", "token-2"), ("state.json", "state-2")] {
            let staged = private.join(format!("{name}.new"));
            fs::write(&staged, content).unwrap();
            fs::rename(&staged, private.join(name)).unwrap();
        }
        // state.json changed for real too, so its copy is dropped with a note; the hook file changed for real.
        fs::remove_file(real.join("state.json")).unwrap();
        fs::write(real.join("state.json"), "state-from-elsewhere").unwrap();
        fs::write(real.join(&leaf), "rewritten by someone").unwrap();

        let config = PrivateConfig {
            mirrors: vec![mirror],
            launch_shell: None,
        };
        let error = format!("{:#}", config.finish(true).unwrap_err());
        assert_eq!(
            fs::read_to_string(real.join("auth.json")).unwrap(),
            "token-2"
        );
        let mut lines = error.lines();
        assert!(
            lines.next().is_some_and(|line| line.contains("hooks.json")),
            "{error}"
        );
        assert!(lines.any(|line| line.starts_with("state.json")), "{error}");
    }

    /// Claude snapshots its configuration into `backups` beside it and prunes old snapshots there. The private
    /// copies' snapshots carry the test workspace's trust, so the directory must start absent in the mirror, never as
    /// a link that would let them, or the pruning, reach the user's real backups.
    #[farhelm_testtrace::test]
    fn claude_trust_keeps_the_backups_directory_out_of_the_real_tree() {
        let scratch = farhelm_teststate::tempdir().unwrap();
        let home = scratch.path().join("home");
        let override_dir = scratch.path().join("vendor-config");
        let isolated = scratch.path().join("case");
        fs::create_dir_all(home.join(".claude/backups")).unwrap();
        fs::write(home.join(".claude/backups/.claude.json.backup.1"), b"real").unwrap();
        fs::create_dir_all(override_dir.join("backups")).unwrap();
        fs::create_dir(&isolated).unwrap();
        let config =
            PrivateConfig::claude_trust_mirrors(&isolated, &home, Some(&override_dir)).unwrap();
        for private in [
            isolated.join("private-home/.claude/backups"),
            isolated.join("private-claude-config/backups"),
        ] {
            assert!(
                fs::symlink_metadata(&private).is_err(),
                "{} must start absent",
                private.display()
            );
            fs::create_dir(&private).unwrap();
            fs::write(private.join("snapshot"), b"private trust").unwrap();
        }
        assert!(config.finish(true).unwrap().is_empty());
        let real: Vec<_> = fs::read_dir(home.join(".claude/backups"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(real, [std::ffi::OsString::from(".claude.json.backup.1")]);
        assert!(
            fs::read_dir(override_dir.join("backups"))
                .unwrap()
                .next()
                .is_none()
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

    /// Claude canonicalizes the chosen alias. The private copy must resolve to its own inode and its lock
    /// must be available even when the user's real config is locked. Neither trust nor lock state goes back.
    #[farhelm_testtrace::test]
    fn claude_trust_copies_an_explicit_symlinked_config_and_keeps_its_lock_private() {
        let scratch = farhelm_teststate::tempdir().unwrap();
        let home = scratch.path().join("home");
        let override_dir = scratch.path().join("vendor-config");
        let isolated = scratch.path().join("case");
        fs::create_dir(&home).unwrap();
        fs::create_dir(&override_dir).unwrap();
        fs::create_dir(&isolated).unwrap();
        let real_target = override_dir.join("settings.json");
        fs::write(&real_target, b"{}").unwrap();
        let real_file = override_dir.join(".claude-custom-oauth.json");
        std::os::unix::fs::symlink(&real_target, &real_file).unwrap();
        std::os::unix::fs::symlink(&real_target, home.join("config-alias")).unwrap();
        let real_lock = override_dir.join(".claude-custom-oauth.json.lock");
        fs::create_dir(&real_lock).unwrap();
        let config =
            PrivateConfig::claude_trust_mirrors(&isolated, &home, Some(&override_dir)).unwrap();
        let mut seams = SupervisorSeams::default();
        config.apply(&mut seams);
        let (_, private_override) = seams
            .launch_env
            .iter()
            .find(|(name, _)| name == "CLAUDE_CONFIG_DIR")
            .unwrap();
        let private_file = Path::new(private_override).join(".claude-custom-oauth.json");
        let canonical_private = fs::canonicalize(private_override).unwrap();
        assert_eq!(
            fs::canonicalize(&private_file).unwrap(),
            canonical_private.join(".claude-custom-oauth.json")
        );
        assert_ne!(
            fs::metadata(&real_target).unwrap().ino(),
            fs::metadata(&private_file).unwrap().ino()
        );
        let private_lock = Path::new(private_override).join(".claude-custom-oauth.json.lock");
        assert!(!private_lock.exists());
        let home_mirror = isolated.join("private-home");
        for alias in [
            Path::new(private_override).join("settings.json"),
            home_mirror.join("config-alias"),
        ] {
            assert!(fs::symlink_metadata(&alias).unwrap().is_file());
            fs::write(alias, b"changed in the fixture").unwrap();
        }
        let probe = std::process::Command::new(seams.launch_shell.unwrap())
            .args([
                "-l",
                "-i",
                "-c",
                r#"printf '%s\n%s\n' "$HOME" "$CLAUDE_CONFIG_DIR""#,
            ])
            .env("HOME", &home)
            .env("CLAUDE_CONFIG_DIR", &override_dir)
            .output()
            .unwrap();
        assert!(probe.status.success());
        assert_eq!(
            String::from_utf8(probe.stdout).unwrap(),
            format!("{}\n{private_override}\n", home_mirror.display())
        );
        let project = fs::canonicalize(scratch.path()).unwrap();
        let value = serde_json::json!({"projects": {project.to_str().unwrap(): {"hasTrustDialogAccepted": true}}});
        fs::write(private_file, serde_json::to_vec(&value).unwrap()).unwrap();
        config.check_hook_setup(true).unwrap();
        config.check_claude_trust(&project).unwrap();
        assert!(config.finish(true).unwrap().is_empty());
        assert_eq!(fs::read(real_file).unwrap(), b"{}");
        assert!(real_lock.is_dir());
    }

    /// Even an empty legacy file can outrank the modern file when read-only. Keep its length and permissions
    /// and preserve absence for the alternate names so the real resolver sees the same candidates.
    #[farhelm_testtrace::test]
    fn claude_trust_preserves_the_default_legacy_file_shape() {
        use std::os::unix::fs::PermissionsExt as _;

        let scratch = farhelm_teststate::tempdir().unwrap();
        let home = scratch.path().join("home");
        let isolated = scratch.path().join("case");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir(&isolated).unwrap();
        let legacy = home.join(".claude/.config.json");
        fs::write(&legacy, b"").unwrap();
        fs::set_permissions(&legacy, fs::Permissions::from_mode(0o400)).unwrap();
        let config = PrivateConfig::claude_trust_mirrors(&isolated, &home, None).unwrap();
        let private_home = isolated.join("private-home");
        let metadata = fs::metadata(private_home.join(".claude/.config.json")).unwrap();
        assert_eq!(metadata.mode() & 0o777, 0o400);
        assert_eq!(metadata.len(), 0);
        assert!(!private_home.join(".claude.json").exists());
        config.check_hook_setup(true).unwrap();
        assert!(config.finish(true).unwrap().is_empty());
    }
}
