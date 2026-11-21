//! Grok and Antigravity discover conversation hooks from their own configuration directories.
//! A launch with consent refreshes only Farhelm's files there before it starts the agent.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use farhelm_proto::AgentKind;
use serde_json::Value;
use tokio::io::AsyncReadExt as _;

use super::{antigravity, grok, records};

/// One file the agent discovers on startup. The vendor module supplies the path and its JSON format.
pub(super) struct HookFile {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl HookFile {
    pub(super) fn json(path: PathBuf, value: Value) -> Result<Self> {
        let mut bytes = serde_json::to_vec_pretty(&value)?;
        bytes.push(b'\n');
        Ok(Self { path, bytes })
    }

    /// Read at most one byte beyond the intended content. A matching prefix of a longer file still needs replacement.
    async fn matches(&self) -> bool {
        let Ok(Some(file)) = records::open_regular_file(&self.path).await else {
            return false;
        };
        let mut actual = Vec::new();
        file.take(self.bytes.len() as u64 + 1)
            .read_to_end(&mut actual)
            .await
            .is_ok()
            && actual == self.bytes
    }

    /// Publish each file in one rename. Agents running alongside this launch can keep reading their open files.
    async fn install(self) -> Result<()> {
        if self.matches().await {
            return Ok(());
        }
        let parent = self.path.parent().context("hook path has no parent")?;
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("creating {}", parent.display()))?;
        tokio::task::spawn_blocking(move || {
            crate::files::overwrite_private_file_sync(
                &self.path,
                &self.bytes,
                &crate::files::RealFs,
            )
            .with_context(|| format!("writing {}", self.path.display()))
        })
        .await?
    }
}

/// Install the requested vendor integration using the supervisor's directories. The caller supplies consent and
/// checks the host's hook policy. Other agent kinds have no persistent conversation hook files.
pub(crate) async fn install(
    kind: AgentKind,
    home: Option<&Path>,
    grok_home: Option<&Path>,
    exe: &str,
) -> Result<()> {
    let files: Vec<_> = match kind {
        AgentKind::Grok => vec![grok::persistent_hook_file(home, grok_home, exe)?],
        AgentKind::Antigravity => {
            antigravity::persistent_hook_files(home.context("the supervisor has no HOME")?, exe)
                .await?
                .into()
        }
        AgentKind::Claude
        | AgentKind::Codex
        | AgentKind::Goose
        | AgentKind::Pi
        | AgentKind::Omp
        | AgentKind::Muse
        | AgentKind::Generic => anyhow::bail!("this agent has no persistent conversation hooks"),
    };
    for file in files {
        file.install().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::MetadataExt as _;

    use serde_json::json;

    use super::*;

    /// Pin the configuration Grok reads from GROK_HOME. In an ordinary shell the command must invoke the stable
    /// executable as one word and omit the announcement, even though the hook also runs outside Farhelm sessions.
    #[farhelm_testtrace::test]
    async fn persistent_hook_file_grok_bytes_use_the_supervisor_override() {
        let home = tempfile::tempdir().unwrap();
        let grok_home = home.path().join("custom-grok");
        let exe = "/stable path/farhelm";
        let destination = grok_home.join("hooks/farhelm.json");
        assert!(!destination.exists());
        install(AgentKind::Grok, Some(home.path()), Some(&grok_home), exe)
            .await
            .unwrap();
        let callback = json!([{
            "hooks": [{ "type": "command", "command": "'/stable path/farhelm' internal hook --vendor grok", "timeout": 60 }],
        }]);
        let expected = json!({
            "hooks": { "SessionStart": callback, "UserPromptSubmit": callback, "Stop": callback },
        });
        assert_eq!(
            fs::read_to_string(destination).unwrap(),
            format!("{expected:#}\n")
        );
        assert!(!home.path().join(".grok").exists());
    }

    /// Antigravity discovers Farhelm's plugin beside existing configuration. A later start should update changed
    /// bytes in its files and retain the inode of each matching file.
    #[farhelm_testtrace::test]
    async fn persistent_hook_files_antigravity_update_each_file_independently() {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join(".gemini/config");
        fs::create_dir_all(&config).unwrap();
        let manual = config.join("hooks.json");
        fs::write(&manual, "{\"other-hooks\": {}}\n").unwrap();
        let plugin = config.join("plugins/farhelm/plugin.json");
        let hooks = config.join("plugins/farhelm/hooks.json");
        assert!(!plugin.exists() && !hooks.exists());
        let exe = "/stable path/farhelm";
        install(AgentKind::Antigravity, Some(home.path()), None, exe)
            .await
            .unwrap();

        let manifest = json!({ "name": "farhelm", "displayName": "Farhelm", "version": "1.0.0",
            "description": "Reports conversations to Farhelm for Restart" });
        let callback = json!([{
            "type": "command", "command": "'/stable path/farhelm' internal hook --vendor antigravity", "timeout": 60,
        }]);
        let expected = json!({ "farhelm": { "SessionStart": callback, "Stop": callback } });
        assert_eq!(
            fs::read_to_string(&plugin).unwrap(),
            format!("{manifest:#}\n")
        );
        assert_eq!(
            fs::read_to_string(&hooks).unwrap(),
            format!("{expected:#}\n")
        );

        // Keep the original file descriptors open so neither inode can be reused after a replacing rename.
        let original_plugin = fs::File::open(&plugin).unwrap();
        fs::write(&hooks, format!("{expected:#}\n ")).unwrap();
        let original_hooks = fs::File::open(&hooks).unwrap();
        install(AgentKind::Antigravity, Some(home.path()), None, exe)
            .await
            .unwrap();
        assert_eq!(
            fs::metadata(&plugin).unwrap().ino(),
            original_plugin.metadata().unwrap().ino()
        );
        assert_ne!(
            fs::metadata(&hooks).unwrap().ino(),
            original_hooks.metadata().unwrap().ino()
        );
        assert_eq!(
            fs::read_to_string(hooks).unwrap(),
            format!("{expected:#}\n")
        );
        assert_eq!(
            fs::read_to_string(manual).unwrap(),
            "{\"other-hooks\": {}}\n"
        );
    }

    /// A plugin directory pointing at the shared configuration must leave the user's hook groups intact.
    #[farhelm_testtrace::test]
    async fn persistent_hook_files_antigravity_refuses_redirected_plugin_directory() {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join(".gemini/config");
        fs::create_dir_all(config.join("plugins")).unwrap();
        let manual = config.join("hooks.json");
        fs::write(&manual, "{\"user-hooks\": {}}\n").unwrap();
        let redirect = config.join("plugins/farhelm");
        std::os::unix::fs::symlink(&config, &redirect).unwrap();
        assert_eq!(fs::read_link(&redirect).unwrap(), config);
        let error = install(
            AgentKind::Antigravity,
            Some(home.path()),
            None,
            "/stable/farhelm",
        )
        .await
        .unwrap_err();
        assert!(
            error.to_string().contains("dedicated plugin directory"),
            "{error:#}"
        );
        assert_eq!(
            fs::read_to_string(manual).unwrap(),
            "{\"user-hooks\": {}}\n"
        );
        assert!(!config.join("plugin.json").exists());
    }
}
