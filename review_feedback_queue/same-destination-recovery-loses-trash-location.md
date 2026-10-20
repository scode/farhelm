# Same-destination recovery may lose a checkout's trash location

Reviewed commit: a380abee651891d775aae473714fa8e409b4601e

## TLDR

Overlapping recovery attempts that choose the same trash location could leave a checkout intact on disk but absent from
Farhelm's trash. The conditional mechanism is established; an affected supported filesystem has not been established.

## Details

Both attempts can compare the journal's destination D with D successfully. After one moves the source into D, the late
attempt can replace D with a fresh destination if its rename returns `AlreadyExists`. Its next rename fails because the
source is missing, leaving the record pointed away from the retained checkout. See
`crates/farhelm-supervisor/src/working_copies.rs:1802–1806,1818–1839` and the journal predicates at `1771–1788`. Startup
recovery runs as blocking work (`store.rs:3315–3337`); trash listing follows recorded destinations and prunes missing
ones (`checkout_trash.rs:277–305`). The changed-value guard works; this candidate concerns an unchanged value.

Suggested bucket: provisionally **highest**, because losing discovery of retained user work may cross that boundary. No
loss of file bytes is established. Confidence: **conditional mechanism confirmed; supported-filesystem/error-order
premise unverified**. No runtime reproduction was performed.

An independent audit found that Linux v6.12's ordinary VFS path checks for a missing source before an existing
destination, refuting that normal Linux schedule
([source](https://github.com/torvalds/linux/blob/v6.12/fs/namei.c#L4811-L4832)). Inspection of Apple's source did not
settle absent-source error precedence through every compound-rename path
([source](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/vfs/vfs_syscalls.c#L8935-L9018)). Supported macOS
filesystems, network coherency and other supported kernels remain unverified. Establish this premise before designing a
broader fix; a source-existence check or stronger attempt identity is only a possible direction, not a settled remedy.

Possible cover: `TRIAGE_OUTCOMES.md`, `cancelled-startup-loses-archive-location.md`, authorizes the value guard and
keeps startup ownership outside that fix. It does not explicitly accept losing archive access when the value stays
unchanged. `plans/reports/archive-recovery-reproduction.report.md:108–113` discloses the residual, but its local-disk
assertion was not authenticated. SPEC.md's lifecycle acceptance excludes similar lasting damage. No exact Planned, BUGS,
queue or filter coverage was found. Proposed drop: none; the independent auditor could not approve exclusion.

Provenance: trailing review through `738598601c142e9e184ee7f0427e8995b2d89e0f`, code reviewer and separate report
reviewer, followed by a fresh independent lifecycle audit. This records an unresolved conditional lead, not a
demonstrated supported-platform failure or permission to resume the previously rejected competing-supervisor
reproduction.
