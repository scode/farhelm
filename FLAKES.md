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

## 2026-11-22 — `the_exit_is_reported_while_a_descendant_holds_the_pty` in `crates/terminal-host/tests/host_lifecycle.rs`

The x86-64 Linux v0.44.1 release job failed this test alone. The test read the helper's stdout link in `/proc` while the
helper was still redirecting stdout to the readiness FIFO. Reading the PID from the FIFO let the parent exit before the
helper's shell had restored stdout to the PTY. The assertion ran in 9 ms. A prior hosted pass with the same fixture took
47 ms, and an isolated retry on v0.44.1 passed in 51 ms. The full workspace suite passed on the hosted retry. Open.

```text
the helper's stdout is "/tmp/<run>/<fixture>/ready"
```

- Failure evidence `0363fbf7-7a8f-4f6d-b2cb-b627e371d917` is in the hosted run's expiring failure artifact. The
  GitHub-hosted runner was Linux x86-64 (Ubuntu 24.04, kernel 6.14.0-1017-azure, eight logical CPUs, locale `C.UTF-8`).
  The tree at `d8c0baa6d3a8a8f791c44e84fa5a3438fa61bcd3` was clean. The selection was
  `cargo nextest run --workspace --exclude farhelm-desktop`, with four slots and zero retries. The only `FARHELM_*`
  names were the recorder's `FARHELM_TEST_RUN_DEFAULT_ROOT` and `FARHELM_TEST_TRACE_DIR`. Machine load and memory
  pressure were unavailable. [Failing job](https://github.com/scode/farhelm/actions/runs/45056978170/job/137314424304)
- Isolated Linux retry `f1492329-18dc-4d14-a6c6-6610282d20f5` used
  `cargo nextest run -p terminal-host --test host_lifecycle -E 'test(=the_exit_is_reported_while_a_descendant_holds_the_pty)'`
  on the same clean tag. One test ran and passed.
  [Hosted retry](https://github.com/scode/farhelm/actions/runs/45056978170/job/137328954707)
- PR #4487 added the fixture assertion. The terminal-host source and fixture were identical in v0.44.0-rc.6, v0.44.0 and
  v0.44.1. The failed assertion occurred before the helper's post-exit PTY output check.

Class: fixture-premise

Cause: established
