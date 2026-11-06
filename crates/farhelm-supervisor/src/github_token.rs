//! GitHub tokens cross the terminal-host boundary only in its child's stdin
//! specification. The supervisor reads the Farhelm-owned file at each spawn;
//! nothing is cached in a launch record or passed through tmux's command line.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// One host's supervisor state directory also owns its managed token. The helm
/// uses this same path for atomic replacement; older supervisors ignore it.
pub fn host_file(state_dir: &Path) -> PathBuf {
    state_dir.join("github-token")
}

/// Share the container reader's validation without sharing its absence policy.
/// Only NotFound means absent; other I/O failures and malformed values refuse
/// delivery with fixed text that cannot quote the file's contents.
pub(crate) fn read_file(path: &Path) -> anyhow::Result<Option<String>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => anyhow::bail!("could not read the GitHub token file"),
    };
    let token = text.trim();
    anyhow::ensure!(!token.is_empty(), "GitHub token file is empty");
    anyhow::ensure!(!token.contains('\0'), "GitHub token file contains NUL");
    Ok(Some(token.to_string()))
}

/// Replace only this child's GH_TOKEN, after its ordinary environment has been
/// assembled. Absence leaves the user's ambient environment alone. Callers
/// supply the path explicitly so tests need no process-environment mutation.
pub(crate) fn inject(env: &mut Vec<(OsString, OsString)>, path: &Path) -> anyhow::Result<()> {
    if let Some(token) = read_file(path)? {
        env.retain(|(name, _)| name != OsStr::new("GH_TOKEN"));
        env.push(("GH_TOKEN".into(), token.into()));
        env.sort_by(|left, right| left.0.cmp(&right.0));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each launch sees replacement bytes, while an absent managed file keeps
    /// ordinary host configuration intact and does not invent an empty token.
    #[test]
    fn each_injection_reads_the_current_file_and_preserves_unmanaged_environment() {
        let root = tempfile::tempdir().unwrap();
        let path = host_file(root.path());
        let original = vec![
            ("GH_TOKEN".into(), "ambient-fictional-token".into()),
            ("KEEP".into(), "kept".into()),
        ];
        let mut env = original.clone();
        inject(&mut env, &path).unwrap();
        assert_eq!(env, original);
        for value in ["fictional-first-token", "fictional-replacement-token"] {
            std::fs::write(&path, format!(" {value}\n")).unwrap();
            inject(&mut env, &path).unwrap();
            assert_eq!(
                env,
                vec![
                    ("GH_TOKEN".into(), value.into()),
                    ("KEEP".into(), "kept".into())
                ]
            );
        }
    }

    /// A malformed file refuses a new spawn without erasing the caller's
    /// environment or disclosing bytes through any diagnostic chain.
    #[test]
    fn invalid_values_refuse_without_contents_or_partial_environment_changes() {
        let root = tempfile::tempdir().unwrap();
        let path = host_file(root.path());
        for bytes in [
            b" \n".as_slice(),
            b"fictional-private-marker\0tail",
            b"fictional-private-marker\xff",
        ] {
            std::fs::write(&path, bytes).unwrap();
            let mut env = vec![("KEEP".into(), "kept".into())];
            let error = inject(&mut env, &path).unwrap_err();
            assert!(!format!("{error:#}").contains("fictional-private-marker"));
            assert_eq!(env, vec![("KEEP".into(), "kept".into())]);
        }
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(read_file(&path).is_err(), "unreadable is not absent");
    }
}
