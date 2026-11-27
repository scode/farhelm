# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`agent-host-tailscale-parity`](reports/agent-host-tailscale-parity.report.md) landed 2026-11-27 in #5126: Let agents pass `--tailscale` on `host add`, surface it on the approval card and dialog, and expose `tailscale` in `agent hosts --json`
- [`agent-blocked-notifications`](reports/agent-blocked-notifications.report.md) landed 2026-11-27 in #5118, #5125, #5135: Fix ntfy waiting alerts and add blocked-agent notifications with instruction injection
- [`helm-replace-refuse-destination-change`](reports/helm-replace-refuse-destination-change.report.md) landed 2026-11-27 in #5142: Make the helm Replace API refuse destination folder changes and fresh managed checkouts
