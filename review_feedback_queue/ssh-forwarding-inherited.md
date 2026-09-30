# The helm's ssh connections inherit agent and X11 forwarding

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If the user's ssh config turns on agent or X11 forwarding for a host, Farhelm's permanent connection to it keeps the
user's ssh agent (and possibly X display) reachable from that host around the clock, so a hostile or prompt-injected
agent there could use the user's ssh keys to log into other machines.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F60 / SEC-SSH-FORWARDING`, tagged **possible**. Anchor and title: `crates/farhelm-helm/src/ssh.rs:63` — the
helm's always-on ssh connections inherit agent and X11 forwarding from the user's ssh config.

The helm reaches every remote host by running the user's own `ssh` binary: one long-lived connection per host runs
`farhelm internal stdio` on the far side, and provisioning reuses the same connection. The extra options Farhelm passes
(`ssh_base_args`, `crates/farhelm-helm/src/ssh.rs:63-125`, option lists at `:73-80` and `:111-122`) are only
`BatchMode`, `ControlMaster`, `ControlPath` and `ControlPersist`. Everything else comes from the user's `~/.ssh/config`,
including forwarding settings such as `ForwardAgent yes`, `ForwardX11 yes` and `RemoteForward`, which are common on
development boxes.

With agent forwarding on, the remote sshd creates an ssh-agent socket (under `/tmp/ssh-*/agent.*`) that lives as long as
the connection, which for the helm means permanently. Supervisors run under systemd, so sessions do not inherit
`SSH_AUTH_SOCK`, but the socket is owned by the same Unix account the agents run as, so any agent process on that host
can find it and use it to authenticate as the user. With X11 forwarding, the remote side gets a `DISPLAY` that reaches
the helm machine's X server, which in trusted mode allows keystroke capture and screenshots there. Farhelm's own traffic
needs none of this.

SPEC.md ("Local authority and trust between hosts") says a remote host must not gain access to secrets on the helm's
machine or another host through Farhelm, and names running permission-skipping agents on disposable remote hosts as an
intended use. With forwarding enabled, a prompt-injected or hostile agent on one host could use the user's ssh keys to
log into other machines the keys open, possibly including the helm machine; without Farhelm, that exposure would exist
only during the user's interactive ssh sessions. (For context: SPEC.md separately accepts, as a temporary exception,
that agents can already create sessions on other Farhelm hosts through the helm; the forwarded agent socket reaches
beyond that, to any machine the keys open and to the helm machine itself.) The open premise is a spec tension the
maintainer should settle: SPEC_impl.md (around line 991) lists agent forwarding among the ssh config features that
motivated using the system ssh binary. That reads as a reason to honor the user's config generally rather than a
requirement to forward on Farhelm's own connections, but it is in tension with SPEC.md's rule.

The suggested fix is to add `-o ForwardAgent=no -o ForwardX11=no -o ClearAllForwardings=yes` before `--` in both
branches of `ssh_base_args` (options given with `-o` on the command line override the config file, and
`ClearAllForwardings` does not affect `ProxyJump`), update SPEC_impl.md to say Farhelm's connections never forward, and
extend the argv tests in `ssh.rs`.

Related: `ssh-config-remotecommand-blocks-host.md` (from a second review) adds `-o RemoteCommand=none` (and possibly
`-T`) to the same ssh argument prefix and the same argv-pinning tests; the two fixes are best landed together.
