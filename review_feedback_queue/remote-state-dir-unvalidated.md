# remote_state_dir is stored without validation

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A host registered with an empty or malformed remote state directory registers fine but never connects, with a confusing
error.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F22 / COR-REMOTE-STATE-DIR-UNCHECKED`, tagged **possible**. Anchors and title: `store.rs:3561`, `store.rs:3635`,
`store.rs:3770` — remote_state_dir is stored unvalidated, unlike remote_farhelm

An ssh host registration carries two optional install fields:

- `remote_farhelm`, the path to the Farhelm binary on the host;
- `remote_state_dir`, the remote state directory, passed as `--state-dir` to `farhelm internal stdio` on the far side.

All three registry write paths check `remote_farhelm` with `remote_farhelm_is_usable` (non-empty, no NUL, has a file
name) and refuse bad values. The three paths are `add_ssh_host` (`store.rs:3561`), `register_probed_ssh_host` (`:3635`)
and `ensure_ssh_hosts` (`:3770`). All three store `remote_state_dir` unchecked.

- An empty value becomes `--state-dir ''` on the remote command line (`ssh.rs:133-136`). The remote proxy then looks for
  the supervisor socket relative to the login directory.
- A NUL byte makes `Command::spawn` fail locally.

Either way the host registers successfully but never connects, and the resulting error points at the wrong cause. This
field decides how the host is reached (it is part of `DialedAs`), and the registry otherwise refuses values that cannot
work.

Suggested fix: add a non-empty, no-NUL check for `remote_state_dir` on all three write paths and in `ensure`'s up-front
validation, refusing with `InvalidRequest`.

User-visible consequence: a host registered with an empty or malformed remote state directory registers fine but never
connects, and the error it shows is confusing.
