## What this was about

The November 24 supervisor triage covered 31 findings about sessions and terminal tabs. Delete could terminate a tab's
shell without letting it save its history when the process table hid its Farhelm markers. Delete now sends a hangup to
the known shell so it can exit cleanly. A missing socket could make the supervisor forget which programs to check and
leave running programs behind. Recovery now keeps the saved process identities while the terminal host or its child
could still be alive. A refused Restart preserves the failed launch's record until it can confirm cleanup, so another
attempt can still find the processes.

The six drafts address 21 findings. Three went to TODO.md, six closed under the small-fix criteria, and one had already
been fixed on main.

## Things you should know

- Linux coverage includes a markerless tab process that ignores TERM and exits on hangup, and a private interactive Bash
  session whose history contains the command it evaluated before Delete. Native macOS and Zsh history persistence remain
  unverified.
- Terminal triage had already fixed the alternate-screen Stop test on main. It waits for the fixture's exit handler and
  requires both frame labels before Stop. This stack closes the duplicate finding and reruns that test on the rebased
  revision. [PR #4862](https://github.com/scode/farhelm/pull/4862/changes)
- The helm and Hosts findings needed test-only changes. The helm fixture counts an already-readable duplicate create
  request. The Hosts fixture checks the real Suspend item for the other host and the inline Resume button on the host
  with a launch pending. It verifies click and focus behavior. An explicit disabled state in accessibility metadata
  would require wiring through the shared controls.
- The environment Delete test now holds directory admission alone before trying Delete. The settled-create test still
  covers the session count. This is the overlap with the environment lifecycle plan. Terminal triage also added a late
  host-loss assertion on Restart's reply and updated how the terminal draws and navigates. The rebase retained those
  changes and the terminal plan's separate follow-up about a program outliving its terminal host.
- Earlier helm drafts landed during validation. Agent-approved Stop now checks that the host is still on the connection
  the user reviewed. Ordinary Stop still reaches the same supervisor operation. The Hosts menu now keeps Re-run and
  Remove tied to what the user saw. The rebase preserves those rules and the helm plan's separate archived decisions.
  The two plans exercise the same Hosts menu and helm test harness.

## Open questions and possible follow-ups

The three cleanup fixes below are in TODO.md. The triage decisions called for preserving enough evidence to retry. Each
needs a broader change to retain that evidence safely, so the plan's complexity rule kept the remaining work moving.

- **Keep failed Restart cleanup retryable after session deletion.** A failed replacement can leave processes after the
  supervisor loses the session entry. A retry needs to remember the exact failed launch, account for a program that can
  still start, and leave alone any new launch that restored the same session. The attempted fix left gaps in both races.
  [PR #4868](https://github.com/scode/farhelm/pull/4868/changes)
- **Keep a failed tab close retryable after the terminal host ends.** A shell sweep can fail after Close ends the
  terminal host. Keep the exact cleanup target and retry deadline across a normal supervisor restart, with a truthful
  error if both host termination and the sweep fail. The scope selection currently comes from the terminal host's tag,
  which recovery loses once the host is gone. [PR #4875](https://github.com/scode/farhelm/pull/4875/changes)
- **Settle an unconfirmed agent start before Stop reports success.** A pending first stage can still start an agent
  after an authorized Stop has replied. Keep the user's Stop intent across a cleanup retry so the final annotation stays
  accurate, and report when cleanup succeeded but saving its outcome failed. The direct early sweep missed both cases.
  [PR #4878](https://github.com/scode/farhelm/pull/4878/changes)

## PRs and outcome dispositions

- Escape invalid IDs in cleanup warnings. Fixed Delete's directory-admission witness and the original-screen read after
  a cancelled seed. Corrected the held-start fixture's description of publication before host start returns. Deferred
  failed Restart cleanup after row loss to TODO.md. Closed the list-permit and borrower-handoff gaps without a change
  because each needs a separate test witness beyond the small test gate.
  [PR #4868](https://github.com/scode/farhelm/pull/4868/changes)
- Closed two findings. Fixed Delete's shell hangup for a pinned tab without readable markers. Separately, the
  interactive Bash test now verifies the actual command in saved history.
  [PR #4874](https://github.com/scode/farhelm/pull/4874/changes)
- Fixed test cleanup by guarding all three owned tab jobs when they start. Deferred retryable tab Close after a failed
  shell sweep to TODO.md. [PR #4875](https://github.com/scode/farhelm/pull/4875/changes)
- Fixed loss of saved agent identities during re-adoption with a missing socket. A Restart retry retires the gone
  current-generation host record only after confirmed process cleanup. Deferred authorized Stop of an unconfirmed start
  to TODO.md. [PR #4878](https://github.com/scode/farhelm/pull/4878/changes)
- Fixed six test witnesses. Recovery reads back the original saved host address after either supervisor exit, Stop
  checks the tree at the success reply, the replacement launch checks both old PIDs, a refused Open Tab proves it made
  no launch handoff, unconditional Delete preserves a session whose retained tab process still runs, and a resumed tab
  receives its final flood line. Closed two other test gaps without changes. The tab-only scope failure needs a fault
  that the test scope manager cannot inject. Proving a real stream pause needs a witness that the sender was parked
  while the producer wrote. [PR #4879](https://github.com/scode/farhelm/pull/4879/changes)
- Fixed six test witnesses. Check the already-readable duplicate create, the retained launch's usable retry key, the
  exact failed replacement stage, the other host's enabled Suspend action, the pending host's disabled inline Resume,
  and the resident launcher admitting and resuming a versioned Codex executable. Discarded the duplicate
  alternate-screen Stop finding because #4862 fixed it. Closed SQLite's stale-statistics test gap and the repo-cache
  cleanup item without changes. SQLite chooses which stale table to analyze. Repo-cache failures need to distinguish
  recoverable debris cleanup from a failed move of the selected cache. That change and fixture exceed the small polish
  gate. [PR #4881](https://github.com/scode/farhelm/pull/4881/changes)

## Checks

- At `a827a3af`, recorded
  `cargo nextest run --locked -p farhelm-supervisor -p farhelm -p farhelm-gui --lib --test e2e -E <selected lifecycle and Hosts cases>`.
  All ten passed without runtime skips, including the alternate-screen Stop case, the Restart reply after host loss, the
  recovered terminal address and the Hosts controls (`782c239e-4c61-46df-947f-2b1449140715`).
- The same revision passed supervisor library Clippy with `-D warnings`, Rust formatting, the delay audit, Markdown
  formatting, changelog format, instruction audit and whitespace checks. The instruction audit reported the existing
  reference to a renamed heading in the archive.
- At `a827a3af`, recorded
  `cargo nextest run --locked -p farhelm-helm -p farhelm -p farhelm-gui --lib --test e2e -E <four create and Restart cases>`.
  All four passed without runtime skips, including the resident versioned Codex launch and resume
  (`860559f1-482d-4f91-ab46-1e26332309e2`).

- At `b9d1952c`, recorded
  `cargo nextest run --locked -p farhelm-helm -p farhelm-gui --lib -E <five helm and Hosts cases>`. All five passed
  without runtime skips. This covers the pending-launch Hosts controls, the two stale-menu-press cases, duplicate create
  and the ordinary Stop route (`9b376fef-f3be-449d-b729-fb208450c25d`). Rust and Markdown format, changelog format,
  instruction and whitespace checks also passed.
- Compared the rest of the helm landing through `7ab8aa17`. Its checkout focus, Wayland follow-up, credential delivery
  and screenshot manifest changes use separate paths from these fixes and tests. The two rebases needed only shared
  bookkeeping conflict resolutions.

- Reused targeted `cargo nextest run` evidence where the rebased code and fixtures match the tested revision. This
  includes the two Delete admission cases and cancelled original-screen seed at `e455fc3f`
  (`056d26f5-bf93-429d-9cba-d42f8ffbea20`), the startup cleanup and held-start tests at `d1c85138`
  (`18b0162b-4a1e-4061-89d7-a511488a9120`), and the shell hangup and containment cases at `77ded0ca`
  (`b07ed74a-6db9-41a2-8d9b-35570fea3f34`).
- Reused the Bash history case at `a074f8ef` (`72dbbd5c-e67f-4c17-87ac-22699f9b36e1`), Close/Stop/Delete fixture cleanup
  at `f2e85acc` (`d1434b59-d5f0-4cad-9611-67ae6e5f6258`), plus confirmed cleanup and recovery cases at `a5ba3531`
  (`1000d06b-e900-4e16-b605-0b7d628f8805`). The later Open Tab, PID and recovery witnesses use `9c5adba5`
  (`77cb8d6d-f50d-4cbd-b24a-567505956896`) and `00aa30e1` (`3d3b9107-2eeb-4f48-9645-838e766ce6ac`).
- Skipped the full test suite in favor of the affected lifecycle and control cases. The Hosts change is test-only.
  Production drawing and golden references came from the terminal stack already on main, so this plan needed no new
  goldens or visual review.

## Review gate

All six drafts passed independent Sol high review and a cold read of their PR wording. Review narrowed the Restart and
Stop work to confirmed cleanup paths, strengthened the process and output witnesses, and put the broader lifecycle
changes into the three follow-up entries. The first Hosts run (`bd61f176-5a49-42b9-9346-7df2519f0448`) failed because
the controls do not set the accessibility flag used by the assertion. The exact Hosts rerun at `d6eb80a0` checked the
real click and focus targets and passed (`4f979881-a7a8-4e80-989e-5f2cbce1f603`).

### Landing

#### What landed with it

Only this plan landed this round, through its six PRs and the landing fix
[#4887](https://github.com/scode/farhelm/pull/4887/changes). Since the executor based it, main gained the last five PRs
of the helm triage plan, which landed in the previous round: credential delivery failures scoped to the saved credential
(#4873), launcher focus after a checkout folder save (#4867), a TODO entry deferring a Wayland paste fix (#4869), a
credential test (#4876), and the screenshot scene list (#4882). None of them touches the supervisor, its cleanup or
recovery code, or the tests this plan changed, and the landing's review found no interaction. Rebasing onto them
conflicted only in the triage ledgers and the review queue's index, where both plans had moved different outcomes to the
archive; I kept both sides, and a script check confirmed the index matches the findings on disk and no outcome is in
both the ledger and the archive.

The landing's review read the whole diff against SPEC.md and SPEC_impl.md and found no bug: Delete's new hangup for tab
shells reaches only shells the cleanup was already going to end, matched by their recorded process identity, so a reused
process number never gets it; a session whose terminal host's socket vanished keeps its unavailable terminal only while
its host or agent is confirmed alive or cannot be checked; a retried Restart keeps the current launch's record until the
cleanup of its processes and their systemd scope is confirmed, across supervisor restarts too; and a malformed session
id is now escaped in the restart cleanup's log lines. It counted no blockers, two should-fix test gaps and six small
points.

#### Fixes made while landing

- **Blank lines left in the review queue's index.** Each of the plan's PRs removed the index entries of the findings it
  settled but left the blank line that had separated each entry, so the index of open review findings collected 31 extra
  blank lines and the Markdown formatter's check failed on it. #4887 removes them and changes nothing else. Main carried
  the extra lines only for the minutes between the plan's first merge and #4887's.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs rebased onto main as it stood at the start of the landing (`8db5ba50`), with the ledger and index
conflict resolutions; the rebased stack's top commit was `7d8e6e4f`. The only later change is the landing fix
[#4887](https://github.com/scode/farhelm/pull/4887/changes), which removes blank lines from the review queue's index;
after it I ran `dprint check` again, and it passed. That final tree is what main now holds outside the plans queue.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `f167b978-868f-4932-9a57-7a3c6af2ec6d`): all 8655 tests passed, with no runtime skip. The plan changed the
  supervisor's service core, cleanup, teardown and terminal host recovery, and many of the end-to-end tests that drive
  them, plus one helm test and one test of the desktop app's Hosts views; the whole suite was the narrowest run that
  covered them.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no
  errors; one warning, about an old citation in the triage archive, already on main) and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: failed on the review queue's
  index, as above, and passed with the landing fix.

Reused: none. The report's own runs covered the same code before the rebase, and everything they covered ran again
above.

Skipped: the desktop app's screenshot comparison and end-to-end smoke, the tests and two clippy runs of the desktop
app's outer shell (which the workspace suite leaves out), since the plan changed nothing the desktop app draws or does;
its one change on that side is a test of the Hosts views in the app's interface code, which the workspace suite ran; the
workspace doctests, since no documentation example changed; a Mac run, though the tab hangup fix is aimed at macOS 26
and later (its new test runs on Linux with a shell that hides the marker the same way); and CI, which runs only on
demand.

#### What the report says that is no longer true

- The report lists the Markdown formatter's check among the checks that passed. The blank lines above are in the plan's
  own PRs as the executor last pushed them, before the landing rebased anything (the same 31, growing PR by PR), so that
  check fails on the plan's final revision and cannot have run on it.

#### Things you should know that the report does not say

- **Two of the plan's new rules are not fully covered by tests; I recommend a follow-up for both.** The code is correct
  today, by the review's reading.
  - Delete now hangs up tab shells that hide Farhelm's marker (zsh on macOS 26 and later does), so they can save their
    history before they are ended. The new test calls the cleanup step directly. The one end-to-end test with such a tab
    shell makes the shell ignore hangups, so it cannot tell a hangup from a kill; if Delete stopped passing the tab
    shells along, every test would still pass. The review's sketch: a tab shell that writes a file when hung up, then a
    Delete, then a check for the file.
  - A session whose host's socket vanished while the host still runs now keeps its terminal as unavailable rather than
    dropping it. The new test covers keeping it; nothing covers dropping it once the host and agent have really ended.
    If that half broke, a session whose host had ended would stay unavailable for good, with Stop and Restart refused
    until Delete. About ten lines in the same test would cover it.
- **A session whose host lost its socket but is still running now stays unavailable until the host ends.** Stop and
  Restart are refused for it meanwhile, since Farhelm cannot confirm what the host is doing; Delete still works. That is
  what SPEC.md's rule that what cannot be examined counts as unconfirmed asks for, but the plan's changelog draft
  describes this fix in internal terms and leaves the visible change out, so release curation should add it.
- **One finding closed without a change is now recorded nowhere.** The cleanup of cached repository copies stops at the
  first repository whose cleanup fails, skipping the others. The plan closed that finding without a fix, within your
  triage decision that allowed closing the remaining small items, because the fix and its test would have gone over
  their size limit of about 30 lines: per the report, a real fix must tell a leftover that can be cleaned up later apart
  from a failed move of the selected cache. It has no queue or TODO entry now. Ask for one if you want it tracked.
- **Small wording and comment points the review noticed.** One SPEC_impl.md paragraph on Restart uses "earlier launch
  record" in two senses two sentences apart. The new SPEC_impl.md paragraph about Delete's hangup sits among the
  paragraphs on systemd scopes rather than next to the one about hangups. A code comment on Restart's consent check
  still says the records are removed on that path. In the cleanup code, one comment does not mention the new tab hangup
  and a rewritten one dropped the note that the cleanup's target limits which processes it may follow. One of the new
  TODO.md entries is not separated from the one before it by a blank line, as its neighbors are.
