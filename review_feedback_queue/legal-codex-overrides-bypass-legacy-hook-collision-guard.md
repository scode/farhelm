# legal Codex overrides bypass the legacy hook-collision guard

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Legal Codex overrides can bypass the legacy hook-conflict refusal.

## Details

F82 — **definite** — `crates/farhelm-supervisor/src/agent_kind/codex.rs:282` — legal Codex overrides bypass the legacy
hook-collision guard

The conflict guard recognizes fewer override spellings than Codex's parser. Whitespace-prefixed keys and whole-table
assignments can evade it, allowing appended Farhelm settings to replace user event hooks or enable explicitly disabled
hooks. This also admits the accompanying trust-bypass flag despite the intended legacy refusal. Parse and trim the
override key boundary, and recognize whole hooks/features table assignments that affect hooks as well as descendant
keys.

## Evidence and triage context

- crates/farhelm-supervisor/src/agent_kind/codex.rs:279–305 checks raw hooks. and features.hooks prefixes without
  trimming or recognizing whole tables.
- crates/farhelm-supervisor/src/service/core.rs:2543–2552 routes legacy launches through legacy_hook_injection; lines
  2706–2715 invoke the integration.
- crates/farhelm-supervisor/src/agent_kind/mod.rs:712–715 appends injection when the guard misses; lines 1318–1328 add
  trust bypass, features.hooks=true, and replacement event arrays.
- Codex rust-v0.149.1, codex-rs/utils/cli/src/config_override.rs:55–81 trims keys and parses TOML values.
- Codex rust-v0.149.1, codex-rs/config/src/overrides.rs:9–12,29–64 applies overrides in order and replaces the addressed
  value.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:1921–1930 and TRIAGE_OUTCOMES.md:1709–1740 accept unreviewed hooks on injected launches.
- That acceptance does not match overriding an explicit legacy hook configuration: SPEC_impl.md:1934–1939 expressly
  requires skipping injection for it.

Caveats:

- The source suggested other; highest is recommended conservatively because an explicit hook-disable setting can be
  overridden while adding trust bypass.
- Actual unwanted command execution requires additional configured hooks.
- Legacy launches only; modern explicit {farhelm_args} placement has a different contract.
- Vendor parser behavior was inspected at rust-v0.149.1; no live Codex execution was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `sr_edges:p1:F1`.

- `sr_edges:p1:F1`: confidence as filed: Definite; confirmed by inspection; suggested bucket as filed: highest.
