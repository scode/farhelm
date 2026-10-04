### What this was about

Hovering an icon or a button in the web UI or the desktop app mostly showed nothing. About a dozen of roughly 110
controls had hover text at all (the sidebar's status dot, agent and permission marks, the activity time, the folder
line, the restart and text-size buttons), and those used the browser's own tooltip, which waits a second or more before
it appears. Nothing a page can do shortens that wait, and WebKit, which the macOS app embeds, also ignores the macOS
tooltip-delay setting. You asked for hover text on every clickable control (plain-word buttons such as "cancel"
included, saying more than the label where it can) and every icon that carries meaning, appearing after about 300 ms, at
once when moving straight on to the next control, also on keyboard focus, sitting above the control so the cursor cannot
cover it, with a browser test that fails on any control without it. Items in a row's `⋯` menu that already show a
description line were to be left alone.

What landed:

- **A tooltip of Farhelm's own** replaces the browser's everywhere. It appears about 300 ms after the pointer comes to
  rest on a control (moving within the control starts the wait over), or when keyboard focus reaches it. Moving straight
  on to another control, or tabbing on, shows the next one at once. It sits about 6 px above the control, shifts
  sideways to stay inside the window, and only goes below, with a larger gap, when there is no room above. It looks like
  the row menu panels (same background, border and shadow, in both themes), wraps at about 280 px and cuts very long
  text at eight lines. It goes away on leaving, on any click, on Escape, when focus moves, when the window loses focus,
  and when the sidebar or another container holding the control scrolls; the terminal scrolling under live output does
  not hide a header button's tooltip. Touch never shows it. No element keeps a browser tooltip, so the slow box can no
  longer appear on top.
- **Every existing hover text moved to it**, unchanged in meaning, including the sidebar marks, the session header's
  title, folder, command and restart texts, the host update button and the "too new" host label.
- **Every control and meaningful icon now has hover text**: the local and remote host marks, the session and host `⋯`
  toggles, the session header's replace, clone and delete buttons and their confirmations, terminal tabs, the
  new-session form (agents, models, effort, permission modes, workspace trust, folders, search results), the hosts list,
  templates, settings, feedback, sign-in and the terminal's own "reconnect now" and "take control" buttons. The texts
  say what the control does or leaves alone ("cancel: keep this session as it is", "replace: start this session, then
  delete the old one and its state").
- **A browser test** walks the screens the existing test fixtures reach and fails on any visible button, tab, menu item,
  link, select, checkbox or radio button without hover text, or any element with a browser tooltip.

### Things you should know

- **The help menu got hover text too.** Its two items show description lines like the row menus' items, but you scoped
  the exemption to the session and host `⋯` menus, so they carry a short hover text that says a little more than the
  description. If you would rather exempt them as well, it is a small change: remove those two hover texts and widen the
  test's exemption.
- **Two hover texts are no longer read out by screen readers.** The browser's own tooltip doubled as text a screen
  reader could announce; Farhelm's tooltip is a visual aid hidden from assistive technology. Where that text mattered,
  the control now carries it itself: the full folder and command on the session header's copy buttons, a host's update
  urgency and versions, a "too new" host's remedy, and the folder on compact sidebar rows (now part of the row's spoken
  name rather than its description). Two texts became hover-only: the exact date and time behind an activity age ("2m"),
  and the "this client was built as farhelm …" text on the version number at the top of the sidebar. Your request said
  accessibility must not regress where a removed tooltip was the only accessible text of an icon-only control or status
  mark; these two sit on visible text and carry no action, so they were left hover-only.
- **Checkboxes and radio buttons carry their hover text on their label.** The original list of what the test counts as a
  control did not include them; the reviewers found the settings dialog's two checkboxes without hover text, so the test
  now checks checkboxes and radio buttons too.
- **The tooltip loads before sign-in.** It is installed for the whole window, so the desktop app's startup and failure
  pages (with their Retry button) get it as well.
- **Screenshots and the demo video.** The docs and README screenshot captures park the pointer in the window's top-left
  corner, where nothing has hover text, so they should be unaffected. The README demo video's recording moves the
  pointer onto controls to click them, so the next re-recording will show Farhelm's tooltips appearing over the clicked
  controls where the pointer rests long enough.
- **Not covered by the browser test:** screens the existing fixtures cannot reach without new test infrastructure, such
  as a host in the middle of an update, an interrupted session's notice, a provisioning plan or the restart-with dialog.
  Their controls were given hover text by going through the source, and a scan of the UI source found no button (tabs
  and menu items included), select or link without it; checkbox and radio labels were checked by hand.
- **Hover texts are written in the UI's lowercase tone** and two existing ones changed slightly: the text-size buttons'
  hover text is now lowercase, and the row's "stale" mark now explains itself ("stale: the host is not connected, so
  this is its last-known state") instead of repeating the word.

### Open questions and possible follow-ups

- Whether to extend the exemption to the help menu's described items (see above).
- The desktop app's WebKit is only checked by hand: a tooltip section was added to the manual Mac checklist
  (`docs/manual-mac-checklist.md`), noting that macOS only moves Tab between buttons when Keyboard navigation is turned
  on in System Settings.

### PRs

- #1589 feat: show hover text quickly in a themed tooltip (the tooltip, moving every existing hover text to it, spec and
  changelog, removes the Maybe-later TODO entry "Custom hover tooltips on buttons and menu items").
- #1590 feat: give every control and meaningful icon hover text (the coverage pass and its browser test, spec and
  changelog, removes the Near-term TODO entry "Hover help on icons and buttons").

### Checks

All on the final code of both PRs, before the last rebase onto main. That rebase brought only documentation, a docs
screenshot refresh and a two-rule CSS font-size fix in unrelated parts of the stylesheet, so the results still apply.

- Browser tests on Chromium and WebKit, through the test-run recorder: every case of the new tooltip test (placement,
  the delay, moving straight on, dismissal by click and Escape, keyboard focus, touch, no browser tooltips left), the
  new coverage test, and the existing tests that check the compact sidebar row: run `96f2e51f`, 34 of 36 passed. The two
  failures were the coverage test expecting more controls on the add-host form than it has; the test was corrected and
  rerun in `e0e990a4` (4 of 4). The tooltip test was not rerun because that fix touched only the coverage test. On the
  first PR, every existing browser test that checked a hover text was run on both engines and passed (runs `d45ea0ac`
  and `6632eddf`, after one missed hover text in the new-session form's search results was fixed).
- Rust: `cargo nextest run -p farhelm-ui` (408 of 408), `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo fmt --all -- --check`,
  `cargo check -p farhelm-ui --features
  desktop`, `scripts/check-desktop-assets.sh`.
- `cd crates/farhelm-ui/js-tests && node --test` (201, including 11 new placement and follow-on cases),
  `python -B scripts/check-test-sleeps.py` (no unannotated delays), `dprint check` on changed files,
  `python3 releasing/check-changelog.py format`.

Skipped: the full browser suite and the full Rust workspace suite. The change is confined to the UI crate and its
browser tests; the browser tests that check hover text or the changed controls, and the coverage test, were run instead.
The desktop smoke test, the installer, provisioning and release checks touch nothing this changed.

### Review gate

Each PR was reviewed by two fresh-context reviewers, Claude Opus 5.5 at high effort and gpt-6-astra at high effort, as
you asked.

- First PR: 13 findings between them, all fixed. The ones that changed behavior: the delay now counts from the pointer
  resting rather than arriving; a dismissal by click or Escape now ends when focus leaves the control; a tooltip of a
  control nested inside a dismissed one no longer lingers after the pointer moves back out; only moving off a control
  opens the "show the next one at once" window, so a dismissal does not make the next tooltip pop instantly; the tooltip
  follows a control that moves without scrolling; and the screen-reader text described above was restored.
- Second PR: 17 findings between them, all fixed except two small suggestions declined: storing the terminal's
  "reconnect now" hover text alongside its label in the reconnect settings the helm sends (unnecessary once the text was
  made true whether or not the terminal retries on its own), and writing a different hover text for each choice menu in
  the templates form instead of one shared sentence. The fixes included hover texts that were wrong for some agents
  (workspace trust "false" means "run the folder untrusted" for Codex, not "add no trust"), the launch button in replace
  mode understating that the old session is deleted, "reset choices" restoring remembered permissions rather than
  clearing them, the settings checkboxes, and making the coverage test check each screen's own controls and reach the
  new-session form after an agent is picked.

### Landing

Landed on 2026-10-04 as #1589 (the tooltip itself) and then #1590 (hover text on every control), each as its own squash
commit on main.

#### What else was on main

Nothing that could interact. Reading every change that reached main between the commit the stack was built on and the
landing showed only the planning queue's own bookkeeping (this plan's delivery and landing claim), which touches no
code, spec or test. No other plan landed in the same round. An independent review by a fresh-context sub-agent, done
before anything merged, reached the same conclusion.

#### A fix made while landing

That independent review also found a problem inside the stack itself, which the report did not mention. In #1589, the
hover text on the new-session form's recent-setup buttons moved from the browser's tooltip to Farhelm's own tooltip. Two
existing browser tests still found those buttons by their browser-tooltip text, which no longer exists, so they would
have failed. That was worked out from the code (the lookup they used only sees the browser's tooltip), not seen in a
run:

- the sidebar test that walks the new-session form's reset notices through every restored choice (on both Chromium and
  WebKit);
- the new-session form's screenshot matrix, a browser test that captures the form in many states for visual review and
  picks recent-setup buttons the same way (Chromium only).

The executor's reruns of "every existing test that checked a hover text" covered tests that assert on the hover text
itself, but not these two, which only use it to find a button. Each of those buttons carries the same full description
as its accessible name, so the fix looks them up by name instead. It went into #1589's commit, the one that made the
change, before anything merged; no product code changed. A search of all browser tests in the final stack (both PRs)
found no other lookups by browser-tooltip text.

#### Checks

- Run now: the two affected browser tests through the test-run recorder on the final stack, on Chromium and WebKit
  (`npx playwright test 'sidebar\.spec\.ts' 'f20_visual_capture\.spec\.ts' -g 'composer reset notices follow every
  restored-choice transition|F20 visual capture matrix'`):
  run `3e74f3e9`, 3 passed and 1 skipped (the screenshot matrix is Chromium-only by design).
  `python -B scripts/check-test-sleeps.py` on the landed main: no unannotated delays.
- Skipped: `dprint check` on the two edited test files, because dprint does not format TypeScript in this repository.
  The full browser suite and the Rust suites, because nothing new reached main besides queue bookkeeping and the fix
  touched only two test lookups, which the run above exercised.
- Reused: the rest of the report's Checks section. The landing fix changed only how two tests find their buttons, and
  main brought no code, so that evidence still applies to what landed.

The landing made two things in the report's Checks section untrue. It says its checks ran on "the final code of both
PRs", but the final code now includes the landing fix, which only the checks above saw. And "every existing browser test
that checked a hover text … passed" did not include the two tests above, which would have failed.
