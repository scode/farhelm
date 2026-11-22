# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `podman-place-host-identity.md` — record the machine and account each new container runs on, refuse twin container hosts for one machine and account, and never treat a container as deleted on a host that now reaches somewhere else
- [pending] `tailscale-ssh-hosts.md` — add an explicit Tailscale host option on SSH hosts (Add host checkbox, Host settings toggle, `--ensure-hosts`) that verifies host keys via a private Farhelm-owned `known_hosts` file from `tailscale status --json` and uses `tailscale nc` in userspace networking
