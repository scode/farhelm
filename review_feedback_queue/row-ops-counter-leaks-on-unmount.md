# The row_ops counter leaks when the list unmounts

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After re-entering the web token mid-operation, the session header's Restart, Replace and new-terminal buttons can stay
dead and nothing is auto-selected until the page is reloaded.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as `F8 / COR-ROW-OPS-COUNTER-LEAK`,
tagged **possible**. Anchors and title: `crates/farhelm-ui/src/list/view.rs:605`,
`crates/farhelm-ui/src/list/view.rs:612`, `crates/farhelm-ui/src/list/view.rs:615` — The shared row_ops counter leaks
when the list unmounts behind the browser token prompt

Sidebar row operations (stop, delete, rename, replace) do not take the shared lock described in F7, because two rows
must be able to act concurrently. Instead, each one adds its id to `ListView`'s `pending` set and increments `row_ops`,
a counter owned by `AppBody` (`begin_row_op`, line 605). The session view's `PaneGate` refuses its own claims while
`row_ops > 0`. The row's spawned task decrements the counter only when it finishes (`end_row_op`, line 615). Stop, for
example, first awaits a full listing refetch.

In the browser build, a 401 on any request raises the token prompt: `AppBody` renders `TokenPrompt` instead of the
authenticated tree. That unmounts `ListView` and drops its in-flight row tasks before they decrement. `pending` lives in
`ListView` and starts empty on remount, but `row_ops` lives in `AppBody` and keeps its stale value. After the user
re-enters the token, `PaneGate::claim` keeps refusing, so the header's Restart, Replace and "+ terminal" stay disabled
or refuse. Auto-select also never runs, because its effect returns early while `row_ops > 0`. A token rotation makes
every in-flight request fail with 401 at once, which is exactly when a row operation is likely to be mid-flight. Only a
reload clears it.

This is browser-only: the desktop build never shows the token prompt (see F4 for its separate remount problem). The
suggested fix is to decrement through a drop guard owned by each row task, or to reset `row_ops` to `pending.len()`
whenever `ListView` mounts, or to move the counter into `ListView` itself.
