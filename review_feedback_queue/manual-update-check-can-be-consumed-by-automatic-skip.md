# A manual update request can be consumed by an automatic check that already decided to skip installation

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A manual update request can disappear when it joins a background check whose policy decision already skipped
installation.

## Details

SPEC.md:807–814 promises the manual path can replace an earlier unwithdrawn staged update and that a request joining an
automatic check gets that check's progress and result. In the pinned tip, settings/app.rs:110–115 and help_menu.rs:394
can submit while an unjoined background check is hidden. The updater snapshots waiting && !status.user_waiting at
updater.rs:1175–1203 and returns Success(Idle) if that holdback applies (1207–1211). It can also return the same result
after check_allowed snapshots a newly unreadable setting as off (1222–1224,1342–1345,476–482), the case added by the
reviewed commit. If the request reaches Shared::check_now after either decision and before run_check settles it,
status.current is still Checking, so the request sets user_waiting and queues no new check (806–818). run_check then
clears user_waiting and treats the automatic-only holdback as its successful answer, even marking user_update_ready if
an older update waits (997–1040). The staged-update race predates the range, while the unreadable-state case was added
by this commit. Neither changed in the later updater edit. Use a distinct automatic-only skip outcome with a
status-locked promotion/requeue for a manual request that could not affect that decision, preserving the separate
withdrawn-update and installer-busy rules.

Confidence: confirmed by inspection of the independent UI and worker decision locks. The consequence requires the
request to arrive after a skip decision but before settlement.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `upgradeBackend/l9`.
