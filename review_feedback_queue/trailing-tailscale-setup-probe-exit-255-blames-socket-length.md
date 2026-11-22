# Failed Tailscale setup probe exit 255 falsely blames state-directory socket length

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When setting up, updating, or checking a Tailscale host fails with an SSH connection error on a machine with a long user
name or state folder path, Farhelm can show a false message blaming the state folder path length and claiming
interactive SSH sign-in is unavailable, even though Tailscale hosts do not put their SSH sockets in the state folder and
do support interactive sign-in.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/backend.rs:2828-2841`.

Commit b83df30487f3e7e8d0df7498562f6d32323f208d widened `SshAccessKind::asks_user`
(`crates/farhelm-helm/src/ssh.rs:219-224`) from `Self::UserConfig => true` to
`Self::UserConfig | Self::Tailscale => true`. While `SystemTransport::sign_in_unavailable`
(`crates/farhelm-helm/src/transport.rs:294-305`) explicitly gates on
`host.ssh_access() == Some(SshAccessKind::UserConfig)`, `crates/farhelm-helm/src/provisioning/backend.rs:2828-2841`
still gates on `access.asks_user()` and then calls
`crate::ssh::sign_in_unavailable(&self.control_dir, SharedConnection::Provisioning)`. For `SshAccessKind::Tailscale`,
shared control sockets live under `/tmp/fhs-<16>/q<16>%C` (100 bytes total, always within both macOS 103-byte and Linux
107-byte limits per `crates/farhelm-helm/src/ssh/tailscale.rs:136-143,158-174`), never under `self.control_dir`.
Consequently, on macOS with any 17-20 character username (which SPEC.md "Supported user environments" supports, and
which exceeds `Provisioning`'s `p%C` limit of 16 characters in the default state directory per
`crates/farhelm-helm/src/ssh.rs:617-621`) or any custom long `--state-dir`, a Tailscale probe that exits 255 before
starting the remote command (e.g., unreachable peer, refused authentication, or host-key mismatch) overwrites the probe
failure detail with "SSH could not connect or log in to the host. The SSH socket needs <N> bytes in the helm's state
directory and the limit is <M>. Farhelm cannot show interactive SSH questions for this connection." (Note: crosses
boundary with group g45-tailscale-provisioning-podman since `backend.rs` is called with `SshAccessKind::asks_user()`
changed in g42.) Fix: check `*access == crate::ssh::SshAccessKind::UserConfig` (or a dedicated helper on
`SshAccessKind`) at `crates/farhelm-helm/src/provisioning/backend.rs:2834`, matching
`crates/farhelm-helm/src/transport.rs:296`.

Reviewer metadata (g42-tailscale-pinned-and-ssh/f1): suggested bucket: other (could not be in highest; misleading
diagnostic only); confidence: confirmed; possible cover: none.

Additional source record at `74fa1951bac92f5a78757100534498c458d86806`,
`crates/farhelm-helm/src/provisioning/backend.rs:2828-2841` (g45-tailscale-provisioning-podman/f2):

In `crates/farhelm-helm/src/provisioning/backend.rs:2828-2841`, when a remote probe exits 255 before emitting
`REMOTE_PROBE_MARKER`, the code checks `access.asks_user()` and then calls
`crate::ssh::sign_in_unavailable(&self.control_dir, crate::ssh::SharedConnection::Provisioning)`. Because
`SshAccessKind::asks_user()` (`crates/farhelm-helm/src/ssh.rs:219-224`) returns `true` for both `UserConfig` and
`Tailscale`, a Tailscale probe hitting exit 255 inspects `&self.control_dir` (`<state_dir>/p%C`) instead of Tailscale's
short `/tmp/fhs-<scope>/q<scope>%C` socket path (`crates/farhelm-helm/src/ssh/tailscale.rs:136-174`). Compare
`crates/farhelm-helm/src/transport.rs:294-305`, which explicitly narrows `sign_in_unavailable` to
`host.ssh_access() == Some(SshAccessKind::UserConfig)` for this exact reason. Note also that `sign_in_unavailable`
checks for `${` in `self.control_dir`, which for Tailscale would already have been rejected earlier by
`ssh_config_path_option` on `UserKnownHostsFile` (`ssh/tailscale.rs:88-93`), so the length check is the main false
positive (e.g. a user with a long home path whose Tailscale host is offline or refuses auth gets told Farhelm cannot
show interactive SSH questions due to state-directory socket length). Fix: narrow `backend.rs:2834` from
`access.asks_user()` to `*access == crate::ssh::SshAccessKind::UserConfig` (matching `transport.rs:296`), and add a unit
test covering Tailscale probes with a long `control_dir`.

Reviewer metadata (g45-tailscale-provisioning-podman/f2): suggested bucket: other (not highest: misleading diagnostic
only, no security or data-loss impact); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g42-tailscale-pinned-and-ssh/f1`,
`g45-tailscale-provisioning-podman/f2`.
