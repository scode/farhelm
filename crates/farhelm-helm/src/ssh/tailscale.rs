//! Resolve an explicitly selected Tailscale host before building any OpenSSH command.
//!
//! The local daemon supplies both the connection address and the SSH host keys. Keep them together so a name
//! resolved from Tailscale never sends OpenSSH to a different address or falls back to its ordinary trust store.

use anyhow::{Context, anyhow, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    io::{Read, Write},
    net::IpAddr,
    os::unix::{
        ffi::OsStrExt as _,
        fs::{DirBuilderExt, FileTypeExt as _, MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::Stdio,
    sync::Mutex,
    time::Duration,
};

const STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const CONTROL_DIR_PREFIX: &str = "fhs-";
const CONTROL_DIR_RANDOM_BYTES: usize = 10;
const CONTROL_DIR_BASENAME_LEN: usize = CONTROL_DIR_PREFIX.len() + CONTROL_DIR_RANDOM_BYTES;
const CONTROL_DIR_RECORD: &str = "control-dir";

/// A state directory has one serving helm. Serializing allocation here gives concurrent dials the same on-disk
/// rendezvous without introducing a lockfile that would live inside a directory the last host can remove.
static CONTROL_DIR_ALLOCATION: Mutex<()> = Mutex::new(());

/// One prepared peer. Every SSH invocation for it uses the address selected with its advertised keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TailscaleSshAccess {
    address: IpAddr,
    known_hosts: String,
    proxy: String,
    control_dir: PathBuf,
    socket_scope: String,
}

impl TailscaleSshAccess {
    /// Ask the local daemon for current keys on each preparation. An unavailable daemon is a connection error.
    pub(crate) async fn prepare(state: &Path, destination: &str) -> anyhow::Result<Self> {
        let fallback = cfg!(target_os = "macos")
            .then(|| Path::new("/Applications/Tailscale.app/Contents/MacOS/Tailscale"));
        let program = find_program(std::env::var_os("PATH").as_deref(), fallback)?;
        Self::prepare_with(&program, state, Path::new("/tmp"), destination).await
    }

    /// Supplying the executable directly keeps the process PATH out of tests and binds the proxy to the same CLI.
    async fn prepare_with(
        program: &Path,
        state: &Path,
        runtime: &Path,
        destination: &str,
    ) -> anyhow::Result<Self> {
        let host = destination_host(destination)?;
        let output = tokio::time::timeout(
            STATUS_TIMEOUT,
            tokio::process::Command::new(program)
                .args(["status", "--json"])
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .context("Tailscale took too long to answer. Check that it is running and retry.")?
        .context("Could not run the local Tailscale command")?;
        // Some non-running states still emit JSON and return nonzero. Preserve the useful daemon verdict.
        let status: Status = serde_json::from_slice(&output.stdout).map_err(|error| {
            if output.status.success() {
                anyhow!(error).context("Could not read the local Tailscale status")
            } else {
                anyhow!("Could not read Tailscale status. Start Tailscale and retry.")
            }
        })?;
        ensure!(
            status.backend_state == "Running",
            "Tailscale is not running. Start Tailscale and retry."
        );
        ensure!(
            output.status.success(),
            "Could not read Tailscale status. Check that it is running and retry."
        );
        let (address, keys) = status.resolve(host)?;
        let proxy = if status.tun {
            "ProxyCommand=none".into()
        } else {
            proxy_option(program)?
        };
        let contents = key_lines(address, keys)?;
        let path = known_hosts_path(state, destination, &contents)?;
        let known_hosts = super::ssh_config_path_option(
            "UserKnownHostsFile",
            path.to_str()
                .context("Tailscale's known_hosts path must be UTF-8")?,
        )
        .map_err(anyhow::Error::msg)?;
        // A shared connection also fixes its network path. A change between direct and userspace modes
        // needs a new master even if the peer's keys and address stay the same.
        let socket_scope = short_digest([contents.as_bytes(), proxy.as_bytes()]);
        let (state, runtime, destination) =
            (state.to_owned(), runtime.to_owned(), destination.to_owned());
        let control_dir = tokio::task::spawn_blocking(move || {
            write_private_file(&path, &contents)
                .context("Could not publish Tailscale host keys")?;
            control_directory(&state, &runtime, &destination)
        })
        .await
        .context("Preparing the Tailscale host keys did not complete")??;
        Ok(Self {
            address,
            known_hosts,
            proxy,
            control_dir,
            socket_scope,
        })
    }

    /// Use the pinned IP as the key alias so OpenSSH looks up the same entry even with a configured port.
    /// Its optional key commands and SSHFP validation have no role in the daemon's key check.
    pub(crate) fn options(&self) -> Vec<String> {
        vec![
            format!("HostName={}", self.address),
            format!("HostKeyAlias={}", self.address),
            self.known_hosts.clone(),
            "GlobalKnownHostsFile=/dev/null".into(),
            "StrictHostKeyChecking=yes".into(),
            "UpdateHostKeys=no".into(),
            "CheckHostIP=no".into(),
            "VerifyHostKeyDNS=no".into(),
            "CanonicalizeHostname=no".into(),
            "KnownHostsCommand=none".into(),
            // The nc bridge copies bytes. It does not pass an open socket to ssh.
            "ProxyUseFdpass=no".into(),
            self.proxy.clone(),
        ]
    }

    /// OpenSSH resolves %C at connection time from the user's effective SSH settings. The pin scope ensures
    /// the connection also authenticated against the current advertised keys and uses the selected network path.
    pub(super) fn control_socket(&self, connection: super::SharedConnection) -> Option<PathBuf> {
        let purpose = match connection {
            super::SharedConnection::Supervisor => 't',
            super::SharedConnection::Provisioning => 'q',
        };
        let name = format!("{purpose}{}%C", self.socket_scope);
        super::control_socket_named(&self.control_dir, &name, super::MAX_SOCKET_PATH)
    }
}

/// A 96-bit suffix keeps per-pin sharing paths short while distinguishing independently prepared key sets.
fn short_digest<'a>(fields: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut hash = Sha256::new();
    for field in fields {
        hash.update((field.len() as u64).to_be_bytes());
        hash.update(field);
    }
    URL_SAFE_NO_PAD.encode(&hash.finalize()[..12])
}

/// Compute the pre-upgrade rendezvous for this destination only. New connections never create it.
fn legacy_control_directory(
    state: &Path,
    runtime: &Path,
    destination: &str,
) -> anyhow::Result<PathBuf> {
    let state = state
        .canonicalize()
        .context("Could not resolve Farhelm's state directory")?;
    // SAFETY: geteuid takes no arguments and reports the effective Unix uid of this process.
    let uid = unsafe { libc::geteuid() };
    let scope = short_digest([
        &uid.to_be_bytes()[..],
        state.as_os_str().as_bytes(),
        destination.as_bytes(),
    ]);
    Ok(runtime.join(format!("fhs-{scope}")))
}

/// Keep the OS-allocated private directory across ordinary helm restarts so earlier masters can still be retired.
/// The record holds only the basename. When the runtime directory is wiped, the next prepare allocates a new
/// directory and replaces the record.
fn control_directory(state: &Path, runtime: &Path, destination: &str) -> anyhow::Result<PathBuf> {
    let _allocation = CONTROL_DIR_ALLOCATION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let record = known_hosts_dir(state, destination)?.join(CONTROL_DIR_RECORD);
    if let Some(path) = recorded_control_directory(&record, runtime)? {
        return Ok(path);
    }
    let dir = tempfile::Builder::new()
        .prefix(CONTROL_DIR_PREFIX)
        .rand_bytes(CONTROL_DIR_RANDOM_BYTES)
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(runtime)
        .context("Could not allocate Farhelm's private Tailscale socket directory")?;
    require_private_control_directory(dir.path(), &dir.path().symlink_metadata()?)?;
    let basename = dir
        .path()
        .file_name()
        .and_then(OsStr::to_str)
        .context("Tailscale socket directory basename must be UTF-8")?;
    write_private_file(&record, basename)
        .context("Could not record Farhelm's Tailscale socket directory")?;
    // Leave cleanup with retirement once the basename is published. Until then TempDir owns failed allocations.
    Ok(dir.keep())
}

/// Treat stale or malformed records as absent. The record cannot introduce a path outside the runtime root,
/// name a pre-upgrade directory, or ask SSH to use a directory we would refuse in the ordinary preparation path.
fn recorded_control_directory(record: &Path, runtime: &Path) -> anyhow::Result<Option<PathBuf>> {
    let metadata = match record.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).context("inspecting the Tailscale socket directory record");
        }
    };
    if !metadata.is_file() || metadata.len() != CONTROL_DIR_BASENAME_LEN as u64 {
        return Ok(None);
    }
    let mut basename = Vec::new();
    let read = fs::File::open(record).and_then(|file| {
        file.take(CONTROL_DIR_BASENAME_LEN as u64 + 1)
            .read_to_end(&mut basename)
    });
    if let Err(error) = read {
        if matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
        ) {
            return Ok(None);
        }
        return Err(error).context("reading the Tailscale socket directory record");
    }
    let Some(path) = std::str::from_utf8(&basename)
        .ok()
        .filter(|name| {
            name.len() == CONTROL_DIR_BASENAME_LEN
                && name
                    .strip_prefix(CONTROL_DIR_PREFIX)
                    .is_some_and(|suffix| suffix.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        })
        .map(|name| runtime.join(name))
    else {
        return Ok(None);
    };
    Ok(is_private_control_directory(&path).then_some(path))
}

/// Inspect the directory itself. A symlink to a directory with the right owner and mode is still a foreign route.
fn is_private_control_directory(path: &Path) -> bool {
    path.symlink_metadata()
        .is_ok_and(|metadata| require_private_control_directory(path, &metadata).is_ok())
}

fn require_private_control_directory(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> anyhow::Result<()> {
    // SAFETY: geteuid takes no arguments and reports the effective Unix uid of this process.
    let uid = unsafe { libc::geteuid() };
    ensure!(
        metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o777 == 0o700,
        "Farhelm's Tailscale socket directory at {} must be private and owned by this account.",
        path.display(),
    );
    Ok(())
}

/// The exact directories inspected for this retirement, including empty ones. A failed exit leaves its socket
/// in place, so the directory can be removed only if the kernel confirms it is empty.
pub(crate) struct SharedSockets {
    pub(crate) paths: Vec<PathBuf>,
    directories: Vec<PathBuf>,
}

impl SharedSockets {
    /// Retire empty directories after asking their masters to exit. Keep an entry if it stopped being private.
    ///
    /// Callers park or replace the old route first. An empty directory can still be named by a prepared command.
    pub(crate) fn remove_empty_directories(self) {
        for directory in self.directories {
            if is_private_control_directory(&directory) {
                let _ = fs::remove_dir(directory);
            }
        }
    }
}

/// Find published mux sockets without asking the daemon for its current address or keys. That lookup could miss
/// masters from earlier generations, and scanning the runtime root could pick up another destination's masters.
pub(crate) fn shared_sockets(state: &Path, destination: &str) -> anyhow::Result<SharedSockets> {
    shared_sockets_at(state, Path::new("/tmp"), destination)
}

fn shared_sockets_at(
    state: &Path,
    runtime: &Path,
    destination: &str,
) -> anyhow::Result<SharedSockets> {
    let record = known_hosts_dir(state, destination)?.join(CONTROL_DIR_RECORD);
    let recorded = match recorded_control_directory(&record, runtime) {
        Ok(recorded) => recorded,
        Err(error) => {
            tracing::warn!(%error, "could not read the Tailscale socket directory record");
            None
        }
    };
    let legacy = legacy_control_directory(state, runtime, destination)?;
    let mut shared = SharedSockets {
        paths: Vec::new(),
        directories: Vec::new(),
    };
    for directory in recorded.into_iter().chain(Some(legacy)) {
        if !is_private_control_directory(&directory) {
            continue;
        }
        match sockets_in(&directory) {
            Ok(sockets) => {
                shared.paths.extend(sockets);
                shared.directories.push(directory);
            }
            Err(error) => tracing::warn!(%error, "could not inspect a Tailscale socket directory"),
        }
    }
    Ok(shared)
}

fn sockets_in(directory: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut sockets = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        // One purpose byte, a 96-bit base64url pin scope and OpenSSH's
        // 40-character %C digest. The longer temporary bind names stay out.
        if let [b't' | b'q', rest @ ..] = name.as_bytes()
            && rest.len() == 16 + super::EXPANDED_TOKEN
            && rest[..16]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(byte))
            && rest[16..].iter().all(u8::is_ascii_hexdigit)
            && entry.file_type()?.is_socket()
        {
            sockets.push(entry.path());
        }
    }
    Ok(sockets)
}

/// PATH wins over the app-bundle fallback, and the resulting absolute executable also supplies the proxy command.
fn find_program(path: Option<&OsStr>, fallback: Option<&Path>) -> anyhow::Result<PathBuf> {
    let found = path
        .into_iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join("tailscale"))
        .chain(fallback.map(Path::to_path_buf))
        .find(|path| {
            path.metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .context("Tailscale was not found. Install Tailscale or add its CLI to PATH.")?;
    std::path::absolute(&found).context("Could not resolve the local Tailscale executable")
}

/// Keep the address format guidance when registration or an edit refuses an otherwise usable SSH destination.
#[derive(Debug, thiserror::Error)]
#[error(
    "Tailscale mode needs a node name, full Tailscale DNS name, or IP address. \
     An SSH login is optional (login@address)."
)]
pub(crate) struct InvalidTailscaleDestination;

/// The destination can be a Tailscale IP or a DNS name, optionally prefixed by a login. A port comes from ssh config.
pub(crate) fn destination_host(destination: &str) -> anyhow::Result<&str> {
    let (user, host) = destination
        .split_once('@')
        .map_or((None, destination), |(user, host)| (Some(user), host));
    let host_name = host.strip_suffix('.').unwrap_or(host);
    let dns_name = host_name.len() <= 253
        && host_name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });
    ensure!(
        user.is_none_or(|user| {
            !user.is_empty()
                && !user.starts_with('-')
                && user
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_.-+".contains(&byte))
        }) && (host.parse::<IpAddr>().is_ok() || dns_name),
        InvalidTailscaleDestination
    );
    Ok(host)
}

/// Go's status response represents unavailable peers and lists as JSON null.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Status {
    backend_state: String,
    #[serde(rename = "TUN")]
    tun: bool,
    #[serde(rename = "Self")]
    local: Option<Peer>,
    peer: Option<BTreeMap<String, Peer>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Peer {
    host_name: String,
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "TailscaleIPs")]
    addresses: Option<Vec<IpAddr>>,
    #[serde(rename = "sshHostKeys", alias = "SSH_HostKeys")]
    keys: Option<Vec<String>>,
}

impl Status {
    /// Exact IPs and qualified names take priority. Short names must identify a single node across the tailnet.
    fn resolve(&self, host: &str) -> anyhow::Result<(IpAddr, &[String])> {
        let peers = || {
            self.local
                .iter()
                .chain(self.peer.iter().flat_map(BTreeMap::values))
        };
        let name = host.trim_end_matches('.');
        let exact_ip = host.parse::<IpAddr>().ok();
        let mut matches = peers().filter(|peer| match exact_ip {
            Some(ip) => peer
                .addresses
                .as_deref()
                .is_some_and(|ips| ips.contains(&ip)),
            None => peer
                .dns_name
                .trim_end_matches('.')
                .eq_ignore_ascii_case(name),
        });
        let peer = match (matches.next(), matches.next()) {
            (Some(peer), None) => Some(peer),
            (None, None) if exact_ip.is_none() && !name.contains('.') => {
                let mut matches = peers().filter(|peer| {
                    peer.host_name.eq_ignore_ascii_case(name)
                        || peer.dns_name.split_once('.')
                            .is_some_and(|(short, _)| short.eq_ignore_ascii_case(name))
                });
                let first = matches.next();
                if let (Some(first), Some(second)) = (first, matches.next()) {
                    let first = first.display_address();
                    let second = second.display_address();
                    bail!(
                        "More than one Tailscale node has that name. Enter a full MagicDNS name or Tailscale IP, such as {first:?} or {second:?}."
                    );
                }
                first
            }
            (None, None) => None,
            _ => bail!("More than one Tailscale node matches this destination. Enter its Tailscale IP."),
        }
        .context("No Tailscale node matches this destination. Enter its node name or Tailscale IP.")?;
        let keys = peer
            .keys
            .as_deref()
            .filter(|keys| !keys.is_empty())
            .context("Tailscale does not advertise SSH host keys for that node.")?;
        let addresses = peer.addresses.as_deref().unwrap_or_default();
        let address = exact_ip
            .or_else(|| addresses.iter().copied().find(IpAddr::is_ipv4))
            .or_else(|| addresses.first().copied())
            .context("Tailscale does not advertise a network address for that node.")?;
        Ok((address, keys))
    }
}

impl Peer {
    /// Name candidates in an ambiguous lookup without dumping the tailnet's status or keys.
    fn display_address(&self) -> String {
        if self.dns_name.is_empty() {
            return self
                .addresses
                .as_deref()
                .and_then(|ips| ips.first())
                .map_or_else(
                    || self.host_name.chars().take(253).collect(),
                    ToString::to_string,
                );
        }
        self.dns_name
            .trim_end_matches('.')
            .chars()
            .take(253)
            .collect()
    }
}

/// Keep the private key files for a single destination together, including earlier prepared snapshots.
pub(crate) fn known_hosts_dir(state: &Path, destination: &str) -> anyhow::Result<PathBuf> {
    let digest = URL_SAFE_NO_PAD.encode(Sha256::digest(destination.as_bytes()));
    Ok(std::path::absolute(state)?.join("tailscale").join(digest))
}

/// Discard private pin generations after the registry can no longer start new dials for this destination.
pub(crate) async fn forget_keys(state: &Path, destination: &str) -> anyhow::Result<()> {
    match tokio::fs::remove_dir_all(known_hosts_dir(state, destination)?).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("removing private Tailscale SSH keys"),
    }
}

/// Freeze the lookup's IP and keys in the name before any caller can launch ssh with this file. Concurrent
/// preparations can refresh the same destination without changing which set an already prepared command reads.
fn known_hosts_path(state: &Path, destination: &str, contents: &str) -> anyhow::Result<PathBuf> {
    Ok(known_hosts_dir(state, destination)?.join(URL_SAFE_NO_PAD.encode(Sha256::digest(contents))))
}

/// The known_hosts grammar has separate host and key fields. Emit just the advertised key type and base64 token.
fn key_lines(address: IpAddr, keys: &[String]) -> anyhow::Result<String> {
    let mut entries = Vec::with_capacity(keys.len());
    for key in keys {
        let mut words = key.split_ascii_whitespace();
        let kind = words.next().filter(|word| {
            ["ssh-", "ecdsa-", "sk-"]
                .iter()
                .any(|prefix| word.starts_with(prefix))
                && word
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"@._-".contains(&byte))
        });
        let blob = words.next().filter(|word| {
            word.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"+/=".contains(&byte))
        });
        let (kind, blob) = kind
            .zip(blob)
            .filter(|_| !key.chars().any(char::is_control))
            .context("Tailscale advertised an invalid SSH host key.")?;
        entries.push(format!("{address} {kind} {blob}\n"));
    }
    // Order is immaterial to the pin. Canonical bytes let repeated lookups share the same file and master.
    entries.sort_unstable();
    entries.dedup();
    Ok(entries.concat())
}

/// Publish the complete destination record or key snapshot in one rename inside the private state directory.
fn write_private_file(path: &Path, contents: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .context("Tailscale destination file has no parent directory")?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .context("Could not create Farhelm's Tailscale destination directory")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .context("Could not create Farhelm's private Tailscale file")?;
    file.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))
        .context("Could not restrict Farhelm's private Tailscale file")?;
    file.write_all(contents.as_bytes())
        .context("Could not write Farhelm's private Tailscale file")?;
    file.persist(path)
        .context("Could not replace Farhelm's private Tailscale file")?;
    Ok(())
}

/// SSH expands percent tokens in ProxyCommand before handing it to the shell. Keep the program path a literal word.
fn proxy_option(program: &Path) -> anyhow::Result<String> {
    let program = program
        .to_str()
        .context("The Tailscale executable path must be UTF-8")?;
    ensure!(
        !program.contains("${") && !program.chars().any(char::is_control),
        "The Tailscale executable path cannot be represented in OpenSSH's proxy command."
    );
    // Use a double-quoted shell word that also survives OpenSSH's preliminary quote check.
    let mut word = String::from("\"");
    for character in program.chars() {
        match character {
            '%' => word.push('%'),
            '\\' | '"' | '$' | '`' => word.push('\\'),
            _ => {}
        }
        word.push(character);
    }
    Ok(format!("ProxyCommand={word}\" nc %h %p"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::{
        CONNECTION_OVERRIDES, SharedConnection, SshAccess, ssh_base_args, ssh_control_exit_args,
        ssh_sign_in_args,
    };

    const FIXTURE: &str = r#"{
        "TUN": true, "BackendState": "Running",
        "Self": {"HostName": "local", "DNSName": "local.tail.example.",
            "TailscaleIPs": ["100.100.12.33"]},
        "Peer": {
            "nodekey:dev": {"HostName": "dev", "DNSName": "dev.tail.example.",
                "TailscaleIPs": ["100.100.12.34", "fd7a:115c:a1e0::1"],
                "SSH_HostKeys": ["ssh-ed25519 AAAA"]},
            "nodekey:a": {"HostName": "race", "DNSName": "build.tail.example.",
                "TailscaleIPs": ["100.100.12.35"], "sshHostKeys": ["ssh-ed25519 BBBB"]},
            "nodekey:b": {"HostName": "race", "DNSName": "queue.tail.example.",
                "TailscaleIPs": ["100.100.12.36"], "sshHostKeys": []}
        }
    }"#;

    /// A short name may match a DNS prefix or a host name, while an explicit IPv6 address stays IPv6.
    #[farhelm_testtrace::test]
    fn finds_peers_by_ip_and_name() {
        let status: Status = serde_json::from_str(FIXTURE).unwrap();
        for (host, ip, key) in [
            ("DEV", "100.100.12.34", "ssh-ed25519 AAAA"),
            ("DeV.Tail.Example.", "100.100.12.34", "ssh-ed25519 AAAA"),
            ("fd7a:115c:a1e0::1", "fd7a:115c:a1e0::1", "ssh-ed25519 AAAA"),
            ("100.100.12.35", "100.100.12.35", "ssh-ed25519 BBBB"),
            ("build.tail.example", "100.100.12.35", "ssh-ed25519 BBBB"),
            ("build", "100.100.12.35", "ssh-ed25519 BBBB"),
        ] {
            let (actual, keys) = status.resolve(host).unwrap();
            assert_eq!(actual, ip.parse::<IpAddr>().unwrap());
            assert_eq!(keys, [key]);
        }
    }

    /// Choosing an ambiguous short name or a node without advertised keys must leave access unprepared.
    #[farhelm_testtrace::test]
    fn requires_one_node_with_a_key() {
        let status: Status = serde_json::from_str(FIXTURE).unwrap();
        for (host, message) in [
            ("race", "full MagicDNS name"),
            ("race", "build.tail.example"),
            ("race", "queue.tail.example"),
            ("100.100.12.33", "does not advertise SSH host keys"),
            ("queue.tail.example", "does not advertise SSH host keys"),
            ("unknown.tail.example", "No Tailscale node"),
        ] {
            assert!(
                status
                    .resolve(host)
                    .unwrap_err()
                    .to_string()
                    .contains(message)
            );
        }
        assert_eq!(
            destination_host("user@dev.tail.example").unwrap(),
            "dev.tail.example"
        );
        assert_eq!(
            destination_host("user@fd7a:115c:a1e0::1").unwrap(),
            "fd7a:115c:a1e0::1"
        );
        for destination in [
            "",
            "-dev",
            "user@@dev",
            "@dev",
            "dev:2222",
            "dev tail.example",
        ] {
            assert!(destination_host(destination).is_err(), "{destination}");
        }
    }

    /// Tailscale discovery uses only the supplied directories, with the app executable as the fallback.
    #[farhelm_testtrace::test]
    fn discovers_executable_cli() {
        let dir = tempfile::tempdir().unwrap();
        let alternate = dir.path().join("Tailscale");
        assert!(find_program(Some(dir.path().as_os_str()), Some(&alternate)).is_err());
        std::fs::write(&alternate, "").unwrap();
        std::fs::set_permissions(&alternate, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            find_program(Some(dir.path().as_os_str()), Some(&alternate)).unwrap(),
            alternate
        );
        let on_path = dir.path().join("tailscale");
        std::fs::write(&on_path, "").unwrap();
        std::fs::set_permissions(&on_path, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            find_program(Some(dir.path().as_os_str()), Some(&alternate)).unwrap(),
            on_path
        );
    }

    /// A later lookup must leave the earlier prepared command pointing at its own complete pin file.
    #[farhelm_testtrace::test]
    async fn prepares_private_key_snapshots() {
        let dir = tempfile::tempdir_in("/tmp").unwrap();
        let program = dir.path().join("tailscale % app");
        let state = dir.path().join("state");
        let install_fixture = |json: &str| {
            std::fs::write(
                &program,
                format!(
                    "#!/bin/sh\nprintf '%s' {}\n",
                    super::super::shell_quote(json)
                ),
            )
            .unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        };
        let dest = "user@dev";
        let before = "100.100.12.34 ssh-ed25519 AAAA\n";
        let after = "100.100.12.34 ssh-ed25519 CCCC\n";
        let old_path = known_hosts_path(&state, dest, before).unwrap();
        let new_path = known_hosts_path(&state, dest, after).unwrap();
        install_fixture(&FIXTURE.replace("Running", "Stopped"));
        let error = TailscaleSshAccess::prepare_with(&program, &state, dir.path(), dest)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Tailscale is not running"));
        assert!(!state.exists());

        // The wrong mode models a pre-upgrade directory occupied by another account without requiring root.
        fs::create_dir(&state).unwrap();
        let legacy = legacy_control_directory(&state, dir.path(), dest).unwrap();
        fs::create_dir(&legacy).unwrap();
        fs::set_permissions(&legacy, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!is_private_control_directory(&legacy));

        install_fixture(FIXTURE);
        let direct = TailscaleSshAccess::prepare_with(&program, &state, dir.path(), dest)
            .await
            .unwrap();
        assert_eq!(direct.proxy, "ProxyCommand=none");
        assert_ne!(direct.control_dir, legacy);
        assert!(is_private_control_directory(&direct.control_dir));
        let record = known_hosts_dir(&state, dest)
            .unwrap()
            .join(CONTROL_DIR_RECORD);
        let basename = fs::read_to_string(record).unwrap();
        assert_eq!(dir.path().join(&basename), direct.control_dir);
        assert_eq!(basename.len(), CONTROL_DIR_BASENAME_LEN);
        let socket = format!("t{}%C", direct.socket_scope);
        assert!(
            super::super::control_socket_named(&Path::new("/tmp").join(basename), &socket, 103)
                .is_some()
        );
        assert_eq!(std::fs::read_to_string(&old_path).unwrap(), before);
        assert_eq!(
            old_path.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(!state.join("known_hosts").exists());

        let rotated_fixture = FIXTURE.replace("AAAA", "CCCC");
        install_fixture(&rotated_fixture);
        let rotated = TailscaleSshAccess::prepare_with(&program, &state, dir.path(), dest)
            .await
            .unwrap();
        assert_eq!(rotated.proxy, direct.proxy);
        assert_ne!(rotated.known_hosts, direct.known_hosts);
        assert_eq!(rotated.control_dir, direct.control_dir);
        assert_ne!(rotated.socket_scope, direct.socket_scope);
        assert_eq!(std::fs::read_to_string(old_path).unwrap(), before);
        assert_eq!(std::fs::read_to_string(new_path).unwrap(), after);

        // A route change alone also requires a fresh connection. It can reuse the same trusted pin file.
        install_fixture(&rotated_fixture.replace("true", "false"));
        let proxy = TailscaleSshAccess::prepare_with(&program, &state, dir.path(), dest)
            .await
            .unwrap();
        assert!(proxy.proxy.ends_with("tailscale %% app\" nc %h %p"));
        assert_eq!(proxy.known_hosts, rotated.known_hosts);
        assert_eq!(proxy.control_dir, rotated.control_dir);
        assert_ne!(proxy.socket_scope, rotated.socket_scope);
        assert!(proxy_option(Path::new("/example/${UNSUPPORTED}/tailscale")).is_err());
        assert!(proxy_option(Path::new("/example/line\nbreak/tailscale")).is_err());
    }

    /// A restart can reuse only a valid record and private directory. Reboot, changed permissions or a symlink
    /// must cause a fresh allocation instead of making the destination unavailable or redirecting a connection.
    #[farhelm_testtrace::test]
    fn reallocates_missing_or_non_private_socket_directory() {
        let state = tempfile::tempdir().unwrap();
        let runtime = tempfile::tempdir().unwrap();
        let prepare = || control_directory(state.path(), runtime.path(), "peer").unwrap();
        let initial = prepare();
        assert_eq!(prepare(), initial);
        fs::remove_dir(&initial).unwrap();
        assert!(!initial.exists());
        let after_reboot = prepare();
        assert_ne!(after_reboot, initial);
        fs::set_permissions(&after_reboot, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!is_private_control_directory(&after_reboot));
        let after_mode_change = prepare();
        assert_ne!(after_mode_change, after_reboot);
        fs::remove_dir(&after_mode_change).unwrap();
        std::os::unix::fs::symlink(&initial, &after_mode_change).unwrap();
        fs::create_dir(&initial).unwrap();
        fs::set_permissions(&initial, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(is_private_control_directory(&initial));
        assert!(after_mode_change.is_dir());
        let after_link = prepare();
        assert_ne!(after_link, after_mode_change);
        assert_eq!(prepare(), after_link);
    }

    /// A persisted name is just one bounded basename under the runtime root. Retiring an invalid record must
    /// leave other private directories alone, and the next prepare replaces it with a usable record.
    #[farhelm_testtrace::test]
    fn rejects_malformed_control_directory_records() {
        let state = tempfile::tempdir().unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let runtime = fixture.path().join("runtime");
        let record = known_hosts_dir(state.path(), "peer")
            .unwrap()
            .join(CONTROL_DIR_RECORD);
        for invalid in [
            "../foo",
            "bad-0123456789",
            "fhs-01234567890",
            "fhs-0123/56789",
            "fhs-0123%56789",
        ] {
            fs::create_dir_all(&runtime).unwrap();
            let invalid_directory = runtime.join(invalid);
            fs::create_dir_all(&invalid_directory).unwrap();
            fs::set_permissions(&invalid_directory, fs::Permissions::from_mode(0o700)).unwrap();
            assert!(is_private_control_directory(&invalid_directory));
            write_private_file(&record, invalid).unwrap();
            assert!(
                recorded_control_directory(&record, &runtime)
                    .unwrap()
                    .is_none(),
                "{invalid}"
            );
            let shared = shared_sockets_at(state.path(), &runtime, "peer").unwrap();
            assert!(shared.paths.is_empty());
            shared.remove_empty_directories();
            assert!(invalid_directory.is_dir());
        }
        let fresh = control_directory(state.path(), &runtime, "peer").unwrap();
        assert_eq!(
            recorded_control_directory(&record, &runtime).unwrap(),
            Some(fresh)
        );
    }

    /// Teardown finds prior key and network generations without asking the
    /// daemon for status. Another destination's socket and an unpublished
    /// temporary bind name must not be returned.
    #[farhelm_testtrace::test]
    fn tailscale_teardown_finds_only_published_generations_for_the_destination() {
        let state = tempfile::tempdir().unwrap();
        let runtime = tempfile::tempdir_in("/tmp").unwrap();
        let owned = control_directory(state.path(), runtime.path(), "peer").unwrap();
        let other = control_directory(state.path(), runtime.path(), "other").unwrap();
        let legacy = legacy_control_directory(state.path(), runtime.path(), "peer").unwrap();
        fs::create_dir(&legacy).unwrap();
        fs::set_permissions(&legacy, fs::Permissions::from_mode(0o700)).unwrap();
        let digest = "0".repeat(super::super::EXPANDED_TOKEN);
        let socket = |root: &Path, scope: &str| root.join(format!("{scope}{digest}"));
        let first = socket(&owned, "taaaaaaaaaaaaaaaa");
        let second = socket(&owned, "qbbbbbbbbbbbbbbbb");
        let unrelated = socket(&other, "taaaaaaaaaaaaaaaa");
        let earlier = socket(&legacy, "teeeeeeeeeeeeeeee");
        let listeners = [&first, &second, &earlier]
            .map(|path| std::os::unix::net::UnixListener::bind(path).unwrap());
        let _unrelated = std::os::unix::net::UnixListener::bind(&unrelated).unwrap();
        let not_a_socket = socket(&owned, "tcccccccccccccccc");
        std::fs::write(&not_a_socket, b"not a mux socket").unwrap();
        let temporary = socket(&owned, "tdddddddddddddddd").with_extension("tmp");
        let temporary_listener = std::os::unix::net::UnixListener::bind(&temporary).unwrap();
        let mut observed = shared_sockets_at(state.path(), runtime.path(), "peer").unwrap();
        observed.paths.sort();
        let mut expected = [second.clone(), first.clone(), earlier.clone()];
        expected.sort();
        assert_eq!(observed.paths, expected);
        assert!(unrelated.exists());

        // A live or unrecognized entry prevents non-recursive cleanup. An invalid legacy directory stays untouched.
        observed.remove_empty_directories();
        assert!(owned.exists());
        assert!(legacy.exists());
        fs::set_permissions(&legacy, fs::Permissions::from_mode(0o755)).unwrap();
        let observed = shared_sockets_at(state.path(), runtime.path(), "peer").unwrap();
        assert_eq!(observed.paths.len(), 2);
        assert!(!observed.paths.contains(&earlier));
        drop((listeners, temporary_listener));
        for entry in [&first, &second, &earlier, &not_a_socket, &temporary] {
            fs::remove_file(entry).unwrap();
        }
        observed.remove_empty_directories();
        assert!(!owned.exists());
        assert!(legacy.exists());
        assert!(other.exists());
        fs::set_permissions(&legacy, fs::Permissions::from_mode(0o700)).unwrap();
        shared_sockets_at(state.path(), runtime.path(), "peer")
            .unwrap()
            .remove_empty_directories();
        assert!(!legacy.exists());
    }

    /// A fresh transport after a helm restart must issue exit against the exact recorded and pre-upgrade masters.
    /// Stand-in SSH reports its commands without connecting anywhere, and the live fixture sockets must remain.
    #[farhelm_testtrace::test]
    async fn fresh_transport_retires_persisted_and_legacy_tailscale_masters() {
        use crate::{
            store::HelmStore,
            transport::{HostTransport, SystemTransport},
        };

        let state = tempfile::tempdir_in("/tmp").unwrap();
        let destination = "user@peer";
        let store = HelmStore::open_fixture(&state.path().join("helm.db"))
            .await
            .unwrap();
        let host = store
            .add_ssh_host_with_mode(destination, None, None, true)
            .await
            .unwrap();
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert_eq!(row.ssh_access(), Some(crate::ssh::SshAccessKind::Tailscale));
        let recorded = tempfile::Builder::new()
            .prefix(CONTROL_DIR_PREFIX)
            .rand_bytes(CONTROL_DIR_RANDOM_BYTES)
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in("/tmp")
            .unwrap();
        let record = known_hosts_dir(state.path(), destination)
            .unwrap()
            .join(CONTROL_DIR_RECORD);
        write_private_file(
            &record,
            recorded.path().file_name().unwrap().to_str().unwrap(),
        )
        .unwrap();
        let legacy_path =
            legacy_control_directory(state.path(), Path::new("/tmp"), destination).unwrap();
        let legacy = tempfile::Builder::new()
            .prefix(legacy_path.file_name().unwrap())
            .rand_bytes(0)
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in("/tmp")
            .unwrap();
        let name = "taaaaaaaaaaaaaaaa0000000000000000000000000000000000000000";
        let (new_socket, legacy_socket) = (recorded.path().join(name), legacy.path().join(name));
        let _listeners = [&new_socket, &legacy_socket]
            .map(|path| std::os::unix::net::UnixListener::bind(path).unwrap());
        let output = state.path().join("exit-argv");
        let program = state.path().join("ssh");
        fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" END >> {}\nexit 255\n",
                super::super::shell_quote(output.to_str().unwrap())
            ),
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            control_directory(state.path(), Path::new("/tmp"), destination).unwrap(),
            recorded.path()
        );
        let transport = SystemTransport::with_ssh_program(state.path(), program);
        transport.release_shared_connections(&row).await;
        let invocations = fs::read_to_string(output).unwrap();
        let commands: Vec<_> = invocations.split_terminator("\nEND\n").collect();
        assert_eq!(commands.len(), 2, "{invocations}");
        for (command, socket) in commands.iter().zip([new_socket, legacy_socket]) {
            assert!(
                command
                    .lines()
                    .any(|arg| arg == format!("ControlPath=\"{}\"", socket.display())),
                "{command}"
            );
            assert!(command.ends_with("-O\nexit\n--\nuser@peer"), "{command}");
            assert!(socket.exists());
        }
    }

    /// Batch, sign-in and exit commands must select the same Tailscale master and keep every SSH override.
    #[farhelm_testtrace::test]
    fn all_shared_operations_keep_the_pinned_transport() {
        let access = SshAccess::Tailscale(TailscaleSshAccess {
            address: "100.100.12.34".parse().unwrap(),
            known_hosts: "UserKnownHostsFile=/tmp/pins".into(),
            proxy: "ProxyCommand=none".into(),
            control_dir: "/tmp/fhs-0123456789abcdef".into(),
            socket_scope: "0123456789abcdef".into(),
        });
        let state = Path::new("/tmp/state");
        let destination = "user@dev";
        for (connection, name) in [
            (SharedConnection::Supervisor, "t0123456789abcdef%C"),
            (SharedConnection::Provisioning, "q0123456789abcdef%C"),
        ] {
            let batch = ssh_base_args(destination, state, connection, &access).unwrap();
            let interactive = ssh_sign_in_args(destination, state, connection, &access).unwrap();
            let exit = ssh_control_exit_args(destination, state, connection, &access)
                .unwrap()
                .unwrap();
            for args in [&batch, &interactive, &exit] {
                for option in CONNECTION_OVERRIDES {
                    assert!(args.windows(2).any(|pair| pair == ["-o", option]));
                }
                for option in [
                    "ControlMaster=auto",
                    "HostName=100.100.12.34",
                    "HostKeyAlias=100.100.12.34",
                    "UserKnownHostsFile=/tmp/pins",
                    "GlobalKnownHostsFile=/dev/null",
                    "StrictHostKeyChecking=yes",
                    "ProxyCommand=none",
                ] {
                    assert!(args.windows(2).any(|pair| pair == ["-o", option]));
                }
                assert!(
                    args.contains(&format!("ControlPath=\"/tmp/fhs-0123456789abcdef/{name}\""))
                );
                assert!(args.ends_with(&["--".into(), destination.into()]));
            }
            assert!(batch.contains(&"BatchMode=yes".into()));
            assert!(interactive.contains(&"BatchMode=no".into()));
            assert!(exit.windows(2).any(|pair| pair == ["-O", "exit"]));
            assert_ne!(
                access.control_socket(state, connection),
                super::super::control_socket(state, connection)
            );
        }
    }
}
