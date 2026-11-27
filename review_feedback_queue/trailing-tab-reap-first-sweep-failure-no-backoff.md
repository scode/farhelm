# An exited tab whose first cleanup sweep fails is retried every tick with no backoff

Reviewed commit: 525f46a2f48d5adb0f048f10cdbb9d85fc76917f

## TLDR

When a terminal tab's shell exits and Farhelm's automatic cleanup cannot confirm the tab's processes reaped before
ending its host (for example the tab ran in its own systemd scope and the systemd user manager cannot currently be
reached, or the scope stays not-gone), the cleanup retries on every supervisor tick, about every two seconds plus each
liveness wake, for as long as the failure lasts. Each attempt briefly takes the session's lifecycle lock and a reap slot
and logs a warning. This commit added a backoff for repeatedly failing closes, but only for tabs an earlier close
already marked, so this first-close case stays a fixed-rate loop.

## Details

Provenance: trailing review through `dbb153ec`, group g04 (tab close retry; Stop on an unconfirmed start), finding f1
(revised in synthesis). Suggested bucket: `other`. Confidence: likely.

- In `close_host_tab` (`crates/farhelm-supervisor/src/service/core/host_tabs.rs:1012-1037` at the tip), a failure of the
  `BeforeEnd` sweep marks the tab (`mark_end_failed`) only when `tab.end_failed` is already true. The comment at
  1025-1029 leaves a first close unmarked "so a retry still roots at the live child", but for the reap's close of a
  finished tab there is no live child: `reap_tab_tree` with `PaneIfLive` gets `TerminalProbe::Dead`/`Gone` and passes no
  root (`core.rs:14369-14378`).
- `HostTab::reap_due` (`host_tabs.rs:201-208`) returns `finished()` on every pass; `reap_dead_host_tabs`
  (`ticker.rs:1631-1661`) starts a close each pass (`TICKER_INTERVAL` 2 s, `ticker.rs:189`, plus notifier wakes);
  `try_close_tab` (`core.rs:14209-14238`) adds no cooldown, and its warning text ("the next tick retries") shows the
  per-tick retry is the existing design for this path.
- A realistic persistent `BeforeEnd` failure: `reap_tab_processes` for a scoped tab whose unit cannot be checked or
  confirmed gone fails the reap (`ScopeKillFailure::Refuse`, `core.rs:14400-14420`).
- The commit's SPEC_impl.md text ("so a close that keeps failing holds the session's lifecycle claim and a reap slot
  only that often") and SPEC.md text ("tries again on its own: after a second or two, then less and less often, at most
  about a minute apart") are worded for the marked cases, so this is a gap in the throttling rather than a strict spec
  contradiction. The unmarked first-close branch predates the commit.

Fix: in the `BeforeEnd` failure branch also mark when the tab is finished (only-if-dead close, so there is no live root
to preserve), keeping a user close of a live tab unmarked. Test with a finished tab and a `TabCloseSweepFault` failing
`BeforeEnd` on the first reap close, asserting the second attempt comes at least about 0.9 s later under a 100 ms
ticker.

Unverified premise: that a scoped tab's sweep fails persistently in practice. The path from such a failure to the
per-tick retry is confirmed in source. Nothing is lost or wrongly killed; the cost is periodic load, lock churn and log
noise on the affected host.

Possible cover: none known. FILTER.md, "A hung systemd user manager", was considered and not applied: the persistent
failure can also come from a scope that stays not-gone, which is not a hung manager.
