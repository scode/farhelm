## What this was about

Six terminal problems were accepted for fixing with a separate, about 100-line gate each: invisible formatting controls
in host errors, a web-link refusal explanation disappearing behind a tunnel-error notice, missed filenames such as
`backup.7z`, right-click copying an old selection, false destination warnings for repeated honest links, and links that
hovered but would not open in the frozen terminal view retained after a takeover or refused reconnect. All six are
implemented in a linear draft stack, in the agreed order. None tripped its size gate or was discarded.

## Things you should know

Host errors and upload status escape formatting controls once when displayed; terminal output bytes stay unchanged. When
a tunnel-error notice covers a web link's destination details, clicking is refused with an explanation that the details
are covered. Redrawing that same link under the resting pointer used to lower the explanation behind the notice. It now
stays visible until the pointer leaves or the link changes. Digit-leading file extensions work while numeric versions
and quantities remain plain text. Copying requires a primary-button press in live and restored screens.

Printing `https://example.com/` twice on consecutive rows could wrongly warn that the displayed address names a
different destination. Those complete repeated addresses now keep the ordinary display. A misleading address split
across rows still warns: `https://git` followed by `hub.com/login` must not pass as `https://hub.com/login`. Only a
complete, unambiguous set of rows that each independently match the actual target receives the repeat exception.

For frozen screens, working links were cheaper than suppressing all three retained link providers. Restoring their
existing gesture tracking makes web and file clicks work without reconnecting the session. Selection drags still do not
open links, and local copying still works. The six counted code/test/doc diffs are 55, 58, 41, 31, 81 and 69 lines,
respectively; bookkeeping and changelog fragments are excluded as agreed.

Review caught a pre-mount error-display dependency and an overlapping filename-regex quantifier; both were corrected.
The latter has a bounded subprocess regression against long numeric-looking filenames. The new snapshot browser fixture
initially failed twice under both engines because its printed filename looked like a URI, then because its leading
separator was trimmed by the fake agent. The final fixture uses an internal separator and passes. Those four failed
executions remain recorded; they were fixture-authoring errors, not classified as latent flakes.

## Open questions and possible follow-ups

No decision is needed to finish this plan. Browser and desktop context-menu delivery remains unverified, as accepted
during triage; the primary-button contract is tested independently of that delivery. Playwright cannot grant WebKit's
live clipboard permissions, so the dedicated live-clipboard case is explicitly skipped there. The restored-screen test
observes the WebKit clipboard write argument and reads the real clipboard under Chromium. No native desktop runtime test
was performed.

## The PRs

- [#2388](https://github.com/scode/farhelm/pull/2388/changes): visible formatting escapes in detach and upload errors.
- [#2392](https://github.com/scode/farhelm/pull/2392/changes): preserve a covered web link's refusal through redraw.
- [#2396](https://github.com/scode/farhelm/pull/2396/changes): recognize digit-leading file extensions conservatively.
- [#2399](https://github.com/scode/farhelm/pull/2399/changes): copy only primary-button terminal selections.
- [#2401](https://github.com/scode/farhelm/pull/2401/changes): avoid false warnings for repeated complete addresses.
- [#2403](https://github.com/scode/farhelm/pull/2403/changes): restore web and file clicks in frozen screens.

## Checks run, reused and skipped

All runtime commands used the recorder. Browser selections used Chromium and WebKit, one worker, zero retries and the
pinned tmux. Complete source identities, reports and untruncated output were checked, including runtime skip markers.
Each browser run used the matching Dioxus web build and the owned test stack. The portable Playwright child commands
below ran from `e2e` through that recorder policy.

- `node --test` in the UI JavaScript harness: 313 passed, run `bf09dc90-bd43-4a54-a128-4e6413552739`, based on 33997789
  plus the initial filename patch. Reused for unchanged modules. Final changed helpers have focused runs:
  `node --test terminal-files.test.js`, 30 passed (`74fa63e7-2f36-4a83-b376-b63f09f8f96d`, base 33997789 plus corrected
  recognizer); `node --test copy-on-select.test.js`, 18 passed (`76d2854e-e0a6-41c0-b988-1d5c33ead917`, base 9c990240
  plus primary-button patch); `node --test terminal-links.test.js`, 28 passed (`6ef8e938-f442-43bd-a76c-78490dbea507`,
  base faa4fc80 plus repeated-link patch).
- Browser selection for held detach and failed upload errors: 4 passed (`ebb96774-655d-4fcc-9b13-a888174c71bf`, base
  f5bb165b plus error-display patch). Both engines showed visible controls without double escaping literal notation.
  Command:
  `npx playwright test 'terminal\.spec\.ts|terminal-attachments\.spec\.ts' -g 'a final detach banner is held|an upload that fails surfaces'`.
- Browser selection for a covered OSC web hover and clipped file disclosure: 4 passed
  (`bfd057e3-05e6-4ad1-8c3f-33cf92701a1f`, base 4384fef1 plus redraw patch). It observes an actual redraw under the
  stationary pointer and preserves the existing file-link behavior. Command:
  `npx playwright test 'terminal-link-clicks\.spec\.ts' -g 'a clipped file disclosure refuses|a tunnel notice covering an OSC web hover'`.
- Browser local and SSH file-download selection: 4 passed (`9fcff225-1203-4d35-8bb8-47318f9610fd`, base 33997789 plus
  corrected filename patch), checking real `backup.7z` download contents. Command:
  `npx playwright test 'terminal-files\.spec\.ts' -g 'file hover and complete browser download'`.
- Browser live and restored-copy selection: 3 passed, 1 explicit WebKit live-clipboard skip
  (`75165363-129a-4a75-95a4-81762cfb0aac`, base 9c990240 plus primary-button patch). Command:
  `npx playwright test 'terminal-clipboard\.spec\.ts|terminal-reconnect\.spec\.ts' -g 'a real drag-select copies|takeover-during-backoff-does-not-steal-the-session'`.
- Browser repeated-link, split-host and ambiguous-continuation selection: 6 passed
  (`56478700-9500-4adf-91db-5a6c9350975f`, base faa4fc80 plus repeated-link patch). Command:
  `npx playwright test 'terminal-links\.spec\.ts' -g 'OSC 8 tagged rows compare the complete (repeated|split-host|ambiguous) link'`.
- Browser exact takeover/refused-reconnect case: 2 passed (`e8c4eeb2-1df0-41c9-b3fb-5d78f24b6bcd`, base 57d496be plus
  final snapshot patch). Web opening, drag rejection, downloaded bytes, copying and absence of a new attachment all
  passed. Earlier runs `0a33050a-c572-46c2-b868-cb12a2b76931` and `68e4ee20-7984-4bad-a3f8-bf6648bb6d26` each failed
  both engines for the fixture mistakes described above; neither is a pass or superseded evidence of success. All three
  attempts used
  `npx playwright test 'terminal-reconnect\.spec\.ts' -g 'takeover-during-backoff-does-not-steal-the-session'`.
- Matching `cargo build` and Dioxus release web builds passed. `node --check` on changed JavaScript, targeted Markdown
  formatting, `python3 releasing/check-changelog.py format`, and the isolated test-sleep checker passed; 304 delays,
  zero missing rationales. The final fixture correction only changed text, so it reuses the same delay inspection.

Earlier successful commands remain applicable because later PRs preserve their tested behavior; the focused copy,
repeated-link and final snapshot runs cover the shared terminal interactions introduced later. URL-only ledger amends do
not change those inputs. Fresh main changed only TODO and planning bookkeeping, new plans and other delivery reports; it
changed no executable dependency or governing instruction. No rebase was needed for that delta. Rust lint, desktop,
installer, provisioning and full browser suites were skipped because no Rust or those subsystems changed and the
selected tests cover the concrete terminal risks. CI is manual-only and was not dispatched.

## The review gate's outcome

Each PR passed its single fresh-context gpt-6.1-sol/high review, with corrections checked by that same reviewer. All six
message cold reads passed. A separate fresh-context scope reassessment after the snapshot fixture corrections found no
unnecessary complexity and confirmed the 69-line fix. The documentation pass is complete. Feedback files and index
entries are removed, each ledger Execution field names its change, bookmark and PR, and every PR remains an open draft.
