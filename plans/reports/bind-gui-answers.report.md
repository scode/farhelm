## What this was about

The TODO entry "Bind GUI answers to what the user saw". Review kept finding buttons, keys and gestures in the web and
desktop app whose handlers acted on whatever was current when the handler ran, not on what was on screen when the user
clicked. Each instance was a narrow timing window nobody had reproduced, but the guards involved are safety ones: YOLO
questions, destructive confirmations, and the destination preview a terminal link shows before it opens. The surfaces
that did bind correctly each had their own home-made check. The goal was to make the right binding the easy one: one
shared mechanism that every question and confirmation answers through, the conditions a destructive action was confirmed
under carried from the click to the request, link covers that declare themselves, input the page holds (a pressed link,
a quick-launch key press) dropped when its owner stops watching, and a written rule for implementers and reviewers.

Six review findings were folded into the entry, and each had to be fixed with the regression test its triage entry
names:

- A Guarded answer in Replace with could delete a source session that was started by another client while the checkout
  preview loaded, because the deletion condition was recomputed when the request was sent. This one was a confirmed
  SPEC.md violation. Fixed in #2576.
- In the same Guarded path, opening Replace with for a second source just before the answer was handled could replace
  the first source instead. Fixed in #2576.
- The session header's YOLO question buttons answered whichever question was open when the click was handled. The code
  documented this as deliberate; triage decided they must answer only the question they were drawn for. Fixed in #2579.
- The quick-launch sequence pill could cover a terminal link's destination preview without the link refusing to open.
  Fixed in #2596.
- A link press held while the terminal reconnected kept its state, so dragging away and back during the reconnect, then
  releasing, could still open the link. Fixed in #2597.
- A quick-launch sequence the app refused left its snapshot of the shortcut bindings queued, so every later keyboard
  launch used the previous one's snapshot and could start a template under an earlier name than it showed. Fixed in
  #2598.

Your decisions from planning (2026-10-21) held throughout. The end state matters more than churn, so prompts that
already worked were moved onto the shared mechanism too, rather than leaving mixed mechanisms. Every prompt moves onto
the shared mechanism, including the trash band and the sidebar row questions. The launcher's opening counter and the
approval cards' arming pause (the short delay before a newly shown approval card accepts an answer) stay separate. A
stale answer does nothing and the current prompt stays, with no notice. Every surface that draws above the link
destination preview counts as a cover, tooltips included. Enforcement is the API's shape plus written rules, with no
check script. The header YOLO reversal goes ahead as triage decided.

## Things you should know

- **What the mechanism does.** Opening any question or confirmation now gives that opening a number from one counter
  shared by the whole app (in the code, a token). The buttons drawn for that opening carry the number. An answer whose
  number is no longer the open one does nothing. An accepted answer closes the prompt in the same step, so a second
  click finds nothing to answer. Before this, several prompts knew only that some prompt was open: the session header's
  Restart, Replace and Delete confirmations, the interrupted session's Replace, and the sidebar row questions. A click
  queued from an earlier opening could therefore answer a later one. Host Remove, the host setup/update/uninstall
  confirmations and the desktop tunnel warning read whichever dialog was open when the click was handled. Most of these
  changes have no visible effect, and their changelog fragments say so. User-facing fragments cover the cases where a
  stale answer could have acted on something else: host Remove, the host setup/update/uninstall plans, the template
  editor's leave prompt, the launcher's YOLO answers on the desktop app, link covers, the held link press and
  quick-launch sequences.
- **Sidebar rows needed a per-row form of the mechanism.** Row questions coexist (one per row) and must survive the row
  going away when a listing is filtered or fails, so a single shared slot could not hold them. I added a per-row variant
  that uses the same numbers and the same accept-once rule. A separate fresh-context reviewer, asked to judge whether
  the design did more than needed, agreed it is the same mechanism extended, not a second one, and that no smaller
  design keeps today's behavior. Stopping a second operation from starting on a row while one runs stays separate row
  state, as the plan asked.
- **Four prompts stay up through their request, by design.** Host Remove, the desktop tunnel warning, the template
  editor's "Save these changes before leaving?" prompt, and a session's Replace YOLO question answered with "don't ask
  again" keep their dialog up while the request they started runs, so a refusal can show in the same place. They accept
  an answer only while their opening is still current, and the request's outcome closes them. The first three also
  refuse a second answer while their request runs. The fourth does not yet: a review of #2579 found that gap, which
  predates the stack, and I left it because it is a second answer to the same opening rather than a stale one, which is
  what this plan set out to close. See the first open question below. The mechanism's documentation and SPEC_impl.md
  name these four as the only exceptions.
- **Behavior changes worth noticing.**
  - In the trash dialog, a Close or Escape drawn before a delete-confirmation band appeared now does nothing if a band
    opened in between. It used to close the dialog over a band the user had not read.
  - In the template editor's leave prompt, Discard and "keep editing" are refused while the prompt's own save runs. The
    buttons already looked disabled then; before, a queued Discard could leave twice or override "keep editing".
  - Escape in the session header now cancels only a question that was drawn before the key press.
- **The launcher's plain YOLO answers had a real gap, fixed beyond the plan's default.** The plan said the "start
  anyway" answers were out of scope unless a concrete gap turned up, and one did. On the desktop app their resubmit went
  through a JavaScript round trip, so the request could go out under a later render's decision about deleting or keeping
  the Replace with source, not the one the user saw. Launching from a picker choice, or with Enter on a recent row, in
  Replace with had the same gap, so those now submit directly too. The browser was not affected.
- **Template Delete and a SPEC.md sentence.** SPEC.md says a stored template that fails the current save rules is
  labelled "delete permanently" before deletion and gets no Undo. Template Delete's Undo offer now follows the label the
  button showed, where it used to recompute the template's kind at click time. The two can differ only if the template
  list reloads between drawing the button and handling the click. The button is disabled for the whole reload, and the
  render that re-enables it also draws the new label, so I found no way for a click to see one label and run under the
  other. If it ever happened, a template labelled "delete" that fails the save rules would be offered an Undo that then
  fails, which departs from that SPEC.md sentence. SPEC_impl.md already words the rule by the label. See the open
  questions.
- **Link covers declare themselves.** The approval cards, the tunnel error notice, the quick-launch pill, Farhelm's own
  tooltip and the trash animation of a session flying into the trash mark themselves as covers. The link check measures
  every marked surface instead of a hand-kept list. The "this link is covered" explanation now draws above every cover
  except that brief trash animation, which the plan allowed to be exempted. A browser test checks that ordering against
  every marked surface. Modal dialogs are not covers: while one is open, every terminal behind it ignores the pointer.
- **Held input.** A held link press is now dropped whenever the terminal stops listening to the mouse, which is what a
  reconnect does. Each quick-launch key press's snapshot of the shortcut bindings is now used or dropped by exactly the
  launch it was taken for, even when that launch is refused. Any snapshots still waiting are dropped when the sidebar
  list goes away, which the browser's sign-in prompt causes.
- **The rule is written down.** SPEC_impl.md's GUI section has a paragraph, "Answers bind to what was drawn". The
  instructions given to every agent reviewer whose findings feed the review queue tell it to flag a handler that acts on
  live state. The launcher's opening counter and the approval cards' arming pause are documented in code as deliberately
  outside the mechanism. Nothing in the types stops a handler from fetching the currently open number at click time and
  answering with it. That is convention, backed by the documentation and the review rule, as you chose.
- **A flake unrelated to this work was recorded.** A farhelm-ui unit test about requests queued behind settings saves
  failed 6 times in 128 retained runs on this machine, including runs by another agent on commits without this stack.
  The last PR adds its FLAKES.md entry, a TODO.md Deflake entry and the deflake sweep exclusion. The leading hypothesis
  is a small product rounding issue: a request waiting behind a hung settings save can be reported as timed out up to a
  millisecond early.
- **The stack was rebased onto the latest main before delivery.** Three conflicts were mechanical: imports in the host
  setup view, the file-link module's list of exports, and TODO.md, where main's new entry "Sweep out browser sign-in and
  failed-save preservation" is kept. The landed changes that touch the same files (failed number-key saves, menu focus
  markers, the changed-host warning, a file-link guard) use different code paths and needed no adjustment. I re-checked
  the survey of prompt surfaces against them and found no new prompt.
- **Size.** 18 PRs, about 5,300 lines added and 1,400 removed across 47 files, most of it tests and documentation.

### The surface inventory

"Before" is how an answer was bound before this stack. "Opening identity" means telling one opening of a prompt apart
from a later opening of the same prompt.

| Surface                                                                                                                                      | Before                                                                  | Now                                                                                             |
| -------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Session header Restart confirmation                                                                                                          | a plain on/off flag; no opening identity                                | numbered opening (#2584); Restart and Restart with still cannot run at the same time, as before |
| Session header Replace and Delete confirmations                                                                                              | "a prompt is open"; target and safety condition from the render         | numbered opening (#2578)                                                                        |
| Interrupted session's Replace confirmation                                                                                                   | "a prompt is open"; target from the render                              | numbered opening (#2578)                                                                        |
| Terminal tab Close confirmation                                                                                                              | keyed by tab; no opening identity                                       | numbered opening (#2578)                                                                        |
| Restart with YOLO question                                                                                                                   | its own opening counter                                                 | the shared numbering (#2578)                                                                    |
| Launcher YOLO question                                                                                                                       | its own opening counter                                                 | the shared numbering (#2578)                                                                    |
| Launcher Guarded answer and the request it sends after the checkout preview                                                                  | launcher opening and deletion condition read when clicked and when sent | both taken from the answered render and sent as taken (#2576)                                   |
| Launcher "start anyway" answers; picker choice and recent-row Enter in Replace with                                                          | on the desktop app, sent under a later render's deletion condition      | sent directly with the answered render's condition (#2595)                                      |
| Sidebar row Delete and Replace questions                                                                                                     | keyed by session; no opening identity                                   | numbered per row; the one-operation-per-row lock stays row state (#2582)                        |
| Sidebar Replace YOLO question                                                                                                                | its own counter                                                         | the shared numbering (#2581)                                                                    |
| Session header Replace YOLO question                                                                                                         | answered whichever question was open                                    | only the question drawn (#2579), on the shared numbering (#2581)                                |
| Trash delete-confirmation band                                                                                                               | compared the band's content; no opening identity                        | numbered opening of the band as shown (#2585)                                                   |
| Host Remove                                                                                                                                  | read whichever dialog was open                                          | numbered opening of the host entry shown (#2586); stays up through the request                  |
| Add-host setup offer                                                                                                                         | its own one-use plan id, compared with the shown destination            | the shared numbering (#2578)                                                                    |
| Host setup / update / uninstall confirmations                                                                                                | read the live pending plan                                              | numbered opening of the plan shown (#2590); a remembered answer is bound the same way           |
| Desktop tunnel security warning                                                                                                              | read the live warning                                                   | numbered opening of the warning shown (#2591); stays up through the consent save                |
| Template editor "Save these changes before leaving?"                                                                                         | no check that the prompt was still open                                 | numbered opening (#2593); stays up through its save                                             |
| Template Delete (one click with Undo; not a prompt)                                                                                          | label from the render, kind recomputed at click                         | the kind the label showed (#2592)                                                               |
| Approval cards                                                                                                                               | answer bound by request id; separate arming pause                       | unchanged by decision; documented as separate (#2599)                                           |
| Launcher opening counter                                                                                                                     | identifies one opening of the launcher                                  | unchanged by decision; documented as separate (#2599)                                           |
| Permission-change Undo notice, skip-confirmation shortcuts, the Delete key shortcut, Restart with dialog, host refusal "remove", trust bands | not prompts; already bound to the render, or they only open a prompt    | unchanged                                                                                       |
| Trash per-entry Delete, sidebar Stop                                                                                                         | no confirmation                                                         | out of scope per the plan                                                                       |
| Link covers                                                                                                                                  | a hand-kept list of two surfaces                                        | every surface that can draw over the preview marks itself (#2596)                               |
| Held link press                                                                                                                              | survived a reconnect                                                    | dropped when the terminal stops listening (#2597)                                               |
| Quick-launch key snapshots                                                                                                                   | a refused sequence's snapshot shifted later launches                    | one per launch, also dropped when the sidebar list goes away (#2598)                            |

## Open questions and possible follow-ups

- **Recommended fix: the session YOLO question's "don't ask again".** If you click Cancel in the same instant as "don't
  ask again", before the buttons redraw as disabled, the question disappears but the Replace you confirmed carries on
  with YOLO. If saving the "don't ask again" host setting then fails, the failure is shown nowhere. The window is a
  single burst of input events, and this predates the stack. The fix is small and has the same shape as the other
  stay-up prompts: refuse the question's other answers while its save runs. I recommend a follow-up for it.
- **Recommended follow-up: one remaining window in the launcher's YOLO and Guarded answers.** When Replace with is
  switched from one source session to another while the first one's YOLO question is showing, there is a moment, after
  the launcher redraws for the new source and before the old question is withdrawn, in which answering the old question
  would start a Replace with of the new source, with that source's own deletion condition but the settings drafted for
  the first. A "don't ask again" given there would not carry over to the new source. The answers now check that the
  launcher has not moved to a new opening (one showing of the launcher for one launch), but in that moment the check
  already sees the new opening. A reviewer found this by reading the code; whether the UI framework can deliver a click
  inside that moment is unverified, and nobody has seen it happen. The fix is for the question to record the launcher
  opening it was asked in and refuse an answer once that differs, which is this plan's own rule applied one level
  further. This window predates the stack: before it, these answers checked no opening at all. Folded finding 2 was
  about an answer handled before the launcher redraws, which is closed; this is the narrower moment just after the
  redraw. I recommend it as a small follow-up.
- **SPEC.md's template Delete sentence.** Either reword SPEC.md so the label the user saw decides whether Undo is
  offered (SPEC_impl.md already says so), or accept that the code departs from it in a window I could not reach. I
  recommend the rewording, since following the label is this plan's rule.
- **Possible simplifications, no action recommended unless you want them.** The desktop tunnel warning could close on
  Allow and drop its separate busy flag, taking it out of the stay-up exception; I kept today's dialog timing and focus.

  Quick-launch snapshots could travel with their launch click instead of being paired by order.
- **A stale quick-launch pill after a browser sign-in.** If the browser's sign-in expires in the middle of a
  quick-launch sequence, the pill comes back after signing in again showing a sequence the page no longer treats as
  armed. Enter then goes to the terminal, and the next sequence prefix resets the pill; launches are not affected. It is
  the same "held input outlives its owner" shape this plan fixed elsewhere, and a one-line fix (clear the sequence when
  the sidebar list goes away). Low priority.
- **A pre-existing overlap.** While the pointer rests on a covered terminal link, its "covered" explanation draws over
  the terminal's top right, where the session header's Delete or Restart confirmation hangs, and can hide part of that
  confirmation's warning sentence. This predates the stack, and the stylesheet's layer notes now say so. Moving the
  explanation to the terminal's bottom while it is raised would fix it.
- **Small leftovers.** A code comment in the launcher's YOLO confirmation component still describes the old resubmit,
  and one of the component's inputs is now unused. Both are harmless. #2595 made them stale without touching that
  component's file, and I kept it to the files its fix needed; I would fold them into the next change to that file.
- **Test gaps I accepted.** No browser test checks the trash animation's cover marker. None checks the two places the
  terminal drops a held link press; unit tests cover the pieces they call. The new quick-launch browser test passed, but
  I did not rerun it with the old code restored to show it fails there; its unit-level counterparts were shown to fail
  against the old behavior. I don't think any of these is worth a dedicated follow-up.
- **Related findings queued on main during this run, not addressed by it.** A one-click trash Delete can hit a row that
  moved under the pointer. Enter on a recent row in Replace with performs the replace while the row says only "launch".
  A browser's failed opt-out of confirmations is replayed after it signs in again.
- **Pre-existing check failures outside the stack.** Desktop-feature clippy fails on one type-complexity lint in the
  approval cards' code, and `dprint check` fails on the UI crate's `Cargo.toml`. Both are on main.

## The PRs

1. #2576 fix: keep a replaced session that restarts during a guarded launch
2. #2578 fix: tie confirmation answers to the showing they were drawn for
3. #2579 fix: answer only the shown YOLO question in a session's Replace
4. #2581 refactor: run the Replace YOLO question on the shared prompt helper
5. #2582 fix: tie sidebar row questions to the showing they were drawn for
6. #2584 fix: tie session Restart confirmation answers to their showing
7. #2585 fix: tie the trash's delete confirmation to the showing it answers
8. #2586 fix: remove the host the Remove confirmation showed
9. #2590 fix: run the host setup, update or uninstall plan that was confirmed
10. #2591 fix: tie the desktop tunnel warning's answers to the warning shown
11. #2592 fix: make template Delete offer Undo exactly when its label said so
12. #2593 fix: keep template edits when a stale discard follows keep editing
13. #2595 fix: launch Replace with under the condition the launcher showed
14. #2596 fix: count the quick-launch pill and tooltips as link covers
15. #2597 fix: drop a held link press when the terminal reconnects
16. #2598 fix: launch a quick-launch sequence with the templates it showed
17. #2599 docs: add the rule that GUI answers act on what the user saw (also removes the TODO entry)
18. #2600 docs: record a_hung_save_releases_waiters_at_their_deadline as a flake

Each folded finding's `TRIAGE_OUTCOMES.md` Execution field names its PR.

## Checks

Run now, on the rebased stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop`: pass.
- `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`: only the approval cards' lint that is
  also on main; nothing from this stack.
- Recorded `cargo nextest run -p farhelm-ui --lib`: run d36635d5, 758 of 758 passed.
- Recorded `cargo nextest run -p farhelm-ui --features desktop`: run 2087762b, 923 of 923 passed.
- Recorded JS unit harness (`node --test` in `crates/farhelm-ui/js-tests`): run 4f67ae27, 333 of 333 passed.
- `python -B scripts/check-test-sleeps.py`: 314 test delays, none without a rationale.
- `dprint check` on every changed Markdown file and `python3 scripts/check-agents-md.py`: pass. One warning, about a
  citation in an older `TRIAGE_OUTCOMES.md` entry, is also on main.
- Recorded browser run 9a9843a1 on Chromium and WebKit, 26 spec files chosen to cover every changed surface (the
  launcher's Guarded and YOLO paths, Replace and Replace with, clone, session confirmations and claims, sidebar rows,
  trash, host settings and setup, multi-host, templates, terminal links, files and tooltips, reconnect, quick launch).
  1440 passed and 4 failed, all on WebKit, while the machine's load average ran between about 20 and 38. Two of the
  failures are open flakes already excluded from deflake sweeps: the takeover new-tab test, and
  `don't ask again on a refused YOLO replace marks the host, then replaces with the override` (terminal-restart). The
  second exercises the session Replace YOLO question this stack rebound, so I compared it with the recorded flake: it
  failed the same way, with the YOLO question never appearing after the click on the interrupted card's "confirm
  replace", before any YOLO answer is given. The three whole-file reruns below include it, and it passed in all of them.
  The other two did not reproduce:
  - `Replace confirms inline, can cancel, selects the fresh session, and surfaces refusal` (terminal-restart): the click
    on the interrupted session's "confirm replace" sent no request. This stack changed that confirmation, so I checked
    whether its new check could have refused the click. In this test only a click on "replace" opens the confirmation,
    and the test clicks it once; if the card had been hidden and redrawn, the confirmation would have closed rather than
    reappeared. The screenshot shows it still open, so it is the opening the click was drawn for, which the check
    accepts. That is reasoning from the code and the screenshot, not from the trace, so treat it as likely rather than
    proven. The screenshot also shows the "could not refresh this session" notice, the fingerprint of the open flake
    "Interrupted-card Replace clicks lost under load" (that spec's injected sessions draw failing background reads that
    redraw the card). Repetitions: 10 of 10 attempts of the test passed on both engines (batch 415673a4), and the whole
    spec file passed 3 of 3 times on both engines (batch 869046c6).
  - `remove-and-re-add-host` (terminal-multihost): the host was removed and re-added and its session came back alive,
    but the session's status read "idle — new output" where the test expects a bare status. It failed on the unread
    marker, which this stack does not touch. 10 of 10 attempts passed on both engines (batch 383de2ae).

  I added no FLAKES.md entries for these two: each is a single observation that did not reproduce and was not shown on
  main. The first likely adds one more lost interrupted-card click to that open flake's record.

Run per PR while building the stack: each PR's own recorded farhelm-ui and JS unit runs, plus desktop-feature runs for
the tunnel warning. All passed except two runs (b58752ff and 8c2c8de7) whose only failure was the settings-save flake
recorded in #2600.

Reused: none; the per-PR runs above are history, superseded by the runs on the rebased stack.

Skipped: the full Rust workspace battery (no Rust change outside the UI crate) and doctests (the UI crate has no
runnable doctests); desktop smoke (no change to desktop startup or packaging); installer and provisioning harnesses (the
helm is untouched); the full browser suite (the selection above covers every changed surface).

## Review gate

Every PR passed a fresh-context Opus 5.5 review at high effort using the plan's charter (plus the repository's
test-authoring checklist where tests changed), and its findings were addressed before the PR was pushed. #2595 and #2598
grew enough after their first review to get a second one. A separate scope review for #2582 confirmed the per-row form
of the mechanism. The reviews caught two real defects in the stack's own work and one pre-existing gap:

- The host setup confirmation's remembered answer could have confirmed a later plan. Fixed in #2590, with a test.
- The tunnel warning's first tests asserted inside a render, where failures are swallowed, so they could never fail.
  They were rewritten in #2591 and shown to fail against the old behavior.
- The launcher's plain YOLO answers sending under a later render's deletion condition on the desktop app, a gap that
  predated the stack, fixed in #2595.

The findings I did not follow were naming and small refactoring suggestions. They are recorded, with reasons, in the
plan's working log (`farhelm-plan-bind-gui-answers-log.md`).

### Landing

Landed as #2576, #2578, #2579, #2581, #2582, #2584, #2585, #2586, #2590, #2591, #2592, #2593, #2595, #2596, #2597,
#2598, #2599 and #2600, merged bottom-up, with the content the report describes and two changelog changes described
below. It was the only plan in this landing round.

#### What landed since the stack was based, and whether it interacted

Nothing but queue bookkeeping landed after the stack's base. The plan was written while several UI changes landed
underneath it, though, so a fresh-context reviewer and I checked that the prompts it changes (each now acting on what
the user saw when answering) keep what that recent work added:

- The specification rules added on 2026-10-22 (#2580) mostly hold: none of the changed prompts keeps or replays state
  across an expired browser sign-in, and the plan adds no new retention of failed saves. The plan's new cleanup when the
  sidebar list goes away (which signing in again causes) drops queued snapshots of the shortcut bindings, not the queued
  sequences themselves. One exception predates the plan but is now asserted by it; see "Things you should know".
- The Templates dialog lock during a number-key save (gui-saves-last-write-wins) is respected: every answer to the
  template leave prompt is refused while that save is in progress.
- The launcher gains no new close path, so it still makes the page usable again before handing off (modal-dialog-shell).
- Closing menus when their rows move (popover-anchor-close) never touches the sidebar's row questions.
- The menu focus markers, the changed-host warning, the shared Replace handling for the sidebar and session header
  (ui-session-actions) and the guarded YOLO answer from the recent rounds are unchanged in behavior.

#### Fixes made during the landing

- Two of the plan's changelog fragments described fixes to work that landed after the last release (v0.30.0, 2026-10-18)
  and has not shipped. A terminal link press held through a reconnect could only survive since the restored frozen
  screen started handling link clicks (#2403, 2026-10-20), and the snapshot a quick-launch sequence takes when Enter is
  pressed came in #2355 (2026-10-20). Following the repository's rule that a fix to unreleased work edits that work's
  fragment, the landing deleted the two new fragments and added their content as context paragraphs to the original
  fragments, in the same PRs (#2597 and #2598). The plan's other twelve fragments concern behavior that shipped (half of
  them mark changes with nothing user-facing), so they stay.

#### Things you should know

- The template editor's leave prompt still lets the user save the same draft again after a failed save. SPEC.md's
  2026-10-22 rule says a failed save is reported and discarded and the GUI does not keep the change it tried to save, so
  this departs from it. The behavior predates this plan, but the plan's new test and code comment now assert it. The
  near-term TODO entry "Sweep out browser sign-in and failed-save preservation" covers removing it; that would also mean
  changing this plan's test.
- A browser test from the launcher rework (launcher-draft-model), "a template shortcut preserves editor focus when
  clearing a quick-launch YOLO question", failed on WebKit 3 of 10 times on main and 3 of 10 with this plan (see
  Checks). It passed on both engines in each of the previous three landing rounds' full browser runs. At that failure
  rate three passes in a row is not unusual, so it may always have been flaky, but a regression from what landed in
  those rounds is not ruled out. It has no FLAKES.md entry yet.
- This plan's last PR adds a FLAKES.md entry, a Deflake entry and a sweep exclusion for the hung-save timing test
  (`a_hung_save_releases_waiters_at_their_deadline`). My draft PR #2528 from an earlier landing round records the same
  test; it duplicates this plan's more thorough record, so I am closing it after this landing.

#### Possible follow-ups

- Record the WebKit template-shortcut focus test above as a flake (a FLAKES.md entry, Deflake entry and exclusion), or
  investigate it as a possible regression. I am opening a draft PR that records it.
- Add to the landed hung-save entry the evidence from #2528: run alone with the desktop feature, the test failed 1, 4
  and 5 times in three 50-iteration stress runs on main, and 9 to 20 times in 50 on the stack of an unrelated landing
  (gui-saves-last-write-wins). That is a higher rate than the entry's 6 failures in 128 executions; I recommend adding
  it, since it shows the test fails alone, not only under load.
- Found at landing, low severity: in the shared confirmation code, the comment on the check that a prompt is still the
  one shown lists three prompts that stay up through their request, while the comment on the type itself and
  SPEC_impl.md list four; the session Replace's YOLO "don't ask again" is the one left out. Possible follow-up: add it.

#### Checks

The plan adds about 5,300 lines and removes about 1,400, almost all in the UI, its asset scripts and browser tests (the
rest is SPEC_impl.md, a reviewer instruction file, the flake record and changelog fragments), and changes prompts across
the whole UI, while its own browser run was a 26-spec selection; that is why this round ran the full browser suite. No
helm or supervisor code changed, so the rest of the Rust workspace did not run. All of the following ran on the stack
before the landing's changelog folds, which changed only changelog files:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 scripts/check-agents-md.py` (an instruction
  file changed; one warning, about a citation main already had), the workspace debug build, the UI's release web build
  and `scripts/check-desktop-assets.sh` (which also builds the desktop app): all passed.
  `python3 releasing/check-changelog.py format` passed again after the folds.
- `cargo nextest run -p farhelm-ui` through the test recorder with pinned nextest and tmux, four slots, no retries, run
  `9e08df55-8251-41e5-992d-83e37d2742e2`: 758 passed. `cargo nextest run -p farhelm-ui --features desktop`, run
  `9fac815f-451e-48c1-8727-330f463f85c7`: 923 passed. `node --test` in `crates/farhelm-ui/js-tests`, run
  `e101ec26-49e8-4f4a-a7fd-c3442403ecb6`: 333 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `4120dcb9-412d-4a76-8ac1-f89e6c8fd37f`: 2482 passed, 14 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), 2 failed, both on WebKit:
  - "don't ask again on a refused YOLO replace marks the host, then replaces with the override", the open flake
    FLAKES.md recorded on 2026-10-20, which the deflake sweep already excludes;
  - "a template shortcut preserves editor focus when clearing a quick-launch YOLO question", a test the launcher rework
    (launcher-draft-model) added, which timed out waiting for the launcher's name field to keep focus. Repeated ten
    times on WebKit it failed 3 of 10 on this stack (run `527195e7-3cbc-4855-94a0-27b0e207d9e9`) and also 3 of 10 on
    main rebuilt on its own (run `108dcf3d-bd51-451c-ba63-9a7e222d7b3d`); five Chromium repeats passed (run
    `8a88b650-5ba1-4686-8937-f5c6eb767d49`). So it predates this plan; see "Things you should know".
- Reused: none. Skipped: the rest of the Rust workspace (no helm or supervisor code changed), the website build (no docs
  page changed), the doctests (no documentation example changed), the desktop smoke test (the desktop app crate is
  unchanged; the tunnel warning change is covered by the desktop-feature unit tests above), the native Mac app (not
  runnable here) and the installer (unchanged).

The report's checks on the rebased stack were the full UI unit, desktop and JavaScript runs and a 26-spec browser
selection; this round also ran the full browser suite. Nothing in the executor's report became untrue at landing, apart
from the two changelog fragments moved as described above.
