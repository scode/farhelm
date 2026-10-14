# Unusable host overrides direct users to a setting that cannot repair them

Reviewed commit: 126bbe267d31f256678f7b9a0466860ebd5a5c50

## TLDR

A checkout failure can tell users to change the folder in Settings even though that host ignores the Settings value.

## Details

Suggested bucket: `other`. Confidence: confirmed. Source: dedicated `gpt-6-astra` high adversarial review. Proposed
drop: none.

crates/farhelm-supervisor/src/service/core.rs:7467–7477 replaces the CLI guidance with 'change the folder in Settings'
for every root-resolution failure. When the effective root is an unusable per-host override, Settings changes only the
global root (checkout_config.rs:78–80), and the override continues to win (460–471). Following the suggested
configuration repair therefore leaves checkout creation broken. Distinguish host overrides in the guidance or retain the
applicable CLI remedy.

Possible cover: SPEC.md:678–684 intentionally keeps per-host overrides CLI-only, and settings.rs:141 explains
inheritance. Those rules establish why the suggested repair cannot work; they do not accept misleading failure guidance.

Status at frozen endpoint: Persists at pinned main, service/core.rs:7519–7525. Later first-use creation handles missing
directories but does not repair unusable overrides such as dangling symlinks or inaccessible ancestors.

Review coverage: full commit diff read: true. No changed diff lines unread. TRIAGE_OUTCOMES.md was searched and relevant
material inspected, not read linearly in its entirety. Unchanged surrounding modules, other queue findings, and
unrelated later changes were not exhaustively reviewed. No Sol reports or working-copy queue additions were opened.
Truncated relevant output was reread in smaller selections.

Runtime evidence: None. Static review only; no browser, network-ordering, or filesystem reproduction. Applied the
supplied checklist to changed tests. No checkout or scratch files created, modified, or deleted; no fetch, rebase, or
helper agents.
