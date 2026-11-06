# First state directory ancestor durability unresolved

Reviewed commit: 771864ac3f38334a8191a424d855669efddb089a

## TLDR

A first-created state directory does not flush every required ancestor explicitly. Whether supported filesystem behavior
can lose the admitted state remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location:
`crates/farhelm-supervisor/src/lib.rs:117-124;service/core.rs:6323-6433 at 771864ac3f38334a8191a424d855669efddb089a`.

Source assessment (read with the later corrections below) (review record g076/d4, event 25):

Reviewed commit 771864ac3f38334a8191a424d855669efddb089a; relevant creation/barrier path also checked at pinned
8a8c9579e04299410652f0e4681975a7198dfc7b (service/core.rs creation 6326, state-dir sync 6358-6362, identity mint
6433-6434; lib.rs:117-124 unchanged). TLDR: On a first run that creates a new state directory or ancestors, the
protection of a published supervisor identity may depend on filesystem behavior beyond the explicit sync calls. A crash
could potentially lose the newly named directory and its database even though the identity commit itself used FULL; that
consequence is unverified after investigation, not a demonstrated loss. Checks/results: ensure_private_dir in
lib.rs:117-124 recursively creates with mode 0700 and chmods, with no containing-parent sync. Production new_claiming
calls it at core.rs:6323, canonicalizes, creates launch/ at 6343, then fsyncs state_dir at 6355-6359. That protects
launch/ and contents, and must not be described as no startup directory barrier. StateDirOwnership::claim at 1474-1505
only canonicalizes, creates/locks supervisor.lock and retains the lock; it adds no ancestor sync. SessionStore opens at
6412-6413 and ensure_host_identity at 6430-6433; store.rs:5672-5712 commits the identity through with_full_sync before
returning. Exact bundled SQLite 0.38.2 source (Cargo.lock libsqlite3-sys pin; Cargo.toml bundled feature) supplies
containing-directory sync for created WAL/journal files, as d1 records; that is not an explicit recursive ancestor
barrier. The primary Linux fsync(2) contract, read both from the installed manual and
https://man7.org/linux/man-pages/man2/fsync.2.html (Linux man-pages 6.19, Description), distinguishes syncing an inode
from ensuring its name in the containing directory is persisted. Thus code has a source-backed first-run namespace
question beyond the protected SQLite transaction. Missing premise: an actual supported filesystem/kernel crash sequence
where the real state-dir fsync plus SQLite commit fails to persist an ancestor directory entry; journal transaction
dependencies may make the sequence safe on a particular filesystem. Neither the generic fsync interface contract nor the
cached SQLite source settles those per-filesystem dependencies, and no crash experiment is authorized. What would settle
it: a precise official filesystem guarantee/source proof covering newly created ancestor chains for the supported
substrate, or separately authorized crash evidence reproducing the production startup sequence. Suggested bucket other
under the rule requiring an established route for highest consequences: the confirmed fact is an omitted explicit
ancestor barrier, while actual directory/identity loss is not established. A shown healthy-supported-filesystem path to
lost protected identity, durable state or installation refusal would require reassessment toward high/highest according
to its concrete consequence. Confidence: unverified after investigation. proposed_drop false. possible_cover unknown; no
queue/plans/filter searched. Governing contract: introducing SPEC_impl.md Runtime state:4091-4107 promises protected
identity publication; pinned 4127-4145 retains it subject only to the specific Mac drive-cache exception. This is not
f1's Mac drive-cache gap, f2's removal rollback, or c1's mutex race. Caveats: pre-existing startup code adjacent to the
assigned durability helper; no assertion that all supported filesystems lose this entry or that a crash was observed; no
claim that ordinary recent-history loss alone violates the specification. No runtime tests, installations, filesystem
modifications, or network experiment; only read-only primary documentation retrieval.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g076/d4`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
