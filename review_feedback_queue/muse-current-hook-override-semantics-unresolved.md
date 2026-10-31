# Muse hook override merging remains unresolved

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Current Muse hook overrides may conflict with Farhelm's generated mapping; the actual vendor merge/duplicate behavior
remains unverified.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The review inspected repository injection, primary vendor documentation/SDK material and readable
installed-package/binary metadata. Those checks did not establish the matching parser's merge/replacement behavior for a
valid user override baseline. This concerns the current path; the separate legacy spelling-preservation concern has
different producer prerequisites. No vendor agent was launched and no security-hook example was developed.

Source locations:
`crates/farhelm-supervisor/src/agent_kind/muse.rs:20-25,32-61 @ d6645f771bee8ed03a66486ef8755834937d759d`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Same f3; no new verified product defect. Reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d. Actual
remaining-premise checks: command -v resolved the installed Muse launcher; read launcher lines 1-180 and searched its
version/download/exec paths. A targeted installed-artifact listing found muse-bin-1.4.14-R7309.1 (414927192 bytes). file
identifies a stripped static ELF, BuildID ee472386d6ee476d3d001a01fc66386aa0d415ef, rather than readable native parser
source. Read-only strings inspection found embedded 1.4.14-R7309.1 (near decimal offset 4772088) and the literal help
sentence 'unknown or clashing keys refuse the run'. This strengthens version attribution but does NOT show whether two
hooks keys are a clash or authenticate a working baseline. No vendor binary was executed. Primary web checks:
https://dev.meta.ai/docs/muse-code/configuration describes the hooks block and per-run -c setting; its changelog's 1.4.4
entry at lines 915-926 confirms repeatable JSON -c overrides for existing settings. Neither describes repeated
identical/ancestor hook-key reject/merge/override semantics.
https://meta-models.github.io/muse-code-sdk/next/guides/extend/hooks/ explicitly dates its hook examples to Muse 1.3.0
(lines 1120-1141), so it is not an exact 1.4.14 CLI baseline. The official SDK mirror's publish-anchor.json reports
host_version 1.4.14; root/clients inventory exposes SDK/protocol artifacts, not the native CLI config parser. A guessed
SDK spawn.ts URL failed; no readable native parser was obtained. Two broad local source-directory metadata probes
produced no completed output and are not evidence of source absence; targeted exact-path checks found only the native
launcher/binary and the ordinary Muse data directory, whose sessions/configuration were not read. Source remains
established that Farhelm admits caller hook flags and injects a second hooks override for current commands without
invoking the legacy guard; preceding f3 events retain the full source trace. TWO native premises remain: (1) a
supported, vendor-valid caller-owned CLI hook override baseline; (2) that this baseline plus the Farhelm override
rejects, instead of merging or replacing. Repeatability plus a hooks settings block does not settle either exact
premise. Needed evidence: version-matched readable native parser/validated primary documentation, or an authenticated
paired baseline/expanded-command witness; forbidden vendor execution was not used as a substitute. Stronger
possible_cover found in pinned website/src/content/docs/docs/how-it-works/agent-hook-injection.md:112-118: it explicitly
names current custom Muse -c hooks=/hooks.* flags as caller-owned incompatibilities 'yours to avoid', while :124-127
separately describes the legacy skip. This corroborates intentional command-agnostic/legacy-only rules in
SPEC_impl.md:3467-3476. It may cover the exact deliberately supplied hook-override setup; it is not automatic coverage
of every nonreporting/failed launch or ordinary agent launch. The authoritative fallback/invisibility promises in
SPEC.md:301-311,2662-2671 still make coverage interpretive, so proposed_drop remains none pending parent synthesis.
Bucket other for established duplicate configuration; high requires the supported ordinary native launch-failure route.
Confidence unverified after investigation. No native Muse safety claim or security/work-loss route. Legacy alternate
spelling/replacement concerns are a different trigger, not f3 coverage.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g014/f3`, `g086/f3`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
