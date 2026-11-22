//! Managed recipe credentials reach only the terminal host child's environment.
//! Read each spawn's values from one snapshot of the private state file.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

/// Size limit for the private file before parsing or passing data to a host.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;

/// The file contract for managed child environment entries. The environment
/// includes credential values, so the snapshot has no debug formatter.
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeCredentials {
    /// Child environment entries. The helm chooses their names by recipe mode.
    pub env: BTreeMap<String, String>,
}

/// The helm uses this basename for atomic delivery into the host state dir.
pub fn host_file(state_dir: &Path) -> PathBuf {
    state_dir.join("agent-credentials")
}

impl RecipeCredentials {
    /// Read one snapshot of the installed credentials for a spawn. A missing
    /// file leaves unmanaged hosts alone. Other failures carry fixed text
    /// because a parser error can quote private data.
    pub(crate) async fn read(path: &Path) -> anyhow::Result<Self> {
        let opened = tokio::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(path)
            .await;
        let file = match opened {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(_) => anyhow::bail!("could not open the recipe credentials file"),
        };
        anyhow::ensure!(
            file.metadata()
                .await
                .is_ok_and(|metadata| metadata.is_file()),
            "the recipe credentials file must be a regular file"
        );
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| anyhow::anyhow!("could not read the recipe credentials file"))?;
        anyhow::ensure!(
            bytes.len() <= MAX_FILE_BYTES,
            "the recipe credentials file is too large"
        );
        let credentials: Self = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("the recipe credentials file is invalid"))?;
        credentials.validate()?;
        Ok(credentials)
    }

    /// Reject malformed process environment entries before a launch
    /// publishes its spec or creates a terminal.
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.env.iter().all(|(name, value)| !name.is_empty()
                && !name.contains(['=', '\0'])
                && !value.contains('\0')),
            "the recipe credentials file has an invalid environment entry"
        );
        Ok(())
    }

    /// Replace only entries that the recipe manages and leave the rest of
    /// the child's composed environment intact.
    pub(crate) fn inject(self, env: &mut Vec<(OsString, OsString)>) {
        if self.env.is_empty() {
            return;
        }
        env.retain(|(name, _)| {
            name.to_str()
                .is_none_or(|name| !self.env.contains_key(name))
        });
        env.extend(
            self.env
                .into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        );
        env.sort_by(|left, right| left.0.cmp(&right.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refused launch can surface this error in logs or to a client. A
    /// malformed file must never send its credential values along with it.
    #[farhelm_testtrace::test]
    async fn malformed_recipe_credentials_do_not_leak_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = host_file(dir.path());
        for input in [
            r#"{"fictional-private":"unknown field"}"#,
            r#"{"env":{"BAD=NAME":"fictional-private"}}"#,
            r#"{"env":{"GOOD":"fictional-private\u0000"}}"#,
        ] {
            std::fs::write(&path, input).unwrap();
            let Err(error) = RecipeCredentials::read(&path).await else {
                panic!("the malformed credentials file was accepted");
            };
            assert!(!format!("{error:#}").contains("fictional-private"));
        }
    }
}
