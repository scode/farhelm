## What this was about

The desktop launcher treated a suspended sandbox or container like any other disconnected host. The refusal left you
without a way to tell why the launch was refused or what to do next. The session card used generic wording while a
provider was suspending or resuming a host. Clone hid some characters in a copied folder or session name. This plan
closes those gaps and records the disposition of the remaining launcher cases from the browser suite.

## Things you should know

- The released browser app resumed the host as part of Launch. On desktop, Launch and quick launch now name the host and
  ask you to resume it in Hosts. A resumable container can show “Stopped” in Hosts while the launch refusal says the
  host “is suspended”. As agreed for this plan, automatic resume during launch stays a separate Near term item.
- The unavailable session card names Tensorlake or the container host while it waits for the provider. Clone spells out
  hidden characters in copied fields, including the numbered default name for a managed checkout. A launch with the copy
  still untouched sends the original characters.
- The desktop checks cover checkout discovery and host selection, refusal help, focus during launch completion, and
  destinations restored from templates. Replace with can retry the same draft after a missing-folder refusal. If its
  source disappears before the reply, success opens the replacement when the source was open. A refusal shows the error
  and falls back to the last visited session still present.

## Open questions and possible follow-ups

- The browser launcher rechecked its checkout setup when settings changed elsewhere while it was open. The desktop
  launcher lacks that refresh trigger. Restoring it would change behavior and give the old external-change case a
  desktop counterpart.
- A test with replies that can finish out of order could verify that a launch completes on another host before Suspend
  finishes, and that checkout setup on a newly selected host settles before its old preview. The ordered fake helm lets
  the desktop tests verify that both later requests are sent.
- A test with a real host and folder could verify that New, Clone and Replace with recover when the original path is
  repaired. The Replace with test scripts a successful retry with the same draft. The shared New and Clone create test
  retries after editing the folder.

## PRs

- Launcher and quick-launch advice for suspended hosts. [#4056](https://github.com/scode/farhelm/pull/4056/changes)
- Provider wording on the unavailable session card. [#4058](https://github.com/scode/farhelm/pull/4058/changes)
- Spelled-out hidden characters in Clone fields. [#4059](https://github.com/scode/farhelm/pull/4059/changes)
- Checkout composer and host picker tests. [#4060](https://github.com/scode/farhelm/pull/4060/changes)
- Restart with hover, footer and template refusal tests. [#4061](https://github.com/scode/farhelm/pull/4061/changes)
- Launcher tabs, prompts and agent choices tests. [#4062](https://github.com/scode/farhelm/pull/4062/changes)
- Focus and selection during launch completion. [#4063](https://github.com/scode/farhelm/pull/4063/changes)
- Launcher search, keyboard navigation and recents. [#4064](https://github.com/scode/farhelm/pull/4064/changes)
- Launcher placement and host availability tests. [#4065](https://github.com/scode/farhelm/pull/4065/changes)

## Checks run, reused and skipped

- On the final stack at `086342d2`, `cargo fmt --all -- --check`,
  `cargo clippy -p farhelm-gui --all-targets -- -D warnings` and 19 tests passed. The run used
  `cargo nextest run -p farhelm-gui -E <launcher and environment cases>` through `scripts/record-test-run.py` with
  selection `last-mile combined launcher and environment paths` and recorder run `517ffe2e-a03b-46a3-b1fa-fb4f19bcb53b`.
  This includes the three #4065 cases for restored template placement, the selected host disappearing and a create sent
  during Suspend. It also covers the earlier suspension advice, Clone text, refusal, focus and destination interactions.
- The same worker commands passed each earlier draft's focused selection. The pre-rebase checks remain applicable
  because the rebase preserved each patch and main changed only plan records, documentation, capture scripts and a
  separate test runner. These are the per-draft recorder runs and their covered revisions:

  | Draft | Revision   | Passed | Recorder run                           |
  | ----- | ---------- | -----: | -------------------------------------- |
  | #4056 | `a41bbc3d` |      2 | `87a224c6-cc82-45ee-9201-df1f6a36ffff` |
  | #4058 | `dceed4bf` |      3 | `c836c0cb-bb27-48d8-aba8-36c92b12e752` |
  | #4059 | `a15af27b` |      4 | `2ab9af56-69f6-4eeb-9204-11c24401ebfe` |
  | #4060 | `e80e339`  |      6 | `1289b16e-9e67-4b51-9e6b-5c1315b0c4c1` |
  | #4061 | `56329c50` |      5 | `85953613-f102-4e3d-acdd-73c6700969e0` |
  | #4062 | `f78f339c` |      4 | `82851a96-6fb6-4a61-8449-816f01b0ca6f` |
  | #4063 | `bd994131` |      7 | `5afc97e8-c5f3-4014-a4ee-e0dda85d9f5b` |
  | #4064 | `4241cbe4` |     14 | `12c74642-c36d-4028-8743-7a3fcb1b6032` |

  #4056 had a later doc comment and parity wording edit with no effect on runtime. The isolated Clone and checkout
  selections were reused because the intervening changes covered separate launch paths. The final stack check exercises
  the overlapping destination and refusal paths together.
- `dprint check` on changed Markdown, `git diff --check`, `python3 releasing/check-changelog.py format` and the
  source-only `python -B scripts/check-test-sleeps.py` passed. `python3 scripts/check-agents-md.py` reported one
  existing archive citation warning and zero errors.
- Native text input owns composing Enter. The model picker tests that Enter only chooses a model through the shared text
  input rule. The window tests check where focus stays after Clone or Replace with. Live terminal input is covered
  separately.
- Skipped the whole-workspace and native Mac suites. The changes use the shared desktop launcher and session-card paths,
  exercised by the focused GPUI tests on Linux. No native Mac integration or capture code changed.

## Review gate

Passed. A fresh GPT-6.2 Sol reviewer at high reasoning reviewed each draft, following the maintainer's override of the
plan's GPT-6.1 Sol reviewer. The refusal now keeps right-to-left host names from reordering the surrounding advice. The
reviews also caught a second place that could display Clone's copied name raw and tightened the tests for checkout
discovery, missing-folder retry, keyboard focus and environment selectors. The reviewer follow-ups confirmed the fixes.
The commit messages and draft titles also passed separate cold reads.

### Landing

#### What landed with it

Nothing else landed in this round. Between the stack's final base and the landing, main gained only plan-queue changes,
so nothing could interact with it (the report's list of what main gained covers the earlier interval before the
executor's last rebase). An independent reviewer read the whole stack against main and the specs. It found no launch
that the stack now refuses wrongly or lets through wrongly, other than the intended change for a suspended or stopped
sandbox or container. It also confirmed that an untouched Clone still launches with the original characters, since only
the field shows them spelled out, and that the launcher and quick launch give the same refusal, with the host's name
kept from reordering the surrounding advice. The fixes below came from that review; I checked each against the code
before making it, and the checks further down ran after them.

#### Fixes made while landing

- **A stopped container was called "suspended".** The new launch refusal said "<host> is suspended. Resume it in Hosts,
  then launch." for every sandbox or container Hosts offers Resume for, including a stopped container, which Hosts
  labels Stopped. The stack had also deleted the spec's rule that Stopped is never labelled Suspended, to make the spec
  match. A stopped one now says "<host> is stopped. Resume it in Hosts, then launch.", the rule is back in the spec, and
  the refusal test covers both wordings (#4056). This makes the report's first "things you should know" bullet (Hosts
  showing "Stopped" while the refusal says "is suspended") out of date.
- **The spec described launching on a suspended host two ways.** The stack added "for now the desktop app refuses a
  launch on a suspended host", but three older passages still described the launch resuming the host as current behavior
  (the launcher showing "Resuming" while it wakes the host, waking a suspended sandbox, and fresh GitHub checkouts on a
  suspended host). Each now says it applies once launches resume a suspended host. The rule that Farhelm never suspends
  a host by itself had been left under the future paragraph and is back with the current behavior (#4056).
- **The changelog's Clone sentence promised too much.** It said Clone keeps the original values for launch; that holds
  only while the fields are unedited (after an edit, what the field shows is what launches, and a test asserts that). It
  now says so (#4059).
- **One entry in the port tracker miscounted its cases** ("four cases are named below" above five) (#4065). The port
  tracker is `docs/gpui/parity/`, which records, for each test of the old browser app, how the desktop app covers it,
  under a status such as "partly", "replaced" or "not yet ported".

#### Things you should know that the report does not say

- **Restart with on a suspended or stopped host now reads like a launch refusal**: "Restart error. <host> is suspended.
  Resume it in Hosts, then launch." (or "is stopped"). It used to say the source host is unavailable and to wait for it
  to reconnect, which a suspended host never does by itself, so the new advice is the more useful one, but its last word
  says "launch" rather than "restart". The plan scoped the new refusal to New, Clone and Replace with, and no test
  covers Restart with's case. I left it; a per-mode wording is a small follow-up if you want one.
- **A few port tracker entries may understate their status.** The reviewer read the entries for the old launcher's tabs,
  launch refusal, named conversation and (possibly) Enter key tests as having nothing left to port while still saying
  "partly", and the entry for the guarded permission choices still lists the "About permission modes" button as "not yet
  ported" without the one-line reason the plan asked for. I did not change those statuses: that needs each old browser
  test's cases read against the desktop tests, which this landing did not do. Nothing in the app depends on them; if you
  want the tracker exact, that is a small documentation follow-up.

#### Checks

The executor ran only focused selections (19 tests on the final stack), so the landing ran the whole test suites of the
desktop app's GUI crates (the launcher, session list, session pane and the rest of the app's window) and the app's
end-to-end smoke. Every check below ran on the final stack, landing fixes included. Builds and lints ran locally; the
test suites ran on boat sandboxes (cloud VMs), from binaries built locally.

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `python3 -B scripts/check-gui-lints.py` and `python3 -B scripts/test-check-gui-lints.py`: passed.
- The check that a GUI change modifies the app's shared foundations only where they allow it
  (`scripts/check-foundation-paths.py`, from the base revision), on each of the nine PRs: nothing refused; it listed the
  expected changes to the shared provider wording, launcher view and host identity code.
- `cargo nextest run -p farhelm-gui -p farhelm-grid-model -p farhelm-input-mapping` from a locally built archive, split
  across two cloud VMs: 1571 and 1567 tests passed, none failed, none reported a runtime skip
  (`4641ef8d-cbc6-4eca-8c1c-e1fd6cabc690`, `0748fc3b-15de-44df-8c64-7a4c9b771515`).
- The app's end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py --no-build`, through the recorder) on a cloud VM: 49
  steps passed, the clipboard paste step skipped as it always is on Linux (`1d005254-01f7-4756-858d-f90e1182e009`).
- The app's screenshot goldens (`python3 scripts/check-gui-goldens.py`, through the recorder), locally: passed
  (`9fde380b-4c8b-4605-a228-3d2b10da9494`).

Reused: nothing. Skipped: the rest of the Rust workspace (supervisor, helm, terminal hosts), the desktop process's own
tests (its startup, updater and window frame), the Linux app lifecycle and the Mac-only checks, since the stack changes
only the app's launcher, session card and their tests.
