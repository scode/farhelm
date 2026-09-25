# The preserved-path diagnostic goes only to the log

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A Delete that deliberately leaves a folder behind reports plain success, so the user does not know the folder remains.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F14 / COR-PLAN-DIAGNOSTIC-LOG-ONLY`, tagged **possible**. Anchors and title:
`service/teardown.rs:490-505` — The "unresolved plan, path preserved" diagnostic goes only to the log

A fresh clone can be interrupted after its registry row recorded the plan but before Farhelm captured the new
directory's identity. Farhelm then cannot prove the directory at the planned path is its own. SPEC.md says an explicit
Delete may retire such a session and plan "with a diagnostic naming the preserved path", leaving the directory untouched
for manual inspection. SPEC_impl.md repeats this.

The code (teardown.rs 490-505) does leave the directory alone. The only diagnostic, though, is a `warn!` in the
supervisor log, emitted only when something exists at the path. The Delete reply is a plain success. A user who deletes
the session from the UI never learns that a folder was deliberately left behind or where it is.

Open premise: whether SPEC's "diagnostic" is meant to reach the user or only the operator's log.

Suggested change: surface the preserved path in the Delete reply or notice, for example as a warning field that the UI
shows.
