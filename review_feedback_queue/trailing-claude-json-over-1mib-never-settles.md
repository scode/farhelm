# Claude first-run seed never settles when ~/.claude.json is over 1 MiB

Reviewed commit: d3eec9ff3276d427294718eee1a51e5766f35c91

## TLDR

After upgrading, every existing recipe host re-checks Claude's `~/.claude.json` once. If that file is larger than 1 MiB
(Claude keeps per-project state there and it grows with use), Farhelm cannot read it, so the host shows an amber
warning, "Claude Code first-run setup is pending (could not read the agent settings (the file must be readable and at
most 1 MiB))", on every connection and credential replacement, forever, even though the user finished Claude's first-run
setup long ago. Nothing the user can do in Farhelm clears it.

## Details

`credential_delivery.rs` `apply` (~832-856) runs `seed_claude_onboarding` whenever `Record.claude_onboarding_seeded` is
false; the field is new with `#[serde(default)]` (~150), so every existing recipe host starts unseeded.
`seed_claude_onboarding` (~911-939) calls `merge_settings(target, &home, ".claude.json", true, ...)`, which reads
through `backend.read_small_file` → `SystemBackend::read_agent_file` (`tensorlake_backend.rs:135-190`). That reader
refuses any file over 1 MiB before `merge()` runs, so even a file that already has `hasCompletedOnboarding` cannot
settle the slot. The error becomes the "pending" warning that `DeliveryWarnings::message()` prepends to the host warning
on every pass; SPEC.md says a delivery failure turns the host status and navigation mark amber. The 1 MiB cap was sized
for `~/.claude/settings.json` and `~/.codex/config.toml`; the Claude trust edit that touches this same file reads up to
16 MiB (SPEC_impl.md, "The edit reads at most 16 MiB"), so the project already treats it as possibly much larger.

Fix: read `~/.claude.json` with the trust edit's 16 MiB bound for this slot, and/or treat an existing file that is too
large to read as settling the slot (the seed matters only for fresh hosts whose file is absent or tiny). A delivery test
with a backend returning the oversize failure would show the slot staying unsettled.

Suggested bucket: `other` (a persistent spurious amber warning; no data loss). It would move to `high` if large
`~/.claude.json` files are common among current Claude Code users. Confidence: likely. The code path is confirmed in
source; unverified after investigation how often real hosts have a file over 1 MiB.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g15-recipe-claude-first-run` record `f1`.
