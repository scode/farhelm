# A failed session create can leave its processes running with nothing left to find them

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

Creating a session can fail after the new agent has already started. When that happens, Farhelm cleans up and then
removes the half-created session. The cleanup has two tools: killing the launch's systemd scope, and a sweep over marked
processes. The session is removed when the sweep succeeds, even if the scope kill could not be confirmed. A detached
background process the sweep can't see keeps running inside the scope: a daemon that rewrote its process title or hid
its environment, such as a database or web server the agent started. The session is gone, the create's error does not
mention the leak, and no later delete or restart can reach that process.

## Details

Source: whole-codebase review, 2026-09-30. A slice reviewer dropped this as "too rare and by design" without a cited
basis; an independent checker found no basis and it was traced and written up separately. Traced by reading code and
specs; not reproduced. It needs four conditions to coincide, so it is rare; it could be argued up to `high` because
nothing ever reports or reaps the leak.

- **The warn-only policy.** `ScopeKillFailure::Warn` is in `crates/farhelm-supervisor/src/service/sweep.rs:223-237`. It
  is documented as being "for cleanup paths that have no user operation to fail, such as a failed create's rollback". In
  `reap_process_tree` (`sweep.rs:1408-1420`), the case of a clean sweep plus a scope error returns `Ok(())` under `Warn`
  and only logs a `warn!`. The scope error travels with the result only when the sweep also fails
  (`sweep.rs:1427-1429`).
- **When the scope kill fails.** `kill_scope_with_grace` (`sweep.rs:1621-1711`) returns an error when
  `confirm_scope_gone` (`sweep.rs:1750-1776`) cannot see the unit collected in time. That happens when the unit is still
  loaded after SIGKILL, or when `systemctl` stops answering. If the manager isn't answering, the SIGTERM and SIGKILL
  were probably never delivered either.
- **Create rollback arms that use `Warn` and then delete the session row:**
  1. `crates/farhelm-supervisor/src/service/core.rs:9253-9328`: tmux reports that creating the terminal failed, and a
     follow-up check confirms no tmux session exists. The reap at 9268-9275 uses `Warn`, then `abandon_launching_record`
     (9320, defined at 13300-13340) deletes the row through `store.delete_session` (13316). The comment at 9255-9266
     contradicts the policy it passes. It says "tmux has no session" is not proof, because "a window that ran far enough
     to create its scope and daemonize something leaves exactly this shape", and that "an unconfirmed reap RETAINS the
     row". Under `Warn`, an unconfirmed scope with a clean sweep counts as confirmed, so the row is deleted anyway.
  2. `core.rs:9482-9567`: the database write that marks the new session running fails. The reap at 9495-9502 uses
     `Warn`. If the tmux kill then succeeds (9543), the row is deleted (9557). This arm has a fourth condition: the row
     delete must succeed right after a database write failed.
  3. `core.rs:9408-9480`: a delete raced the create and the row is already gone. `Warn` removes the scope failure from
     the error text, so the failed create does not mention it.
- **What must stack up.**
  - (a) The create fails at one of these points after the agent's scope exists.
  - (b) The scope teardown cannot be confirmed, because the user manager is hung or overloaded or the unit stays loaded.
  - (c) The surviving process can't be seen by the marker sweep. SPEC.md:627-631 names real cases: title-rewriting
    daemons such as nginx or Postgres started by `pg_ctl`, and non-dumpable processes. Roots are `None` in these arms,
    so only markers and the scope reach anything.
  - Arm 1 also needs the tmux session to be gone while something it started lives on. Not reproduced; the code's own
    comment asserts it can happen.
- **What is left, and who reaps it.** A `farhelm-<session>-<generation>.scope` stays loaded with the process inside it.
  The only code that enumerates scope units is Delete's per-session glob
  (`crates/farhelm-supervisor/src/service/teardown.rs:389-406`, using `scope::launch_unit_glob`). Its `units_matching`
  is not called anywhere else in the supervisor. Delete needs a row, and the row was just deleted. Session ids are never
  reused, and the reservation is settled `Failed`. So nothing ever reaps the scope; it lasts until its processes exit or
  the user's systemd manager stops.
  - The restart rollbacks at `core.rs:10859`, `11154` and `11216` also use `Warn`, but they keep the session row, and a
    later Delete finds the scope through the same glob. They are recoverable and not part of this finding.
  - Creates that allocated a fresh checkout keep their row (`retain_create_refusal`), so they are also recoverable.
- **What the spec promises.** SPEC.md:638-648 ("Lifecycle operations"): when cleanup can't be confirmed, the operation
  "fails visibly instead of reporting success or carrying on". It also says "when a launch or tab ran in a systemd
  scope, the scope must be confirmed gone, even if the portable sweep found nothing", and "a later attempt may retry the
  cleanup". SPEC_impl.md:1790-1792 repeats this. The create does fail, but deleting the row removes any way to retry,
  and the error leaves out the scope. "Partial deletion" (SPEC.md:1506-1512) covers only an explicit Delete's file
  removal, so it doesn't excuse this.
- **The choice is deliberate, on a false premise.** The test
  `under_warn_a_scope_that_never_goes_away_is_reported_but_not_fatal` (`sweep.rs:2585-2619`) justifies `Warn` with
  "treating the unconfirmed unit as fatal there would fail a cleanup nobody can retry". That premise is false for arms 1
  and 2: they already have a retain-the-row path for sweep failures (9276-9294 and 9543/9567).
- **How to verify.** Write a core-level test on the create path with a scope manager whose kills fail, like the existing
  `fake_failing_kills` used at `core.rs:15223` for restart. Pair it with a tmux fake that fails creating the session and
  reports no session afterwards. Assert that the row is deleted and the error does not mention the scope.
- **Fix shape.**
  - Pass `ScopeKillFailure::Refuse` in arms 1 and 2. The existing `Err` handling then keeps a launching record that Stop
    or Delete can retry.
  - In arm 3 the row is gone either way, so surfacing the scope error in the returned error is all that can be done
    there.
  - Update the `ScopeKillFailure` doc and the test rationale to match.
