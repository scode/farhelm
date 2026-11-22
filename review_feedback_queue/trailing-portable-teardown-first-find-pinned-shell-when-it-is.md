# Portable teardown can first find the pinned shell when it is too late for hangup

Reviewed commit: fc2b0b46e6211c7ca4001d05d90221f35ee5bd97

## TLDR

One partial process-table read can cause a running unscoped shell to be stopped forcibly without a chance to handle the
history-saving hangup.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/sweep.rs:1185`.

tip sweep.rs:1526-1531 validates the explicit root before the sweep. On Linux the bulk snapshot can omit that root after
a per-PID read error and still return a table (procs.rs:986-1016). sweep.rs:639-667 only carries a seed when present in
that table. The partial-success case at :861-867 replaces the seed set with what the table returned, unlike a hard
enumeration error, which retains it. The only SIGHUP phase is :1180-1215. When the subsequent scan can read the same
shell and its marker again, it is re-admitted at :648-652 and goes directly to quiesce and force at :1217-1261. The
original read error is preserved and the sweep refuses (:1263-1267). That correctly communicates a cleanup problem but
does not give the newly readable shell the promised opportunity in SPEC.md:2074-2075 and SPEC_impl.md:4713-4716. This
differs from d4: the portable hangup call never receives this shell's PID, and the operation reports a failure.
Revalidate independently pinned shell roots immediately before the graceful phase after a partial walk, preserving
identity checks and bounded escalation. Existing sweep.rs:3553-3587 covers hangup with readable markers only. Bucket:
other for a conditional lost grace opportunity in an already failing cleanup. Confidence: source-supported control flow
given one initial per-PID read failure and subsequent recovery. The missing assertion is a partial initial snapshot
followed by the same readable, marked shell appearing before quiescence. No runtime failure or actual history loss was
observed. Possible coverage: lifecycle_unit.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `stop/n1`.
