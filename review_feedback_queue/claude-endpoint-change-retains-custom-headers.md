# Claude endpoint change retains custom headers

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313

## TLDR

Changing Claude's API endpoint can retain custom authorization headers and send their old secret to the new endpoint.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location:
`crates/farhelm-helm/src/provisioning/agent_endpoints.rs:11-32 at ca28b604b876770b1ab757783416f83ad1709313`.

Source assessment (read with the later corrections below) (review record g048/d2, event 35):

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313. NEW discovery, distinct Claude branch of authentication
preservation. TLDR: Creating or rerunning a Tensorlake host from a reused image/config can send a previous Claude
gateway's custom-header secret to the recipe's newly selected server, even when the recipe supplies its own valid token.
Suggested bucket highest, confidence confirmed from Farhelm source plus official documented Claude custom-header
behavior for supported config; highest-capable parent independent disposition. proposed_drop false; possible_cover
related g048:l1 (Codex env_key/header preservation), not exact duplicate because that finding and its proposed
provider-table fix address a different agent/config format. g053:f1 concurrent-edit loss and g071:l5 wrong-resource
adoption do not cover this route.

Concrete supported input: a systemd image with stock Claude and ~/.claude/settings.json contains env.ANTHROPIC_BASE_URL
for gateway A, its existing ANTHROPIC_AUTH_TOKEN, and ANTHROPIC_CUSTOM_HEADERS carrying a gateway-specific secret under
a distinct header, e.g. X-Gateway-Key. Such extra headers are expressly supported as gateway authentication/routing
inputs. Select that image/snapshot in a Tensorlake recipe with endpoint B and valid token B. agent_endpoints.rs:11-32
replaces only base URL and bearer-token env keys; the existing custom-header string survives. The merge writes URL/token
B but leaves the gateway-A key. On the next ordinary Claude launch, B receives that extra secret as an additional
header. This uses a deliberately ordinary differently named header, so it does not depend on Authorization/header
override order or simultaneous credential-source selection. A never-before-customized stock image is not asserted to
contain such a key. No malicious config, generated-name collision, simultaneous edit, other GUI, missing-read coherent
snapshot or accidental opaque-id use is needed.

Repository route/contracts: SPEC.md Topology:286-295 describes endpoint installation and preservation, and :330-336
supports reuse images/snapshots with optional endpoints. tensorlake_add.rs:419-420 accepts the source unchanged;
:447-449 configures its endpoint; :619-636 reads the existing user settings file, applies this merge and installs the
resulting complete JSON. Those unchanged paths were inspected in the prior passes; no other group's patch inventory was
read. The new merge's only owned fields are ANTHROPIC_BASE_URL/ANTHROPIC_AUTH_TOKEN, so existing
ANTHROPIC_CUSTOM_HEADERS is visibly retained. This also applies to a customized existing sandbox on a normal setup
rerun.

Official primary evidence, fetched 2026-11-05: https://code.claude.com/docs/en/env-vars, Set variables in settings files
(:98-111 in fetched rendering), documents that settings.env becomes process environment; its ANTHROPIC_CUSTOM_HEADERS
entry (:157) defines extra request headers and mentions support from v2.1.227.
https://code.claude.com/docs/en/llm-gateway-connect, Send additional headers (:386-405), documents those headers in
addition to the credential and explicitly places the string in settings.env. The latter's cloud-route discussion also
permits credentials in other custom headers (:476-477); that statement establishes intended use, not a cloud-provider
trigger here. These sections suffice for the differently named custom-secret header route. Private verbatim official
Markdown snapshots are claude-env-vars-2026-11-05.md and claude-gateway-2026-11-05.md. Documentation is current
unversioned vendor text, date-preserved; no claim is made that it is immutable version-specific implementation source.

Exact-version availability check: official npm metadata for @anthropic-ai/claude-code@2.1.285 (the version in this
pinned repository's screen fixtures) was fetched into claude-2.1.285-metadata.json. It identifies native platform
optional packages and bin/claude.exe rather than a readable CLI JS implementation. No native binary was
downloaded/executed or installed. Metadata shasum 2c81d81bc9c682564b1a90eb8a66073762fabe8a, official tarball URL
https://registry.npmjs.org/@anthropic-ai/claude-code/-/claude-code-2.1.285.tgz. The finding relies on explicit primary
documentation, not inferring undocumented behavior from an older SDK or executing requests.

Ruled-out subclaim: https://code.claude.com/docs/en/authentication, Authentication precedence:327-334, says a nonempty
ANTHROPIC_AUTH_TOKEN outranks ANTHROPIC_API_KEY/apiKeyHelper. Therefore an old normal API key is not claimed to override
a supplied nonempty token. Tokenless fallback was considered but is unnecessary and not claimed; no generic
unavailable-evidence concern remains. Custom headers are additional fields independent of the selected bearer
credential.

Temporal status: the same Claude merge is unchanged at inclusive pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b
(agent_endpoints.rs:24-31); prior path history inspection found no later modification. Fix direction: explicitly refuse
or remove stale credential-bearing custom headers when changing the selected endpoint, or require endpoint-specific
consent to retain them; do not silently move old gateway credentials along with unrelated settings. Preserve unrelated
noncredential settings. Focused merge tests can cover retained custom-header auth without modifying process environment.
Caveats: requires preexisting custom-header credentials and an endpoint change; no actual user's key or deployed image
was examined, no measured frequency, no runtime/agent/endpoint experiment. Parent may consolidate the two agent-specific
auth findings, but a Codex-only fix cannot settle d2.

Completion evidence: the pinned-tip Claude merge was directly re-read at
8a8c9579e04299410652f0e4681975a7198dfc7b:agent_endpoints.rs:11-32 and retains the same custom headers. Private official
Markdown snapshot SHA-256 values: env-vars 1fd17b4fc397984209edb9c96feaaa8e52dce859db3972b8076fbcaa8bdfa9ee;
gateway-connect f010cb8ab81da39cd1d71574f3973d38725d7a96ab8beeefd9e0b7bd45cb3231. possible_cover also includes the exact
SPEC_impl.md Adding Tensorlake hosts sentence at 5778-5780 limiting Claude writes to its two named env keys. That
sentence describes the merge but does not expressly accept transporting existing endpoint-specific secrets to another
server; parent must independently adjudicate any spec-accepted highest exclusion. The finding is not dropped on this
reviewer judgment. Discovery pass 1 completed after this finding; no additional verified or
unverified-after-investigation finding was added in this pass.

Recording coverage: Pending settings-credentials plan explicitly changes only three Claude env keys; it does not remove
ANTHROPIC_CUSTOM_HEADERS. No exact cause removal.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g048/d2`; supporting records
`g060/f11`, `g056/g056-c11`, `g063/g048:d2`, `g071/c12`. The retained private log preserves earlier wording and its
corrections. This record does not claim runtime reproduction.
