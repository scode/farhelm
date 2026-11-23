# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`stop-and-suspend-timeout-feedback`](reports/stop-and-suspend-timeout-feedback.report.md) landed 2026-11-23 in #4668, #4671, #4676: Distinguish Stop and suspend's 20-second reply notice from supervisor timeouts in helm and desktop app
- [`remove-own-setting-permission`](reports/remove-own-setting-permission.report.md) landed 2026-11-23 in #4669, #4672, #4673, #4685: Remove "Agent's own setting" permission mode, treat unproven launches as YOLO, and offer guarded Replace
- [`tensorlake-leftovers-discovery-failure`](reports/tensorlake-leftovers-discovery-failure.report.md) landed 2026-11-23 in #4675: Keep previously discovered Tensorlake leftovers visible alongside cleanup rows when discovery fails
- [`codex-recipe-credentials-at-launch`](reports/codex-recipe-credentials-at-launch.report.md) landed 2026-11-23 in #4679, #4680, #4693: Supply Codex recipe API endpoint credentials at launch instead of writing ~/.codex/config.toml
- [`explain-stale-launcher-choices`](reports/explain-stale-launcher-choices.report.md) landed 2026-11-23 in #4684: Show inline stale-choice warning and keep query when a New Session launcher choice disappears
- [`approval-card-program-wording`](reports/approval-card-program-wording.report.md) landed 2026-11-23 in #4688, #4694: Name the shell or main program accurately in Stop and Delete approval cards
- [`wayland-clipboard-file-paste`](reports/wayland-clipboard-file-paste.report.md) landed 2026-11-23 in #4682: Read text/uri-list local files in Wayland clipboard paste with same-offer fallback
- [`toolkit-reduced-motion`](reports/toolkit-reduced-motion.report.md) landed 2026-11-23 in #4681, #4687, #4707: Follow GPUI's live reduced-motion setting everywhere and remove the desktop startup check
- [`fallback-unattachable-remembered-tab`](reports/fallback-unattachable-remembered-tab.report.md) landed 2026-11-23 in #4690, #4695: Fall back to the main terminal when a remembered shell tab receives an explicit initial attach refusal
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
