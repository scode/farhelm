//! The desktop client's private state file, which holds the two device
//! credentials, and the one atomic JSON write both desktop state files use.

use super::*;

pub(super) const APP_STATE_FILE: &str = "desktop-client.json";
#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct PersistedState {
    pub(super) native_device_secret: Option<String>,
    pub(super) webview_device_secret: Option<String>,
    /// Monotonic proof that the real webview JavaScript stack completed an
    /// authenticated WebSocket handshake. It survives restart so the smoke
    /// gate can distinguish new readiness from old persisted credentials.
    #[serde(default)]
    pub(super) webview_auth_generation: u64,
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
/// The native credential refresh and the webview credential commit can
/// arrive concurrently. Locking the whole merge is what prevents either
/// writer from reinstalling a snapshot that silently drops the field the
/// other just committed. The file holds credentials and nothing else now:
/// the list preference that once shared it lives in the helm (SPEC.md,
/// Session list), so no non-credential writer ever contends here.
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
/// The single write path for both desktop state files (the credentials here
/// and the window frame in [`super::window_state`]). There used to be one copy
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
            native_device_secret: Some("secret".to_string()),
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
    /// The file holds device credentials, so its mode is part of the
    /// contract, and a second write must not merge with or append to the
    /// first.
    #[farhelm_testtrace::test]
    fn a_write_replaces_the_record_privately() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(APP_STATE_FILE);
        let first = PersistedState {
            native_device_secret: Some("first".to_string()),
            webview_device_secret: Some("webview".to_string()),
            webview_auth_generation: 1,
        };
        atomic_write_json(&path, &first).unwrap();
        let second = PersistedState {
            native_device_secret: Some("second".to_string()),
            ..PersistedState::default()
        };
        atomic_write_json(&path, &second).unwrap();
        let read = read_state(&path).unwrap();
        assert_eq!(read.native_device_secret.as_deref(), Some("second"));
        assert_eq!(read.webview_device_secret, None);
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

    /// A state file written by a build that still kept the list preference
    /// beside the credentials (`remembered_selection`, `list_sort`) must
    /// decode as the credentials alone, and a rewrite must drop the stale
    /// fields rather than carry them forward.
    ///
    /// The preference moved into the helm (SPEC.md, Session list); a
    /// relaunch after the upgrade reads exactly such a file, and refusing it
    /// would log the operator out of the desktop app for no reason. The
    /// dropped-on-rewrite half pins that the fields really are gone from
    /// the type and not merely tolerated, so nothing can quietly revive a
    /// per-client copy by reading them back.
    #[farhelm_testtrace::test]
    fn a_state_file_with_the_retired_preference_fields_decodes_as_credentials_only() {
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
        // one call both proves the credentials decoded and performs the
        // rewrite whose output the raw-JSON assertions below inspect.
        let state = update_state(&state_path, |state| {
            state.webview_auth_generation += 1;
        })
        .unwrap();
        assert_eq!(state.native_device_secret.as_deref(), Some("native-old"));
        assert_eq!(state.webview_device_secret.as_deref(), Some("webview-old"));
        assert_eq!(state.webview_auth_generation, 8);

        let rewritten: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
        assert_eq!(rewritten["webview_auth_generation"], 8);
        assert!(
            rewritten.get("remembered_selection").is_none() && rewritten.get("list_sort").is_none(),
            "the retired preference fields must not survive a rewrite: {rewritten}"
        );
    }
}
