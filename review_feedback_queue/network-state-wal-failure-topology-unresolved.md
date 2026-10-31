# Network-backed WAL failure topology remains unresolved

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The WAL conversion has a conditional network-filesystem concern, but no supported topology producing the remaining
persistent failure was established.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Pinned bundled SQLite/VFS source and supported-state-path contracts were inspected. Default Apple NFS declines WAL
rather than reaching the alleged bad converted state; a concrete supported conversion-success followed by persistent
shared-memory failure remains missing. Checkout NFS support alone does not prove state-directory support. Preserve the
investigation and exact missing topology rather than claiming database corruption or an observed failure.

Source locations: `plans/reports/sqlite-wal.report.md:133-136; crates/farhelm-supervisor/src/db.rs:102-139`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Completed bounded bundled-source investigation for d6645f771bee8ed03a66486ef8755834937d759d. Suggested bucket OTHER on
established source uncertainty; hypothetical high/bricking is not current severity. Exact cached dependency/archive
identity verified against Cargo.lock: libsqlite3-sys0.38.2, SQLite3.53.2, archive checksum
f1d20bef17f513b9b3004532233187769cd072d790971f4e4da0e346eb6401e8; sqlite3.c
checksum0a409f1633283fa31a9126b11fbfd64a1991c5d30defad07e5745d4667f5e23d matches archive member.
sqlite3.c:104561-104611/67150-67158 checks VFS xShmMap support before conversion and writes header version2
through84637-84665. Actual -shm open/lock/mmap is later via68220-68245/70096-70123 and45139-45420. Non-readonly
mapping/open failures propagate error; no generic rollback-mode recovery exists for arbitrary SHMMAP errors. Thus
conversion-success is weaker than a proven usable WAL read. Default Apple locking-style1 is
selected40253-40258/48688-48695; nfsIoMethods46135-46146 has no xShmMap and autolock46192-46208 selects it for NFS,
refuting a normal newly-converted Mac/NFS producer. Linux defaults to posixIoMethods46039-46048/48688-48695 with actual
fcntl and MAP_SHARED. The first post-reboot connection truncates/rebuilds shared memory under DMS lock45037-45101;
ordinary -shm naming45187-45194 uses pathname, not remount device/inode identifiers. Existing WAL reopen follows
version2 header76469-76487 or -wal existence62944-62971 and recreates normal WAL state, not a stale stored mmap. Farhelm
store openers call configure_durability before schema access (supervisor/store.rs:2848-2862; helm/store.rs:4501-4541);
supervisor/core.rs:1261-1289 and helm/token_control.rs:510-536 hold filesystem ownership locks. Multi-host simultaneous
state use has different shared-memory/locking premises; no such producer is substituted for the requested supported
single-host history. Deliberately moving state to another filesystem, changing permissions or reshaping sidecars is
likewise not evidence of an ordinary same-layout remount failure. SPEC.md:1244-1253,4043-4069 concerns ordinary
supported checkout identities/remounts; it does not alone establish an NFS state-dir layout. SPEC.md:3437-3452 accepts
local I/O failures but is not used to drop database damage. Actual accessible checks still do not identify a supported
concrete mount/VFS history where WAL conversion succeeds and later ordinary shm operations persistently fail. Missing
evidence: exact supported filesystem/mount/kernel behavior causing that transition, pinned OS/VFS proof or an authorized
reproduction; generic NFS warning and capability-check gap do not supply it. No such example was invented and no
experiment was run. Confidence: verified source mechanics, consequence unverified. proposed_drop now none: retain OTHER
investigated uncertainty for independent disposition; earlier exclusion proposals remain in append-only history.
possible_cover none found. No corruption/work-loss route asserted or unilaterally dropped. Newline f2 independent a026
agreement reported by parent is accepted as disposition of that narrow separate refutation.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g006/f4`, `g165/f4`, `g166/f2`. These are
original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts
remain in the private review evidence. No fix or triage decision was executed.
