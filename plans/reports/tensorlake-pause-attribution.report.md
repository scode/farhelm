# Give later Tensorlake pauses their own time

## What this was about

After a Farhelm Suspend and a resume outside Farhelm, a later Tensorlake idle pause could show **Suspended from
Farhelm** with the first pause's time in Hosts and the sandbox's session card. Two earlier fixes left races around Retry
and a concurrent Suspend. You approved one draft with room to handle those cases together.

Farhelm now drops the earlier attribution once it sees the sandbox running again. A later pause gets the time Farhelm
noticed it. An explicit Suspend on an already paused sandbox uses its own completion time when the saved time belongs to
an earlier pause. Repeating Suspend during the same pause keeps the original completion time.

## Things you should know

If the Tensorlake API cannot answer, a successful connection to the sandbox's Farhelm supervisor also establishes that
the sandbox ran. Review found that this matters during outages because Farhelm makes no further status calls while the
host is connected. Farhelm keeps the running observation across Retry and clears older saved times after the connection
attempt. The cleanup can finish late without clearing the time of a newer Suspend.

Farhelm can still show the old time if it misses the resume and re-pause entirely, for example when both happen between
checks or while the helm is stopped. If clearing the saved time fails, a restart followed by an outside pause can bring
back that time. A backwards clock step or a millisecond tie can also retain it. A late Running reply can drop a newer
Suspend's time and make it look like an outside pause.

## Open questions and possible follow-ups

The session card still says **Suspended outside Farhelm** when attribution is missing. The plan leaves that wording for
a separate decision. If Farhelm could not observe who suspended the sandbox, the card could show only the time it
noticed the pause.

## Pull request

- Corrects suspend attribution in Hosts and on the session card.
  [#4329](https://github.com/scode/farhelm/pull/4329/changes)

## Checks

All 192 selected Linux helm tests passed with no runtime skips. The Tensorlake lifecycle tests drove a scripted provider
through the actual HTTP routes, connection manager and SQLite store. The recorder ID is
`49f18a0e-31c6-4e07-a738-5111818968e5`.

```sh
cargo nextest run --locked -p farhelm-helm --lib \
  -E 'test(/^(manager::tests|hosts::tests|tensorlake::hosts::tests)::/)'
```

The 192-test run on `7c230242` covers the draft's production code. The rebase added a desktop PATH TODO and queue
bookkeeping. One focused case passed again with the final assertion and no runtime skip
(`f9adab8f-0785-4f5c-845b-19de7f21c94d`). It verifies that Retry after the connection task dies still uses the evidence
that the sandbox was running.

```sh
cargo nextest run --locked -p farhelm-helm --lib \
  -E 'test(=manager::tests::a_suspended_sandbox_is_never_dialed_and_resumes_by_itself)'
```

`cargo clippy --locked -p farhelm-helm --lib --tests -- -D warnings`, Rust and Markdown formatting, changelog format,
the Rust test delay check and `git diff --check` passed. The instruction citation checker passed with four existing
warnings. The full workspace battery and a live Tensorlake run were skipped because the helm fixtures exercise the
changed state, storage, and lifecycle paths with controlled Running and Suspended answers. No GUI code or wire shape
changed.

## Review

The independent review gate passed after the scope reassessment. The fixes cover the later pause's noticed time on
Retry, running evidence through an API outage, keeping timestamp cleanup outside the dial deadline, and confirming that
Resume itself clears the saved time. The product, test and spec diff totals 350 changed lines including removals. The
commit message passed its cold read.

### Landing

#### What landed with it

Since this plan's work started from main, main gained remember-active-terminal (the app remembers each session's chosen
terminal tab) and antigravity-subagent-check (Restart stays on Antigravity's main conversation). An independent reviewer
and I checked both against this plan and found no interaction: neither touches the helm's host connections, its
Tensorlake code or its saved host data.

The review checked the races the report names. Suspend pauses the helm's connection to the host before it asks
Tensorlake, so its saved time is later than any Running answer the helm already had. The report's remaining case, a late
Running reply dropping a newer Suspend's time, can only happen when that pause does not take effect within its wait,
which makes it rarer than the report suggests, but it is still possible. Clearing an old saved time first checks that
the saved time is still older than the Running answer, so a late clear cannot erase a newer Suspend's time. When Retry
restarts the helm's connection to a host, Farhelm still remembers that it saw the sandbox running.

The plan also stopped Hosts from reading the saved Suspend time on its own; Hosts now shows only the time that comes
with the host's Suspended status. After a helm restart Hosts still shows the saved time, because the first Suspended
status the helm reports already includes it.

#### Fixes made while landing

One went in as a second PR, [#4335](https://github.com/scode/farhelm/pull/4335/changes): the code comments on the
suspended state's two times still described Hosts reading the saved time itself. It changes no behavior.

#### Checks

The plan changes only the helm, plus SPEC_impl.md and bookkeeping, so the landing ran the helm package's tests and the
two lint (clippy) runs the validation guide calls for on the maintainer's own test machine (an arm64 Linux VM that
builds and tests the tree itself), and the small checks locally on x86_64.

On the plan as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop -E 'package(farhelm-helm)'`, on the test machine
  (`6c34775e-cee0-4e78-a69c-dd9510a346ab`): all 1776 tests in the helm package passed, a wider run than the report's
  192-test selection, with no runtime skips.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test machine,
  not through the recorder: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

The landing fix (#4335) changes one doc comment, so the runs above were reused for it; `cargo fmt --all -- --check`
passed again locally.

Skipped: the rest of the workspace suite (nothing outside the helm changed, and what the helm sends the app about hosts
is unchanged), the GUI and desktop checks, and a live Tensorlake run, which the helm's scripted Tensorlake tests stand
in for.

#### Things you should know that the report does not say

- The report counts "the later pause's noticed time on Retry" among the fixes, but one case of it has no test: a host
  still shows Suspended, Tensorlake answers Running, and Retry interrupts the connection attempt before it finishes. The
  next pause should then get a new time rather than the first pause's. Reading the code shows it handles that, but the
  test meant to show it reaches Suspended from a connected host, where the old time is already gone, so it would pass
  without the fix. Possible follow-up: a helm test that interrupts the connection attempt with Retry while the host
  still shows Suspended. I did not add it at landing because it needs new support in the helm's test harness.
