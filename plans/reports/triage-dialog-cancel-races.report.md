### What this was about

Several confirmation prompts in the app could act on an answer the user had just withdrawn. Clicking **cancel** and then
one of the prompt's other buttons can arrive as one burst of events, before the screen redraws to remove the prompt. The
second button's handler still had the prompt it was drawn with, so it went ahead:

- **The YOLO question when replacing a session from the session list.** The new session started with no approval prompts
  and the old one was deleted. "Start, and don't ask again on this host" also switched the host's question off.
- **The YOLO question in the session launcher**, including when the launcher was opened by **replace with**. The
  cancelled launch went out anyway. The launcher's answers were form submit buttons, so they sent the form whether or
  not the question was still open.
- **The YOLO question inside Restart with.** The agent restarted with no approval prompts, and was stopped first if it
  was working and the dialog was set to stop it. A "don't ask again" answer after cancel had no question left to name
  its host, so it went through as a one-off yes instead of being refused.
- **The feedback dialog.** Cancel or Escape right after **send** closed the dialog mid-send, losing the typed text and
  any word on whether the message arrived.

In triage you chose to fix these with the shared confirmation mechanism the session view already uses. That mechanism is
a slot an answer must claim and empty before it acts. You set a complexity gate, bouncing an item back if that approach
didn't fit or the fix turned complex. You also asked for a sweep of other prompts with the same flaw.

What landed:

- **The sidebar, launcher and Restart with YOLO questions** now act only by claiming the question they were drawn for,
  so an answer after cancel does nothing.
  - "Don't ask again" now also claims the question, so the question leaves the screen while the host setting is saved.
    It comes back with the reason if saving fails.
  - The launcher's answers are plain buttons that submit the form themselves, and only after claiming the question.
- **Restart with's question now belongs to the settings it asked about.** If you edit the settings after the question
  appears, an answer is refused with "the settings changed after the YOLO question; restart again to be asked about
  these settings".
- **The feedback dialog's cancel and Escape** check whether a send is in flight at the moment they run.
- **The sweep** found two more prompts with the same flaw. Both were already in the review queue, untriaged:
  - **Add host's setup question.** A "yes" queued behind cancel installed Farhelm on the remote machine.
  - **A host row's setup question** (setting up a failed host again). "Yes, and don't ask in the future" queued behind
    cancel turned the setup question off for every later host.

  Both are fixed in an extra PR at the top of the stack, which removes them from the queue. They never went through
  triage and have no outcome records: your triage decision for the sidebar question put sweep findings in scope under
  the same gate, so I fixed them here and noted it in that decision's record.

### Things you should know

- **The host row's setup question was not moved onto the shared mechanism.** The question shows the setup plan the helm
  prepared for that host. Its confirmation already checked, at the moment it ran, that this plan was still the one on
  offer and withdrew it, which is why a "yes" after cancel already did nothing there. Only the permanent answer's
  preference write came before that check, so the fix moves the write after it. The row keeps one pending plan for its
  setup, update and uninstall confirmations alike, so moving it onto the mechanism would restructure that panel, which
  your gate rules out. One reviewer saw this as substituting for the design you asked for; the other agreed it was the
  right call. Add host was moved onto the mechanism. Ask for a follow-up if you want the row moved anyway.
- **SPEC.md's wording is now slightly off.** Its sentence on the YOLO question says that if "don't ask again" fails,
  "the confirmation stays up with the reason". The question now disappears while the setting saves and reappears with
  the reason, so the end state matches but "stays up" does not. I left SPEC.md alone because these outcomes were "fix
  code". A one-word spec edit ("comes back with the reason") would settle it.
- **Restart with no longer approves settings edited after the question.** Before, an answer approved whatever the dialog
  showed when answered. The triage record asked for the answer to be bound to the settings being approved.
- **The shared YOLO question still has an option to make its answers submit buttons.** No prompt uses it any more. I
  left that shared code untouched so these fixes stayed inside each prompt and its parent, as your gate requires. No
  action needed; it can go in any later cleanup.
- **The Restart with dialog itself needed no change.** #1660 made the session view, which receives the dialog's answer,
  refuse an unclaimed one; #1661 only adds a browser test, so it is typed `test:` with no changelog entry, not the
  `fix:` the plan expected.
- **The launcher now submits through the browser's form submission call**, the same one its recent-launch Enter key
  already used. It ran in the browser tests on Chromium and WebKit (WebKit stands in for the desktop app's engine), but
  not in the desktop app itself. Worth a glance the next time you launch a YOLO session there.

### Open questions and possible follow-ups

- Whether to move the host row's setup question onto the shared mechanism (see above). My recommendation is to leave it:
  the behaviour is fixed, and the move costs a restructure of the hosts panel.
- Whether to reword SPEC.md's "stays up with the reason" (see above). I recommend the small edit.

### PRs

- #1658 `fix: stop a cancelled YOLO question from replacing a session`
- #1659 `fix: stop a cancelled launcher YOLO question from launching`
- #1660 `fix: refuse a cancelled YOLO answer in the restart with dialog`
- #1661 `test: cover a cancelled YOLO answer in the restart with dialog`
- #1662 `fix: keep the feedback dialog open while its message is sending`
- #1663 `fix: ignore a host setup answer queued behind cancel` (the sweep's two prompts)

Each fix carries a changelog fragment.

### Checks

Run ids are the recorder's retained test runs (`scripts/record-test-run.py`).

- **Run on the final stack after rebasing onto the latest main:**
  - the UI crate's tests for every touched module (138 tests, run e2ae61ee);
  - clippy for the UI crate.
- **Run on the stack before that rebase** (the rebase brought only bookkeeping and unrelated template fixes):
  - browser specs on Chromium and WebKit:
    - Restart with and the YOLO guard (46 passed, run 7962fdbc);
    - feedback and provisioning (112 passed, run 6fcda094);
    - the new burst tests again on their final versions (6 passed, run 947b2e19);
  - workspace clippy (all targets, the shipped binary, the desktop feature), fmt, dprint, the changelog format check and
    the test-sleep check.
- **Not verified:** I did not show each new browser burst test failing on the code before its fix, since that needs a
  rebuild per fix. The reviewers traced each test against the old code and expect it to fail there.
- **Skipped:**
  - the full Rust battery: the changes are confined to the UI crate's prompts, which have focused tests;
  - other browser specs: no other surface changed;
  - desktop runtime, installer and release checks: unaffected.

### Review gate

Each PR was reviewed by two fresh-context reviewers, Claude Opus 5.5 and gpt-6-astra, both at high effort. PRs 1, 2 and
6 (#1658, #1659, #1663) had a second round on their fixes.

- **The session list's YOLO question:** an answer that could not start yet (another operation holding the page, or the
  same row showing its own replace prompt) claimed the question and then did nothing, so the question vanished. Each
  answer now checks it can start before claiming the question.
- **The launcher:** an earlier version removed the shared question's submit-button option, which touched other prompts'
  files against your gate. That was reverted. The launcher also gained the browser burst test.
- **Restart with:** comment and test fixes.
- **The feedback dialog:** its burst test now waits for send to be enabled, and the Escape case closes with Escape.
- **The sweep PR:**
  - the add dialog's decision moved into a function with its own test;
  - the browser tests now watch the preference writes themselves.

Declined, with reasons recorded in the plan log:

- **Moving the host row onto the mechanism** (above).
- **A second check in the hosts panel that the add dialog is still open.** Every way of closing the dialog now empties
  its plan first.
- **Rewording the shared YOLO question's comments**, to keep that shared code unchanged.
- **Removing a harmless redundant state write on a stale launcher answer.**

### Landing

Landed on 2026-10-05 (UTC) as six squash commits on main, in stack order: #1658 (the session list's YOLO question),
#1659 (the launcher's), #1660 (Restart with's), #1661 (its browser test), #1662 (the feedback dialog) and #1663 (the two
host setup questions the sweep found). Nothing else reached main while they merged.

#### What else was on main

Nothing that could interact: between the commit the stack was built on and the landing, main gained only the planning
queue's own bookkeeping.

#### The sweep's two extra fixes

The two sweep fixes in #1663 address review items that never went through triage. Your decision on the session list's
YOLO question put a sweep "for other prompts that still guard by hand" in scope, under the same gate, and #1663 records
the sweep under that decision's entry in the triage ledger rather than adding entries of its own. Only Add host's
question was moved onto the shared confirmation mechanism; the host row's setup question was fixed by moving one write,
as the report explains.

#### Review before merging

A separate reviewer that had not worked on the plan checked the stack before anything merged. It found nothing outside
the six PRs that the changes break. In particular, the launcher's switch from submit buttons to submitting the form from
the answer's own handler is safe: only one launcher form is ever on screen, Enter in a field always went through the
Launch button and still does, the recent-launch Enter key uses the same path, and the tests that look for a submit
button only ever find Launch. The YOLO answers keep their hover text, and none of the docs screenshot, README image or
demo video scripts drives a surface the stack changed. The review-feedback queue's index matches its files, and each
removed item goes with its own index line.

It also found three things the landing did not change, which are yours to weigh:

- **A small new gap in the launcher.** An answer to the launcher's YOLO question now takes the question first and then
  submits the form. If another operation is already running on the page by then, the submit does nothing and the
  question is already gone, so the user sees it vanish with nothing launched. The answer's consent is kept, so the next
  Launch starts the YOLO session without asking again; a "don't ask again" answer is lost, though, so the host keeps
  asking. Before this stack the question stayed up in that case. The window is narrow, because the answers are disabled
  whenever the page was busy when they were last drawn. Reviewers fixed the same pattern for the session list's question
  (an answer now checks it can start before taking the question); the launcher did not get that fix. A follow-up could
  give it the same check.
- **Not every YOLO question changed.** The question for Replace from the session header still stays up while "don't ask
  again" saves, so #1660's commit message ("The other YOLO questions now behave the same way") and the report's SPEC.md
  note overgeneralize. SPEC.md's "the confirmation stays up with the reason" is still accurate for the header's Replace;
  the one-word edit the report recommends ("comes back with the reason") would be wrong for it unless the sentence
  describes both.
- **The shared confirmation helper's own documentation disagrees with some callers.** It says an event handler must
  close only the question it was drawn for and must not use the reads meant for drawing the screen; a few handlers in
  the launcher, the session view and the hosts panel close whatever question is open, or read it the way the screen
  does, each with a comment saying why. Nothing misbehaves; either the documentation or those calls should change so the
  two agree.

#### Checks

- Run now, on the final stack, through the test-run recorder: the session-list test that answers "don't ask again" to a
  refused YOLO replace from the row menu, which drives the path #1658 changed and was not among the report's runs, on
  Chromium and WebKit (run `f129da98`, 2 passed).
- Reused: the report's checks. The code on main after the last merge is identical to the final stack they and the run
  above used, and the only other commits since the stack was based are the planning queue's bookkeeping.
- Skipped: further runs, for the same reason.

The landing review found one thing in the report above to be wrong: its SPEC.md note and the one-word edit it
recommends, as explained above.
