# Flakes

The LATENT flakes currently believed active: tests that pass on the machine they were written on and fail elsewhere, or
sometimes, or only under load, whose cause was not obvious the moment they failed. Every flake in this file is open, and
each has a Deflake entry in TODO.md and an exclusion line in `deflake/known-flakes.txt`. The three come and go together.

When a flake is closed (fixed, dismissed, or moot because its test no longer exists), the PR that closes it adds a dated
closing entry saying which and why. It then moves that entry and every earlier entry for the same flake to
`FLAKES_ARCHIVE.md`, in the same PR that removes its TODO entry and exclusion line. That keeps this file short enough to
read in full, while the archive keeps the history that shows which seams of the test suites keep producing flakes and
whether the fixes hold. Before recording a failure, search the archive for the test's name. A closed flake that recurs
is the most useful thing that history can show, and its new entry goes here, because it is open again, and cites the
archived one by date and title.

NOTE: This is not a record of every red test. A test that was written, flaked in the same working session, and was fixed
before it left that session is not a latent flake and belongs neither here nor in the archive.

Each entry has one observation paragraph under a dated heading naming the test and its file, without line numbers (they
move). Say what was observed, where (which machine or runner, what load), what the cause turned out or is suspected to
be, and the disposition: fixed in which PR, ignored with what reason, or open. When a fixed flake recurs, add a new
dated entry rather than editing the old one.

New entries also identify the retained run or release job, tested commit and dirty-tree status, exact selection and
concurrency, locale, and relevant `FARHELM_*` variable names. Use portable runner descriptions and redacted
paths/commands; never put hostnames, usernames, credentials, or raw private manifests here. Distinguish a recorded
executable identity (a browser's build, say) from a configured or reported version. The recorder and retention rules are
in `docs/test-run-evidence.md` and AGENTS.md.

Record as much of the following as can be had without a new run, and say "unavailable" rather than guessing. Each item
is something that has been easy to note at the time and hard or impossible to recover later:

- The other tests that failed in the same run, or that this one failed alone. A cluster points at a shared cause; a lone
  failure points at the test.
- How long the failing attempt took next to a typical pass, whether it failed fast on an assertion or ran into a
  timeout, and which step the run's own timestamps show stalling.
- What else was running on the machine: other test runs or builds, load average and memory pressure where available, and
  the tests sharing its slot. "Under load" is often suspected here and rarely measured.
- For a dirty tree, what the uncommitted change was (a diffstat or a one-line description), and the versions of the
  tools involved: compiler, Node, Playwright and browser builds, OS and kernel.
- Each reproduction attempt's command, conditions (alone, under added load, with a stand-in) and result as failures out
  of attempts. "Did not reproduce" is evidence only with its count and conditions.
- Hypotheses ruled out, each with the observation that ruled it out.
- The latest changes to the test, its fixture, and the code it covers, or that there were none recently.
- Earlier entries for the same test, and how often it has passed since it was last seen, when that is known.
- Where the full evidence lives (a retained run on one machine, or a hosted artifact) and whether it expires; copy what
  matters into the entry before it does.
- Values that vary between runs and could matter: ports, process IDs, seeds, test order.

Quote the failure itself when the agent recording the entry has it at hand: the assertion or panic message, the
Playwright error, or a script's `FAIL` lines, copied verbatim from the console, the retained run's output, or a CI log
it can read. Put it in a fenced block right after the observation paragraph, trimmed to the lines that say what failed,
with paths and identities redacted as above; no quoted line may start with two hash signs and a space, which would read
as a new entry. Do not reconstruct a message that was never captured; when the run lost it (an interrupted run, a
truncated log), say so in the paragraph instead. Retained runs are private to the machine that made them and hosted CI
artifacts expire, so without the quote a later reader often cannot tell what actually failed, or whether two
observations are the same failure.

After the paragraph (and the quoted failure, if any), add `Class: <class>` and `Cause: <confidence>` as separate
paragraphs. Classes are `readiness`, `replay-live`, `fixture-premise`, `peer-lifecycle`, `process-interference`,
`budget`, `ambiguous-observable`, `pointer-focus`, `substrate`, `product`, `deterministic-regression`, or `unknown`.
Confidence is `established`, `hypothesis`, or `unknown`; `Cause:` is the last line. These fields apply only to new
entries. Missing historical fields are missing evidence, not an implicit cause classification.

## 2026-11-09 — a crash-retry create's Delete refused while the user manager stayed silent (crates/farhelm/tests/e2e/create_idempotency.rs)

`create_idempotency::a_crash_before_the_outcome_commit_still_replays_the_same_session` failed in two consecutive landing
rounds' full Rust runs on Tensorlake sandboxes (4 CPUs, 8 GiB, Ubuntu 24.04 clones with systemd as PID 1 and a lingering
per-user manager), each a workspace nextest archive split by hash partition over nine sandboxes, four slots per sandbox,
zero retries, through the recorder's generic mode: run `cf6e2b79-c129-4882-9c05-218f4aed65bd` on clean stacked commits
`2cc53615`/`160f30e2` and run `614967f5-8f43-4dd9-9fcd-c6ba7d693206` on clean stacked commits `68da3700`/`e5fd6349`. The
tmux was the repository's pinned 3.7c build shipped to the sandbox; its executable SHA256 and the locale were not
recorded (unavailable). Ambient `FARHELM_*` names were scrubbed by the recorder. Both times the test failed alone in its
shard, after about 15.3 s against about 1 s for a local pass, on the same assertion: Delete of the retried session
refused because the recorded `-0.scope` could not be checked while the systemd user manager was "not usable now". The
test reopens a supervisor on the same state directory; the reopened supervisor's fresh probe of the user manager has a
15-second bound, which matches the failure's duration, so the probe most likely timed out under the shard's load and
Delete refused the recorded scope by design, the same exposure as the 2026-09-30 entry for
`an_ambiguous_planned_checkout_never_adopts_a_foreign_directory`. Reproduction: five consecutive local passes on this
development machine (run `84880653-65d2-4dd4-8940-b3d7fc663468`, `--stress-count 5`), and three passes each on three
sandboxes rerunning the create-idempotency and session-lifecycle modules from the same archive (runs `41f38188`,
`1ad1e77f`, `3aa87de6`), where a different test on one sandbox reported the user manager unusable and skipped. The run
just before these, on the same sandbox template without the scoped-cleanup change (#3237), passed it; whether that
change made the probe's timeout likelier is not established. The retained failing runs live on this machine and expire
with temporary-file cleanup; the failure is quoted below. Disposition: open, excluded from deflake sweeps.

```text
delete the retried session: killing process tree: the process-tree sweep confirmed nothing is running, but the named cgroup scope(s) could not be torn down (scope farhelm-<session>-0.scope could not be checked because this host's systemd user manager is not usable now, so its cgroup may still hold processes); the session is kept so the operation can be retried
```

Class: substrate

Cause: hypothesis — the reopened supervisor's user-manager probe ran out its 15-second bound under load.

## 2026-11-09 — a failed-create replay test's create reply did not arrive in time (crates/farhelm-supervisor/src/service/core.rs)

`service::core::tests::a_failed_create_replays_its_error_and_a_changed_override_conflicts` failed in two consecutive
landing rounds' full Rust runs on Tensorlake sandboxes (4 CPUs, 8 GiB, Ubuntu 24.04 clones with systemd as PID 1), each
a workspace nextest archive split by hash partition over nine sandboxes, four slots per sandbox, zero retries, through
the recorder's generic mode: run `eaaa34eb-7392-427b-80cc-328b1c6c9406` on clean stacked commits `68da3700`/`e5fd6349`
and run `d230a97e-df2f-4da1-aa99-192fe6910671` on clean stacked commit `980ac423`. The tmux was the repository's pinned
3.7c build shipped to the sandbox; its executable SHA256 and the locale were not recorded (unavailable). Ambient
`FARHELM_*` names were scrubbed by the recorder. Both times it failed alone in its shard, after about 10.05 s, on the
test's own 10-second wait for the reply to its in-process create, against about 0.2 s per iteration locally. Neither
landing changed the create path. Five consecutive local passes on this development machine on the later tree (run
`9f342a7d-f413-47fd-9723-71351aae8b1d`, `--stress-count 5`); not reproduced on a sandbox. Load on the sandboxes was not
measured; the same shards also run the e2e suite's real supervisors. Whether the create waits on something slow under
load (a systemd user-manager probe, as in the crash-retry entry above, is a candidate) is not established. The retained
failing runs live on this machine and expire with temporary-file cleanup; the failure is quoted below. Disposition:
open, excluded from deflake sweeps.

```text
owned create reply must arrive: Elapsed(())
```

Class: unknown

Cause: unknown

## 2026-11-17 — retired session deletion refused when the user manager stopped answering (crates/farhelm/tests/e2e/containerized_sessions.rs)

`containerized_sessions::retired_session_refuses_terminal_and_lifecycle_use` was the sole failed test in the first
v0.40.0 x86_64 release workspace run, recorder `8346914f-7b4d-4f82-9c7d-ed2c4b007feb` at clean `addf7bcac`. It ran on
hosted Ubuntu 24.04 (Linux 6.14.0-1017-azure, eight CPUs, load unavailable) with nextest 0.9.148,
`cargo nextest run --workspace --exclude farhelm-desktop`, four slots, zero retries, and locale `C.UTF-8`. The only
`FARHELM_*` input was the recorder-owned trace directory. The supervisor first recorded a scoped launch, then the
reopened supervisor's user-manager probe timed out at 15 seconds. Delete kept the row because it could not confirm the
scope was empty. The test took 15.340 s, compared with 0.397 s on the same product source in RC.6's hosted workspace
run. The reason the user manager stopped responding is unknown. The test last changed in #3330. The release job's
failure artifact retains the report and traces until hosted retention expires. A focused local run on the clean stable
tag passed in 2.000 s with a working systemd user manager, recorder `3dbb1a5f-8029-4422-9f75-75de2c26a523`,
`cargo nextest run -p farhelm --test e2e -E 'test(=containerized_sessions::retired_session_refuses_terminal_and_lifecycle_use)'`,
four available slots, one selected test, zero retries. Disposition: open and excluded from Deflake sweeps.

```
called `Result::unwrap()` on an `Err` value: killing process tree: the process-tree sweep confirmed nothing is running, but the named cgroup scope(s) could not be torn down (scope farhelm-<session>-0.scope could not be checked because this host's systemd user manager is not usable now, so its cgroup may still hold processes); the session is kept so the operation can be retried
```

Class: substrate

Cause: established
