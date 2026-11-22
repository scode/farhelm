# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`tailscale-ssh-hosts`](reports/tailscale-ssh-hosts.report.md) landed 2026-11-22 in #4637, #4638, #4639, #4640: add an explicit Tailscale host option on SSH hosts (Add host checkbox, Host settings toggle, `--ensure-hosts`) that verifies host keys via a private Farhelm-owned `known_hosts` file from `tailscale status --json` and uses `tailscale nc` in userspace networking
