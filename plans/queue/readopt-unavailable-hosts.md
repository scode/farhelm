# Re-adopt terminal hosts that become unavailable while the supervisor runs

Written against main at 856099b8 on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **A terminal whose terminal host becomes unavailable stays unavailable until the supervisor restarts.** When a running
> supervisor loses the use of a terminal host, the terminal stays unavailable for the rest of that supervisor's life
> even after the cause has passed: the session lists with an unknown status, its terminal does not attach ("the terminal
> host is unavailable: ..."), and its verbs refuse (a prompt is refused, a guarded Delete cannot tell and refuses),
> while the host and its program keep running untouched. Only the supervisor's next start adopts the host again. A
> runtime becomes unavailable mid-life in `crates/farhelm-supervisor/src/hosts/terminal.rs` when its connection ends and
> the reconnect fails (a timed-out, closed or refused connect, `Unavailability::Connect`), when the connection ends
> `RECONNECT_LIMIT` (3) times within `RECONNECT_WINDOW` (60 s) (`Unstable`), when a checkpoint's encode task is lost
> with the emulator (`EmulatorLost`), or when a request fails (`Request`, `ChildUnpinned`); `Unavailability::may_pass`
> already marks these reasons as able to pass. Retrying while the supervisor runs (`Supervisor::start_readoption`,
> `readopt_pass` and `retried_placeholder` in `crates/farhelm-supervisor/src/service/core/host_handover.rs`, module docs
> "Adopting again") looks only at startup placeholders (`HostTerminal::unavailable`, `is_placeholder`) held as a
> session's agent terminal. It never looks at a runtime that ran, and never at tabs: a tab whose host startup could not
> use becomes a placeholder (`HostTab::placeholder`, `crates/farhelm-supervisor/src/service/core/host_tabs.rs`, "After a
> restart") that lists the tab and refuses its verbs until the next start, and a tab runtime that becomes unavailable
> later stays so as well. Leaving mid-life retry out was a deliberate narrowing during the terminal-host work (a scope
> reassessment rated it acceptable but not needed), and SPEC_impl.md says so in "Across supervisor restarts" (the
> **Re-adoption** paragraph) and in the host connection paragraph ("Unavailable ends a runtime, not a host's chances");
> a fix updates both. Likely fix: let re-adoption also take agent runtimes that ended `Liveness::Unavailable` for a
> reason that `may_pass`, and tab placeholders and tab runtimes the same way, woken by the liveness notifier (an
> unavailable runtime is announced on it) as well as by `readoption_wake`. Keep what the placeholder retry guarantees:
> the session's lifecycle claim taken without waiting, the entry (or tab cell) still holding that very handle for the
> same generation, a scan of only the one socket the handle names so that no other runtime's host is taken over (exactly
> one runtime per host, the runtime module's "One owner"), the `stopping` fence checked under the session map before the
> swap, and the backoff (1 s doubling to a minute, never reset). Decide what `Unstable` should get: the reconnect limit
> exists so that a runtime does not fight whoever keeps taking the host over, and retrying it on the backoff brings that
> fight back, only slower. Reproduce with the in-process test host
> (`crates/farhelm-supervisor/src/hosts/terminal/test_host.rs`): call `drop_connection` three times within a minute on a
> session's host and check that the session stays unknown while the host still serves; `become_unreadable` will not do,
> since an unsupported protocol version is a reason that does not pass and is rightly never retried. The existing test
> that reads an unreadable host as unknown is in `crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs`.
> Not checked when this was written: whether Stop and Restart also refuse for such a runtime (only a prompt and a
> guarded Delete were confirmed).

### What planning found

NOTE: Line numbers are approximate, from main at 856099b8. Correct them to the truth as you go.

Three corrections to the entry:

- **Which reasons happen mid-life.** `Request` and `ChildUnpinned` do not. Both come only from `adopt_connected`
  (`terminal.rs` around 1762-1905), which runs when a runtime is made (startup, create, re-adoption). A reconnect's
  `resume` (around 3965-3982) turns an unpinned child into a client error that counts toward the reconnect limit. A
  runtime that ran can end with:
  - from `reconnect` (around 3876-3957): `Unstable`; a failed connect, read through `AdoptError::from` (around
    1172-1186) as `Gone`, `Busy` or
    `Connect(Timeout | Closed | NotAccepting | Io | UnsupportedVersion | Refused | Malformed)`; or `Replaced` when
    another process serves the socket;
  - from the run loop: `EmulatorLost` (around 2869-2961), and `Snapshot(_)` when a mid-stream snapshot record does not
    restore (around 3156);
  - from `shut_down`: `ShutdownUnconfirmed` and `Replaced`, only when a verb shuts the host down.

  `Busy` does not occur in practice (a take-over `Hello` is never answered with it, `client.rs` around 12-14). The
  entry's "refused" is ambiguous: `Connect(Refused { .. })` (the host answered `Hello` with an error) does not pass; a
  socket that refuses connections for the whole refusal window reads as `Gone`, or `NotAccepting` while a process still
  names it.
- **Four drops, not three.** `reconnect` checks the limit before each attempt and gives up on the fourth ending: the
  adoption's connection plus three reconnects must each be dropped, each after the previous reconnect has landed (wait
  on `TestHost::connections()` or `HostTerminal::reconnects()`; without that wait two drops can hit one connection).
  `a_connection_that_keeps_ending_makes_the_terminal_unavailable` in `hosts/terminal/tests.rs` (around 2657) does the
  same count.
- **An exited runtime can go unavailable.** `Exited` ranks below `Unavailable` (`terminal.rs` around 493-500), so a
  connection lost after the exit replaces it. A re-adoption replays the exit record and reads `Exited` again.

The entry's unchecked item, confirmed: for a runtime that went unavailable mid-life, every verb behaves as it does for a
startup placeholder, because `HostStatus::probe` (`terminal.rs` around 794-818) errors for any `Unavailable` and ignores
`is_placeholder`. Stop refuses (`handlers.rs` around 812-856), Restart refuses (`core.rs` around 12914-12919), a prompt
refuses (around 12446), a guarded Delete refuses (around 14666), and an unconditional Delete ends the host through
`end_launch_host` with the adopted process. Attach is refused with "...cannot be read (<reason>), and will not be until
the supervisor restarts" (`handlers.rs` around 1367-1385). A tab close is refused with a conflict (`host_tabs.rs` around
937-948) unless the tab is `end_failed`, and the tab reap never closes such a tab (`HostTab::finished` is false for
unattributed evidence, around 180-186). Status lists the session as Unknown.

`Unavailability::may_pass` (`terminal.rs` around 583-605):

- **passes:** `Connect(Timeout | Closed | NotAccepting | Io(_))`, `Busy`, `Request(_)`, `ChildUnpinned { .. }`,
  `Unstable`, `EmulatorLost`, `NotScanned(_)`;
- **does not pass:** `Connect(UnsupportedVersion | Refused | Malformed)`, `Snapshot(_)`, `Replaced`,
  `ShutdownUnconfirmed`, `Unidentified(_)`.

So the mid-life reasons that pass are `Connect(Timeout | Closed | NotAccepting | Io)`, `Busy`, `Unstable` and
`EmulatorLost`.

How the placeholder retry works today (`host_handover.rs` around 1218-1557): the task starts in the constructor when the
supervisor owns its state directory, runs one pass, then sleeps until the earliest due time or a `readoption_wake`
notification (sent today only after `serve`'s reload and when an orderly stop begins). `readopt_pass` gathers entries
that satisfy `retried_placeholder`, drops schedule entries no longer current (which resets their attempts), schedules
new ones 1 s out, and for each due one sets the next due time (`readopt_delay`, 1 s doubling, capped at 60 s) before
calling `readopt_one`. That takes the lifecycle claim with `try_claim`, checks the fence (`holds_placeholder`: no stop
begun, same generation, `Arc::ptr_eq` on the handle), scans only the handle's socket (`probe_placeholder`), and installs
under `host_adoption` then `sessions` after checking the fence again (`install_readopted`), aborting a runtime it made
but does not install. Nothing wakes the task when a runtime goes unavailable; the liveness notifier announces it, but
its single consumer is the ticker, whose `Wake::HostChanged` arm (`ticker.rs` around 1049-1056) drains the changes and
runs the reap.

Tabs live in `SessionCells::host_tabs`, a leaf `std` mutex shared across entry swaps (`core.rs` around 4704-4717). A tab
has no generation, but it has a stable `tab_id` and its handle, which is how closing finds it already
(`forget_host_tab`, `close_host_tab_claimed`). `names_tab` and `adopt_tab` (`host_tabs.rs` around 1341-1385) are the
existing identity check and adoption for a tab host.

### Acceptance criteria

- **Agent runtimes (PR 1).** An agent terminal whose runtime became `Unavailable` mid-life for a reason that passes is
  adopted again while the supervisor runs, once the cause has passed: the session lists its real status, the terminal
  attaches, and Stop, Restart, prompt and Delete work, with the host and its program untouched. The test host reproduces
  it with four gated `drop_connection` calls (`Unstable`) and with a connect that times out mid-life and then recovers.
- **Tabs (PR 2).** The same for a tab placeholder and for a tab runtime that became unavailable: the tab keeps its id,
  and closing it works afterwards. A tab runtime that ran also keeps its place in the tab strip and its scope. A startup
  tab placeholder has neither (it carries `created_ns = u64::MAX` and `scoped = false`, so it lists last with its scope
  unknown) and takes both from its host's tag when re-adopted, as `adopt_tab` does at startup. A tab whose host turns
  out to be gone is not left listed forever, and its record is not leaked.
- **Every placeholder guarantee kept**, for runtimes and tabs alike: the lifecycle claim taken without waiting; the
  entry (or tab cell) still holding that very handle, for the same launch generation (or tab id); a scan of only the one
  socket the handle names, so no host another runtime here holds is ever taken over; the `stopping` fence checked again
  under the session map before the swap; a runtime made but not installed is stopped.
- **The backoff per the Decisions below:** 1 s doubling to a minute, set before each try; in every `Unstable` episode,
  not only the first, the first try comes no sooner than the 60 s reconnect window after the runtime gave up; the
  attempt count never reset for a session and launch (or a tab), including after a successful re-adoption; and kept
  counts never make the retry task spin (see the schedule proposal).
- **Reasons that do not pass stay final**, as today; `become_unreadable` still produces no new connection and leaves the
  same handle in place.
- **Woken promptly:** a runtime that goes unavailable is scheduled without waiting for an unrelated wake.
- **An exited runtime** that then went unavailable is re-adopted as exited, with nothing recorded twice.
- **User-visible text and specs:** the attach refusal no longer tells the user that a passing reason lasts until the
  supervisor restarts; SPEC_impl.md says what is now retried (see the outline); SPEC.md needs no change.
- **Changelog and TODO:** a changelog fragment per root `AGENTS.md`, "Changelog fragments", kind `fixed`, added in PR 1
  and edited by PR 2. PR 2, the last code PR, removes the TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-11-03, on the planning page:

- **`Unstable` is retried,** with the first try no sooner than the 60 s reconnect window after the runtime gave up, and
  the backoff count never reset for that session and launch. Waiting out the window means a re-adoption never overlaps
  the episode that tripped the limit, and the never-reset count bounds a real fight with whoever keeps taking the host
  over to about one try a minute. The maintainer added, verbatim: "with complexith gate, if this somehow sprawls to
  become difficult block the plan". What that means for you is under Fallbacks, "The `Unstable` complexity gate".
- **After a successful re-adoption the backoff count is kept** for that launch (or tab); it does not drop back to 1 s.
- **Scope:** agent terminals and tabs both, as two PRs: agent runtimes first, then tabs.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort, every PR (see Review gate). If it cannot be
  reached when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute
  reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:**

- SPEC_impl.md: the host connection paragraph (around 1893-1895), "Across supervisor restarts" (the tab sentence around
  2375 and **Re-adoption** around 2380-2392), and the attach-refusal sentence (around 2466-2471).
- The runtime module's "One owner" (`terminal.rs` around 17-28) and the session map's lock-order notes (`core.rs` around
  4840-4853).
- Root `AGENTS.md`, "The live install is off-limits": never touch the maintainer's own Farhelm; tests make their own
  supervisors. Also "Sharing the machine with other agents", "Tests and validation", "Conventional Commits", "Changelog
  fragments", "TODO.md and FLAKES.md".
- `.agents/validation.md`, `.agents/test-authoring.md`, `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- **One predicate** replaces `retried_placeholder` and the copy in `host_attach_refusal` (`handlers.rs` around 1370):
  `Liveness::Unavailable { reason } if reason.may_pass()`, placeholder or not. Every passing reason is retried,
  `EmulatorLost` included (a deterministic encode loss then repeats at most once a minute).
- **The schedule.** The key becomes an enum, for example `Agent { id, generation }` and `Tab { id, tab_id }`. Today
  `readopt_pass` prunes the schedule to the keys being retried now, which is what resets the count; instead, a key is
  pruned only when its session and generation (or its tab) no longer exist, so the count outlives a successful
  re-adoption. Two rules then matter:
  - The next wake time `readopt_pass` returns is the minimum due time over keys being retried now, never over every kept
    key: a healthy runtime's kept entry has a due time in the past, and counting it makes the task wake at once, over
    and over. `readoption_backs_off_without_spinning` in `service/core/tests/host_terminals.rs` was written against
    exactly that spin; extend it or add a test that a pass made after a successful re-adoption returns no due time, or
    one in the future.
  - Whenever a key starts being retried (it was not retried in the previous pass), it is due at
    `now + readopt_delay(attempts)`, and when its reason is `Unstable` at
    `now + max(RECONNECT_WINDOW, readopt_delay(attempts))`. That makes every `Unstable` episode wait out the window, not
    only the first; a kept count of 1 after a quick success would otherwise give a 2 s retry, back inside the fight the
    decision rules out. Counting from when the pass first sees the key is enough, since that is never earlier than when
    the runtime gave up.

  No decay of the count after a long healthy run.
- **The wake:** the ticker's `Wake::HostChanged` arm looks at each change it drains and calls
  `readoption_wake.notify_one()` for a passing `Unavailable`. `Notify` keeps a permit, so no wake is lost. A second
  consumer on the liveness notifier and periodic polling were considered and rejected.
- **The probe is reused as it is.** `probe_placeholder` already reads only `old.socket()` and applies the startup
  identity rules there (the tag names the session's launch, or an earlier one of its agent's), and `install_readopted`
  fences on the handle and the generation; neither depends on `is_placeholder`. Use it for every retried agent handle,
  renamed if the name now misleads (`probe_retried`, say). Tabs use `names_tab` and `adopt_tab` the same way. A
  re-adoption reads the host from the start, as the supervisor's next start would.
- **Rejected: a tag and peer check for a runtime that ran** (the found host's tag equal to `old.tag()`, its peer equal
  to `old.adopted_process()`, a mismatch becoming `Replaced`). The runtime's own `adopted_peer` rule (`terminal.rs`
  around 4044) protects what a reconnect carries over, the journal position it resumes from and the process it reports
  as the one that may end its host. A re-adoption carries neither: `adopt_found(.., Adoption::Recovered, ..)` reads the
  host from the start and pins whatever process it adopts, as the next supervisor start would, with no peer comparison.
  The tag half adds nothing either: the runtime was adopted under the same tag rules at the same socket, socket names
  are drawn at random per launch, and "One owner" is met by scanning the one socket, not by the tag. Do not add it.
- **No wait for the old runtime's task to end before probing.** On the `Turn::Unavailable` path the old runtime holds
  its connection a little longer, until `settle_encoding` and `finish` return, but an `Unavailable` liveness is final:
  that runtime never reconnects, so a probe's take-over cannot start a fight, and the first retry comes at least 1 s
  later (60 s for `Unstable`) anyway.
- **A failed retry** swaps in a fresh placeholder with the new reason, as placeholders do today, so a reason that stops
  passing stops the retries; Delete stays sound through the launch record. A tab's replacement placeholder keeps the old
  `HostTab`'s `created_ns` and `scoped` (a placeholder otherwise carries `u64::MAX` and `false`).
- **A tab whose host is found gone** gets a handle that reads `Gone` (a small new constructor generalizing
  `HostTerminal::unavailable`, for example `HostTerminal::ended`), so the existing reap closes it: sweep, end, record
  removal. Dropping the tab would leak its record and survivors until the next start. The new handle carries the old
  handle's `adopted_process()`, so closing a gone tab still confirms that process ended before the tab is forgotten
  (`end_tab_host`'s gone path); without it the close falls back to ending the host by its socket.
- **`end_failed` tabs** are not re-adopted; the reap owns them. An agent runtime aborted by a failed unconditional
  Delete whose last liveness was a passing `Unavailable` would be retried, which is fine since the Delete retry then
  ends it through the new runtime; say so in the docs.
- **`settle_tab`** maps `AtRecordedSocket::NoAnswer` to `Unidentified` (`host_tabs.rs` around 1437-1449), so a tab whose
  interrupted start timed out at startup is not retried. Leave it, and list it as a known gap in the report.
- **The lifecycle claim** stays held through the probe, as today, so a concurrent Stop, Restart or Delete cannot act on
  a host mid-adoption. A user's verb can then wait up to the connect deadline (about 5 s), as it already can for startup
  placeholders.
- **The attach refusal** for a passing reason says the supervisor keeps trying, for runtimes as well as placeholders.
- **Testing the `Unstable` delay without a production setting.** Follow `readoption_backs_off_without_spinning`: drive
  `readopt_pass` with a test-owned `ReadoptSchedule`, assert that the pass returns a due time at least
  `RECONNECT_WINDOW` away, then make the retry happen through a `#[cfg(test)]` method on `ReadoptSchedule` that marks a
  key due now, or by calling the retry (`readopt_one`, made `pub(super)`) directly. The supervisor's own task schedules
  that key 60 s out, so it cannot interfere within the test, and `RECONNECT_WINDOW` stays a constant in production. If
  you still find a seam field (`SupervisorSeams`, `core.rs` around 943) simpler, it is small and does not trip the
  complexity gate.
- **Tests:** unit tests with the in-process test host are the gate. An end-to-end test that forces `Unstable` on a real
  host by repeated test-side take-overs is optional, worth adding only if it runs in seconds.

## Outline

1. `plan/readopt-unavailable-hosts/01-agent-runtimes`: the shared predicate, the schedule key (with the `Agent` variant,
   or both), kept attempts, the `Unstable` delay, the ticker wake, the attach message, a test-host seam that stops and
   resumes answering, the agent tests, the SPEC_impl.md host connection, **Re-adoption** and attach-refusal updates, and
   the changelog fragment.
2. `plan/readopt-unavailable-hosts/02-tabs`: tab placeholders and tab runtimes, the gone-reading handle, the tab tests,
   the SPEC_impl.md tab sentence and the tab part of **Re-adoption**, the fragment edit, and the TODO.md entry removal.

Files likely touched:

- `crates/farhelm-supervisor/src/service/core/host_handover.rs`: module docs ("Adopting again"), the predicate, the
  schedule and its key, the `Unstable` delay, gathering tabs, the probe (reused, perhaps renamed), and the docs of
  `ReadoptSchedule`, `start_readoption` and `readopt_pass`.
- `crates/farhelm-supervisor/src/service/core/host_tabs.rs`: the tab probe and install (or shared ones), module docs
  "After a restart", the `HostTab::placeholder` and `HostTab::host` docs.
- `crates/farhelm-supervisor/src/service/ticker.rs`: the `Wake::HostChanged` arm, module docs, the variant's doc.
- `crates/farhelm-supervisor/src/service/core.rs`: the `readoption_wake` and `host_liveness` docs.
- `crates/farhelm-supervisor/src/service/handlers.rs`: `host_attach_refusal`.
- `crates/farhelm-supervisor/src/hosts/terminal.rs`: module docs ("Unavailable is final for a runtime, not for a host",
  around 269-279), the `may_pass` and `is_placeholder` docs, and the new constructor.
- `crates/farhelm-supervisor/src/hosts/terminal/test_host.rs`: the stop-answering seam (accept but never send `Welcome`,
  or stop accepting), which gives `Connect(Timeout)` mid-life.
- Tests in `service/core/tests/host_terminals.rs` and the `host_tabs` tests module. Unit-test runtimes must be adopted
  from a stand-in at a socket in the state directory's host directory (`adopt_test_host_at(&socket_in_state(..), ..)` or
  `discoverable_host`): a stand-in in a plain tempdir is judged `Absent` by the scan, which would record an exit.
- `SPEC_impl.md` as listed above; `releasing/changelog.d/`; `TODO.md` in PR 2.

Tests to write, at least: an `Unstable` runtime re-adopted (four gated drops, premise asserted while the host serves); a
`Connect(Timeout)` runtime re-adopted after the test host answers again, with the ticker running on a long tick so the
wake is what triggers it; a non-passing reason left alone; the fence for a runtime (entry replaced, or a stop begun:
nothing installed and the made runtime ends, like `a_readoption_that_loses_its_fence_installs_nothing`); attempts kept
across a successful re-adoption with no spin afterwards, and `Unstable`'s first due time at least the window in a later
episode too; an exited runtime re-adopted without a second exit record; and for tabs, a runtime and a placeholder
re-adopted in place, a gone host closed by the reap with its record removed, and an `end_failed` tab untouched.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-supervisor`, `python3 releasing/check-changelog.py format`, `python -B scripts/check-test-sleeps.py` with the
interpreter `docs/test-sleep-check.md` prepares, and these `.agents/validation.md` selections through
`python3 scripts/record-test-run.py`:

- "host terminal switch points" and "host terminals across supervisor restarts", for both PRs;
- "terminal tabs in terminal hosts", for PR 2. It includes the end-to-end test
  `an_unreadable_tab_host_at_restart_is_listed_refused_and_deleted` (`crates/farhelm/tests/e2e/host_tabs.rs`, around
  1422), whose stopped tab host will now be retried in the background: each try holds the session's lifecycle claim up
  to the connect deadline, and the test's close and Delete wait for that claim. Its assertions should still hold, but it
  may run a few seconds slower, with more varied timing;
- "host terminal runtime" if `terminal.rs` changes beyond docs;
- optionally "reclaiming orphaned terminal hosts".

NOTE: Those selections filter by test name. A new test must match an existing selection's filter (the restart selection
names its tests one by one), or the same change updates that selection's filter and description and then runs
`python3 scripts/check-agents-md.py`.

## Fallbacks

### The `Unstable` complexity gate

The maintainer's words: "with complexith gate, if this somehow sprawls to become difficult block the plan". The
outline's machinery for `Unstable` is small: one `max(RECONNECT_WINDOW, ..)` where `readopt_pass` sets the due time of a
key that starts being retried with reason `Unstable` (see the schedule proposal), on top of the attempts kept per key
that the second decision needs anyway. Its test follows the existing backoff test's pattern, asserting the returned due
time and then making the retry happen directly, so no production setting is expected; a small seam field, if you prefer
one, is still within the outline. If building it, or testing it, turns out to need substantially more, do not push
through. Signs of that: changing the runtime's reconnect loop or its limit, carrying a give-up time out of the runtime
through new state, a new timer or task beside the existing schedule, reworking how seams reach the supervisor, or tests
that can only pass by waiting out a real minute.

When you see one, run the scope reassessment below. If its findings show a design within the outline's size, take it and
log the DECISION. If the work still looks like it sprawls, block the plan per `plans/.agents/executing.md`, "Executing
one plan" (step 10), after pushing what is consistent; do not build the larger version. The question states what the
smallest working `Unstable` handling turned out to need and why, and offers the options: leave `Unstable` final as today
while every other reason is retried; retry it on the ordinary backoff with the count never reset (it converges to one
try a minute after about six episodes, but the first few retries come quickly while a fight is on); or build the larger
version, with its cost. Say which you recommend.

### Other departures that need the scope reassessment

- changing the reconnect limit, the reconnect window, or which reasons `may_pass`;
- changing startup settle semantics (`settle_tab`);
- probing without the lifecycle claim, or a second consumer on the liveness notifier;
- backoff state that outlives the supervisor (on disk);
- an end-to-end test that runs for a minute or more.

Decisions to log: every planner proposal you change, the shape of the schedule key, how the `Unstable` test makes its
retry happen (and a seam field, if you add one), every test changed and why, and every review finding you decided not to
follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-readopt-unavailable-hosts-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/readopt-unavailable-hosts/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples, and the `Unstable`
  complexity gate, are under Fallbacks above; the examples are not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished. The `Unstable` complexity gate under
Fallbacks is one such block, agreed in advance.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
