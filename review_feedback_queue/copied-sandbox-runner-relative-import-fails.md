# Copied sandbox runner fails before applying --repo

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A maintainer using the runner's existing alternate-repository option from a copied script can no longer start it,
including its cleanup-only sweep command, unless the copy also has a checkout-shaped e2e directory beside it.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static
module-resolution and initialization-order defect; no runtime execution or observed operational incidence claimed.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:scripts/tensorlake/sandbox-tests.ts:30-34,246-258.

the new top-level import at :30 resolves ../../e2e/host-terminal-specs.ts relative to the script, while :33 explicitly
documents --repo as supporting a copy run from elsewhere and :254 applies that override only after imports. New
allProjects filtering at :129-132 combines REPO's spec names with imported HOST_TERMINAL_SPECS from the script's own
tree.

the new static import at scripts/tensorlake/sandbox-tests.ts:30 resolves ../../e2e/host-terminal-specs.ts relative to
the script before main parses/applies --repo at :246-254. The adjacent retained comment at :33 explicitly says --repo
supports a copy run from elsewhere. Baseline
a4f30ed8574d5e50f01771b90b1020b959f6acd4:scripts/tensorlake/sandbox-tests.ts:24-30 has the same alternate-repository
contract and no local module import. Baseline :77-88 and tip :81-92 confirm that running browser shards already needed
bootstrap.sh/run-shard.sh beside the script; those sidecars do not supply the newly required ../../e2e module. The sweep
dispatcher at tip :258 does not need either browser inventory or provisioning sidecars, but the new eager import still
blocks it. A runner kept in another full checkout instead mixes REPO's test filenames with the importing checkout's
HOST_TERMINAL_SPECS at :129-132; differing-inventory omission is conditional and is not needed to establish the
copied-script regression. Fix: load the target repository's inventory after --repo is applied and only when host-project
enumeration needs it; keep cleanup independent of that test inventory.

Possible coverage and caveats: none found. Coverage searched at tip: SPEC.md, SPEC_impl.md, TODO.md Planned (empty
:23-25), unlanded plans/queue, BUGS.md, review_feedback_queue/INDEX.md, TRIAGE_OUTCOMES.md for sandbox runner, alternate
repository, relocation, copied-script, relative/static import and classification causes. No trigger/consequence/scope
match found; unrelated relocation/checkouts hits were not treated as coverage. .agents/tensorlake.md:31-59 specifies
normal in-checkout invocation but does not revoke the source's --repo mode. Previously read FILTER.md has no established
rare-trigger match for using an explicit supported input; no rarity was inferred merely from it being optional tooling.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g119/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb. This
was a source-only review; runtime occurrence and frequency were not measured.
