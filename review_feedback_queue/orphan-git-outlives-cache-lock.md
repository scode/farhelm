# A surviving Git child can outlive the repository-cache lock

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

If a checkout's launch shim dies while its Git child survives, another checkout preparation or startup cache cleanup can
acquire the cache lock while that Git process still uses the cache. The overlap is established; damage to a completed
checkout has not been demonstrated.

## Details

`crates/farhelm-supervisor/src/launch.rs:1656–1715` holds the repository-cache lock in the launch shim. The descriptor
is CLOEXEC (`:1858–1881`), so Git does not inherit the lock. Child launch and waiting (`:1060–1100,1730–1741`) supply no
parent-death coupling. Killing the shim alone releases the lock while its child can keep running. Session markers allow
a later Stop or Delete to find descendants; they do not preserve cache exclusion until that cleanup.

Another preparation or startup sweep can then acquire the same lock. The cache's specified serialization and eviction
assumptions rely on the lock covering active Git use. A successfully completed `--dissociate` copies borrowed objects,
so it is not evidence that a completed checkout remains dependent on the cache. The unresolved question is what
overlapping orphaned Git, another preparation and cache mutation can do beyond causing failed preparations.

Suggested bucket: provisionally **highest**, preserving the unresolved integrity consequence. Confidence: **confirmed
lock-lifetime gap; unverified damaging consequence**. No concrete completed-checkout corruption, file deletion or
runtime reproduction was established. Do not present the possible overlap as proven data loss.

Possible cover: FILTER.md, “Work continued in a checkout whose preparation was killed partway,” and the exact ledger
item `interrupted-checkout-retains-evictable-cache-dependency.md:13392–13406` cover a visibly failed preparation, the
user continuing to work in that same incomplete checkout, and later loss of borrowed objects when the cache is cleaned.
They do not cover surviving writers overlapping other preparations or cache mutation. SPEC_impl.md:2053–2086 requires
foreground Git work under the lock through dissociation; it does not expressly accept the surviving-child gap. SPEC.md's
power-loss durability allowance does not establish coverage for continuing concurrent processes. No matching Planned,
BUGS or queue item was found. The existing payload-worker lock item concerns a different cache and owner.

Proposed drop: the producer proposed dropping the speculative completed-checkout damage claim; independent fresh-context
disposition review 3 said KEEP the unresolved overlap. That does not establish the stronger damage claim. An isolated
check should pause Git, terminate only its shim, attempt a second cache owner, and inspect the resulting operations
before choosing a repair or severity. Ownership should cover the actual cache users if serialization is required after
shim death.

Provenance: `supervisor-files-and-checkouts-1/f4`, latest `turn2.result.md` and `findings.jsonl:5`;
`drop-review-3/verdict.md` KEEP. No new runtime evidence was generated for this draft.
