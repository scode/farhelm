//! The desktop client's private state file, and the one atomic JSON write both
//! desktop state files use.
//!
//! The file no longer holds credentials. The app's two device secrets are
//! minted in memory by its embedded helm on every launch (see
//! [`farhelm_helm::EmbeddedReady`]), so there is nothing durable to keep, and
//! a secret left on disk would only be something to leak. Files written by
//! older builds still carry `native_device_secret` and
//! `webview_device_secret`; serde ignores the unknown fields on read, and the
//! next write drops them.

use super::*;

pub(super) const APP_STATE_FILE: &str = "desktop-client.json";
#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct PersistedState {
    /// Monotonic proof that the real webview JavaScript stack completed an
    /// authenticated WebSocket handshake. It survives restart so the smoke
    /// gate can tell a new launch's readiness from an earlier one's.
    #[serde(default)]
    pub(super) webview_auth_generation: u64,
    /// Whether the updater installs new releases on its own
    /// (`super::updater`). Absent means on, which is the default for every
    /// installation, including files written before the setting existed. A
    /// setting of this app installation, not a helm preference: SPEC.md
    /// "Session list" keeps it out of the helm's shared preferences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) install_updates_automatically: Option<bool>,
}
/// Atomically replace the destination while retaining the old record on
/// failure. Windows needs its replace-capable move API because
/// `std::fs::rename` rejects an existing destination there.
fn replace_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };

        let source: Vec<u16> = temporary
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let target: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: both strings are NUL-terminated and remain alive for the
        // duration of the synchronous kernel call.
        let replaced = unsafe {
            MoveFileExW(
                source.as_ptr(),
                target.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if replaced == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::fs::rename(temporary, destination)
    }
}
static STATE_FILE_WRITE: Mutex<()> = Mutex::new(());
pub(super) fn read_state(path: &Path) -> anyhow::Result<PersistedState> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("reading desktop state from {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PersistedState::default()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Serialize every read-modify-replace of the desktop state file.
///
/// Locking the whole merge is what prevents two writers from reinstalling a
/// snapshot that silently drops a field the other just committed: the
/// webview readiness record and the automatic-updates setting share the
/// file.
pub(super) fn update_state(
    path: &Path,
    mutate: impl FnOnce(&mut PersistedState),
) -> anyhow::Result<PersistedState> {
    let _guard = STATE_FILE_WRITE
        .lock()
        .expect("desktop state-file lock poisoned");
    let mut state = read_state(path)?;
    mutate(&mut state);
    atomic_write_json(path, &state)?;
    Ok(state)
}

/// Replace the file at `path` with `value` as JSON, readable only by this
/// user, so that a crash at any point leaves either the previous record or
/// the new one and never a mixture.
///
/// The single write path for both desktop state files (the readiness record
/// here and the window frame in [`super::window_state`]). There used to be one copy
/// per file, and they had drifted: only one flushed the directory after the
/// rename, and only the other removed its temporary file on failure and used
/// [`replace_file`] rather than a bare rename. This keeps all of it: a private
/// temporary sibling, flushed before it replaces the destination, removed if
/// anything up to the replacement fails, and a flush of the parent directory
/// afterwards so the replacement itself survives a crash or power loss.
pub(super) fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let bytes =
        serde_json::to_vec(value).with_context(|| format!("encoding {}", path.display()))?;
    let temporary = path.with_extension("json.tmp");
    let write = (|| -> anyhow::Result<()> {
        let mut file =
            File::create(&temporary).with_context(|| format!("opening {}", temporary.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(&bytes)?;
        file.sync_all()?;
        replace_file(&temporary, path).with_context(|| format!("installing {}", path.display()))
    })();
    if write.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    write?;
    sync_parent_dir(path)
}

/// Flush the directory entry the replacement just changed.
///
/// Unix only: a directory cannot be opened as a `File` on Windows, where
/// [`replace_file`] already asks the move itself to reach the disk
/// (`MOVEFILE_WRITE_THROUGH`) before returning.
fn sync_parent_dir(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        let parent = path
            .parent()
            .with_context(|| format!("{} has no parent directory", path.display()))?;
        File::open(parent)
            .and_then(|dir| dir.sync_all())
            .with_context(|| format!("syncing {}", parent.display()))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: a write that fails after its temporary file exists removes
    /// that file and leaves the previous record readable.
    ///
    /// The credential writer used to leave `desktop-client.json.tmp` behind
    /// when the final replacement failed; the window writer cleaned up. With
    /// one writer for both, this pins the cleanup for both. The failure is
    /// forced by making the destination a directory, which a file cannot
    /// replace, so the temporary file is created and flushed first.
    #[farhelm_testtrace::test]
    fn a_failed_replacement_removes_its_temporary_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(APP_STATE_FILE);
        std::fs::create_dir(&path).expect("occupy the destination");
        let state = PersistedState {
            webview_auth_generation: 3,
            ..PersistedState::default()
        };
        assert!(atomic_write_json(&path, &state).is_err());
        assert!(path.is_dir(), "the occupied destination must be untouched");
        assert!(
            !path.with_extension("json.tmp").exists(),
            "the temporary file must not outlive a failed write"
        );
    }

    /// Spec: a successful write replaces the record in full and leaves it
    /// readable only by this user.
    ///
    /// The window-frame file shares this writer, and private mode is the
    /// writer's contract for both files whatever they hold; a second write
    /// must not merge with or append to the first.
    #[farhelm_testtrace::test]
    fn a_write_replaces_the_record_privately() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(APP_STATE_FILE);
        let first = PersistedState {
            webview_auth_generation: 5,
            ..PersistedState::default()
        };
        atomic_write_json(&path, &first).unwrap();
        let second = PersistedState::default();
        atomic_write_json(&path, &second).unwrap();
        let read = read_state(&path).unwrap();
        assert_eq!(read.webview_auth_generation, 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    /// A state file written by an older build, which kept the desktop's
    /// device secrets there (and, older still, the list preference), must
    /// decode, and a rewrite must drop every retired field.
    ///
    /// A relaunch after the upgrade reads exactly such a file, and refusing
    /// it would fail startup for no reason. Dropping the secrets on rewrite
    /// matters more than tidiness: the desktop's credentials are now
    /// in-memory ones minted per launch, and a stale secret left on disk is
    /// only something to leak (the old stored rows it names stay valid
    /// until a rotation or the cap removes them). The dropped-on-rewrite
    /// half also pins that the fields are gone from the type, not merely
    /// tolerated, so nothing can quietly start reading them again.
    #[farhelm_testtrace::test]
    fn a_state_file_from_an_older_build_decodes_and_loses_its_retired_fields() {
        let root = tempfile::tempdir().unwrap();
        let state_path = root.path().join(APP_STATE_FILE);
        std::fs::write(
            &state_path,
            r#"{
                "native_device_secret":"native-old",
                "webview_device_secret":"webview-old",
                "webview_auth_generation":7,
                "remembered_selection":{"helm":"helm-a","id":"session-1"},
                "list_sort":"title"
            }"#,
        )
        .unwrap();

        // `update_state` reads, mutates, and returns the decoded state, so
        // one call both proves the old file decoded and performs the
        // rewrite whose output the raw-JSON assertions below inspect.
        let state = update_state(&state_path, |state| {
            state.webview_auth_generation += 1;
        })
        .unwrap();
        assert_eq!(state.webview_auth_generation, 8);

        let rewritten: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
        assert_eq!(rewritten["webview_auth_generation"], 8);
        for retired in [
            "native_device_secret",
            "webview_device_secret",
            "remembered_selection",
            "list_sort",
        ] {
            assert!(
                rewritten.get(retired).is_none(),
                "the retired field {retired} must not survive a rewrite: {rewritten}"
            );
        }
    }

    /// Spec: the automatic-updates setting survives the readiness record's
    /// rewrite, and a file that never had it gains no field.
    ///
    /// Both records share this file through one locked read-modify-write;
    /// a rewrite for readiness must not reset the user's choice, and an
    /// absent field must keep meaning "on" rather than being written out
    /// as an explicit value the default can no longer change.
    #[farhelm_testtrace::test]
    fn the_automatic_updates_setting_survives_other_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(APP_STATE_FILE);
        update_state(&path, |state| state.webview_auth_generation += 1).unwrap();
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(raw.get("install_updates_automatically").is_none(), "{raw}");

        update_state(&path, |state| {
            state.install_updates_automatically = Some(false)
        })
        .unwrap();
        update_state(&path, |state| state.webview_auth_generation += 1).unwrap();
        let state = read_state(&path).unwrap();
        assert_eq!(state.install_updates_automatically, Some(false));
        assert_eq!(state.webview_auth_generation, 2);
    }
}
