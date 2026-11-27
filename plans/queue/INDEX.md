# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 733e6b] `settings-wait-while-saving.md` — Make Settings Close/Escape and editors wait for saves, rip out slow-helm concurrency, and finish surviving Settings follow-ups (after `custom-agent-delete-prompt.md`)
- [complete] `terminal-tab-hover-close.md` — Move shell tab close button to a fixed left icon slot revealed on hover
- [pending] `agent-blocked-notifications.md` — Fix ntfy waiting alerts and add blocked-agent notifications with instruction injection (after `settings-wait-while-saving.md`)
- [held] `agent-host-tailscale-parity.md` — Let agents pass `--tailscale` on `host add`, surface it on the approval card and dialog, and expose `tailscale` in `agent hosts --json`
- [held] `shift-cmd-click-copy-file.md` — Add Shift-Cmd-click (Linux: Shift-Ctrl-click) on terminal file links to copy UTF-8 text or macOS image contents to the clipboard
- [held] `shift-cmd-v-paste-upload.md` — Add Shift-Cmd-V (Linux: Ctrl-Alt-V) to upload clipboard text via per-session attachments and paste the host path into the terminal
- [held] `codex-fork-and-vscode-resume.md` — Accept Codex `fork` SessionStart and `vscode` root transcript header sources in two stacked PRs so those sessions become resumable
