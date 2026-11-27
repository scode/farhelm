# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`shift-cmd-click-copy-file`](reports/shift-cmd-click-copy-file.report.md) landed 2026-11-27 in #5121, #5124: Add Shift-Cmd-click (Linux: Shift-Ctrl-click) on terminal file links to copy UTF-8 text or macOS image contents to the clipboard
- [`shift-cmd-v-paste-upload`](reports/shift-cmd-v-paste-upload.report.md) landed 2026-11-27 in #5123: Add Shift-Cmd-V (Linux: Ctrl-Alt-V) to upload clipboard text via per-session attachments and paste the host path into the terminal
- [`codex-fork-and-vscode-resume`](reports/codex-fork-and-vscode-resume.report.md) landed 2026-11-27 in #5127, #5128: Accept Codex `fork` SessionStart and `vscode` root transcript header sources in two stacked PRs so those sessions become resumable
- [`cli-effort-reject-flag-values`](reports/cli-effort-reject-flag-values.report.md) landed 2026-11-27 in #5129: Drop `allow_hyphen_values` on CLI `--effort` and `--model` so omitted values fail at parse time
- [`agent-host-tailscale-parity`](reports/agent-host-tailscale-parity.report.md) landed 2026-11-27 in #5126: Let agents pass `--tailscale` on `host add`, surface it on the approval card and dialog, and expose `tailscale` in `agent hosts --json`
- [`remove-legacy-sandbox-sessions-api`](reports/remove-legacy-sandbox-sessions-api.report.md) landed 2026-11-27 in #5130: Remove the unused legacy `POST /api/sandbox-sessions` helm endpoint and migrate its tests to `/api/environment-sessions`
- [`restart-startup-scope-cleanup`](reports/restart-startup-scope-cleanup.report.md) landed 2026-11-27 in #5133: Clean the scope and shut down an unheld answering replacement host during Linux supervisor startup pending-scope cleanup
- [`brick-test-inline-welcome`](reports/brick-test-inline-welcome.report.md) landed 2026-11-27 in #5132: Update macOS release brick-test instructions and unit tests for the inline Welcome view
- [`claude-drift-trust-copy-back`](reports/claude-drift-trust-copy-back.report.md) landed 2026-11-27 in #5134: Add an atomic-rename regression test proving Claude drift copy-back never overwrites real ancestor trust
- [`shell-tab-first-close-sweep-backoff`](reports/shell-tab-first-close-sweep-backoff.report.md) landed 2026-11-27 in #5139: Apply the existing 1 s to 60 s retry backoff when an exited shell tab's first automatic close sweep fails
