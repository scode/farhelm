# The sftp upload has no deadline until the remote temporary appears

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If a host's connection stalls just as an upload starts, its setup or update spins indefinitely and even "Remove host"
hangs until the helm is restarted.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F2 / COR-SFTP-NO-DEADLINE-BEFORE-TEMP`, tagged **definite**. Anchors and title: `provisioning/backend.rs:1280-1366`,
`provisioning/service.rs:821-837`, `hosts.rs:680-686`, `ssh.rs:92-106` — The sftp upload has no deadline until the
remote temporary appears, while the run holds the host write lock and a run slot, and Remove cannot recover it

The binary upload runs `sftp` and supervises it with `capture_sftp_child`. That function has no fixed deadline; it
decides "stalled" only from growth of the remote temporary file, which it measures by polling `stat` over a separate ssh
command every two seconds. The idle deadline (60 s) is armed only after the first poll that returns a size
(backend.rs:1298, 1354-1361). Until then the timeout branch awaits `std::future::pending()` (backend.rs:1342-1347), i.e.
never fires, and a failed size poll does not arm it either. The ssh argv built in `ssh_base_args` (ssh.rs:92-106) sets
no `ConnectTimeout`, `ServerAliveInterval` or `ServerAliveCountMax`.

So if sftp stalls before the remote file is opened — a dead ssh ControlMaster socket, a half-dead TCP path, an sftp
subsystem or key exchange that hangs, a hung remote filesystem — the upload waits until the kernel gives up on the TCP
connection (on the order of 15 minutes for a reused shared connection), or forever if the far side is alive but stuck.
Every other remote command in this module is bounded (30 s per command, 15 s per probe).

The wait is expensive because of what the run holds. `start_run` (service.rs:821-837) takes the host's `host_write_lock`
— the same per-host mutex that the host's session-cache writers, `set_destination` (retarget), `set_alias` and
`remove_host` take — and one of the four fleet-wide run slots (`MAX_CONCURRENT_RUNS = 4`), and holds both for the whole
run. `remove_host` (hosts.rs:680-686) waits for that lock _before_ calling `forget_host`, which is the only thing that
aborts a running provisioning task, so "Remove host" hangs behind the very run it would cancel. Retargeting and alias
edits for that host hang the same way, and four stuck hosts stop provisioning for the whole fleet.

SPEC.md "Remote input, session defaults, and availability" expects one host's failures not to disrupt ordinary controls,
and "Healthy local filesystems" explicitly does not excuse ordinary cancellation or disconnection. The suggested fix has
three parts: arm a bounded deadline as soon as sftp is spawned (or count failed polls as no progress); add
`ConnectTimeout`/`ServerAliveInterval`/`ServerAliveCountMax` to provisioning's ssh and sftp argv; and let `remove_host`
abort an in-flight run before it waits for the host lock.

User-visible consequence: if a host's connection stalls just as an upload starts, its setup or update spins indefinitely
and even "Remove host" hangs until the helm is restarted.
