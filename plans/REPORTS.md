# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`stop-closes-tabs`](reports/stop-closes-tabs.report.md) landed 2026-11-08 in #3132, #3133: Stop closes the session's terminal tabs, asking first when tabs are open (skippable), with a precondition when it did not ask; Restart keeps tabs
- [`tensorlake-credentials-polish`](reports/tensorlake-credentials-polish.report.md) landed 2026-11-08 in #3165: Settings → Credentials shows one plain-text storage warning for the whole section and labels its key field "Tensorlake API key"
- [`openssh-state-path-refusal`](reports/openssh-state-path-refusal.report.md) landed 2026-11-08 in #3168, #3170: Tensorlake: refuse helm state paths containing characters older OpenSSH cannot carry
- [`agent-settings-compare-replace`](reports/agent-settings-compare-replace.report.md) landed 2026-11-08 in #3173: Tensorlake hosts: credential delivery never overwrites an agent settings file that changed after it was read
- [`maintainer-tool-safety`](reports/maintainer-tool-safety.report.md) landed 2026-11-08 in #3169, #3172: maintainer tools: agent-tui stop deletes nothing it cannot identify; fixture scrubber catches userinfo URLs
- [`triage-misc-nov7`](reports/triage-misc-nov7.report.md) landed 2026-11-08 in #3171, #3174, #3175, #3176, #3177, #3179, #3180, #3181: host removal written durably, Custom-words edit cancels a pending launch, a spec clause, two Podman TODO lines, three discards
- [`recipe-bases`](reports/recipe-bases.report.md) landed 2026-11-08 in #3178: A Tensorlake recipe gets a base (Tensorlake Ubuntu, your image or your snapshot) that Farhelm always builds on with packages, Claude Code and Codex, checking the base's requirements first
- [`tensorlake-settings-prompts`](reports/tensorlake-settings-prompts.report.md) landed 2026-11-08 in #3186, #3187: Tensorlake settings: bind the leftover Delete question, word the recipe Delete question honestly
- [`tensorlake-name-ownership`](reports/tensorlake-name-ownership.report.md) landed 2026-11-08 in #3188, #3189, #3190: Tensorlake: adopt or delete a sandbox by name only when this helm asked for it, or spec that shared state is unsupported
