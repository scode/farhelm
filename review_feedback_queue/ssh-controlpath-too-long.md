# The ssh ControlPath overflows the socket path limit for long state dirs

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a Mac with a username of 11+ characters, or a Linux account with a long name or custom state directory, every remote
host stays unreachable with a misleading "no supervisor is running" hint even though ssh works by hand.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F19 / COR-CONTROLPATH-TOO-LONG`, tagged **definite**. Anchors and title: `transport.rs:112`, `ssh.rs:91`,
`crates/farhelm-helm/src/provisioning/backend.rs:368`, `crates/farhelm-helm/src/provisioning/backend.rs:852` — The ssh
ControlPath under the state directory overflows the Unix socket path limit in common setups, so every ssh host fails

Every ssh connection the helm makes runs with `ControlMaster=auto` and `ControlPath="<state_dir>/ssh-cm-%C"`. This
covers normal dialing (`transport.rs:112`) and provisioning's ssh commands (`provisioning/backend.rs:368`, `:852`), with
the argv built in `ssh_base_args` (`ssh.rs:91`). OpenSSH expands `%C` to 40 hex characters. When it becomes the
connection master, it first binds a temporary socket at `<ControlPath>.<16 random characters>`. The full socket path is
therefore `len(state_dir) + 65` bytes.

Unix socket paths must fit in `sun_path`: 107 usable bytes on Linux, 103 on macOS. OpenSSH treats an over-long path here
as fatal (`cleanup_exit(255)`, after authentication) rather than falling back to no multiplexing. So any state directory
longer than 42 bytes on Linux, or 38 on macOS, breaks every ssh host:

- On Linux, the default `/home/<user>/.local/state/farhelm` is 27 bytes plus the username, so it fails for usernames of
  16 or more characters.
- On macOS the same default layout under `/Users/<user>` fails for usernames of 11 or more characters.
- Any deeper `XDG_STATE_HOME` or `--state-dir` fails regardless of username.

`ssh_base_args` checks the path is UTF-8 but never checks its length.

The reviewer verified this with a throwaway `sshd` (OpenSSH 9.6p1) on loopback inside a `mktemp -d`. A 42-byte state
directory connected (exit 0). A 43-byte one failed with
`unix_listener: path "…/ssh-cm-<40hex>.<16>" too long for Unix domain socket` and exit 255. Everything was cleaned up by
recorded PID and `ssh -O exit`.

Because the channel dies before the handshake, the helm's diagnostic (`annotate_ssh_handshake_eof`) suggests the remote
supervisor is not running, which is the wrong advice. Provisioning's ssh commands fail the same way.

Suggested fix: keep the ControlPath short and independent of the state directory's depth. For example, use a 0700
directory under `$XDG_RUNTIME_DIR`, or `/tmp/fh-<uid>-<short hash>/`, containing just `%C`. Alternatively, fall back to
`ControlMaster=no` with a warning when `len + 65` exceeds the limit. Pin the maximum in a test.

User-visible consequence: on a Mac with a username of 11 or more characters, or on a Linux account with a long name or a
custom state directory, every remote host stays unreachable with a misleading "no supervisor is running" hint, even
though ssh works by hand.
