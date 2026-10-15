//! Best-effort managed-checkout trust edits for Claude Code.
//!
//! Claude has no supported interactive trust switch. This opt-in integration
//! shares its mkdir lock and edits one project key, preserving the rest of the
//! vendor document. Claude's unlocked exit save can still race us; compatibility
//! with a future Claude version is not a guarantee of this module.

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The exact host-local file and project key to undo at checkout retirement.
/// Keep the lock alias as well as the resolved write target: Claude 2.1.300
/// explicitly locks the selected config filename, even when it is a symlink.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustRecord {
    pub config_path: PathBuf,
    pub lock_path: PathBuf,
    pub project_path: String,
}

/// Inputs from the launch shell, separated from file mutation so tests never
/// change the test process's environment. Custom OAuth has its own config file.
pub(crate) struct Environment {
    pub home: PathBuf,
    pub config_dir: Option<PathBuf>,
    pub custom_oauth: bool,
}

impl Environment {
    /// Snapshot only the path-selection inputs in the actual launch process.
    pub(crate) fn current() -> anyhow::Result<Self> {
        Ok(Self {
            home: std::env::var_os("HOME").context("HOME is unset")?.into(),
            config_dir: std::env::var_os("CLAUDE_CONFIG_DIR")
                .filter(|v| !v.is_empty())
                .map(Into::into),
            custom_oauth: std::env::var_os("CLAUDE_CODE_CUSTOM_OAUTH_URL")
                .is_some_and(|v| !v.is_empty()),
        })
    }

    /// Follow Claude's modern/legacy precedence, without creating vendor state.
    /// Empty writable legacy files yield to the modern file; an empty read-only
    /// legacy file remains selected when no modern file exists, as in Claude.
    fn config_path(&self) -> PathBuf {
        let directory = self
            .config_dir
            .clone()
            .unwrap_or_else(|| self.home.join(".claude"));
        let modern = self
            .config_dir
            .as_ref()
            .unwrap_or(&self.home)
            .join(if self.custom_oauth {
                ".claude-custom-oauth.json"
            } else {
                ".claude.json"
            });
        let legacy = directory.join(".config.json");
        match fs::metadata(&legacy) {
            Ok(meta)
                if meta.is_file()
                    && (meta.len() > 0 || (meta.mode() & 0o222 == 0 && !modern.exists())) =>
            {
                legacy
            }
            _ => modern,
        }
    }
}

/// Resolve a removal record before writing anything. The caller persists it
/// first, so a crash after the trust write still leaves cleanup evidence.
/// An absent config means Claude has never written one; do not invent it.
pub(crate) fn resolve(
    environment: &Environment,
    cwd: &Path,
) -> anyhow::Result<Option<TrustRecord>> {
    let selected = environment.config_path();
    let config_path = match fs::canonicalize(&selected) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        fs::metadata(&config_path)?.is_file(),
        "Claude config is not a regular file"
    );
    let mut lock_path = selected.into_os_string();
    lock_path.push(".lock");
    Ok(Some(TrustRecord {
        config_path,
        lock_path: lock_path.into(),
        project_path: fs::canonicalize(cwd)?
            .to_str()
            .context("checkout path is not UTF-8")?
            .to_string(),
    }))
}

/// Own one mkdir lock, never removing or stealing somebody else's stale lock.
/// Identity checks also keep Drop from removing a replacement lock if Claude
/// decided ours was stale while the process was suspended.
struct ConfigLock {
    path: PathBuf,
    identity: (u64, u64),
    acquired: Instant,
}

impl ConfigLock {
    /// Wait only for lock contention. Other filesystem failures are not helped
    /// by retries. A short injected budget keeps contention tests deterministic.
    fn acquire(
        path: &Path,
        budget: Duration,
        mut wait: impl FnMut(Duration),
    ) -> anyhow::Result<Self> {
        let started = Instant::now();
        loop {
            match fs::create_dir(path) {
                Ok(()) => {
                    let meta = fs::symlink_metadata(path)?;
                    return Ok(Self {
                        path: path.to_owned(),
                        identity: (meta.dev(), meta.ino()),
                        acquired: Instant::now(),
                    });
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists
                        && started.elapsed() < budget =>
                {
                    wait(Duration::from_millis(25).min(budget.saturating_sub(started.elapsed())));
                }
                Err(error) => return Err(error).context("acquiring Claude config lock"),
            }
        }
    }

    /// Refuse publication after a slow read or if the lock was replaced. This
    /// leaves ample margin before Claude's ten-second stale-lock threshold.
    fn check(&self) -> anyhow::Result<()> {
        let meta = fs::symlink_metadata(&self.path)?;
        ensure!(
            meta.is_dir()
                && (meta.dev(), meta.ino()) == self.identity
                && self.acquired.elapsed() < Duration::from_secs(4),
            "Claude config lock expired or was replaced"
        );
        Ok(())
    }
}

impl Drop for ConfigLock {
    fn drop(&mut self) {
        if let Ok(meta) = fs::symlink_metadata(&self.path)
            && meta.is_dir()
            && (meta.dev(), meta.ino()) == self.identity
        {
            let _ = fs::remove_dir(&self.path);
        }
    }
}

/// Set or remove only the recorded project's trust key under Claude's lock.
/// Removal without a record is a no-op for checkouts predating this feature.
/// Errors are for the caller's diagnostic, never a reason to fail launch/Delete.
pub(crate) fn edit(record: Option<&TrustRecord>, trusted: bool) -> anyhow::Result<()> {
    edit_with_budget(record, trusted, Duration::from_secs(12))
}

/// The mutation seam exposes contention budget, not ambient process state.
fn edit_with_budget(
    record: Option<&TrustRecord>,
    trusted: bool,
    budget: Duration,
) -> anyhow::Result<()> {
    edit_with_retry(record, trusted, budget, std::thread::sleep)
}

/// Mediate the contention wait so a fixture can release a known held lock
/// at the retry boundary, proving eventual acquisition without timing guesses.
fn edit_with_retry(
    record: Option<&TrustRecord>,
    trusted: bool,
    budget: Duration,
    wait: impl FnMut(Duration),
) -> anyhow::Result<()> {
    let Some(record) = record else {
        return Ok(());
    };
    // A config that is gone holds no trust to remove. Without this, startup
    // recovery would keep the removal record and retry it on every start.
    if !trusted
        && fs::symlink_metadata(&record.config_path)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        return Ok(());
    }
    let lock = ConfigLock::acquire(&record.lock_path, budget, wait)?;
    // Read the recorded canonical target under the lock. O_NOFOLLOW refuses
    // a replacement symlink at that target; the selected alias stays intact.
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&record.config_path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= 16 * 1024 * 1024,
        "Claude config is not a bounded regular file"
    );
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "Claude config grew beyond read limit"
    );
    let mut document: Value =
        serde_json::from_slice(&bytes).context("Claude config is not JSON")?;
    let object = document
        .as_object_mut()
        .context("Claude config is not an object")?;
    if trusted {
        let projects = object
            .entry("projects")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .context("Claude projects is not an object")?;
        let project = projects
            .entry(&record.project_path)
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .context("Claude project is not an object")?;
        if project.get("hasTrustDialogAccepted") == Some(&Value::Bool(true)) {
            return Ok(());
        }
        project.insert("hasTrustDialogAccepted".into(), Value::Bool(true));
    } else {
        let Some(project) = object
            .get_mut("projects")
            .and_then(Value::as_object_mut)
            .and_then(|p| p.get_mut(&record.project_path))
            .and_then(Value::as_object_mut)
        else {
            return Ok(());
        };
        if project.remove("hasTrustDialogAccepted").is_none() {
            return Ok(());
        }
    }
    let bytes = serde_json::to_vec_pretty(&document)?;
    let parent = record
        .config_path
        .parent()
        .context("config has no parent")?;
    let temporary = parent.join(format!(".farhelm-claude-{}.tmp", uuid::Uuid::new_v4()));
    // Keep the existing mode: the generic private-file helper forces 0600,
    // which would silently change this vendor-owned file's contract.
    let result = (|| -> anyhow::Result<()> {
        let mut staged = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        staged.set_permissions(fs::Permissions::from_mode(metadata.mode() & 0o7777))?;
        staged.write_all(&bytes)?;
        staged.sync_all()?;
        lock.check()?;
        let current = fs::symlink_metadata(&record.config_path)?;
        ensure!(
            (current.dev(), current.ino()) == (metadata.dev(), metadata.ino()),
            "Claude config was replaced while editing"
        );
        fs::rename(&temporary, &record.config_path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A vendor file carries unrelated authentication and preferences. The
    /// trust integration may change one key, including during its inverse.
    #[farhelm_testtrace::test]
    fn trust_round_trip_preserves_other_keys_mode_and_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("vendor.json");
        let alias = dir.path().join(".claude.json");
        let before = serde_json::json!({"unrelated":{"nested":[1,true,"value"]},"projects":{"other":{"hasTrustDialogAccepted":true},dir.path().to_str().unwrap():{"keep":42}}});
        fs::write(&config, serde_json::to_vec(&before).unwrap()).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o640)).unwrap();
        std::os::unix::fs::symlink(&config, &alias).unwrap();
        let env = Environment {
            home: dir.path().into(),
            config_dir: None,
            custom_oauth: false,
        };
        let record = resolve(&env, dir.path()).unwrap().unwrap();
        assert_eq!(record.config_path, config);
        assert_eq!(record.lock_path, dir.path().join(".claude.json.lock"));
        assert!(
            !before["projects"][&record.project_path]
                .as_object()
                .unwrap()
                .contains_key("hasTrustDialogAccepted")
        );
        edit(Some(&record), true).unwrap();
        let mut expected = before.clone();
        expected["projects"][&record.project_path]["hasTrustDialogAccepted"] = true.into();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&config).unwrap()).unwrap(),
            expected
        );
        assert_eq!(fs::metadata(&config).unwrap().mode() & 0o777, 0o640);
        assert!(fs::symlink_metadata(&alias).unwrap().is_symlink());
        edit(Some(&record), false).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&config).unwrap()).unwrap(),
            before
        );
        assert!(!record.lock_path.exists());
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            2,
            "no temporary file or lock remains"
        );
    }

    /// Removal after the user deleted Claude's config (or its directory)
    /// succeeds without creating anything, so the caller can clear its record.
    #[farhelm_testtrace::test]
    fn removing_trust_from_a_missing_config_is_nothing_to_do() {
        let dir = tempfile::tempdir().unwrap();
        let record = TrustRecord {
            config_path: dir.path().join("gone").join(".claude.json"),
            lock_path: dir.path().join("gone").join(".claude.json.lock"),
            project_path: "/checkouts/repo-1".into(),
        };
        edit(Some(&record), false).unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        assert!(
            edit(Some(&record), true).is_err(),
            "trusting still needs the file"
        );
    }

    /// Bad vendor shapes must survive byte-for-byte; a trust attempt cannot
    /// repair them by discarding unrelated state. Legacy checkouts do nothing.
    #[farhelm_testtrace::test]
    fn trust_refuses_non_objects_and_leaves_no_debris() {
        let dir = tempfile::tempdir().unwrap();
        let env = Environment {
            home: dir.path().into(),
            config_dir: None,
            custom_oauth: false,
        };
        assert!(resolve(&env, dir.path()).unwrap().is_none());
        edit(None, false).unwrap();
        for content in [
            "[]",
            "null",
            "invalid",
            r#"{"projects":[]}"#,
            r#"{"projects":{"bad":false}}"#,
        ] {
            fs::write(dir.path().join(".claude.json"), content).unwrap();
            let mut record = resolve(&env, dir.path()).unwrap().unwrap();
            if content.contains("bad") {
                record.project_path = "bad".into();
            }
            assert!(edit(Some(&record), true).is_err(), "{content}");
            assert_eq!(fs::read_to_string(&record.config_path).unwrap(), content);
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }

    /// Contention never authorizes stealing Claude's lock, even when its
    /// stale threshold has elapsed; the next Claude process owns that policy.
    #[farhelm_testtrace::test]
    fn trust_waits_then_refuses_foreign_lock() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".claude.json"), "{}").unwrap();
        let record = resolve(
            &Environment {
                home: dir.path().into(),
                config_dir: None,
                custom_oauth: false,
            },
            dir.path(),
        )
        .unwrap()
        .unwrap();
        fs::create_dir(&record.lock_path).unwrap();
        let identity = fs::metadata(&record.lock_path).unwrap().ino();
        assert!(edit_with_budget(Some(&record), true, Duration::from_millis(30)).is_err());
        assert_eq!(fs::metadata(&record.lock_path).unwrap().ino(), identity);
        assert_eq!(fs::read_to_string(&record.config_path).unwrap(), "{}");
        let mut retries = 0;
        edit_with_retry(Some(&record), true, Duration::from_secs(1), |_| {
            retries += 1;
            assert_eq!(fs::metadata(&record.lock_path).unwrap().ino(), identity);
            fs::remove_dir(&record.lock_path).unwrap();
        })
        .unwrap();
        assert_eq!(
            retries, 1,
            "the edit retried the held lock before publishing"
        );
        let document: Value =
            serde_json::from_slice(&fs::read(&record.config_path).unwrap()).unwrap();
        assert_eq!(
            document["projects"][&record.project_path]["hasTrustDialogAccepted"],
            true
        );
    }

    /// Config precedence belongs to the vendor, including custom OAuth and
    /// legacy files. Inject paths so parallel tests cannot alter one another.
    #[farhelm_testtrace::test]
    fn trust_config_resolution_matches_vendor_precedence() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("config");
        fs::create_dir(&config_dir).unwrap();
        let env = Environment {
            home: dir.path().into(),
            config_dir: Some(config_dir.clone()),
            custom_oauth: false,
        };
        assert_eq!(env.config_path(), config_dir.join(".claude.json"));
        fs::write(config_dir.join(".config.json"), "").unwrap();
        assert_eq!(env.config_path(), config_dir.join(".claude.json"));
        fs::write(config_dir.join(".config.json"), "{}").unwrap();
        assert_eq!(env.config_path(), config_dir.join(".config.json"));
        fs::remove_file(config_dir.join(".config.json")).unwrap();
        assert_eq!(
            Environment {
                custom_oauth: true,
                ..env
            }
            .config_path(),
            config_dir.join(".claude-custom-oauth.json")
        );
    }
}
