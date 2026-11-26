//! Running the `git` CLI: one place that decides what every git child process sees.
//!
//! The tool shells out to git rather than linking a git library: every host has git, the operations are few, and the
//! CLI's behavior (including Cloudflare Artifacts' smart-HTTP quirks) is what attended clones see too.
//!
//! Every child gets the same controlled environment, because the tool runs unattended on hosts whose git setup it
//! does not own:
//!
//! - Inherited `GIT_*` variables are removed, so a caller's `GIT_DIR`, `GIT_INDEX_FILE` or `GIT_CONFIG_COUNT` (from a
//!   hook, or a test harness) cannot redirect the tool to another repository or splice in config.
//! - System and global config are ignored (`GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_GLOBAL=/dev/null`): a user's hooks
//!   path, signing setup, credential helpers (including the macOS keychain), excludes file or `url.insteadOf` must not
//!   change what a records push does. Only the clone's own config applies.
//! - Settings the tool depends on are passed as `GIT_CONFIG_KEY_<i>`/`GIT_CONFIG_VALUE_<i>` pairs: no hooks, no
//!   signing, no credential helper, no excludes file, and a low-speed abort so a stalled transfer fails instead of
//!   holding the clone's lock forever.
//! - With a token, the pairs also carry `http.extraHeader = Authorization: Bearer <token>`, the whole token string
//!   including its `?expires=` suffix (Cloudflare's recommended form for git over HTTPS). Passing it in the child's
//!   environment keeps it off every command line (visible to other users in `ps`), out of the clone's config, and out
//!   of output; git redacts the header in its own traces.
//! - `GIT_TERMINAL_PROMPT=0` and a null stdin: nothing may ever wait for a password.
//! - `LC_ALL=C`, because the push rejection reasons are matched as text.

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use super::Failure;

/// Git settings every child process gets, as `(key, value)`. See the module docs for why each one is here.
const FIXED_CONFIG: &[(&str, &str)] = &[
    ("core.hooksPath", "/dev/null"),
    ("core.excludesFile", "/dev/null"),
    ("commit.gpgSign", "false"),
    ("credential.helper", ""),
    // Abort an HTTP transfer that stays below 1 KB/s for a minute.
    ("http.lowSpeedLimit", "1000"),
    ("http.lowSpeedTime", "60"),
];

/// The environment variables a git child gets on top of the inherited ones (minus inherited `GIT_*`).
///
/// This is the only place the token goes, and it goes into a value, never a key or an argument.
pub fn child_env(token: Option<&str>) -> Vec<(String, String)> {
    let mut config: Vec<(String, String)> = FIXED_CONFIG
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    if let Some(token) = token {
        config.push((
            "http.extraHeader".to_string(),
            format!("Authorization: Bearer {token}"),
        ));
    }
    let mut env = vec![
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
        // Record names and directories are plain path components, but a pathspec should never be a pattern.
        ("GIT_LITERAL_PATHSPECS".to_string(), "1".to_string()),
        ("LC_ALL".to_string(), "C".to_string()),
        ("GIT_CONFIG_COUNT".to_string(), config.len().to_string()),
    ];
    for (i, (key, value)) in config.into_iter().enumerate() {
        env.push((format!("GIT_CONFIG_KEY_{i}"), key));
        env.push((format!("GIT_CONFIG_VALUE_{i}"), value));
    }
    env
}

/// A git runner bound to one credential (or none).
pub struct Git {
    token: Option<String>,
}

impl Git {
    pub fn new(token: Option<String>) -> Self {
        Self { token }
    }

    /// Starts building `git -C <dir> ...` with the controlled environment.
    pub fn at(&self, dir: &Path) -> GitCall {
        let mut command = Command::new("git");
        command.arg("-C").arg(dir);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        for (key, value) in child_env(self.token.as_deref()) {
            command.env(key, value);
        }
        GitCall {
            command,
            args: Vec::new(),
            stdin: None,
        }
    }
}

/// One git invocation being built. `args` mirrors the arguments for error messages; they never hold a secret.
pub struct GitCall {
    command: Command,
    args: Vec<OsString>,
    stdin: Option<Vec<u8>>,
}

/// What a finished git process produced.
pub struct GitOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl GitOutput {
    /// Stdout as text with the trailing newline removed (for single-value commands like `rev-parse`).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).trim_end().to_string()
    }
}

impl GitCall {
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.args.push(arg.as_ref().to_os_string());
            self.command.arg(arg);
        }
        self
    }

    pub fn env(mut self, key: &str, value: impl AsRef<OsStr>) -> Self {
        self.command.env(key, value);
        self
    }

    pub fn stdin(mut self, input: Vec<u8>) -> Self {
        self.stdin = Some(input);
        self
    }

    /// Runs git and returns its output whatever the exit status; for commands whose status is an answer
    /// (`merge-base --is-ancestor`, a rejected `push`).
    pub fn output(mut self) -> Result<GitOutput, Failure> {
        let has_stdin = self.stdin.is_some();
        self.command
            .stdin(if has_stdin {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = self
            .command
            .spawn()
            .map_err(|e| Failure::failed(format!("cannot run git: {e}")))?;
        // Write stdin on its own thread so a child that fills its stdout pipe before reading all of stdin cannot
        // deadlock against us.
        let writer = match (self.stdin.take(), child.stdin.take()) {
            (Some(input), Some(mut pipe)) => {
                Some(std::thread::spawn(move || pipe.write_all(&input)))
            }
            _ => None,
        };
        let output = child
            .wait_with_output()
            .map_err(|e| Failure::failed(format!("waiting for git: {e}")))?;
        if let Some(writer) = writer {
            // A write error means git exited early; its status and stderr below say why.
            let _ = writer.join();
        }
        Ok(GitOutput {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    /// Runs git and fails (exit 1) unless it succeeded, quoting git's stderr.
    pub fn run(self) -> Result<GitOutput, Failure> {
        let described = self.describe();
        let output = self.output()?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(Failure::failed(format!(
                "{described} failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )))
        }
    }

    /// `git <args>` for error messages, without `-C <dir>` (the caller's context names the clone).
    pub fn describe(&self) -> String {
        let args: Vec<_> = self.args.iter().map(|a| a.to_string_lossy()).collect();
        format!("git {}", args.join(" "))
    }

    /// The command as it will be spawned, for tests that check what reaches the child.
    #[cfg(test)]
    pub fn command(&self) -> &Command {
        &self.command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "art_v1_secretsecret?expires=1760000000";

    /// The token reaches git only as the value of an `http.extraHeader` config pair in the child's environment, whole
    /// (with its `?expires=` suffix) and as a Bearer header. It must never be an argument: command lines are visible
    /// to every user on the host, and they end up in error messages and logs.
    #[test]
    fn token_travels_only_in_the_environment() {
        let call = Git::new(Some(TOKEN.to_string()))
            .at(Path::new("/clone"))
            .args(["fetch", "origin"]);
        let command = call.command();
        for arg in command.get_args() {
            assert!(
                !arg.to_string_lossy().contains("secret"),
                "token in argv: {arg:?}"
            );
        }
        assert!(!call.describe().contains("secret"));
        let env: Vec<(String, String)> = command
            .get_envs()
            .filter_map(|(k, v)| {
                Some((
                    k.to_string_lossy().into_owned(),
                    v?.to_string_lossy().into_owned(),
                ))
            })
            .collect();
        let count: usize = env
            .iter()
            .find(|(k, _)| k == "GIT_CONFIG_COUNT")
            .unwrap()
            .1
            .parse()
            .unwrap();
        let pairs: Vec<(String, String)> = (0..count)
            .map(|i| {
                let get = |name: String| env.iter().find(|(k, _)| *k == name).unwrap().1.clone();
                (
                    get(format!("GIT_CONFIG_KEY_{i}")),
                    get(format!("GIT_CONFIG_VALUE_{i}")),
                )
            })
            .collect();
        let headers: Vec<_> = pairs
            .iter()
            .filter(|(k, _)| k == "http.extraHeader")
            .collect();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].1, format!("Authorization: Bearer {TOKEN}"));
        for (key, value) in &env {
            if value.contains("secret") {
                assert!(key.starts_with("GIT_CONFIG_VALUE_"), "token in {key}");
            }
        }
    }

    /// Without a token (a local bare remote) no Authorization header is configured at all.
    #[test]
    fn no_token_means_no_header() {
        assert!(
            child_env(None)
                .iter()
                .all(|(_, v)| !v.contains("Authorization"))
        );
    }

    /// The isolation settings are always present: they are what make the tool's git behave the same on every host.
    #[test]
    fn isolation_settings_are_always_present() {
        let env = child_env(None);
        for key in [
            "GIT_CONFIG_NOSYSTEM",
            "GIT_CONFIG_GLOBAL",
            "GIT_TERMINAL_PROMPT",
            "LC_ALL",
        ] {
            assert!(env.iter().any(|(k, _)| k == key), "{key}");
        }
        // File names under notes/ may contain `[`, `*`, `?` or a leading `:`; as pathspecs they must stay names.
        assert!(
            env.iter()
                .any(|(k, v)| k == "GIT_LITERAL_PATHSPECS" && v == "1")
        );
        assert!(env.iter().any(|(_, v)| v == "core.hooksPath"));
    }
}
