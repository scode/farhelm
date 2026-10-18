# Cancelled startup can lose a checkout's archive location

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

An interrupted desktop startup can leave checkout files intact in the trash while Farhelm records a different,
nonexistent archive path. Later recovery and cleanup can forget the real location, making the work undiscoverable
through Farhelm. No file-byte deletion has been established.

## Details

Desktop starts its supervisor child before starting the embedded helm (`crates/farhelm-ui/src/desktop.rs:420–478`). A
readiness failure or timeout (`:914–946`) drops the child's stdin before a `DesktopBootstrap` exists, so that object's
child wait and helm-thread join (`:651–663`) do not run. Early desktop process termination also closes stdin. This is
startup cancellation, not normal Quit after the window opens.

The CLI's stdin EOF resolves the stop future (`crates/farhelm/src/main.rs:1501–1528`). The constructor select then drops
supervisor startup and its ownership guard (`crates/farhelm-supervisor/src/service/core.rs:15293–15296`). Its archive
recovery worker (`:5785–5790`; `store.rs:3317–3336`) retains the database connection but no state-directory ownership.
Runtime shutdown waits for already-started blocking work, so a successor can acquire the released lock while that old
worker remains alive. Discovery probes the socket rather than joining the old process.

The retained harmful schedule requires an existing archive-name collision. An interrupted Delete can leave source S and
pending destination D while an unrelated archive already occupies D: journaling precedes collision resolution
(`working_copies.rs:1576–1582`). Old worker A reads this state and validates S. After startup cancellation releases
ownership, successor B recovers it, journals fresh D1 and moves S there, leaving the row pending. A resumes its cached
collision branch, journals D2 before trying the rename (`:1995–2012`), then fails because S is gone.

The process-local connection mutex is not a transaction spanning the filesystem work. The SELECT statements finish
before it, and the destination UPDATE commits independently, guarded only by row ID and `archive_pending`, not the
previous destination (`working_copies.rs:1711–1721`). A can overwrite B's D1 with D2. Later recovery sees S and D2
absent; explicit Delete retires the row (`store.rs:3491–3495`), and trash listing forgets absent D2
(`checkout_trash.rs:277–294`). Neither discovers the files at D1.

Suggested bucket: provisionally **highest**, because whether lost archive discovery crosses into loss of user-owned work
remains unresolved. The startup producer suggested `high`, with potentially-highest review; the recovery producer
suggested `other` for the established journal inconsistency, also retaining that uncertainty. Confidence: **likely**;
ownership, transaction lifetimes and the conditional schedule are supported statically. The combined collision,
cancellation and sufficiently long overlap were not reproduced. No wrong-target rename or destruction was established.

Possible cover: SPEC.md, “Healthy local filesystems,” accepts I/O errors and hangs, explicitly not ordinary cancellation
on healthy storage. Managed-path ownership and `archive-rename-move-replacement-directory.md` concern outside path
manipulation. This schedule has overlapping Farhelm recovery workers. Partial Delete permits a visible failure that
names the retained location; it does not clearly accept overwriting that location with an absent path. The
session-status/history filter concerns session records and excludes lost work. Existing
`archive-recovery-clears-reused-path-trust.md` concerns removal of a replacement folder's trust choice, not journal
destination loss. `payload-workers-outlive-state-lock.md` concerns helm payloads, not this supervisor archive journal.
No exact Planned, BUGS, queue or ledger coverage was found.

Proposed drop: none. Preserve ownership through the blocking recovery work, as startup trust cleanup does, or otherwise
prevent an old worker from overwriting a successor's journal. Verification should control the two collision branches and
inspect the retained path. The producer's earlier simpler rename-error sequence is superseded: it depended on unverified
`EEXIST` versus `ENOENT` precedence and is not evidence for this draft.

Provenance: `supervisor-startup-1/f2` and `supervisor-status-and-recovery-1/f5`, both latest `turn2.result.md`. One
underlying recovery-ownership issue; no new runtime evidence was generated for this consolidation.
