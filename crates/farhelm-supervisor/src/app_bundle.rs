//! The Mac app bundle's side-by-side version layout, as the supervisor sees
//! it (SPEC_impl.md, "Side-by-side versions inside Farhelm.app").
//!
//! An update on the Mac writes a new `Contents/Versions/<v>/farhelm` beside
//! the old ones instead of replacing the program a running Farhelm starts.
//! A supervisor that runs from one of those folders hands its sessions the
//! bundle's forwarder, `Contents/MacOS/farhelm`, for everything that
//! outlives the supervisor (hook command lines, reporter variables, the
//! session `PATH` entry), and publishes which version it is in the Running
//! record so the forwarder can send a session's commands back to that
//! version while it runs.
//!
//! Recognition is by path alone and is not limited to macOS. The shape
//! only exists where the installer built it, which is only on the Mac, so
//! on Linux, in development builds and in tests nothing matches and the
//! supervisor behaves exactly as it did before the layout existed. Keeping
//! the rule platform-independent is what lets it be tested here.

use std::path::{Path, PathBuf};

/// The Running record's file name, in the supervisor's state directory
/// beside `supervisor.sock`.
///
/// The forwarder finds it as `$(dirname "$FARHELM_SUPERVISOR_SOCK")/` plus
/// this name, so renaming it, or moving it away from the socket, breaks
/// every session already running (see SPEC_impl.md, "What running sessions
/// hold across versions").
pub const RUNNING_RECORD: &str = "running-version";

/// Where a supervisor running from the versioned layout sits in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedProgram {
    /// The `<v>` folder name the program runs from: what the Running
    /// record names, and what the installer's cleanup keeps.
    pub version: String,
    /// The bundle's forwarder, `Contents/MacOS/farhelm`.
    pub forwarder: PathBuf,
}

/// Recognize `<anything>.app/Contents/Versions/<v>/farhelm`.
///
/// The version is taken from the path, not from the version compiled into
/// the program, because the path is what the forwarder and the installer
/// can see: a record that named the compiled version could name a folder
/// that does not exist.
///
/// `<v>` must be a plain folder name that is valid UTF-8, since it is
/// written into the record and read back by a shell script. `installed`
/// is never a version: it is the Installed record's own name in the same
/// folder. Anything else that does not match returns `None`.
pub fn versioned_program(exe: &Path) -> Option<VersionedProgram> {
    if exe.file_name()? != "farhelm" {
        return None;
    }
    let version_dir = exe.parent()?;
    let version = version_dir.file_name()?.to_str()?;
    if version.is_empty() || version == "installed" || version.starts_with('.') {
        return None;
    }
    let versions = version_dir.parent()?;
    if versions.file_name()? != "Versions" {
        return None;
    }
    let contents = versions.parent()?;
    if contents.file_name()? != "Contents" {
        return None;
    }
    let bundle = contents.parent()?;
    if !bundle.file_name()?.to_str()?.ends_with(".app") {
        return None;
    }
    Some(VersionedProgram {
        version: version.to_string(),
        forwarder: contents.join("MacOS").join("farhelm"),
    })
}

/// The `Contents` folder of the app bundle whose `Contents/MacOS/` holds
/// `exe`, when `exe` sits in one: `<anything>.app/Contents/MacOS/<file>`.
///
/// The desktop app uses this to decide whether it is running inside a
/// bundle at all before it looks for `Contents/Versions/`. It is the same
/// shape [`versioned_program`] derives the forwarder from, so the two can
/// never disagree about what counts as the bundle: a `MacOS` folder that is
/// not inside `<x>.app/Contents` is not one.
pub fn bundle_contents_of_main_program(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    if macos.file_name()? != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()? != "Contents" {
        return None;
    }
    if !contents.parent()?.file_name()?.to_str()?.ends_with(".app") {
        return None;
    }
    Some(contents.to_path_buf())
}

/// Make `state_dir`'s Running record agree with this supervisor: write its
/// version when it runs from the layout, remove a leftover one otherwise.
///
/// Called once the supervisor holds the right to serve and before it
/// accepts a connection, so that by the time any session can reach it the
/// record already names it. The write is the crate's best-effort atomic
/// tier (`crate::files`): a complete, fsynced 0600 file renamed into place,
/// so the forwarder never reads a half-written version. The record's shape,
/// one line holding the version, is read by forwarder scripts that later
/// installers write, so it does not change.
///
/// Failing to write the record fails `serve`: a supervisor in the layout
/// that cannot say which version it is would send its sessions' commands
/// to whatever is installed. Failing to remove a stale one only warns,
/// because outside the layout (every Linux host) the record does not
/// matter, and a supervisor there must not gain a new way to refuse to
/// start. Removing a stale record from a supervisor outside the layout (a
/// development build on the Mac) keeps the forwarder from sending this
/// supervisor's sessions to a version that is not the one running; they
/// reach the Installed version instead.
///
/// `ownership` is the caller's claim on the state directory, moved into the
/// blocking filesystem work and dropped only when that work has finished.
/// A stop can cancel the caller while the write or removal is still running
/// on a blocking thread, and the supervisor can then release its lock; if
/// the work did not hold the claim itself, a successor could take the lock,
/// publish its own record, and have it overwritten or removed by the
/// predecessor's late operation.
pub async fn publish_running_record<G: Send + 'static>(
    state_dir: &Path,
    program: Option<&VersionedProgram>,
    ownership: G,
) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let record = state_dir.join(RUNNING_RECORD);
    let path = record.clone();
    let contents = program.map(|program| format!("{}\n", program.version).into_bytes());
    let outcome = tokio::task::spawn_blocking(move || {
        let _ownership = ownership;
        match contents {
            Some(bytes) => {
                crate::files::overwrite_private_file_sync(&path, &bytes, &crate::files::RealFs)
            }
            None => match std::fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            },
        }
    })
    .await
    .context("joining the Running record update")?;
    match (program, outcome) {
        (_, Ok(())) => Ok(()),
        (Some(_), Err(error)) => {
            Err(error).with_context(|| format!("publishing {}", record.display()))
        }
        (None, Err(error)) => {
            tracing::warn!(
                record = %record.display(),
                %error,
                "could not remove a stale Running record"
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The installed shape is recognized, and the forwarder is the bundle's
    /// `Contents/MacOS/farhelm`: the exact path desktop-managed supervisors
    /// ran from before this layout, which sessions they started still hold.
    #[farhelm_testtrace::test]
    fn a_program_in_a_version_folder_is_recognized() {
        let program = versioned_program(Path::new(
            "/Users/u/Applications/Farhelm.app/Contents/Versions/0.21.0/farhelm",
        ))
        .expect("the installed shape");
        assert_eq!(program.version, "0.21.0");
        assert_eq!(
            program.forwarder,
            PathBuf::from("/Users/u/Applications/Farhelm.app/Contents/MacOS/farhelm")
        );
    }

    /// Everything that is not exactly that shape keeps the old behavior,
    /// so Linux installs, development builds, the old bundle layout and
    /// tests are untouched. `installed` is the Installed record's name and
    /// must never be taken for a version.
    #[farhelm_testtrace::test]
    fn other_shapes_are_not_the_versioned_layout() {
        for path in [
            "/home/u/.local/bin/farhelm",
            "/repo/target/debug/farhelm",
            "/Users/u/Applications/Farhelm.app/Contents/MacOS/farhelm",
            "/Users/u/Applications/Farhelm.app/Contents/Versions/installed/farhelm",
            "/Users/u/Applications/Farhelm.app/Contents/Versions/.0.21.0.partial/farhelm",
            "/Users/u/Applications/Farhelm.app/Contents/Versions/0.21.0/farhelm-desktop",
            "/Users/u/Applications/Farhelm/Contents/Versions/0.21.0/farhelm",
            "/Users/u/Applications/Farhelm.app/Versions/0.21.0/farhelm",
        ] {
            assert_eq!(versioned_program(Path::new(path)), None, "{path}");
        }
    }

    /// The desktop app's main program is recognized only inside
    /// `<x>.app/Contents/MacOS/`, never in any other folder that happens to
    /// be called `MacOS`.
    #[farhelm_testtrace::test]
    fn the_main_program_is_recognized_only_inside_an_app_bundle() {
        assert_eq!(
            bundle_contents_of_main_program(Path::new(
                "/Users/u/Applications/Farhelm.app/Contents/MacOS/farhelm-desktop"
            )),
            Some(PathBuf::from("/Users/u/Applications/Farhelm.app/Contents"))
        );
        for path in [
            "/opt/package/MacOS/farhelm-desktop",
            "/opt/package/Contents/MacOS/farhelm-desktop",
            "/home/u/.local/bin/farhelm-desktop",
        ] {
            assert_eq!(
                bundle_contents_of_main_program(Path::new(path)),
                None,
                "{path}"
            );
        }
    }

    /// A supervisor in the layout publishes its version, replacing whatever
    /// record was there; one outside it removes a leftover record, and a
    /// missing record is not an error.
    #[farhelm_testtrace::test]
    async fn the_running_record_follows_the_serving_supervisor() {
        let state = tempfile::tempdir().expect("state dir");
        let record = state.path().join(RUNNING_RECORD);
        std::fs::write(&record, "0.20.0\n").expect("fixture: a previous record");
        let program = VersionedProgram {
            version: "0.21.0".to_string(),
            forwarder: PathBuf::from("/x/Farhelm.app/Contents/MacOS/farhelm"),
        };
        publish_running_record(state.path(), Some(&program), ())
            .await
            .expect("publish");
        assert_eq!(
            std::fs::read_to_string(&record).expect("record"),
            "0.21.0\n"
        );

        publish_running_record(state.path(), None, ())
            .await
            .expect("remove");
        assert!(
            !record.exists(),
            "a supervisor outside the layout removes the record"
        );
        publish_running_record(state.path(), None, ())
            .await
            .expect("an absent record is fine");
    }
}
