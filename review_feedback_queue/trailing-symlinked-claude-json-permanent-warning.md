# A symlinked ~/.claude.json leaves a permanent, unexplained warning

Reviewed commit: d3eec9ff3276d427294718eee1a51e5766f35c91

## TLDR

If a recipe image ships `~/.claude.json` as a symlink (or a dangling link) and the linked file does not already say
Claude's first-run setup is done, Farhelm deliberately leaves it alone, but then marks the host amber with "Claude Code
first-run setup is pending (could not install the agent settings)" (or "... could not read the agent settings ..." for a
dangling link) on every connection and credential replacement. The warning never clears and does not say the link is the
reason, so the user cannot tell what to fix.

## Details

`seed_claude_onboarding` (`credential_delivery.rs` ~911-939) calls
`merge_settings(..., ".claude.json", regular_only=true, ...)`. `read_agent_file` (`tensorlake_backend.rs:135-190`)
follows the link, so `merge()` sees the target's bytes; if the key is missing, `install_bytes_checked` with
`regular_only` refuses with `DESTINATION_CHANGED` (local path `backend.rs` ~1302-1304, remote guard
`[ -f dest ] && [ ! -L dest ] || exit 79` ~1347-1350). `merge_settings` treats the first `DESTINATION_CHANGED` as a
concurrent edit, re-reads, retries, and maps the second refusal to "could not install the agent settings" (~1330-1340).
`apply` records the pending warning and never sets `claude_onboarding_seeded`, so every later pass repeats two full
reads and two refused installs and re-publishes the warning. A dangling link fails earlier in `read_agent_file`. The
test `conditional_settings_install_preserves_a_matching_symlink` checks only the backend refusal; no delivery-level test
covers what the host shows.

Fix: detect the link or non-regular case explicitly (a distinct backend refusal rather than `DESTINATION_CHANGED`, so it
is not retried as a race) and either settle the slot (the image owns that file, as SPEC.md already says for a recipe
that selects a different config file) or show a specific, stable reason such as "~/.claude.json is a link; Farhelm
leaves it to the image".

Suggested bucket: `other` (persistent unexplained warning on hosts from unusual images). Confidence: confirmed for the
code path; how often images ship a linked `~/.claude.json` is unknown. Possible cover: SPEC_impl.md's credential
delivery paragraph says "This seed leaves symlinked and non-regular destinations in place" and "A failure warns and
retries on reconnection", which triage may read as accepting a permanent warning. The reviewer proposed a drop on that
basis; it is recorded because the misnamed cause and the never-settling retry are not obviously what that text intends.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g15-recipe-claude-first-run` record `l2`.
