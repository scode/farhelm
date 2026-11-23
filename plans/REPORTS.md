# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`codex-recipe-credentials-at-launch`](reports/codex-recipe-credentials-at-launch.report.md) landed 2026-11-23 in #4679, #4680, #4693: Supply Codex recipe API endpoint credentials at launch instead of writing ~/.codex/config.toml
- [`fix-muse-conversation-reporting`](reports/fix-muse-conversation-reporting.report.md) landed 2026-11-23 in #4696, #4708: Pass non-secret session identity args to Muse Code hooks across environment scrubbing
- [`gui-goldens-in-releases`](reports/gui-goldens-in-releases.report.md) landed 2026-11-23 in #4692: Document keeping GPUI screenshot goldens development- and landing-only and remove the TODO entry
- [`repo-cache-lock-redesign`](reports/repo-cache-lock-redesign.report.md) landed 2026-11-23 in #4697, #4709: Replace repository cache Git lock deletion with atomic quarantine-and-rebuild
- [`brick-test-remote-mac-uninstall`](reports/brick-test-remote-mac-uninstall.report.md) landed 2026-11-23 in #4700, #4702, #4703, #4710: Extend release brick test P1 with a Tart-managed remote Mac uninstall pass
- [`remove-host-signin-lock`](reports/remove-host-signin-lock.report.md) landed 2026-11-23 in #4714, #4718: Stop removing an SSH host from blocking on open sign-in prompts and probes
- [`button-restyle`](reports/button-restyle.report.md) landed 2026-11-23 in #4691, #4699, #4704, #4711, #4712, #4720: Restyle buttons and button-like controls without outlines, with the larger session toolbar
- [`eye-catching-pending-upgrade`](reports/eye-catching-pending-upgrade.report.md) landed 2026-11-23 in #4706, #4719, #4722: Highlight pending desktop updates with a tinted red pill and periodic reminder animation
- [`host-auto-update-after-2fa`](reports/host-auto-update-after-2fa.report.md) landed 2026-11-23 in #4721, #4726, #4730: Prompt for SSH sign-in and resume automatic remote-host updates
- [`custom-agents`](reports/custom-agents.report.md) landed 2026-11-23 in #4723, #4727, #4731, #4732, #4733: Add user-registered custom agents, the custom-agent contract, CLI reporter, and docs
- [`positional-hook-attribution`](reports/positional-hook-attribution.report.md) landed 2026-11-23 in #4724, #4725, #4728, #4729, #4734: Attribute Grok and Codex hook reports to the process that ran the hook, as the default rule for every agent
- [`osc8-link-disclosure-fixes`](reports/osc8-link-disclosure-fixes.report.md) landed 2026-11-23 in #4737, #4738, #4739: Fix false terminal hyperlink mismatch warnings and ignore concealed hyperlink cells
