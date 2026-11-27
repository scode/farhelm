# A tab kept after a failed last sweep can stay listed as live after its host was signalled

Reviewed commit: 525f46a2f48d5adb0f048f10cdbb9d85fc76917f

## TLDR

When the user closes a terminal tab, its host does not answer the orderly shutdown in time and is ended by signal
instead, and the cleanup sweep after that fails, the tab now stays in the session's tab strip looking like a live tab
even though its terminal is gone. It stays there until a later automatic retry succeeds, which can be up to a minute
between tries for as long as the sweep keeps failing, and Stop's "tabs open" check counts it meanwhile. Before this
commit the close forgot such a tab.

## Details

Provenance: trailing review through `dbb153ec`, group g04 (tab close retry; Stop on an unconfirmed start), finding f2
(first discovery pass). Suggested bucket: `other`. Confidence: likely.

- The new `(Ok(()), Err(survivors))` arm of `close_host_tab`
  (`crates/farhelm-supervisor/src/service/core/host_tabs.rs:1071-1080` at the tip) keeps the tab in the session's cell;
  its comment says "The tab stays, hidden and marked". Hidden holds only when the tab reads finished
  (`HostTab::finished`, `host_tabs.rs:189-195`), which is what the e2e test
  `a_tab_close_whose_last_sweep_fails_keeps_the_tab_for_the_reap` observes on the orderly-Shutdown path.
- `end_launch_host` (`host_launch.rs:702-737`) also returns `Ok` when Shutdown does not confirm the end: it calls
  `host.abort()` and ends the adopted process with the verified signal. `HostTerminal::abort`
  (`hosts/terminal.rs:2421-2427`) only cancels the runtime task and records no liveness change; the
  `HostTab::end_failed` docs (`host_tabs.rs:151-163`) say such a runtime's status "stays what it last was, possibly
  \"running\" for good". So for a user close of a running tab on that path, `finished()` stays false.
- `listed_host_tabs` (`host_tabs.rs:278-284`) has no `end_failed` filter (`end_failed` is read only in `host_tabs.rs`
  and `host_handover.rs:1466`), so the strip lists the dead tab and `has_open_tabs_for_stop` counts it.
- The same visible state already existed for the `(Err(_), _)` arm, where the host end itself failed and the host may
  really run. The new arm adds it for a host confirmed ended, and the comment's "hidden" is false on this path.

Fix: on the `(Ok, Err)` arm, record that the host is confirmed ended so `finished()` reads true (or filter out
`end_failed` tabs whose end succeeded), and correct the comment; or keep it listed deliberately and say so in SPEC.md.

Unverified premises: that a user-closed tab's Shutdown can fail to confirm while the verified signal succeeds and the
sweep after it then fails too (a compound, rare condition), and what attaching to the aborted runtime shows (not
traced). The listing consequence follows from the cited source. Nothing is lost or wrongly killed; the retry and Delete
still clean up.

Possible cover: none known. The `end_failed` field docs accept a running-looking status for a stopped runtime, but in
the context of an end that could not be confirmed. FILTER.md, "Rare, self-correcting glitches", was considered and not
applied: the stale entry lasts as long as the sweep keeps failing and also changes Stop's tabs-open answer.
