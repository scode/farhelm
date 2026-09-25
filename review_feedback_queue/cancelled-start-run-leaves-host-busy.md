# A cancelled run start leaves the host permanently busy

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A badly timed page reload while an update is starting leaves the host showing a stuck "running" job and refusing every
further update until the helm restarts.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F8 / COR-START-RUN-CANCEL-BUSY`, tagged **possible**. Anchors and title: `provisioning/service.rs:811-827` — Cancelling
the confirm request inside start_run leaves the host permanently busy

`start_run` marks the host busy and stores a "Running" progress view (service.rs:811-820), then awaits `host_write_lock`
and a registry read (`host_row`) before it spawns the background task — and that task is the only thing that ever clears
`busy`. These awaits run inside the HTTP request handler. If the handler's future is dropped at either await, for
example because the browser disconnects or the page reloads and the server cancels the request, nothing removes the host
from `busy`: there is no drop guard, and `forget_host` (the removal path) clears progress and plans but not `busy`. Only
the explicit `host_row` error branch cleans up. The plan has already been consumed.

From then on every setup or update of that host returns 409 Busy until the helm restarts, and the panel shows a
"running" run that will never progress. The window is short unless the host lock is contended (by a cache write, a
retarget, or a removal), which is also when a user is most likely to give up and reload. Suggested change: move the
awaits into the spawned task, or hold a guard that clears `busy` and the progress entry unless the task was spawned.

User-visible consequence: a badly timed page reload while an update is starting leaves the host showing a stuck
"running" job and refusing every further update until the helm restarts.
