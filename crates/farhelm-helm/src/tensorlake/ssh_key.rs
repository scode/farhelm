//! Farhelm's own ssh key for Tensorlake sandboxes.
//!
//! Tensorlake's ssh gateway admits a client by a public key registered with
//! the user's Tensorlake account, and only a user login can register one, not
//! an API key. Farhelm therefore keeps a key pair of its own in the helm's
//! state directory and shows the one-time `tl sbx ssh keys add` command that
//! registers it. Connections pass the private key to ssh with
//! `IdentitiesOnly=yes` and `IdentityAgent=none` for builds and hosts, so the
//! user's agent cannot promote another configured key ahead of Farhelm's.
//! During public-key authentication, the shared options exclude certificates
//! and token keys that could precede Farhelm's generated key. Other configured
//! file keys can still follow it (see `super::ssh`). This
//! is the one place Farhelm stores an ssh credential of its own; SPEC.md's
//! "Farhelm stores no SSH credentials of its own" carries the exception.
//!
//! The pair is generated only on an explicit action (Settings → Tensorlake,
//! Create SSH key, or an add-host probe), by running `ssh-keygen` with explicit
//! arguments and no shell. Reading the settings never generates it: Settings
//! mounts every section when it opens, and a key created then would leave an
//! unencrypted credential on every helm, Tensorlake user or not. It is generated into a
//! fresh temporary directory and renamed into place as a whole, so a helm
//! that dies mid-generation leaves either no key or a complete one, never a
//! private key without its public half.

use std::{
    io,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

/// The key directory's name under the helm's Tensorlake directory, and the
/// key file names inside it.
const KEY_DIR: &str = "ssh-key";
const PRIVATE_KEY: &str = "id_ed25519";
const PUBLIC_KEY: &str = "id_ed25519.pub";
/// Prefix of the temporary directories a generation works in. A leftover one
/// is a generation that died; the next generation removes it.
const TEMP_PREFIX: &str = ".ssh-key.tmp-";

/// How long `ssh-keygen` may take. Generating an ed25519 key is instant; the
/// bound only stops a wedged process from holding the generation lock.
const KEYGEN_TIMEOUT: Duration = Duration::from_secs(30);

/// Where the key pair lives under the helm's Tensorlake directory.
fn key_dir(tensorlake_dir: &Path) -> PathBuf {
    tensorlake_dir.join(KEY_DIR)
}

/// The private key's path under `tensorlake_dir`, whether or not it exists:
/// what ssh is pointed at for a Tensorlake host. Pure, so that dialing a host
/// can never generate a key; only the user's explicit request does
/// ([`SshKeyStore::ensure`]).
pub(crate) fn private_key_path(tensorlake_dir: &Path) -> PathBuf {
    key_dir(tensorlake_dir).join(PRIVATE_KEY)
}

/// A ready key pair on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SshKey {
    pub(crate) private_key_path: PathBuf,
    pub(crate) public_key_path: PathBuf,
    /// The public key line, trimmed.
    pub(crate) public_key: String,
}

/// The key's home: `<state>/tensorlake/ssh-key/`. One generation at a time
/// per helm; the helm's own state-directory lock already keeps a second helm
/// process out.
pub(crate) struct SshKeyStore {
    tensorlake_dir: PathBuf,
    generation: tokio::sync::Mutex<()>,
}

impl SshKeyStore {
    /// `tensorlake_dir` must be absolute: the registration command names the
    /// public key by path, and a relative path would mean something else in
    /// whatever directory the user runs it from.
    pub(crate) fn new(tensorlake_dir: PathBuf) -> Self {
        debug_assert!(tensorlake_dir.is_absolute());
        Self {
            tensorlake_dir,
            generation: tokio::sync::Mutex::new(()),
        }
    }

    /// The key pair, generating it first if this helm has none yet.
    ///
    /// `comment` becomes the public key's comment, so the key is
    /// recognizable in the user's Tensorlake key list. The error is a
    /// sentence for display.
    pub(crate) async fn ensure(&self, comment: &str) -> Result<SshKey, String> {
        if let Some(key) = self.read().await? {
            return Ok(key);
        }
        let _generation = self.generation.lock().await;
        // A concurrent request may have finished the generation while this
        // one waited for the lock.
        if let Some(key) = self.read().await? {
            return Ok(key);
        }
        self.generate(comment).await?;
        self.read()
            .await?
            .ok_or_else(|| "Farhelm generated its ssh key but could not find it afterwards".into())
    }

    /// The helm's Tensorlake directory, which also holds Farhelm's
    /// known_hosts file for the gateway.
    pub(crate) fn tensorlake_dir(&self) -> &Path {
        &self.tensorlake_dir
    }

    fn key_dir(&self) -> PathBuf {
        key_dir(&self.tensorlake_dir)
    }

    /// The existing pair, `None` when there is none yet. Never creates one. A key directory
    /// without a readable pair is an error, not "none": generating over it
    /// would replace a key the user may already have registered.
    pub(crate) async fn read(&self) -> Result<Option<SshKey>, String> {
        let dir = self.key_dir();
        match tokio::fs::metadata(&dir).await {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(format!("Farhelm could not read {}", dir.display())),
        }
        let private_key_path = private_key_path(&self.tensorlake_dir);
        let public_key_path = dir.join(PUBLIC_KEY);
        let unreadable = || {
            format!(
                "Farhelm's SSH key in {} is incomplete or unreadable. Remove that folder and create a new key; if the old one was registered, first remove it from your Tensorlake account with `tl sbx ssh keys rm`, since the new key gets the same name",
                dir.display()
            )
        };
        if !tokio::fs::metadata(&private_key_path)
            .await
            .is_ok_and(|meta| meta.is_file())
        {
            return Err(unreadable());
        }
        let public_key = tokio::fs::read_to_string(&public_key_path)
            .await
            .map_err(|_| unreadable())?
            .trim()
            .to_string();
        if !public_key.starts_with("ssh-ed25519 ") || public_key.contains('\n') {
            return Err(unreadable());
        }
        Ok(Some(SshKey {
            private_key_path,
            public_key_path,
            public_key,
        }))
    }

    /// Run `ssh-keygen` into a fresh temporary directory, then rename that
    /// directory into place.
    async fn generate(&self, comment: &str) -> Result<(), String> {
        let parent = &self.tensorlake_dir;
        create_private_dir(parent)
            .await
            .map_err(|_| format!("Farhelm could not create {}", parent.display()))?;
        remove_leftovers(parent).await;
        let temp = parent.join(format!("{TEMP_PREFIX}{}", uuid::Uuid::new_v4()));
        create_private_dir(&temp)
            .await
            .map_err(|_| format!("Farhelm could not create {}", temp.display()))?;
        let result = async {
            run_keygen(&temp.join(PRIVATE_KEY), comment).await?;
            // ssh-keygen already writes the private key 0600; set it anyway,
            // since ssh refuses a private key others can read and this file
            // must stay usable whatever umask the helm started with.
            tokio::fs::set_permissions(
                temp.join(PRIVATE_KEY),
                std::fs::Permissions::from_mode(0o600),
            )
            .await
            .map_err(|_| "Farhelm could not restrict its ssh key's permissions".to_string())?;
            tokio::fs::rename(&temp, self.key_dir()).await.map_err(|_| {
                format!(
                    "Farhelm could not move its new ssh key into {}",
                    parent.display()
                )
            })
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_dir_all(&temp).await;
        }
        result
    }
}

/// Create a directory only its owner can enter; an existing one is fine.
async fn create_private_dir(path: &Path) -> io::Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&path)
    })
    .await
    .map_err(io::Error::other)?
}

/// Remove temporary directories a generation that died left behind. Best
/// effort: one that cannot be removed only costs a few bytes.
async fn remove_leftovers(parent: &Path) {
    let Ok(mut entries) = tokio::fs::read_dir(parent).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(TEMP_PREFIX))
        {
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
        }
    }
}

/// `ssh-keygen -q -t ed25519 -N "" -C <comment> -f <path>`, with no input,
/// under [`KEYGEN_TIMEOUT`]. An empty passphrase is deliberate: the helm uses
/// the key unattended, and the state directory's permissions are what
/// protect it, as they protect the helm's other secrets.
async fn run_keygen(private_key: &Path, comment: &str) -> Result<(), String> {
    let mut command = tokio::process::Command::new("ssh-keygen");
    command
        .args(["-q", "-t", "ed25519", "-N", "", "-C", comment, "-f"])
        .arg(private_key)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command.spawn().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "ssh-keygen is not installed on the helm's machine; install OpenSSH's client tools to create Farhelm's ssh key".to_string()
        } else {
            "Farhelm could not run ssh-keygen".to_string()
        }
    })?;
    let output = tokio::time::timeout(KEYGEN_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "ssh-keygen did not finish in time".to_string())?
        .map_err(|_| "Farhelm could not run ssh-keygen".to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail: String = stderr
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !farhelm_proto::text::is_presentation_unsafe(*c))
        .take(300)
        .collect();
    Err(if detail.is_empty() {
        "ssh-keygen failed".to_string()
    } else {
        format!("ssh-keygen failed: {detail}")
    })
}
