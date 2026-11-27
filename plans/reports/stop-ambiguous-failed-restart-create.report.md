# Report: Stop's refusal on an unconfirmed launch now says what happened

## What this was about

#5091 (not yet released) changed Stop for a session whose latest launch was never confirmed and that has no main
terminal recorded: rather than record an exit it never saw and report success, Stop now refuses and points to Delete, or
to Restart where the session offers it. That state comes from three places: a create that was interrupted, a create
whose host reported an error after its terminal had already started, and a Restart that failed without knowing whether
its new agent started. The refusal began "this session's start was never confirmed, so there is no terminal for Stop to
stop and nothing was stopped …". That is wrong for the second and third cases (a terminal may exist without being
recorded, and a Restart has a confirmed start behind it), and it never said that the session's open shell tabs, which an
ordinary Stop closes, were left alone too.

When the plan was written you decided to keep the uniform refusal rather than teach Stop to act in those cases, and to
fix the wording.

What is on main now:

- The refusal reads: "this session's latest launch was never confirmed, so Stop cannot tell what is running and nothing
  was stopped, open terminal tabs included; its agent may still be starting or running. Delete the session to end
  whatever its launch left, or Restart it if Restart is offered".
- The specification's Stop section says the same: a session in this state is refused whoever asks, with nothing changed,
  its open tabs included, because with no terminal to probe Stop cannot tell what is running.
- The changelog entry #5091 left (unreleased) is reworded to match, and now says plainly which cases Stop used to handle
  and no longer does (next section).
- The tests that check the refusal's words are updated, and the TODO entry is removed.

## Things you should know

- **Two cases Stop used to handle are refused, and the changelog entry now says so.** #5091's entry mentioned only an
  interrupted creation and said nothing about these. My first rewording made it worse by listing a failed Restart among
  the things fixed, and the review caught it: Stop used to work after a failed Restart (it ended the new launch's
  processes and closed the tabs), and after a create that failed once its terminal had started. The entry now says both
  are refused, and how to get out: after a failed Restart, Restart again and then Stop; otherwise, and after such a
  failed create, Delete.
- **Nothing in the app depends on the old words.** The desktop app, `farhelm agent stop` and the docs only show the
  message; nothing matches on its text, so the rewording needs nothing else.
- **No new test that a refused Stop leaves open tabs alone.** The refusal returns before any tab code runs, which the
  review confirmed by reading, so I did not add one.

## Open questions and possible follow-ups

None.

## PRs

- #5141 fix: word Stop's unconfirmed-launch refusal accurately (the message, the code's notes, the spec sentence, the
  changelog entry, the tests, the TODO removal).

## Checks

Run during this plan, through the test recorder:

- Run `e34cb134`,
  `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(stop_refuses_an_unconfirmed_terminal_less_launch_whoever_asks) | test(stop_refuses_an_unconfirmed_start_and_leaves_it_for_delete)'`:
  2 passed. These are the supervisor's test that Stop refuses such a launch for every caller with exactly this message
  and records nothing, and the end-to-end test that Stop of a session whose create was interrupted refuses with words
  saying the launch was never confirmed and nothing was stopped, keeps the launch for Delete, and that Delete then ends
  it. It ran after the review fixes.
- `cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test
  sleep check, `dprint check` on SPEC.md and TODO.md, and the changelog format check: all clean.

Run now, on the change as it lands: `cargo check -p farhelm-supervisor -p farhelm --all-targets`, clean.

Reused: run `e34cb134`, from before the final rebase. What landed in between (the brick test's Welcome instructions, a
Claude drift test, the supervisor ending a failed Restart's leftover terminal host when it starts, and the removal of an
unused helm route) touches neither Stop nor its refusal.

Earlier runs, superseded: `b6469935` (the same two tests, 2 passed, before the review fixes changed the message).

Skipped: everything else, since only a message, comments, the spec, the changelog, the two tests above and TODO.md
changed.

## Review gate

A fresh-context Opus 5.5 review at high effort found no blocker. It confirmed by reading that the refusal comes before
anything is recorded, signalled or closed in all three cases, and that nothing matches on the message's text. Its three
should-fixes are all applied: the changelog caveat above; the spec still saying "start" where it now lists a Restart
case (now "latest launch"); and a code comment above the refusal that kept the old wording. Its two small points were
fixed too: the message now says the agent "may still be starting or running", matching Delete's own refusal for the same
state, and the end-to-end test's description was rewrapped. The fixes were not reviewed again. Run `e34cb134` ran after
them.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the previous round: ntfy alerts for waiting and blocked agents,
Restart of a Claude session before its first prompt starting Claude fresh, a recipe's Delete question staying open on
Keep editing, and an exited terminal tab's failed cleanup backing off. The Claude Restart change touches Restart, but
neither Stop nor the refusal this plan rewords. In the same round, before this plan, three more landed:
helm-replace-refuse-destination-change (the helm refuses a **Replace with** request that changes folder or checkout),
agent-delete-notices-bell-alignment (wording of an agent Delete's cleanup warnings) and marked-right-click-cancel-focus
(focus after cancelling a bulk Stop or Delete). None touches this refusal.

An independent reviewer and I read the change and found nothing to fix.

#### Fixes made while landing

- **A TODO.md conflict.** The round's four plans each removed their own TODO entry, next to each other, so stacking them
  conflicted there. I removed all four entries; no code changed.

#### What the landing made untrue in the report

Nothing untrue. The report's "What is on main now" list was written before landing; it is accurate now. The refusal
still applies after a failed Restart, whether that Restart was resuming or, since the previous round's Claude change,
starting fresh.

#### Checks

Run once, on the round's four plans stacked on main as it stood when the landing started
(helm-replace-refuse-destination-change, agent-delete-notices-bell-alignment, marked-right-click-cancel-focus, then
stop-ambiguous-failed-restart-create), so each plan was tested together with the others and with the previous round's
changes, which none of the executors' own runs included. The four plans touch the helm, the supervisor, the command
line's end-to-end tests and the desktop app's session list, so the whole Rust test suite ran. The SSH test worker was
still unreachable, so the slow suites ran on boat sandboxes (x86_64 Ubuntu 24.04 VMs), and clippy ran locally:

- Locally, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent` (the build with the desktop app's
  test-automation hooks), all with `-D warnings`: clean.
- On three boats, every test of the workspace and the desktop app that runs by default (`cargo nextest run --workspace`,
  built once, split in three and run through the recorder), recorded runs 2021dc06-f8a9-499f-a932-5b205c06b17a,
  830feae8-2980-4f70-8213-1864c2a3a428 and ce9cbcb2-c7c1-45e1-bcea-70e2a57a9224: 9,186 passed, one failed, and no test
  that quietly passed for lack of systemd, Podman or SSH on the machine. The one failure is the test-trace macro's
  compile check, which builds a scratch crate offline from the machine's own download cache; a boat has no such cache,
  so that test always fails there, as `.agents/boat.md` records, and no plan in this round touches it. Run locally
  (`cargo nextest run -p farhelm-testtrace-macros --test compile_contract`), recorded run
  aa311d19-8ca8-4224-b6ca-8af001538521, it passed.
- On a fourth boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  5692442c-6eba-4f7a-84ea-a783f3917c82: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run. Its steps include the session row's menu, Replace, Stop, Delete and Mark read; none uses marked sessions.
- Locally, the full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run
  852c4855-d075-47b1-8cbb-dd08f75e9f93: passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown: passed.

Reused: nothing. Skipped: the opt-in tests, among them the real-agent drift tests, which spend vendor turns; the
installer and provisioning suites, since no plan in this round touches them; and any run on a Mac, since none was
available to the landing.
