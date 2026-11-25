# Codex config written by Farhelm 0.36.0 on Tensorlake hosts is never cleaned

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

Farhelm 0.36.0 set up Tensorlake hosts by writing the recipe's Codex endpoint into Codex's own config file on the
sandbox, including the endpoint token, and recorded nothing about it. Today's Farhelm never cleans that file on a host
from then that was not connected while 0.37 to 0.43 was installed. Codex on that sandbox can keep using the old endpoint
and token even after the user picks a ChatGPT login or no endpoint. Per the implementation spec's own text, when the
recipe now names an endpoint without a token, the old token can be sent to the new endpoint's address; that rests on how
Codex merges its settings, which is not verified against Codex itself.

## Details

Pre-existing: the behavior comes from 67d8541dc (v0.44.0) and eddd763f5 (v0.45.0), both before the reviewed range. It is
adjacent to 9e96b1771d, whose new SPEC_impl.md:6224-6226 sentence explicitly accepts only the Claude analog and leaves
this case unstated.

Locations: `crates/farhelm-helm/src/provisioning/credential_delivery.rs:1132-1138` (`LegacySettings::Codex` =>
`needs_cleanup` only for `Some(EndpointStamp::Endpoint)`) and `:1367-1381` (a NULL record becomes `Record::default()`);
`crates/farhelm-supervisor/src/agent_kind/mod.rs:1521-1543` (`codex_endpoint_args`: `-c model_provider=farhelm` and
`-c model_providers.farhelm={...}`, with `env_key` only when the recipe has a token); SPEC_impl.md:6219-6223.

v0.36.0's `agent_endpoints.rs` `codex()` (around lines 36-68 at tag v0.36.0) set `model_provider = "farhelm"` and
`[model_providers.farhelm]` `base_url` and `experimental_bearer_token = <token>` in `~/.codex/config.toml` at host setup
(`configure_agents` in `tensorlake_add.rs`). v0.36.0 kept no delivery record. v0.37.0 through v0.43.x rewrote or cleared
that file on first connection for a NULL record and stamped it. From 67d8541dc and eddd763f5 on,
`retire_legacy_settings` cleans Codex only under an Endpoint stamp ("Released Farhelm records a managed Codex settings
write only as Endpoint, so settings found under Absent are the user's own"), and a NULL record goes straight to Cleared.
That reasoning holds for Absent, but a NULL record on a host set up by v0.36.0 still has a Farhelm write behind it.

Resulting states. (1) The recipe now has no Codex endpoint, or uses a Codex login: no `-c` override is added, so
`config.toml` still selects `model_provider=farhelm` with the old `base_url` and bearer, and Codex talks to the old
endpoint instead of the selected login. (2) The recipe now has a tokenless Codex endpoint: the launch overrides
`model_providers.farhelm` without `env_key`. SPEC_impl.md:6222-6223 itself says "An old Codex settings bearer can
survive a provider override that omits a bearer", which would send the old token to the new `base_url`. The route also
needs a v0.36.0-era host that never connected under v0.37.0 through v0.43.x, so it is rare, and Tensorlake is
experimental.

Verify by reading the code points above and `git show v0.36.0:crates/farhelm-helm/src/provisioning/agent_endpoints.rs`.
Fix: treat a host row whose delivery record is NULL and which predates schema 77 (or any NULL record whose `config.toml`
holds `[model_providers.farhelm]` with name "Farhelm") as carrying a legacy Endpoint write, and run `clear_codex`. Or
record the case in SPEC_impl.md as accepted, with the maintainer's decision.

Bucket: highest, resting on SPEC_impl.md's own statement that an old Codex settings bearer can survive a provider
override that omits a bearer, so the old token can be sent to a new tokenless endpoint's address (route 2). Without that
Codex merge behavior it would be high: the selected login or endpoint silently does not take effect, and the old token
stays on the host (route 1).

Confidence: confirmed that the file stays uncleaned (source at v0.36.0, v0.37.0 and the tip); likely for exposure, with
the unverified premise that Codex merges a `-c model_providers.farhelm={...}` inline table with the file's existing
table instead of replacing it (asserted by SPEC_impl.md:6222-6223, not checked against Codex).

Possible cover: none in the spec. SPEC_impl.md:6219-6221 says "For Codex, only an Endpoint stamp records a settings
write. An old Absent stamp leaves settings alone", and the new acceptance sentence names only "An old Claude settings
write". The triage outcome "trailing-unstamped-host-skips-legacy-claude-settings-cleanup.md", named in 9e96b1771d's
message, might apply only if that triage also considered Codex (the reviewer did not read it). Related:
`trailing-legacy-claude-settings-v036-hosts-never-cleaned.md`, the Claude analog.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g24 record f2; claude-opus-5-5
reviewer, source analysis only, nothing executed.
