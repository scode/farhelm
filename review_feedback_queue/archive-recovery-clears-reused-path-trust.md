# Archive recovery can clear a replacement directory's trust

Reviewed commit: 30fcdb7f81dbe5c3314158e550b7df774f910a6e

## TLDR

After an interrupted checkout archive, restarting Farhelm or retrying Delete can erase the Claude trust choice for a
different directory subsequently created at the original pathname.

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

## Delete retry reaches the same reused-path cleanup

Additional reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36.

Retrying Delete after an interrupted archive can also erase the Claude trust choice for a replacement directory at the
old pathname. This extends the startup finding to a separate cleanup path; a startup-only fix leaves it intact.

Suggested bucket: `other`, unchanged: the established consequence is loss of a trust preference, not increased
authority. Confidence: likely; the static path is confirmed, but the interrupted-archive/vendor sequence was not
reproduced. Provenance: retained `supervisor-status-and-recovery-1/f2`, second-turn synthesis. Proposed drop: none;
consolidate into `archive-recovery-clears-reused-path-trust.md`. No credential loss or loss of user work was
established. The directory admission guard prevents concurrent Farhelm allocation, not ordinary sequential replacement
before retry.

`crates/farhelm-supervisor/src/working_copies.rs:1890–1917` returns `MetadataComplete` for a matching archived
destination while permitting a foreign object at the source. Delete retry accepts that result
(`service/teardown.rs:1021–1055`) and authorizes trust cleanup for every successful archive result (:739–741). Retiring
`ArchivePending` bypasses the Allocated missing-source late-release check (`store.rs:3491–3494`), so the safeguard at
`teardown.rs:799–803` does not revoke that authorization. The deferred worker (:905–925) invokes
`remove_preparation_trust_sync` (`launch.rs:1276–1305`; `agent_kind/claude/trust.rs:250–260`), which deletes the
recorded path's trust key without checking that the old source is still absent. An earlier failed or interrupted Delete
can leave the pending row after the rename already completed; the retry need not move the replacement directory to erase
its preference.

Apply the reused-path check to Delete-retry cleanup authorization as well as startup recovery. Add a regression with a
matching pending archive destination and a trusted replacement at the old source, then retry Delete and verify that its
trust remains. This is separate from the review's startup-worker cancellation finding.

Possible cover: the existing item covers the startup manifestation; its implicated service references and required
regression target startup, so its broad recommendation alone does not establish coverage of this distinct authorization.
SPEC_impl.md:2036–2044 prohibits retrying old trust removal against a reused path, and SPEC.md:3137–3145 preserves
ordinary replacement protection. No matching Planned or separate plan was found by the source reviewer; the Codex trust
plan changes launch policy and does not cover Claude cleanup. The partial-failure session-history filter does not cover
changing a replacement directory's preference. Extend the existing TLDR and index description to name startup recovery
and Delete retry.
