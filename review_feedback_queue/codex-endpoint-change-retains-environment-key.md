# Codex endpoint change retains environment key

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313

## TLDR

Changing Codex's API endpoint can leave its old environment-key setting active and send that credential to the newly
selected endpoint.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location:
`crates/farhelm-helm/src/provisioning/agent_endpoints.rs:54-69 at ca28b604b876770b1ab757783416f83ad1709313`.

Source assessment (read with the later corrections below) (review record g048/l1, event 21):

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313. TLDR: A Tensorlake recipe can direct Codex to a new server
while Codex still sends an API key belonging to the image's previous server. The new recipe token does not take
precedence over all preserved authentication settings. Suggested bucket: highest (credential disclosure);
highest-capable for the parent's independent disposition check. Confidence: confirmed by source for a concrete supported
input; no runtime reproduction or live credentials used. proposed_drop: false. possible_cover: unknown; parent performs
coverage matching.

Farhelm mechanism: agent_endpoints.rs:36-69 selects model_provider=farhelm and overwrites that provider's base_url,
experimental_bearer_token and requires_openai_auth=false, while retaining other keys in the same provider table. A
preexisting env_key therefore survives the endpoint change. tensorlake_add.rs:419-420 accepts an image/snapshot
unchanged, :447-449 configures its endpoints, and :619-636 reads its existing ~/.codex/config.toml and writes the merged
result. SPEC.md Topology:330-336 explicitly supports reuse images/snapshots with preinstalled agents, and :286-293
promises the recipe endpoints while preserving unrelated settings. Only preexisting Farhelm supervisor state is refused;
a user's Codex configuration is not. Farhelm's own clean package build uses the official installer
(tensorlake/build_script.rs:31-32,121-134) and does not itself generate env_key; this finding's producer is a legitimate
user-maintained reuse image, or a customized existing sandbox on setup rerun.

Concrete source-derived sequence: a supported systemd image has stock Codex and ~/.codex/config.toml containing
[model_providers.farhelm] with an old base_url and env_key=OLD_PROVIDER_API_KEY, and the user's login rc files export
that variable with the old provider's key. This is ordinary documented Codex provider configuration. Select that image
in a recipe with endpoint B and recipe token B, then create the host and launch Codex normally. Farhelm changes base_url
to B but preserves env_key. The login-shell environment is established by SPEC.md's environment contract:2707-2721 and
SPEC_impl.md:4109-4122; launch.rs:675-722 constructs the interactive login-shell command, and :1260-1271 preserves its
exported environment. Codex resolves OLD_PROVIDER_API_KEY first and sends that credential to B, rather than token B. If
that variable is unset/empty, Codex instead errors without falling back to token B. No malicious config, agent
misreading, unauthorized retained opaque id, or image carrying a supervisor DB is needed. The credential leak requires
the supported old-provider config plus its exported key; a fresh stock image without these fields is not an affected
trigger.

Official upstream evidence and immutable versions: inspected rust-v0.163.0 at 83086188ae0614bd882818ff1aa23f3194962012,
the version named in SPEC_impl.md:4501, and also latest stable rust-v0.166.0 at 6ff1e1ca3b30fc8b914720681fcf5fe62e95f856
(official release published 2026-10-29, fetched 2026-11-05). GitHub annotated tag objects resolve to these exact
commits. The 0.163.0 file initially fetched at the obsolete core path returned 404; its actual model-provider-info crate
source was subsequently retrieved successfully, so there is no remaining inaccessible-source premise. At 0.166.0,
codex-rs/model-provider-info/src/lib.rs:521-537 reads env_key and errors if missing, and
codex-rs/model-provider/src/auth.rs:284-309,379-391 chooses it before experimental_bearer_token and before
requires_openai_auth. Thus requires_openai_auth=false blocks ambient OpenAI-login fallback, but not explicit env_key.
The same precedence exists at 0.163.0 auth.rs:197-221,292-305. Provider validation permits env_key plus
experimental_bearer_token when auth/aws are absent.

Header precedence is now settled and narrows the earlier hypothesis: at 0.166.0 model-provider-info/src/lib.rs:438-464
builds static http_headers first, then env_http_headers overwrite matching names. codex-client/src/provider.rs:79-90
seeds the request with those headers. model-provider/src/bearer_auth_provider.rs:35-41 inserts the selected bearer
Authorization, and codex-api/src/auth.rs:75-98 overlays it on request headers. Consequently a valid recipe bearer token
overwrites a configured static/env Authorization header when no env_key supersedes it; old Authorization alone is not
the established leak. Other preserved credential-bearing custom headers, such as api-key, are not removed by that bearer
overlay. codex-api/src/endpoint/session.rs:93-107 and :134-148 applies this auth immediately before
transport.execute/stream, establishing actual outbound use.

Primary source permalinks:
https://github.com/openai/codex/blob/6ff1e1ca3b30fc8b914720681fcf5fe62e95f856/codex-rs/model-provider/src/auth.rs#L379 ;
https://github.com/openai/codex/blob/6ff1e1ca3b30fc8b914720681fcf5fe62e95f856/codex-rs/model-provider-info/src/lib.rs#L521
;
https://github.com/openai/codex/blob/6ff1e1ca3b30fc8b914720681fcf5fe62e95f856/codex-rs/codex-api/src/endpoint/session.rs#L134
. Official OpenAI documentation https://learn.chatgpt.com/docs/config-file/config-advanced and
https://learn.chatgpt.com/docs/config-file/config-reference (fetched 2026-11-05) documents custom providers, env_key,
http_headers and env_http_headers; immutable implementation establishes precedence. Downloaded primary-source copies are
in g048/upstream-* (0.163.0) and g048/v0166-* (0.166.0).

Suggested fix: treat authentication fields of Farhelm's selected provider as owned by the recipe when setting/changing
its endpoint: remove or explicitly refuse conflicting env_key and credential-bearing header/auth configuration, or
replace the dedicated provider with a fresh coherent definition while preserving unrelated providers/root settings.
Retaining arbitrary fields in the same selected provider is not equivalent to preserving unrelated settings. Add focused
merge checks for preexisting env_key and header auth, without mutating the test process environment. Caveats: source
inspection only, no agent launch or network endpoint request; no actual deployed custom image or exposed credential was
examined. This is a supported-input consequence, not a claim that the stock package image already contains such
credentials.

Final temporal synthesis: direct inspection at inclusive pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b,
agent_endpoints.rs:43-67, confirms the same merge still retains env_key and configured header inputs while changing
endpoint/token. No later repair was found. Latest stable 0.166.0 at immutable commit
6ff1e1ca3b30fc8b914720681fcf5fe62e95f856 establishes a currently supported upstream version independently of any
uncertainty about the 0.163.0 support window. Its exact authentication/header/send paths are cited above. No
upstream-precedence premise remains unresolved. This is a confirmed source-derived supported-input credential route,
with no assertion that an actual deployed image already has an old key. Highest-capable parent independent disposition
remains required. This event consolidates the complete current finding, superseding the earlier preliminary uncertainty
and support-window inference. Synthesis only; no discovery search.

Recording coverage: Pending settings-credentials plan removes experimental_bearer_token on some mode changes, not the
preserved env_key in the reused farhelm provider. No exact cause removal.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g048/l1`; supporting records
`g060/f5`, `g039/g039-i2`, `g056/g056-c6`, `g063/g048:l1`, `g071/c10`. The retained private log preserves earlier
wording and its corrections. This record does not claim runtime reproduction.
