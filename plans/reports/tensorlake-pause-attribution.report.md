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
