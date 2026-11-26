//! The records config file and how the tool finds its slot.
//!
//! One config per host (`~/.fh-records/config` by default) lists the remote, the credentials, where slot clones live,
//! and every slot. It is TOML:
//!
//! ```toml
//! remote = "https://<account_id>.artifacts.cloudflare.net/git/records/agent-records.git"  # or a local path
//! token_file = "~/.fh-records/agent-records_token"   # optional; absent for a local bare repo remote
//! clones_dir = "~/.fh-records/clones"                 # optional; default shown
//!
//! [artifacts]                                         # optional; only `backup` reads it
//! account_id = "..."
//! namespace = "records"
//! repo = "agent-records"
//! api_token_file = "~/.fh-records/cloudflare-api_token"
//! api_base = "https://api.cloudflare.com/client/v4"   # optional; tests point it at a local server
//!
//! [[slot]]
//! name = "exec-1"
//! checkout = "/abs/path/to/checkout"
//! ```
//!
//! `~` (alone or as `~/...`) in a path value means the process's `HOME`; every path must be absolute after that.
//! Unknown keys are errors, so a typo cannot silently fall back to a default.
//!
//! Slot names match `[a-z0-9][a-z0-9-]*`, except the reserved `attended` and `status`: attended flows keep their
//! working clones in `<clones_dir>/attended/` and the plan status tooling its read-only clone in `<clones_dir>/status/`,
//! so a slot by either name would share a directory with them. Duplicate slot names or checkout paths are errors,
//! since either would make "which slot am I" ambiguous. A slot whose checkout does not exist is fine; nothing here
//! requires any listed checkout to exist.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::Failure;
use super::names::is_identifier;

/// The Cloudflare REST API's base URL, used when `[artifacts]` names no `api_base`.
const DEFAULT_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Slot names that would collide with other users of `clones_dir`.
const RESERVED_SLOT_NAMES: &[&str] = &["attended", "status"];

/// A loaded, validated config with every path expanded.
#[derive(Debug)]
pub struct Config {
    pub remote: String,
    pub token_file: Option<PathBuf>,
    pub clones_dir: PathBuf,
    pub artifacts: Option<Artifacts>,
    pub slots: Vec<Slot>,
}

/// `[artifacts]`: where `backup` mints its read token.
#[derive(Debug)]
pub struct Artifacts {
    pub account_id: String,
    pub namespace: String,
    pub repo: String,
    pub api_token_file: PathBuf,
    pub api_base: String,
}

/// One `[[slot]]`: a writer and the checkout it runs in.
#[derive(Debug)]
pub struct Slot {
    pub name: String,
    pub checkout: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    remote: String,
    token_file: Option<String>,
    clones_dir: Option<String>,
    artifacts: Option<RawArtifacts>,
    #[serde(default)]
    slot: Vec<RawSlot>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArtifacts {
    account_id: String,
    namespace: String,
    repo: String,
    api_token_file: String,
    api_base: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSlot {
    name: String,
    checkout: String,
}

/// Reads and validates the config at `path`. Every problem is exit 1 and names the file.
pub fn load(path: &Path, home: Option<&Path>) -> Result<Config, Failure> {
    let text = fs::read_to_string(path).map_err(|e| {
        Failure::failed(format!(
            "cannot read the records config {}: {e}",
            path.display()
        ))
    })?;
    parse(&text, home).map_err(|f| f.context(&format!("records config {}", path.display())))
}

/// Parses and validates config text; `home` is what `~` expands to.
pub fn parse(text: &str, home: Option<&Path>) -> Result<Config, Failure> {
    let raw: RawConfig =
        toml::from_str(text).map_err(|e| Failure::failed(e.to_string().trim_end().to_string()))?;
    if raw.remote.trim().is_empty() {
        return Err(Failure::failed("`remote` is empty"));
    }
    let remote = expand_remote(&raw.remote, home)?;
    let token_file = raw
        .token_file
        .as_deref()
        .map(|t| expand(t, home, "token_file"))
        .transpose()?;
    let clones_dir = expand(
        raw.clones_dir.as_deref().unwrap_or("~/.fh-records/clones"),
        home,
        "clones_dir",
    )?;
    let artifacts = raw.artifacts.map(|a| artifacts(a, home)).transpose()?;

    let mut slots = Vec::new();
    let mut names = HashSet::new();
    let mut checkouts = HashSet::new();
    for raw_slot in raw.slot {
        let name = raw_slot.name;
        if !is_identifier(&name) || RESERVED_SLOT_NAMES.contains(&name.as_str()) {
            return Err(Failure::failed(format!(
                "slot name `{name}` is not allowed (names match [a-z0-9][a-z0-9-]* and `attended` and `status` are \
                 reserved)"
            )));
        }
        if !names.insert(name.clone()) {
            return Err(Failure::failed(format!("slot `{name}` is listed twice")));
        }
        let checkout = expand(&raw_slot.checkout, home, "checkout")?;
        if !checkouts.insert(comparable(&checkout)) {
            return Err(Failure::failed(format!(
                "checkout {} is listed for two slots",
                checkout.display()
            )));
        }
        slots.push(Slot { name, checkout });
    }
    Ok(Config {
        remote,
        token_file,
        clones_dir,
        artifacts,
        slots,
    })
}

/// Validates `[artifacts]`. The three names go into a URL path, so they are restricted to characters that need no
/// escaping there.
fn artifacts(raw: RawArtifacts, home: Option<&Path>) -> Result<Artifacts, Failure> {
    for (key, value) in [
        ("account_id", &raw.account_id),
        ("namespace", &raw.namespace),
        ("repo", &raw.repo),
    ] {
        let ok = !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !ok || value.starts_with('.') {
            return Err(Failure::failed(format!(
                "[artifacts] {key} `{value}` must be letters, digits, `-`, `_` or `.`"
            )));
        }
    }
    let api_base = raw.api_base.unwrap_or_else(|| DEFAULT_API_BASE.to_string());
    if !(api_base.starts_with("https://") || api_base.starts_with("http://")) {
        return Err(Failure::failed(format!(
            "[artifacts] api_base `{api_base}` is not an http(s) URL"
        )));
    }
    Ok(Artifacts {
        account_id: raw.account_id,
        namespace: raw.namespace,
        repo: raw.repo,
        api_token_file: expand(&raw.api_token_file, home, "api_token_file")?,
        api_base: api_base.trim_end_matches('/').to_string(),
    })
}

/// Expands a leading `~` against `home` and requires an absolute result.
///
/// Only `~` and `~/...` are understood; `~user` is refused rather than guessed at.
fn expand(value: &str, home: Option<&Path>, key: &str) -> Result<PathBuf, Failure> {
    let path = if value == "~" || value.starts_with("~/") {
        let home =
            home.ok_or_else(|| Failure::failed(format!("`{key}` uses ~ but HOME is not set")))?;
        home.join(value[1..].trim_start_matches('/'))
    } else if value.starts_with('~') {
        return Err(Failure::failed(format!(
            "`{key}`: only ~ and ~/... are understood, not `{value}`"
        )));
    } else {
        PathBuf::from(value)
    };
    if !path.is_absolute() {
        return Err(Failure::failed(format!(
            "`{key}` must be an absolute path (or start with ~/), not `{value}`"
        )));
    }
    Ok(path)
}

/// `remote` may be a URL or a local path; only a local path starting with `~` is expanded.
fn expand_remote(value: &str, home: Option<&Path>) -> Result<String, Failure> {
    if value.starts_with('~') {
        Ok(expand(value, home, "remote")?
            .to_string_lossy()
            .into_owned())
    } else {
        Ok(value.to_string())
    }
}

/// A path in the form slot matching compares: canonical when it exists (so symlinks and `..` cannot make one
/// checkout look like two), lexically normalized otherwise (an absent checkout can still collide with another entry).
fn comparable(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        let mut normal = PathBuf::new();
        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normal.pop();
                }
                other => normal.push(other),
            }
        }
        normal
    })
}

/// The nearest directory at or above `cwd` that holds `.git` or `.jj` (a file or a directory, so git worktrees and
/// jj workspaces count), canonicalized.
pub fn find_checkout(cwd: &Path) -> Option<PathBuf> {
    let start = fs::canonicalize(cwd).ok()?;
    start
        .ancestors()
        .find(|dir| dir.join(".git").exists() || dir.join(".jj").exists())
        .map(Path::to_path_buf)
}

impl Config {
    /// The slot whose checkout is the nearest checkout containing `cwd`.
    ///
    /// Refuses (exit 1) when `cwd` is in no checkout or the checkout is not listed: guessing a slot would let one
    /// checkout write another's records.
    pub fn slot_for(&self, cwd: &Path, config_path: &Path) -> Result<&Slot, Failure> {
        let checkout = find_checkout(cwd).ok_or_else(|| {
            Failure::failed(format!(
                "{} is not inside a checkout (no .git or .jj above it)",
                cwd.display()
            ))
        })?;
        self.slots
            .iter()
            .find(|slot| comparable(&slot.checkout) == checkout)
            .ok_or_else(|| {
                Failure::failed(format!(
                    "no [[slot]] in {} lists the checkout {}",
                    config_path.display(),
                    checkout.display()
                ))
            })
    }

    /// The repo token from `token_file`, or `None` when the config names none (a local bare repo remote).
    pub fn read_token(&self) -> Result<Option<String>, Failure> {
        self.token_file
            .as_deref()
            .map(|path| read_secret(path, "token_file"))
            .transpose()
    }
}

/// Reads a one-line secret file, trimming surrounding whitespace.
///
/// A secret that still contains a line break would let the file inject a second HTTP header, so it is refused. The
/// error never includes the file's content.
pub fn read_secret(path: &Path, key: &str) -> Result<String, Failure> {
    let text = fs::read_to_string(path)
        .map_err(|e| Failure::failed(format!("cannot read {key} {}: {e}", path.display())))?;
    let secret = text.trim();
    if secret.is_empty() {
        return Err(Failure::failed(format!(
            "{key} {} is empty",
            path.display()
        )));
    }
    if secret.contains(['\r', '\n']) || secret.chars().any(char::is_control) {
        return Err(Failure::failed(format!(
            "{key} {} must hold one line",
            path.display()
        )));
    }
    Ok(secret.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/someone";

    fn parse_ok(text: &str) -> Config {
        parse(text, Some(Path::new(HOME))).unwrap()
    }

    fn parse_err(text: &str) -> String {
        let failure = parse(text, Some(Path::new(HOME))).unwrap_err();
        assert_eq!(failure.code, 1);
        failure.message
    }

    /// The contract's example config loads, `~` expands against HOME everywhere a path is allowed, and the defaults
    /// (clones_dir, api_base) apply when the keys are absent.
    #[test]
    fn example_config_loads_with_expansion_and_defaults() {
        let config = parse_ok(
            r#"
            remote = "https://example.invalid/git/records/agent-records.git"
            token_file = "~/.fh-records/agent-records_token"

            [artifacts]
            account_id = "0123abcd"
            namespace = "records"
            repo = "agent-records"
            api_token_file = "~/.fh-records/cloudflare-api_token"

            [[slot]]
            name = "exec-1"
            checkout = "~/git/one"

            [[slot]]
            name = "landing"
            checkout = "/srv/landing"
            "#,
        );
        assert_eq!(
            config.remote,
            "https://example.invalid/git/records/agent-records.git"
        );
        assert_eq!(
            config.token_file.unwrap(),
            Path::new("/home/someone/.fh-records/agent-records_token")
        );
        assert_eq!(
            config.clones_dir,
            Path::new("/home/someone/.fh-records/clones")
        );
        let artifacts = config.artifacts.unwrap();
        assert_eq!(artifacts.api_base, "https://api.cloudflare.com/client/v4");
        assert_eq!(
            artifacts.api_token_file,
            Path::new("/home/someone/.fh-records/cloudflare-api_token")
        );
        assert_eq!(config.slots[0].checkout, Path::new("/home/someone/git/one"));
        assert_eq!(config.slots[1].name, "landing");
    }

    /// A minimal config (remote only, no slots, no token) is valid: a local bare remote needs nothing else, and
    /// `backup` needs no slot.
    #[test]
    fn minimal_config_is_valid() {
        let config = parse_ok(r#"remote = "/srv/records.git""#);
        assert!(
            config.token_file.is_none() && config.artifacts.is_none() && config.slots.is_empty()
        );
    }

    /// Each config mistake is refused with a message naming it: duplicates make the slot ambiguous, bad names could
    /// escape the clones directory, reserved names would share a clone directory another user of it owns, relative
    /// paths depend on the caller's working directory, and unknown keys are usually typos that would otherwise
    /// silently fall back to a default.
    #[test]
    fn config_mistakes_are_refused() {
        let slot = |name: &str, checkout: &str| {
            format!("[[slot]]\nname = \"{name}\"\ncheckout = \"{checkout}\"\n")
        };
        let base = "remote = \"/r.git\"\n";
        let cases = [
            (
                format!("{base}{}{}", slot("exec-1", "/a"), slot("exec-1", "/b")),
                "listed twice",
            ),
            (
                format!("{base}{}{}", slot("exec-1", "/a"), slot("exec-2", "/a/")),
                "listed for two slots",
            ),
            (
                format!(
                    "{base}{}{}",
                    slot("exec-1", "/a/b/.."),
                    slot("exec-2", "/a")
                ),
                "listed for two slots",
            ),
            (format!("{base}{}", slot("Exec-1", "/a")), "not allowed"),
            (format!("{base}{}", slot("../x", "/a")), "not allowed"),
            (format!("{base}{}", slot("attended", "/a")), "not allowed"),
            (format!("{base}{}", slot("status", "/a")), "not allowed"),
            (
                format!("{base}{}", slot("exec-1", "relative/path")),
                "absolute",
            ),
            (format!("{base}{}", slot("exec-1", "~other/x")), "only ~"),
            (format!("{base}clones_dir = \"clones\"\n"), "absolute"),
            (format!("{base}colnes_dir = \"/x\"\n"), "unknown field"),
            ("remote = \"\"\n".to_string(), "empty"),
            ("token_file = \"/t\"\n".to_string(), "remote"),
            (
                format!(
                    "{base}[artifacts]\naccount_id = \"a/b\"\nnamespace = \"n\"\nrepo = \"r\"\napi_token_file = \"/t\"\n"
                ),
                "account_id",
            ),
        ];
        for (text, expected) in cases {
            let message = parse_err(&text);
            assert!(message.contains(expected), "{text:?}: {message}");
        }
    }

    /// Without HOME, `~` cannot be expanded and is an error rather than a path relative to the working directory.
    #[test]
    fn tilde_without_home_is_refused() {
        let failure = parse("remote = \"/r.git\"\n", None).unwrap_err();
        assert!(failure.message.contains("HOME"), "{}", failure.message);
    }

    /// Secrets are trimmed, and a file holding two lines is refused, since the token becomes an HTTP header value and a
    /// line break would smuggle in a second header.
    #[test]
    fn secrets_are_one_trimmed_line() {
        /// Removes the test's directory even when an assertion fails, so failed runs leave nothing behind.
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let dir = std::env::temp_dir().join(format!("xtask-secret-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let _cleanup = Cleanup(dir.clone());
        let file = dir.join("token");
        fs::write(&file, "  abc?expires=1\n").unwrap();
        assert_eq!(read_secret(&file, "token_file").unwrap(), "abc?expires=1");
        fs::write(&file, "abc\nInjected: yes\n").unwrap();
        let failure = read_secret(&file, "token_file").unwrap_err();
        assert!(
            !failure.message.contains("abc"),
            "the message must not echo the secret: {}",
            failure.message
        );
        fs::write(&file, "\n").unwrap();
        assert!(read_secret(&file, "token_file").is_err());
    }
}
