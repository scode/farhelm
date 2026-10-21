# Case aliases evade the local instruction-file check

Reviewed commit: 6de76aaf152ae9942ed036c579ade96031eb441f

## TLDR

On a case-insensitive checkout, a differently capitalized local Claude instruction filename can pass the check while
potentially suppressing default project instructions. Actual installed Claude alias handling remains unverified.

## Details

`scripts/check-agents-md.py:77–83` preserves Git inventory spelling; `202–205` compares basenames case-sensitively with
`CLAUDE.md` and `CLAUDE.local.md`. Thus an inventoried `claude.local.md` escapes the diagnostic while ordinary
case-insensitive filesystem lookup of the canonical spelling reaches it. Both added fixtures use canonical
capitalization (`341–342`). The existing nonlocal check shares this gap; the finding concerns the added local guard.

[Claude's instruction-loading documentation](https://code.claude.com/docs/en/memory#when-claude-code-reads-agents-md)
says local instruction files suppress default AGENTS loading; it does not specify case handling. The downstream result
is an inference from filesystem equivalence, not verified vendor code or execution. Normalize protected ASCII basenames
or otherwise account for filesystem equivalence, preserving the inventory boundary, with a distinguishing fixture.

Suggested bucket: **other**, under the maintainer-tooling rule. Confidence: **likely**; checker omission established,
vendor lookup and a case-insensitive execution fixture unverified. No product credentials, data or processes were lost.
Proposed drop: none.

Possible cover: final `TRIAGE_OUTCOMES.md:17134–17148` records the broad correction complete, but does not explicitly
accept aliases. Ignored files and ancestors were reserved by the original finding; this case remains inventoried. The
checkout-name case decision at `3603–3617` concerns a different value. No exact Planned, BUGS, queue or filter match was
found. Provenance: `tooling-checks-implementation-1/f2`, discovery pass 1, trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `tooling-checks-implementation-1/f2`.
