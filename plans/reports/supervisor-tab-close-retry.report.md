### What this was about

Closing a terminal tab ends the tab's terminal host and then sweeps for anything the tab's shell left running after the
host's hangup. Three things went wrong when that cleanup failed:

- If the host ended but the final sweep could not confirm the leftovers were gone, Farhelm forgot the tab and its launch
  record before reporting the error. Neither closing the tab again nor Farhelm's own automatic cleanup could retry, so
  leftovers could stay until the session was deleted.
- If ending the host failed too, the error told the user the tab's processes had been reaped, even though the sweep for
  leftovers had failed.
- When an automatic retry of a failed close itself failed at its first sweep, the next retry was not pushed back, so
  Farhelm retried on every cleanup pass instead of backing off.

You decided to fix these in memory only, with no new database table and no change to the tab's launch record, and to
record your cleanup policy in SPEC.md: on Linux with systemd, Farhelm relies on systemd scopes to end a session's and a
tab's processes; without systemd, and for programs that ignore the hangup when their host is killed, Farhelm applies
cheap sanity checks only. A supervisor restart in the middle of a failed close leaves whatever remains to the session's
Delete.

What was built: a failed close of either kind now keeps the tab and its record, and Farhelm retries on its own after a
second or two and then less and less often, at most about a minute apart, until a retry succeeds; there is no cap on the
number of retries, and a supervisor restart or the session's Delete also ends them. A tab that is still visible (its
host could not be confirmed ended) can also be closed by hand, which retries at once. When both the host end and the
sweep fail, the error names both. A failing automatic retry now pushes the next one back. The policy is in SPEC.md under
"Session view", next to the rule for tabs that exit on their own.

### Things you should know

- A tab whose host did end but whose final sweep failed is hidden from the tab strip while Farhelm retries, because its
  shell is gone. A tab whose host could not be confirmed ended stays visible, as before.
- A related case is not covered and was already like this before: when a tab's shell exits on its own and Farhelm's
  first automatic close of it fails at the sweep before the host is ended, that close is retried on every cleanup pass
  with no backoff. The plan's third fix covers only retries of a close that had already failed once. See the follow-up
  below.
- The policy sentence on programs that ignore the hangup overlaps a paragraph that the plan
  `supervisor-outliving-programs` (Stop and Restart ending an agent that outlived its killed terminal host) adds to
  SPEC.md "Lifecycle operations". This plan's text was placed in "Session view" so the two do not collide. Once both
  have landed, this paragraph's general policy sentence could be shortened to a pointer to that one.
- The PR removes the TODO entry "Keep a failed tab close retryable after the terminal host ends."
- No release-note headline was asked for; the PR carries the routine changelog fragment every fix gets.

### Open questions and possible follow-ups

- Possible follow-up, recommended but not urgent: give a finished tab's first automatic close the same backoff when its
  first sweep fails (mark it for retry instead of leaving it to be retried on every pass). This is a few lines in the
  same function, plus a test shaped like this plan's third. It can wait because it costs only repeated cleanup attempts
  while a sweep keeps failing, which is rare, and loses nothing.

### PRs

- #5087 fix: keep a failed tab close retryable until its last sweep succeeds

### Checks

Run now:

- `cargo fmt --all -- --check`; `cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`;
  `cargo clippy -p farhelm --bins -- -D warnings` (the shipped build, without test hooks); the test sleep check (0
  delays without a reason); `dprint check` on the changed docs; `python3 releasing/check-changelog.py format`.
- Recorder run `816ce3f7-76a8-49c6-b4bc-96c52ab1494d`: the three new end-to-end tests plus the existing test of a failed
  host end being retried after its backoff: 4 passed. The new tests: a close whose final sweep fails keeps the tab
  hidden with its record until the sweep succeeds; a close whose host end and final sweep both fail names both failures
  and does not say the processes were reaped; a failing automatic retry pushes the next one at least 1.8 seconds later.
- Recorder run `ce6b3965-d698-4add-a345-36a074934723`: the new tests with all three fixes reverted on purpose: all three
  failed, each at its own check, so they tell the fixes from their absence.
- Recorder run `396d5706-1ab9-4a8f-84ce-fd1148eebe8f`: every existing test of terminal tabs (opening, closing, automatic
  cleanup, tabs across supervisor restarts and with Stop and Delete): 47 passed, no runtime skips. It ran before the
  review's test and documentation changes, which did not change the close's behavior.

Reused after the rebase: all of the above ran on the pre-rebase revision of the PR's commit; the rebased revision on the
PR differs from it only by the launcher plan that landed meanwhile, which changed the desktop launcher and its docs
only, nothing the tab close touches.

Skipped: the full workspace battery and the desktop, GUI and installer suites; the change is confined to the
supervisor's tab close, which the selections above cover.

### Review gate

One fresh-context Opus 5.5 reviewer at high effort. Round 1 found the three fixes correct and the new test-only failure
hook shaped like the existing ones and absent from the shipped build. It found SPEC.md claiming such tabs stay hidden
(true only when the host ended), a timing race in one test, comments citing the wrong SPEC section, out-of-date notes on
the retry fields, and the pre-existing first-close case above. All but that last one were fixed. Round 2: no remaining
findings.
