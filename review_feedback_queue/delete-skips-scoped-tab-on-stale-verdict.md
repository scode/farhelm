# Delete skips a scoped tab's cgroup on a stale "no user manager" verdict

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

Deleting a session can report success while a background process started from one of its terminal tabs keeps running.
Three things have to line up:

1. The session's agent was launched without its own systemd scope, for example because the user manager was unreachable
   at that moment.
2. One of its tabs was later opened inside a scope.
3. The supervisor currently believes the user manager is unusable, for example because its first check after a restart
   failed or timed out.

When all three hold, Delete never checks or kills the tab's scope. Anything in it that the process sweep cannot see
survives, and the session row that would allow a retry is gone. SPEC.md says such a tab service "dies when the tab is
closed, reaped, or deleted with its session", naming a non-dumpable `ssh-agent` started by the tab's startup files as an
example. Closing the tab instead handles this case correctly.

## Details

Source: whole-codebase review, 2026-09-30, slice sup-lifecycle.

Reviewer's confidence: confirmed (traced end to end in code; not reproduced at runtime).

Reviewer's bucket suggestion: other.

Possible cover for triage to check: TRIAGE_OUTCOMES.md `## tab-close-skips-scope-on-stale-verdict.md` (outcome
`fix code`, execution complete). It fixed the same hole for Close Tab, the ticker's reap of exited tabs and the
failed-open unwind (`reap_tab_tree`), by recording `@farhelm-tab-scoped` on the window and treating a scoped tab's unit
as recorded evidence. Delete's own handling of tab scopes was outside that item and still has the hole. SPEC.md
"Lifecycle operations" states the rule it breaks ("A current belief that the host has no usable systemd user manager
does not excuse skipping a scope that such a launch or tab may have"), but no fix, TODO or queue item covers Delete.

**How Close Tab already handles this.**

- `open_tab_window` (crates/farhelm-supervisor/src/service/core.rs ~12050-12078) decides per open whether the tab gets a
  scope, and writes `@farhelm-tab-scoped` on the window.
- `reap_tab_tree` (core.rs ~12817-12834) reads that marker back:
  - `Scoped` becomes `ScopeUnits::recorded`. That earns the one re-probe and fails the close if the unit cannot be
    checked.
  - `Unmarked` becomes recorded or `possible`.
  - Only `Unscoped` stays `derived`.

**What Delete does** (crates/farhelm-supervisor/src/service/teardown.rs):

- **Line 364:** `ScopeUnits::recorded(entry.scope.clone())`. Only the agent's current launch unit is recorded.
- **Lines 366-378:** tab units come from `session_tabs_including_dead` (core.rs ~11609-11620), which keeps only the tab
  id and drops the window's scoped marker. Every tab unit is added with `extend_derived`, whatever the window says.
- **Line 385:** the stale-verdict re-probe runs only when `entry.scope.is_some()`. For an agent launched unscoped,
  nothing re-probes.
- **Lines 390-406:** the tab and launch globs go through `units_matching`, whose `tools(false)` (scope.rs ~846-853)
  returns `None` on a negative or recent timed-out verdict. The resulting `Err` hits the `Err(e) if !available()` arm,
  which only logs at `debug!`.

**What happens next** (crates/farhelm-supervisor/src/service/sweep.rs 1356-1391):

- `reap_process_tree` runs with nothing recorded or possible, so `earns_reprobe` is false.
- It drops every derived name with a `debug!` ("derived scope names are skipped"), runs only the portable sweep, and
  returns `Ok(())` when that sweep is clean.
- Delete then kills tmux, removes artifacts and commits the row deletion. The tab's cgroup is never asked about.

**How the preconditions arise.**

- _Agent unscoped, tab scoped._ Scope is chosen separately for each launch and each tab open. One path is an agent
  launch that saw `TimedOut`, followed more than 60 s later (`TIMED_OUT_REPROBE_INTERVAL`) by a tab open whose probe
  succeeds. Another is an agent launch and a tab open made by supervisor processes that saw different manager states.
  Pre-scope rows default to `launch_scoped = 0`, which gives the same shape.
- _Negative verdict at delete time._ A usable verdict lasts for the life of the process, so this needs a supervisor that
  has not yet seen a usable manager. That means either a first probe after restart that returned `Absent` (cached until
  something calls `reprobe()`), or a `TimedOut` within the last 60 s.

**What survives:** anything in the tab's scope that the portable sweep cannot see.

- A process that reparented and scrubbed or overwrote its environment.
- A non-dumpable process, such as `ssh-agent` or a setuid program, whose `/proc/<pid>/environ` its owner cannot read.

SPEC.md (Session view) explicitly places such startup-file services in the tab's cleanup guarantee on hosts with a user
manager. After the delete the row is gone, and the only search that could find the unit (`tab_unit_glob`) is keyed to
that session, so it never runs again.

**How to verify:**

1. Use a fake `ScopeManager` whose first probe is negative, for example
   `fake_reprobing_with_matching_units_vanishing(false, true, …)` or `fake_probing`.
2. Build an entry with `scope: None` and a tab window whose `@farhelm-tab-scoped` says scoped.
3. Delete it. The recorded `ScopeOp`s show no re-probe, no `List`, and no `Exists` or `Kill` for the tab unit, and the
   delete succeeds.

The existing test `tab_reap_reprobes_a_stale_verdict_and_refuses_an_unconfirmed_scope` shows the fixed close-path
behavior for comparison.

**Fix shape:**

- Have Delete's tab rediscovery keep each tab's scoped marker.
- Classify tab units with `reap_tab_tree`'s rules: `Scoped` goes to recorded; `Unmarked` goes to recorded if the
  session's launch was scoped and to `possible` otherwise; `Unscoped` goes to derived.
- Before enumerating the globs, re-probe whenever any unit is recorded or possible, not only when `entry.scope` is set.
- A scoped tab whose unit still cannot be checked then fails the delete with the row kept, as a recorded launch unit
  already does.

Rebase note (main at 1ec60cc): 7dae793 strengthened the spec backing. SPEC_impl.md now says "Stop, Restart, Delete and
tab close on a session or tab whose scope is recorded (... a tab window marked as opened in a scope ...) refuse until it
has passed", and lists a tab window that records a scope as evidence for a re-probe. Delete's code
(`teardown.rs:364-406`) is byte-identical at 1ec60cc and still does neither for a scope-marked tab of an unscoped
session.
