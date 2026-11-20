# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`stop-agent-podman-containers`](reports/stop-agent-podman-containers.report.md) landed 2026-11-19 in #4281, #4283, #4287: Stop agent- and tab-started Podman containers through Podman before the scope kill, with notices on failure
- [`tooltip-placement-audit`](reports/tooltip-placement-audit.report.md) landed 2026-11-19 in #4285, #4288, #4289, #4290, #4292: Hover help placement standard across the app: menus beside the panel, sidebar and rail stacks beside their edge, top band and dialog buttons below, dialog lists beside, tall controls at the pointer
- [`settings-search`](reports/settings-search.report.md) landed 2026-11-19 in #4293, #4294: Search field in Settings: section list narrows to matches, pane lists matches, jump to the setting
- [`ssh-askpass-2fa`](reports/ssh-askpass-2fa.report.md) landed 2026-11-20 in #4296, #4299, #4300, #4302, #4303, #4305, #4308: SSH sign-in prompts: relay what ssh asks (2FA codes, passwords, host keys) to an attention row and popover, inline in Add host; Sign-in needed after cancel or expiry; keepalives; remove SSH needs 2FA?
- [`auto-update-hosts-default`](reports/auto-update-hosts-default.report.md) landed 2026-11-20 in #4313, #4314, #4315, #4316: Update remote hosts automatically on by default for new and upgraded installs (pre-change opt-outs cleared); brick test step 5 observes the automatic update; devbuilds start with it off or get a TODO follow-up
- [`remember-active-terminal`](reports/remember-active-terminal.report.md) landed 2026-11-20 in #4328, #4331: Switching back to a session shows the terminal last chosen in it
- [`antigravity-subagent-check`](reports/antigravity-subagent-check.report.md) landed 2026-11-20 in #4326, #4327, #4332: Live-check Antigravity, then refuse subagent conversation reports so Restart follows the main conversation; docs say where to find a conversation ID
- [`tensorlake-pause-attribution`](reports/tensorlake-pause-attribution.report.md) landed 2026-11-20 in #4329, #4335: A later Tensorlake pause no longer shows Farhelm's old suspend label and time
- [`desktop-login-shell-path`](reports/desktop-login-shell-path.report.md) landed 2026-11-20 in #4338, #4339, #4340: The desktop app runs ssh and everything else with the login shell's PATH, even from the Dock; a bell notice when it cannot be read
- [`shell-sessions`](reports/shell-sessions.report.md) landed 2026-11-20 in #4333, #4334, #4336: Bash and Zsh as session types beside the agents, with no permission mode (protocol bump)
