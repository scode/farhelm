# GUI template and number-key saves: last-write-wins, and failed key saves discarded

## Since the last review

This round follows your answer to the blocked question, which settled that a failed GUI save is reported and then
discarded (now in SPEC.md, "One GUI at a time" and "Signing in again").

- [#2573](https://github.com/scode/farhelm/pull/2573/changes) was dropped: closed unmerged, as your answer asked. It was
  the 194-line fix that kept your own key changes after a failed save and replayed them later.
- [#2562](https://github.com/scode/farhelm/pull/2562/changes) changed: its spec exception (GUI template and number-key
  saves may overwrite an agent's edit) no longer carves out a failed number-key save, and it now names rename and Undo
  as part of the accepted template overwrite (see below).
- [#2583](https://github.com/scode/farhelm/pull/2583/changes) was added: the simple fix, report and discard.

## What this was about

Number keys (Cmd+0–9 in the Mac app, Ctrl+Shift+0–9 elsewhere) launch templates, and the ⚡ mark picks the template
quick launch uses. In the Templates dialog you can give each template a key, and an agent or the `farhelm` command line
can assign or move keys too. SPEC.md promised that a GUI save never undoes a change an agent or the command line made
while that screen was open, but template and number-key saves do exactly that: the editor saves its whole copy of a
template, and a key change re-reads the keys and writes the whole set back. The 2026-10-21 triage found that
contradiction, plus a worse case: when a key save failed (most often an expired browser sign-in), the dialog kept the
whole set it had tried to save, showed it as current, and wrote it back on the next key change, rename or sign-in retry,
undoing any key an agent had moved in the meantime. A number key could then launch the template the agent had
deliberately moved off it.

You decided "Spec, plus fix the failed-save case", then, when the first fix came out at 194 lines, that a failed GUI
save is simply reported and discarded, and asked for the simplest fix under that rule even if it meant abandoning the
work.

## Things you should know

The spec now accepts that GUI saves of templates and number keys can overwrite an agent's or the command line's edit
(SPEC.md, "One GUI at a time", with the Concepts entry for templates and the number-key paragraph agreeing). A separate
code review queued today added a note to the triage finding this plan carried out: the same overwrite also happens when
you rename a template (the dialog saves its copy under the new name, then deletes the old one, which may hold an agent's
newer version) and when you undo a delete (Undo restores the copy the dialog had loaded, not the one deleted). The
exception now names both. That wording is one clause in SPEC.md, "One GUI at a time" (#2562); see the open questions.

After a failed number-key save, the Templates dialog now shows "the number key could not be saved" and goes back to the
keys the helm has, and nothing remembers the attempted change: the next read shows the helm's keys, and neither the next
key change, a rename, nor the retry after signing in again writes it back. Set the key again if you still want it. The
same holds when a rename's key move fails, and when a save is cut off without an answer (for example when the connection
to the helm drops before it replies). A save that is still in progress keeps showing until it settles, so changing two
keys quickly still saves both.

The ⚡ quick-launch mark and every other GUI preference still keep a failed value and retry it after signing in again,
as before. That now contradicts SPEC.md's 2026-10-22 failed-save rule, so the code on main lags the spec there. Your
TODO entry "Sweep out browser sign-in and failed-save preservation" covers removing that; this plan changed only number
keys.

The fix is small in code (about 35 lines of production code), with about 36 lines of code comments, 22 of SPEC_impl.md
text and 106 lines of tests, including a new browser test that fails the save, checks the error and the restored keys,
and then shows that the next save keeps a key an agent set in between.

## Open questions and possible follow-ups

Keep rename and Undo inside the accepted overwrite? Your decision covered GUI template saves in general; I named these
two explicitly because the new review note showed they reach the same overwrite. The alternative is a follow-up that
fixes them instead (rename would have to re-read the template before saving under the new name, and Undo would have to
keep what was deleted). Recommended: keep the acceptance, since both are ordinary whole-template GUI saves.

Renaming a template also moves its ⚡ mark by re-reading and rewriting it, so an agent's ⚡ change that lands in that
moment is undone. As main stands that breaks the spec's general rule that a GUI write must not undo an agent's change,
because the exception names templates and number keys but not the ⚡ mark. Recommended: ask for a follow-up adding the
⚡ mark to the exception; it is the same kind of race the exception already accepts.

A browser test unrelated to this plan, "initial prompt survives a recent but clears on Clone and Replace prefills" in
`e2e/tests/templates.spec.ts`, fails now and then on WebKit only: the launcher shows no recent setup when the test
expects one. It failed once in 10 runs on this plan's code and once in 20 on unmodified main, so it is pre-existing. It
needs a FLAKES.md entry, its Deflake entry in TODO.md and its line in the deflake known-flakes list; this plan did not
add them because that bookkeeping is outside its PRs (recorder runs `04b80b2a-27ec-4dca-a089-aaa6f2a0b592` and
`e2ab5e14-0246-408d-b503-8c2833d87405` hold the evidence).

## The PRs

- [#2562](https://github.com/scode/farhelm/pull/2562/changes): accept last-write-wins for GUI template and number-key
  saves in SPEC.md, naming rename and Undo.
- [#2583](https://github.com/scode/farhelm/pull/2583/changes): discard a failed number-key save instead of replaying it;
  based on #2562.

## Checks run, reused and skipped

- #2583, on its final code: recorded `cargo nextest run -p farhelm-ui --lib` passed 739 of 739, run
  `1c569579-3cab-4d16-a4bb-09c2ba4480eb` (pinned nextest and tmux, four slots, zero retries). An earlier run on the
  first version, `bef538a3-f19a-4250-bcda-50a492122b51`, failed one test that asserted the old keep-and-replay rule; it
  was moved to the ⚡ mark, which still follows that rule.
- #2583, browser: recorded `npx playwright test 'templates\.spec\.ts'` on Chromium and WebKit, run
  `f9a3f3e6-5718-4b69-918d-3481879a4287`: 101 of 102 passed, including the new failed-save test on both engines; the one
  failure is the pre-existing WebKit flake above. The tree it ran on differs from the final one only in code comments, a
  no-op helper's removal and a unit test.
- #2583 on its final code: `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check` on every changed file, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` and the isolated test-sleep checker (314 delays, none missing a rationale)
  passed.
- #2562 is specification and bookkeeping text only: dprint and the agents-md check passed on its final revision; no
  runtime tests.
- Reused across the final rebase (the runs above were made before it): main moved by a batch of review-queue records
  only; the rebase conflicted in the review-queue index and in the finding file #2562 deletes, both resolved, and
  touched no code.
- Skipped: the wider Rust, desktop, installer and provisioning suites, which nothing here reaches. CI was not
  dispatched; it does not run on PRs.

## The review gate's outcome

Each PR got a fresh-context Opus 5.5 review at high effort with the plan, its Decisions and the full triage entry, and
the test-authoring checklist for #2583. For #2562, after the rebase, it found nothing blocking; its wording suggestions
were applied, and a re-review after the rename and Undo clause confirmed the clause against the code. For #2583, the
first review found that a failed rename still left the failed keys on screen (fixed), suggested discarding a cut-off
save too (done, which also removed now-dead cleanup code), stale comments (fixed), and a missing browser test (added). A
second round found only a stale comment and dead code (both fixed). A resume check at the start of this round confirmed
the earlier executor's work and state. Fresh cold readers checked both commit messages. Both PRs cleared the review
gate.

### Landing

Landed as #2562 and #2583, merged bottom-up, with the same content the report describes; #2573 stays closed unmerged, as
the answer asked. It was the only plan in this landing round.

#### What landed since the stack was based, and whether it interacted

The stack was based on current main, which already held #2580, the specification change behind the answer: an expired
browser sign-in is treated as reloading from scratch, and a failed GUI save is reported and discarded. A fresh-context
reviewer and I read the plan against it and against recent UI work:

- #2562's text agrees with #2580: no failed-save exception is left, and the accepted overwrite is stated as an exception
  to the rule that a GUI write must not undo an agent's change. The exception covers saves that succeed; the failed-save
  rule covers saves that fail.
- #2583 discards the attempted keys on every path the report names: the next key change, a rename's key move, the retry
  after signing in again, and a save cut off without an answer. Other preferences that share the same saving code, such
  as the ⚡ quick-launch mark, are unchanged, as the report says.
- Restoring the helm's keys after a failed save cannot leave an open number-key menu pointing at the wrong row, because
  every key menu is disabled while a save is in progress. That matters since the recent change that closes menus when
  their rows move (popover-anchor-close). The launcher's template shortcuts, which the recent launcher rework
  (launcher-draft-model) touched, only read the keys.

#### Fixes made during the landing

None.

#### Things you should know

- This plan landed although it makes an existing unit-test flake fail more often. The test,
  `a_hung_save_releases_waiters_at_their_deadline` in the UI's save-ordering code, checks that a settings read stuck
  behind a save that never finishes gives up when its 30 ms budget runs out. Run alone 50 times at a time, it failed
  about 1 run in 15 on main (10 of 150) and about 1 in 4 with this plan (41 of 150); see Checks. The plan does not
  change that code, though it changes another part of the same file's library. The open hypothesis, in the unmerged
  draft PR #2528 that adds the test's FLAKES.md entry, is a deadline rounded down to whole milliseconds, so the read
  gives up up to a millisecond early: harmless to users, but enough to fail the test's exact check. Why this plan makes
  it more frequent was not tested. Until #2528 or a fix lands, desktop-feature UI test runs will fail on this test
  often, with nothing on main explaining why.
- This plan makes the Templates dialog stay locked, including Close and Escape, until a number-key save settles, which
  can take up to the 60-second request timeout if the helm stalls; before, it was locked only while re-reading the keys.
  That is why the report's sentence "changing two keys quickly still saves both" does not hold in the dialog: the second
  change waits for the first. I recommend accepting the lock as the simple consequence of discarding failed saves,
  unless a stalled helm is common enough to matter.

#### Possible follow-ups

- Merge the flake record (#2528), or fix the rounding behind it, so desktop-feature UI test runs stop failing on that
  test without explanation.
- Found at landing, low severity: a save whose outcome is unknown (its reply was lost or timed out after the helm may
  have applied it) is reported as "the number key could not be saved", and the dialog shows the keys it last read. If
  the helm did apply it, the number key launches by the new keys while the dialog showed the old ones until its next
  read. SPEC.md asks the desktop app to say when an outcome is unknown. Possible follow-up: word that case as unknown.
- Informational, older than this plan and covered by the near-term TODO entry "Sweep out browser sign-in and failed-save
  preservation": if the app re-reads preferences after signing in again while a number-key save is still in progress,
  and that save then fails, the reviewer believes the attempted keys can stay on screen until the next fresh read; they
  did not confirm whether that re-read waits for the save. Launches always read fresh, so only the display would be
  affected.

#### Checks

The plan changes about 200 lines; its code part is in the UI's shared preference-saving code, which every GUI preference
save goes through, and in the Templates list. That is why this round ran the UI's unit tests and the browser specs for
templates, number-key shortcuts, quick launch and Settings. All of the following ran on the stack as it landed:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (one warning, about a citation main already had), the workspace debug build and
  the UI's release web build: all passed.
- `cargo nextest run -p farhelm-ui` through the test recorder with pinned nextest and tmux, four slots, no retries, run
  `1de21559-6dcd-463a-a310-6c413ba38ccd`: 739 passed.
- `cargo nextest run -p farhelm-ui --features desktop`, run `825930bd-9b5b-47a2-9d1d-a9b8e3163821`: 899 passed, 1
  failed: `api::save_order::tests::a_hung_save_releases_waiters_at_their_deadline`, the timing test that the unmerged
  draft PR #2528 records in FLAKES.md, whose own code this plan does not change. Repeated alone with nextest's stress
  mode, 50 iterations at a time, it failed 10 of 50 on the stack (run `25378352-1585-4c08-bd83-1164673b5089`), then,
  alternating main and the stack back to back, 1, 4 and 5 of 50 on main (runs `50cdeddd-9320-48d7-9d33-b2efb94e1fbd`,
  `51ac110a-7dc8-47d8-ba61-1e2643522786`, `474fd738-d650-4dc2-afc6-c98d710bbd8f`) against 12, 9 and 20 of 50 on the
  stack (runs `47f8f8c9-fc3d-4935-bb2b-ed6de9ad850b`, `283d6cb7-5fd3-4c66-9f5a-cbcc9cca4958`,
  `bacf1ea5-34e3-46ba-b1d8-0f47865d1101`). So the flake is real on main, fails alone and not only under load, and fails
  about four times as often with this plan; see "Things you should know" for why the plan landed anyway.
- `npx playwright test` in `e2e/` for `templates`, `launcher-template-shortcuts`, `template-list-shortcut`,
  `quick-launch` and `settings` on Chromium and WebKit, three workers each with its own helm and supervisors, no
  retries, run `533a5121-c41f-4b8d-bd55-f395a2a32291`: 242 passed, none failed, including the plan's new failed-save
  test.
- Reused: none. Skipped: the rest of the Rust workspace and the JavaScript harness (no helm, supervisor or asset
  JavaScript changed), the rest of the browser suite (the preference-saving paths are covered by the specs above), the
  website build (no docs page changed), the doctests (no documentation example changed), the desktop smoke test (the
  desktop app crate is unchanged), the native Mac app (not runnable here) and the installer (unchanged).

The report's checks were its own selections; this round ran the UI unit tests and the browser specs above on the plan as
it landed. Apart from the report's sentence about saving two keys quickly, which does not hold in the dialog as
described above, nothing in the executor's report became untrue at landing.
