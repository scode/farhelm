# sftp misparses IPv6 and ssh:// destinations

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Setting up or updating a host registered by IPv6 address or `ssh://…:port` URI always fails at the upload, or sends the
Farhelm binary to a different machine.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F29 / SEC-SFTP-DESTINATION-PARSE`, tagged **definite**. Anchors and title: `provisioning/backend.rs:848`,
`provisioning/backend.rs:851`, `ssh.rs:62`, `store.rs:1321` — The payload upload uses sftp, which parses IPv6-literal
and ssh:// destinations as a different host than ssh does

Every provisioning step except the upload runs `ssh [options] -- <destination> <command>` (argv from `ssh_base_args`,
ssh.rs:62). The upload instead runs `sftp -b - [options] -- <destination>` (backend.rs:848-851). sftp does not treat its
destination the way ssh does: it parses `[user@]host[:path]`, so the first colon ends the host name. The registry's
`destination_is_usable` (store.rs:1321) allows colons. Reviewers checked offline, using `-F /dev/null` with a
ProxyCommand that only echoes, and by recording the ssh argv sftp generates:

- `fe80::1` → sftp dials host `fe80`
- `alice@2001:db8::5` → host `2001` (which resolves as the IPv4 address `0.0.7.209`)
- `ssh://alice@build.example:2222` → host `ssh`; the user and port are lost
- `::1` and ordinary host names agree between ssh and sftp

The truncated name is then resolved through DNS search domains, `/etc/hosts`, and the user's ssh config — including any
`Host *` block, which may forward the ssh agent. So the one step that writes the farhelm binary can go to a machine that
was never registered, while the real host never receives the temporary and the run fails, or hangs (F2). SPEC.md
requires that an operation not accidentally affect the wrong object. ssh's host-key checking limits the damage when the
unintended host is unknown to the user's `known_hosts`.

Suggested change: stream the payload over the same ssh argv every other step uses (for example `sh -c 'cat > tmp'` with
the payload on stdin, followed by the existing digest check); or translate the destination into sftp's grammar
(bracketed IPv6); or refuse these destination forms at plan time.

User-visible consequence: setting up or updating a host registered by IPv6 address or `ssh://…:port` URI always fails at
the upload, or sends the Farhelm binary to a different machine.
