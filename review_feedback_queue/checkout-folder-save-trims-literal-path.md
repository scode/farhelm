# Saving a configured folder removes significant trailing whitespace

Reviewed commit: 126bbe267d31f256678f7b9a0466860ebd5a5c50

## TLDR

Saving an existing checkout-folder setting can change which directory it names, even without editing it.

## Details

Suggested bucket: `other`. Confidence: confirmed. Source: dedicated `gpt-6-astra` high adversarial review. Proposed
drop: none.

crates/farhelm-ui/src/checkout_root.rs:79–80 unconditionally trims the loaded draft before saving. A valid
CLI-configured path such as '/work/checkouts ' becomes '/work/checkouts' when Settings Save is clicked. The server
accepts and preserves the original literal spelling (crates/farhelm-helm/src/checkout_config.rs:238–240,487–517). The
resulting configuration either points at another directory or prevents checkout creation at this historical revision.
Preserve valid path text, or explicitly refuse questionable whitespace rather than silently replacing the path. This
does not move or delete existing checkouts.

Possible cover: checkout_root.rs:10–13 deliberately describes trimming pasted whitespace, but neither reviewed
specification explicitly excludes legitimate trailing-whitespace folder names. Current SPEC.md:747 accepts clearing
all-whitespace fields; that does not cover altering a nonempty path. TRIAGE_OUTCOMES.md's
checkout-discovery-strips-valid-trailing-path-characters.md concerns a different tool and consequence.

Status at frozen endpoint: Persists at pinned main, checkout_root.rs:85–86.

Review coverage: full commit diff read: true. No changed diff lines unread. TRIAGE_OUTCOMES.md was searched and relevant
material inspected, not read linearly in its entirety. Unchanged surrounding modules, other queue findings, and
unrelated later changes were not exhaustively reviewed. No Sol reports or working-copy queue additions were opened.
Truncated relevant output was reread in smaller selections.

Runtime evidence: None. Static review only; no browser, network-ordering, or filesystem reproduction. Applied the
supplied checklist to changed tests. No checkout or scratch files created, modified, or deleted; no fetch, rebase, or
helper agents.
