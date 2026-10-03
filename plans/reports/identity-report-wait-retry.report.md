# Conversation reports through busy periods and supervisor restarts

## What this was about

After `/clear` or another conversation switch, Resume could reopen the previous conversation. A valid report was lost if
the supervisor waited more than a second for the session's local record lock, or if the hook met a supervisor restart.
Claude might not send another report until its next session start.

The agreed behavior is to preserve reports while the supervisor is busy updating a session’s local record and retry them
across short supervisor absences. The two draft PRs remove the supervisor’s one-second lock deadline, log slow waits,
check Claude’s sender before waiting, and give the reporting hook a 30-second total budget under 60-second
Claude/Codex/Grok timers. Neither has been marked ready or merged.

## Things you should know

The session record lock prevents simultaneous updates to a session’s saved state. The supervisor now keeps waiting for
that lock instead of refusing a report after one second; the hook still stops after 30 seconds in total. Those are
separate limits: Claude, Goose and Pi reports can still be applied after the hook exits, while Codex, Grok and OMP must
still have a live hook when their checks run under the lock. Claude’s sender check happens before waiting, while the
reporting process is alive. The hook retries missing or refused sockets and dropped connections for roughly four
seconds. A connection that stays alive for at least one second earns a fresh reconnect window if it drops; repeated
immediate drops share the current window. All attempts share the 30-second total budget. Explicit supervisor refusals
and incompatible protocol versions are final.

The new outer timers apply to newly launched Claude/Codex sessions. Existing sessions keep their old timers until
relaunched. Grok users must raise their manually configured hook timers to 60 seconds; the old three-second setting can
kill a retry early. Pi and OMP keep their existing client-side reporting plugins and their two-second child-process
timers. Changing the published plugin would make the supervisor reject reports from already-running OMP sessions until
those sessions were relaunched. Their later turns send new reports. With the supervisor down, each report may now last
until that timer kills it and leave no hook-log line. The plugins run reports in sequence. Whether waiting for a report
also delays the agent’s next turn was not verified; the possible user impact and review choice are described below.

The completed TODO and feedback entry are removed, and the triage record points to both draft PRs. The stack was rebased
onto main at `adb2bf0c`. The relevant upstream changes were separate admission limits for management and listing,
lifecycle panic replies, and protocol version 36. Report-only connections keep their own admission path; the busy-host
refusal does not replace the unbounded report-lock wait. The only text conflict was in the feedback index, resolved by
retaining both sides' completed-item removals.

## Open questions and possible follow-ups

One ordering edge remains: if two different conversations from the same session both report across a supervisor restart,
the older report can reconnect last and overwrite the newer one. Resume can then select the older conversation. Retrying
the same report is safe; separate hook processes do not establish an order between different reports.

This is surfaced for your decision, not recorded as an accepted limitation. You can accept these bounded reliability
fixes with that narrow overlap case documented, or ask for a follow-up that establishes ordering before landing. I
recommend reviewing this stack on its current scope and treating cross-report ordering as a separate design decision;
adding an epoch or queue here would exceed the agreed retry mechanism.

Keeping Pi/OMP’s two-second timers is an explicit compatibility decision. Whether the new retries can make an agent turn
wait for that full interval while the supervisor is down remains unverified; no new turn delay has been accepted. You
can accept that uncertainty for this stack or request a focused measurement before landing. I recommend measuring this
separately before deciding whether to change those plugins, because a plugin change would affect running OMP sessions.

## The PRs

- [PR1: preserve reports behind busy capture claims](https://github.com/scode/farhelm/pull/1504/changes) — jj change
  `puvxunnp`, bookmark `plan/identity-report-wait-retry/01-wait-for-claim`, commit `648fd446`.
- [PR2: retry conversation reports across supervisor restarts](https://github.com/scode/farhelm/pull/1535/changes) — jj
  change `trmmopou`, bookmark `plan/identity-report-wait-retry/02-hook-retry`, commit `8902359b`.

## Checks run, reused and skipped

- Hook and budget-parser tests: 47 passed, recorder run `2aae8ac8-36b4-4463-9ca1-fc704923ad1c`. This includes retry
  after a long-lived connection drops and the cap on repeated immediate drops. Reused after rebase because the hook and
  parser behavior are unchanged by it; the rebuilt real-hook tests below cover their integration with the new supervisor
  and protocol.
- Rebuilt real-hook end-to-end selection: 23 passed on the rebased stack, run `9c17beab-d1be-48be-9463-9a2596187f8e`. It
  covers silent hooks, no supervisor, a connected silent supervisor, log outcomes and vendor reports. No runtime
  substrate skips were reported.
- Rebuilt supervisor contention and injected-timer selection: 49 passed, run `d9ea3d51-d19b-4ef5-a991-64ad70a40b56`,
  covering Claude, Pi and OMP reports waiting past the old lock deadline. The two retry fixtures were then given more
  scheduling headroom; both passed again on the final commit, run `d68efb26-e444-4f35-85af-127c978056c6`.
- `cargo clippy -p farhelm --bin farhelm --tests -- -D warnings` passed.
  `cargo check --locked --release -p farhelm --bin farhelm` passed without warnings, specifically checking the
  debug-only budget seam is absent from the shipped binary.
- Formatting, changed Markdown formatting, changelog format, diff whitespace, and the isolated test-sleep source checker
  passed. The checker inspected 275 delays with no missing rationale. The website build passed and verified all internal
  links on `814f1743`; its result is reused on `8902359b`, whose only subsequent website edit rewords an ordinary prose
  paragraph without changing frontmatter, links or generated examples.
- Earlier PR1 review and contention-test evidence were reused as the starting point: the rebased PR1 patch has the same
  stable patch ID as reviewed commit `d432ff72`. New contention tests on the rebased stack supplement that evidence.
- Full Rust, browser, desktop, installer and release batteries were skipped: the changes affect hook transport, report
  admission and prose. Focused real-hook and supervisor tests cover those paths; there is no changed browser or desktop
  interaction. No hosted CI or deployment was triggered.
- Earlier failed test/compile attempts remain retained in the private working log. They were development fixture/setup
  corrections, not silently replaced by later passes and not classified as latent flakes. The unconfigured-system-Python
  sleep-check attempt ran no check; the isolated interpreter's successful check is the evidence used here.

## Review gate outcome

Both PRs passed the required fresh-context Opus 5.5 review at high effort. PR1’s previously approved patch is unchanged
by the rebase. PR2’s full review and scope reassessment found no remaining production defect; the final acceptance pass
confirmed its documentation and fixture corrections. That pass required the final bookkeeping amendment to be pushed.
GitHub now reports PR1 at `648fd446` and PR2 at `8902359b`, so that requirement is satisfied. Both PRs remain drafts
with the intended stack bases.

Two optional polish findings were deferred: diagnostic phase wording and a test comment that could explain the
long-lived-connection threshold more fully. Neither blocks the review gate. The cross-report ordering risk above remains
a maintainer decision, not an accepted limitation.

The PR wording passed its required cold read. The maintainer report also passed its required fresh-context native cold
read after clarifying the lock and hook budgets and the Pi/OMP caveat; its public-repository hygiene check passed.
