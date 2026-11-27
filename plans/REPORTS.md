# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`shift-cmd-click-copy-file`](reports/shift-cmd-click-copy-file.report.md) landed 2026-11-27 in #5121, #5124: Add Shift-Cmd-click (Linux: Shift-Ctrl-click) on terminal file links to copy UTF-8 text or macOS image contents to the clipboard
- [`shift-cmd-v-paste-upload`](reports/shift-cmd-v-paste-upload.report.md) landed 2026-11-27 in #5123: Add Shift-Cmd-V (Linux: Ctrl-Alt-V) to upload clipboard text via per-session attachments and paste the host path into the terminal
- [`codex-fork-and-vscode-resume`](reports/codex-fork-and-vscode-resume.report.md) landed 2026-11-27 in #5127, #5128: Accept Codex `fork` SessionStart and `vscode` root transcript header sources in two stacked PRs so those sessions become resumable
- [`cli-effort-reject-flag-values`](reports/cli-effort-reject-flag-values.report.md) landed 2026-11-27 in #5129: Drop `allow_hyphen_values` on CLI `--effort` and `--model` so omitted values fail at parse time
- [`agent-host-tailscale-parity`](reports/agent-host-tailscale-parity.report.md) landed 2026-11-27 in #5126: Let agents pass `--tailscale` on `host add`, surface it on the approval card and dialog, and expose `tailscale` in `agent hosts --json`
