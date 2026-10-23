# Report: the supervisor keeps its state folder claimed until its background work ends

## What this was about

Only one supervisor may own a state folder at a time; it holds a lock file there while it runs. Some of its work runs on
background threads: the cleanup sweep at startup (old cache files and prepared checkouts) and removing a failed launch's
files. When a supervisor stopped, it released the lock as soon as its main task ended, while that background work could
still be running. A new supervisor could then take the folder and prepare a launch, and the old cleanup could delete the
fresh checkout or launch files, failing that launch. Under systemd the successor waits for the old process to exit, so
this hit the desktop app's supervisor (quitting and reopening during a long startup) and supervisors started by hand.

You chose the structural fix on 2026-10-22: the supervisor keeps its lock until all its background work has ended, as
the helm already does, with a size gate of about 200 lines.

What landed: a stopping supervisor now holds the folder's lock until every background job it started has finished. The
lock moves, the moment the supervisor takes it during startup, to the part of the process that waits for those jobs at
exit, so a stop that arrives while the supervisor is still starting is covered too. This is the same arrangement the
helm has used since its version of this fix, and SPEC_impl.md's paragraph on it now covers both.

## Things you should know

- A reopened desktop app waits up to about 20 seconds for the previous supervisor to let go of the folder. If that
  supervisor is still in a startup cleanup that runs longer, the new one now refuses to start and says a just-quit
  Farhelm may still be shutting down, instead of starting and having its first launch fail. Opening it again once the
  cleanup has finished works. The changelog entry says this.
- A background job that never finishes would now keep the old supervisor process, and its claim, alive indefinitely. The
  helm has had the same property since its version of this fix.
- Size: 199 changed lines of code, tests and spec, just under the gate of about 200. Most of it is the test and the
  plumbing that hands the lock over at startup.

## Open questions and possible follow-ups

- Whether to make the successor wait longer than about 20 seconds, so a reopen during a long startup cleanup waits
  instead of refusing. I left the wait unchanged, since that changes startup behaviour beyond this outcome. My
  recommendation is to leave it unless a refused reopen is actually seen: a longer wait means a reopen that shows
  nothing for longer, and a refusal that names the cause is easy to retry.

## PRs

- [#2637](https://github.com/scode/farhelm/pull/2637/changes)
  `fix: keep the supervisor's state folder claimed until its work ends`: the change, its test, the SPEC_impl.md
  sentence, the changelog fragment, and the triage bookkeeping.

## Checks

Run now:

- `cargo nextest run -p farhelm-supervisor --lib` selecting the new claim-lifetime test and the four existing waiting
  and zero-wait claim tests, through the test recorder: run `5002b02a-b392-431b-9ae4-7c7412d9efba` on the final version,
  5 of 5 passed (earlier versions: runs `2ec31c6e-4475-49a5-a02a-4b70768bb429` and
  `20d2a242-a866-427c-9efb-ebd4feafad84`, also 5 of 5).
- The new test 20 times in a row on the final version, through `scripts/hunt-rust-tests.py`, which runs each attempt
  through the test recorder: hunt `6c530e20-be55-4abf-8ae4-a96c91298322`, 20 of 20 passed. One earlier run of the
  reworked test failed (run `89967d9d-7ac6-4c28-beda-5fe8b0a6eef6`) because of a race in the test's own setup, not in
  the product: the test shut down before its stand-in background job had started, and a job that has not started when
  shutdown begins is discarded. The test now waits for that job to start; a production job that never started does no
  harm. An earlier repetition attempt (hunt `ce944547-80dc-484a-a977-eeaaf676d8cf`) only timed out while compiling and
  gave no result.
- The new test against two deliberately broken versions, both of which it caught: the claim released before the runtime
  shut down (run `910482d6-d9d9-441e-9732-b3f94e56c0e0`) and the claim not handed over at the moment it is taken (run
  `7e082039-8280-4848-a634-3060a7480c92`). Both changes were then restored.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all -- --check` for the whole workspace, on the final
  version: clean.
- `python -B scripts/check-test-sleeps.py` in its isolated environment: no unannotated delays.
- `dprint check` on the changed documents and `python3 releasing/check-changelog.py format`: clean.

Reused: `cargo nextest run -p farhelm --test e2e --test supervisor_tether` selecting the supervisor stop tests, which
run the real `farhelm supervisor run` process and stop it by SIGTERM, by SIGHUP, and by the desktop app's connection to
it closing (including during startup), plus the startup-failure test for that connection: run
`d95b0994-9d07-426a-9482-0ba99002e0fe`, 6 of 6 passed, on the version after the first review (an earlier run,
`f422cc67-88d3-464f-a348-6384e8ba1cb5`, on the first version). After it only the new test and one documentation comment
changed, so the start and stop code it exercised is the same in the final version.

Skipped: the full Rust and browser batteries. The change is confined to how the supervisor process starts and stops and
when it releases its lock, and the selected unit and real-process tests cover those paths.
`cargo check -p farhelm-desktop`: the desktop app is unchanged and starts the supervisor as a separate
`farhelm supervisor run` process.

## Review gate

A fresh-context Opus 5.5 reviewer at high effort, reviewing correctness, design and idiom with the repository's
test-authoring checklist, found nothing blocking but one real gap: the claim was handed over only after the supervisor
had finished starting, so a stop during startup could still release it early. That is fixed by handing it over the
moment it is taken. I also applied its other findings: the caveat about the 20-second wait in the changelog and in the
code's documentation, a test that goes through the real supervisor construction and bounds its waits, and the size
wording in the triage ledger.

Because that fix added code, a second fresh reviewer with the same charter checked the revised version. It confirmed the
gap is closed and found no product problems, but showed the test could not tell whether the claim was handed over early
or late. The test now makes startup stop right after taking the claim and checks the claim is already held outside,
which the second deliberately broken version above confirms. Its remaining points (clearer failure messages, naming, a
rewrapped comment) are applied.
