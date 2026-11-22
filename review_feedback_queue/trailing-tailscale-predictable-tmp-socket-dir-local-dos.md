# Predictable /tmp socket directory lets another local user permanently block Tailscale SSH

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

On a shared machine with more than one user account, another local user can pre-create the temporary folder Farhelm
expects for a Tailscale host's shared SSH connections and permanently prevent Farhelm from connecting to, signing in to,
or setting up that Tailscale host.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/ssh/tailscale.rs:158-213`.

Bucket note: reviewer suggested `other` while explicitly surfacing the cross-account Unix security boundary for
permanent local denial of service; placed in `high` for triage visibility of that boundary.

Introduced in b83df30487f3e7e8d0df7498562f6d32323f208d and refined in 74fa1951bac92f5a78757100534498c458d86806. In
`crates/farhelm-helm/src/ssh/tailscale.rs:158-213`, `control_directory_path` computes
`/tmp/fhs-<short_digest(uid, canonical_state_dir, destination)>` and `control_directory` calls
`DirBuilder::new().mode(0o700).create(&path)` followed by `require_private_control_directory`
(`metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o777 == 0o700`), returning a fatal error from
`TailscaleSshAccess::prepare_with` (lines 99-104) with no fallback if that path is already occupied. Every input to the
digest is known or observable to other local Unix users (`uid` from `/etc/passwd`, default state directory path, and
`destination` from guessable node names or `ssh` argv). Because `/tmp` has the sticky bit (`1777`), another local user
who pre-creates `/tmp/fhs-<scope>` (as a file, directory, or symlink) cannot be evicted by the victim, permanently
breaking every Tailscale SSH operation for that destination (supervisor transport, interactive sign-in, provisioning,
and Podman over Tailscale SSH) until reboot or moving `--state-dir`. Note: `require_private_control_directory` preserves
confidentiality and integrity (no foreign socket hijacking or credential theft is possible); the impact is strictly
availability (local DoS). Fix direction: on Linux, place the directory under `XDG_RUNTIME_DIR` (`/run/user/<uid>`,
already private and owned by the user) with a short prefix/digest that stays within `MAX_SOCKET_PATH`; or persist a
random per-helm secret salt inside the private `state_dir` (0600) and include it in `control_directory_path`'s digest
(still stable across helm restarts for teardown while unguessable to other local users); or fall back to an unshared
connection / random private directory when the deterministic rendezvous is occupied by another uid.

Reviewer metadata (g42-tailscale-pinned-and-ssh/l3): suggested bucket: other (crosses the Unix-account security boundary
on multi-user systems only for local denial of service / availability, not confidentiality, integrity, or loss of user
data/credentials/processes; surface to triage in case cross-account local DoS is treated as highest); confidence:
confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g42-tailscale-pinned-and-ssh/l3`.
