//! Routes to a machine, shared by supervisor transport and provisioning.
//!
//! A route describes execution and authentication. Callers still own stdin,
//! cancellation, output capture and their byte protocols. Container routes
//! borrow their place's supervisor master, so releasing a container connection
//! must never close that master's other sessions.

use crate::ssh::{SharedConnection, SshAccess, SshAccessKind, shell_quote};
use crate::store::{HelmStore, HostId, HostKind, HostRow};
use anyhow::Context as _;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The local supervisor uses its socket. Every other machine runs a child
/// process, including a container whose Podman itself runs locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostAccess {
    Local,
    Remote(RemoteRoute),
}

impl HostAccess {
    /// The helper's stdout is exclusively the framed supervisor protocol.
    /// Environment expiry is part of the invocation contract, so no help
    /// negotiation runs ahead of the helper.
    pub(crate) fn stdio_argv(&self, farhelm: &str, state_dir: Option<&str>) -> Vec<String> {
        let mut argv = vec![farhelm.into(), "internal".into(), "stdio".into()];
        if let Some(state_dir) = state_dir {
            argv.extend(["--state-dir".into(), state_dir.into()]);
        }
        if let Some(limit) = self.stdin_idle_limit_secs() {
            argv.extend(["--stdin-idle-limit-secs".into(), limit.to_string()]);
        }
        argv
    }

    /// Environment helpers must expire when their helm stops writing. Closing
    /// a Podman client alone can leave its in-container process alive.
    pub(crate) fn stdin_idle_limit_secs(&self) -> Option<u64> {
        match self {
            Self::Local => None,
            Self::Remote(route) => route.stdin_idle_limit_secs(),
        }
    }
}

/// Only ordinary hosts can hold containers. Keeping this separate from
/// `RemoteRoute` makes recursive container nesting unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[allow(
    dead_code,
    reason = "place routes become constructible with Podman storage"
)]
pub(crate) enum BaseRoute {
    Local,
    Ssh {
        destination: String,
        #[serde(skip_serializing_if = "SshAccessKind::is_user_config")]
        access: SshAccessKind,
    },
}

/// The login identity established when a container is created. Podman exec
/// does not perform a login, so its user, home and runtime directory must be
/// supplied together for provisioning and the supervisor proxy alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ContainerUser {
    pub(crate) name: String,
    pub(crate) home: String,
    pub(crate) uid: u32,
}

/// A remote machine reached through SSH or a rootless container execution.
/// The SSH representation retains the provisioning plan's established JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum RemoteRoute {
    Ssh {
        destination: String,
        #[serde(skip_serializing_if = "SshAccessKind::is_user_config")]
        access: SshAccessKind,
    },
    #[allow(
        dead_code,
        reason = "container routes become constructible with Podman storage"
    )]
    PodmanExec {
        via: BaseRoute,
        container: String,
        user: ContainerUser,
    },
}

/// Connection purpose affects direct SSH routes. Nested exec selects the
/// place's supervisor master, including provisioning inside the container.
/// Ordinary SSH fallback remains available when a noninteractive login works.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ConnectionPolicy {
    Shared(SharedConnection),
    OneShot,
}

/// Preserve argv boundaries until the shell which actually parses them.
/// Scripts are one argument of `sh -c`, never part of an executable path.
pub(crate) enum RemoteCommand<'a> {
    Argv(&'a [String]),
    Script(&'a str),
}

impl RemoteCommand<'_> {
    /// The argv executed on the destination machine, before route wrapping.
    fn argv(&self) -> Vec<String> {
        match self {
            Self::Argv(argv) => argv.to_vec(),
            Self::Script(script) => vec!["sh".into(), "-c".into(), (*script).into()],
        }
    }

    /// Podman has an explicit inner shell. Exec keeps its lifetime and exit
    /// status attached to the requested process without a second pipe owner.
    fn script(&self) -> String {
        match self {
            Self::Argv(argv) => format!("exec {}", quote_argv(argv)),
            Self::Script(script) => (*script).into(),
        }
    }
}

/// A process description only. No process is spawned while lowering a route.
pub(crate) struct ProcessCommand {
    pub(crate) program: PathBuf,
    pub(crate) args: Vec<String>,
}

impl ProcessCommand {
    /// Let each caller retain its own capture, process-group and stdin policy.
    pub(crate) fn command(&self) -> tokio::process::Command {
        let mut command = tokio::process::Command::new(&self.program);
        command.args(&self.args);
        command
    }
}

impl RemoteRoute {
    /// Environments establish a writer-heartbeat contract with the helper.
    /// Ordinary user SSH access keeps its existing unbounded stdin lifetime.
    fn stdin_idle_limit_secs(&self) -> Option<u64> {
        match self {
            Self::Ssh {
                access: SshAccessKind::UserConfig,
                ..
            } => None,
            Self::Ssh {
                access: SshAccessKind::Tensorlake,
                ..
            }
            | Self::PodmanExec { .. } => Some(120),
        }
    }

    /// Human-readable execution destination for plans and bounded diagnostics.
    pub(crate) fn destination(&self) -> &str {
        match self {
            Self::Ssh { destination, .. } => destination,
            Self::PodmanExec { container, .. } => container,
        }
    }

    /// Lower each parsing layer exactly once. Local Podman receives literal
    /// argv. Remote Podman adds the SSH login shell around that same argv.
    pub(crate) fn command(
        &self,
        control_dir: &Path,
        policy: ConnectionPolicy,
        payload: RemoteCommand<'_>,
    ) -> anyhow::Result<ProcessCommand> {
        match self {
            Self::Ssh {
                destination,
                access,
            } => {
                let access = SshAccess::resolve(*access, control_dir)?;
                Ok(ProcessCommand {
                    program: "ssh".into(),
                    args: ssh_command_args(
                        destination,
                        control_dir,
                        policy,
                        &access,
                        &payload.argv(),
                    )?,
                })
            }
            Self::PodmanExec {
                via,
                container,
                user,
            } => {
                let argv = vec![
                    "exec".into(),
                    "-i".into(),
                    "--user".into(),
                    user.name.clone(),
                    "--workdir".into(),
                    user.home.clone(),
                    "--env".into(),
                    format!("HOME={}", user.home),
                    "--env".into(),
                    format!("USER={}", user.name),
                    "--env".into(),
                    format!("LOGNAME={}", user.name),
                    "--env".into(),
                    format!("XDG_RUNTIME_DIR=/run/user/{}", user.uid),
                    "--".into(),
                    container.clone(),
                    "sh".into(),
                    "-c".into(),
                    payload.script(),
                ];
                match via {
                    BaseRoute::Local => Ok(ProcessCommand {
                        program: "podman".into(),
                        args: argv,
                    }),
                    BaseRoute::Ssh {
                        destination,
                        access,
                    } => {
                        let access = SshAccess::resolve(*access, control_dir)?;
                        let mut remote = vec!["podman".into()];
                        remote.extend(argv);
                        let mut args = ssh_command_args(
                            destination,
                            control_dir,
                            ConnectionPolicy::Shared(SharedConnection::Supervisor),
                            &access,
                            &remote,
                        )?;
                        // A quiet user config must not hide a refused mux
                        // session before a failed noninteractive fallback.
                        args.splice(0..0, ["-o".into(), "LogLevel=ERROR".into()]);
                        Ok(ProcessCommand {
                            program: "ssh".into(),
                            args,
                        })
                    }
                }
            }
        }
    }
}

/// The common SSH boundary for transport, provisioning and recipe scripts.
/// Access can already be resolved for a builder whose key was selected before
/// execution. Quoting applies to the remote argv only, after SSH's terminator.
pub(crate) fn ssh_command_args(
    destination: &str,
    control_dir: &Path,
    policy: ConnectionPolicy,
    access: &SshAccess,
    argv: &[String],
) -> anyhow::Result<Vec<String>> {
    let mut args = match policy {
        ConnectionPolicy::Shared(connection) => {
            crate::ssh::ssh_base_args(destination, control_dir, connection, access)?
        }
        ConnectionPolicy::OneShot => {
            let options = match access {
                SshAccess::UserConfig => Vec::new(),
                SshAccess::Tensorlake(access) => {
                    access.oneshot_options().map_err(anyhow::Error::msg)?
                }
            };
            crate::ssh::ssh_oneshot_args(destination, &options)
        }
    };
    args.push(quote_argv(argv));
    Ok(args)
}

/// A shell command formed from literal argv, with no active metacharacters.
fn quote_argv(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert current storage kinds in one place. The store-backed resolver below
/// refreshes the row first, and is where a future place-host join belongs.
pub(crate) fn host_access(row: &HostRow) -> anyhow::Result<HostAccess> {
    Ok(match row.kind {
        HostKind::Local => HostAccess::Local,
        HostKind::Ssh | HostKind::Tensorlake => HostAccess::Remote(RemoteRoute::Ssh {
            destination: row
                .destination
                .clone()
                .context("an SSH host has no destination")?,
            access: row
                .kind
                .ssh_access()
                .context("an SSH host has no access policy")?,
        }),
    })
}

/// Resolve registry access at the start of each operation. Keeping the database
/// read here lets container access join its editable place row in the same
/// snapshot once storage supplies that relationship.
pub(crate) async fn resolve_host_access(
    store: &HelmStore,
    host: HostId,
) -> anyhow::Result<HostAccess> {
    let row = store
        .list_hosts()
        .await?
        .into_iter()
        .find(|row| row.id == host)
        .with_context(|| format!("host {host} is no longer registered"))?;
    host_access(&row)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hostile container, user and home spellings distinguish literal argv
    /// from accidental interpolation at either of the two shell boundaries.
    fn container_route(via: BaseRoute) -> RemoteRoute {
        RemoteRoute::PodmanExec {
            via,
            container: "container's $name; echo wrong".into(),
            user: ContainerUser {
                name: "user's $name".into(),
                home: "/home/user's $dir".into(),
                uid: 1234,
            },
        }
    }

    /// SSH must reconstruct exactly the same Podman argv the local route
    /// executes. The inner script survives its own shell layer unchanged.
    #[farhelm_testtrace::test]
    fn local_and_nested_exec_preserve_each_argv_layer() {
        let script = "printf '%s\\n' \"$HOME\"; echo 'literal quote'";
        let local = container_route(BaseRoute::Local)
            .command(
                Path::new("/state"),
                ConnectionPolicy::Shared(SharedConnection::Provisioning),
                RemoteCommand::Script(script),
            )
            .unwrap();
        assert_eq!(local.program, Path::new("podman"));
        assert_eq!(
            local.args,
            [
                "exec",
                "-i",
                "--user",
                "user's $name",
                "--workdir",
                "/home/user's $dir",
                "--env",
                "HOME=/home/user's $dir",
                "--env",
                "USER=user's $name",
                "--env",
                "LOGNAME=user's $name",
                "--env",
                "XDG_RUNTIME_DIR=/run/user/1234",
                "--",
                "container's $name; echo wrong",
                "sh",
                "-c",
                script
            ]
        );
        let nested = container_route(BaseRoute::Ssh {
            destination: "place's $host".into(),
            access: SshAccessKind::UserConfig,
        })
        .command(
            Path::new("/state"),
            ConnectionPolicy::OneShot,
            RemoteCommand::Script(script),
        )
        .unwrap();
        let separator = nested.args.iter().position(|arg| arg == "--").unwrap();
        assert_eq!(nested.args[separator + 1], "place's $host");
        let parsed = shell_words::split(&nested.args[separator + 2..].join(" ")).unwrap();
        assert_eq!(parsed[0], "podman");
        assert_eq!(&parsed[1..], local.args);
        assert!(
            nested.args.contains(&"ControlPath=\"/state/%C\"".into()),
            "nested exec always borrows the supervisor master"
        );
        assert!(!nested.args.iter().any(|arg| arg.contains("p%C")));
        assert_eq!(&nested.args[..2], ["-o", "LogLevel=ERROR"]);
        assert!(nested.args.iter().any(|arg| arg == "ControlMaster=auto"));
    }

    /// A container proxy must expire independently of the Podman client.
    /// Ordinary SSH retains its invocation without the environment deadline.
    #[farhelm_testtrace::test]
    fn environment_stdio_is_unconditional_and_inner_exec_is_quoted() {
        let route = container_route(BaseRoute::Local);
        let access = HostAccess::Remote(route.clone());
        let argv = access.stdio_argv("/program's $bin", Some("/state's $dir"));
        assert_eq!(&argv[argv.len() - 2..], ["--stdin-idle-limit-secs", "120"]);
        let process = route
            .command(
                Path::new("/state"),
                ConnectionPolicy::OneShot,
                RemoteCommand::Argv(&argv),
            )
            .unwrap();
        let script = process.args.last().unwrap();
        let words = shell_words::split(script).unwrap();
        assert_eq!(words[0], "exec");
        assert_eq!(&words[1..], argv);
        let ordinary = HostAccess::Remote(RemoteRoute::Ssh {
            destination: "host".into(),
            access: SshAccessKind::UserConfig,
        });
        assert_eq!(
            ordinary.stdio_argv("farhelm", None),
            ["farhelm", "internal", "stdio"]
        );
    }
}
