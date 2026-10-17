# Archive recovery can clear a replacement directory's trust

Reviewed commit: 30fcdb7f81dbe5c3314158e550b7df774f910a6e

## TLDR

After an interrupted checkout archive, restarting Farhelm can erase the Claude trust choice for a different directory
subsequently created at the original pathname.

## Details

Suggested bucket: `other`, for the demonstrated loss of a trust preference. Confidence: likely; the static path is
confirmed, but the interrupted-archive and vendor sequence was not reproduced. This was examined through the
potential-highest exclusion gate because it changes security-related state; no increased authority, credential loss or
user-work loss was established. Provenance: a fresh Astra high exclusion auditor found this distinct path while checking
startup cleanup ownership; the parent independently inspected the implicated code. Proposed drop: none.

An archive rename can complete before interruption leaves its row `ArchivePending` and its trust-removal record intact.
While Farhelm is down, the user can create and trust a different directory at the old pathname. In
`crates/farhelm-supervisor/src/working_copies.rs:1905–1917`, a matching archived destination produces `MetadataComplete`
without requiring the original pathname to remain absent; the surrounding comment explicitly permits a foreign object
there. At the reviewed commit, `crates/farhelm-supervisor/src/service/core.rs:5739–5782` treats every successful archive
reconciliation as grounds for trust removal. Unlike the retired-record branch, it does not check whether the old path
now exists. `crates/farhelm-supervisor/src/launch.rs:1288–1292` invokes removal, and
`crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:250–260` deletes the recorded pathname's trust key.

The ownership and admission guards prevent concurrent Farhelm allocation; they do not prevent this sequential reuse
while Farhelm is down. Apply the reused-path protection to successful pending-archive reconciliation too. A focused test
should leave an already-moved pending archive, replace its old path, and verify startup preserves the replacement's
trust.

Possible cover: SPEC_impl.md:1958–1960 describes cleanup after successful reconciliation but also says old removal never
retries against a reused path. That tension is not clear acceptance. The best-effort cleanup and stale-writer decisions
have different triggers and consequences. SPEC.md's supported-environment rules preserve the foreign-replacement guard
for ordinary removal/recreation. No matching Planned, BUGS, queue, ledger or filter coverage was found. Persists at
afc828776596f2e6639d16d4203ccc8b3d0f5862, where the unconditional cleanup is `service/core.rs:5785–5828`.
