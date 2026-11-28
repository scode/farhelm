# Report: quiet hours for ntfy alerts

## What this was about

ntfy alerts went out at one saved priority all day and all night, so an Urgent alert that is right during the working
day also rang a phone at 3 a.m. The TODO entry asked for control of ntfy urgency by schedule. You settled the first
shape: keep the existing default priority, and add one quiet hours setting, a range of whole hours in local time that
lowers alerts to a priority you choose (any of the five ntfy priorities, Low by default) or silences them (Off). You
also asked that Send test use the priority alerts would have right now and say so, and that with quiet hours set to Off
it still send, at the normal priority, so you can check the connection.

What is on main now:

- Settings → Notifications → ntfy alerts has a **Quiet hours** switch (off by default) after **Clear saved token**, with
  **Start** and **End** hour pickers (00:00 to 23:00, starting at 22:00 and 07:00) and a full-width **Quiet hours
  priority** choice: Off, Min, Low, Default, High, Urgent. A note under the switch says the hours follow the clock of
  the machine the helm runs on, that the range can run past midnight, and that Send test still sends.
- The range runs from the start hour up to, not including, the end hour: 22:00 to 07:00 covers 22:00 through 06:59, and
  09:00 to 17:00 covers 09:00 through 16:59. Save refuses an hour outside 0 to 23 or a start equal to the end, in fixed
  words, whether or not quiet hours are on.
- The helm decides quiet hours at the moment an alert actually goes out, after its grace period. An alert held across
  the start of quiet hours goes out quietly. With quiet hours set to Off the alert is dropped, and it does not use up
  one of the five sends a minute the alerts after quiet hours need.
- Send test now says what it did: "Test alert sent (High)" outside quiet hours, "Test alert sent (Low — quiet hours)"
  inside them, and "Test alert sent (High — quiet hours active and set to Off; real alerts are silenced)" when quiet
  hours send nothing.
- Settings saved before this read as quiet hours off, and an older app's save leaves the stored quiet hours alone. No
  database change was needed: quiet hours are stored with the other ntfy settings.
- Settings search finds the new controls, including by "do not disturb", "night", "silent" and an hour such as "23:00".
- The specification, the implementation notes, the "Ntfy alerts" part of the docs' Session list page and the
  Notifications part of the Settings page describe quiet hours. A changelog fragment is in, and the TODO entry is
  removed.

## Things you should know

- **"Local time" is the helm machine's clock, not the viewer's.** In the desktop app that is your Mac. A helm running on
  a server uses the server's time zone, which may not be yours; the form's note says whose clock it is but does not show
  the time or zone it reads.
- **Off means dropped, not delayed.** An alert silenced by quiet hours is gone. Nothing is sent in the morning to say
  what was silenced overnight. That is what the plan's Off asked for; the next alert after quiet hours is sent as usual.
- **Every select in the app changed a little.** The hour pickers needed it: a select's list used to be as tall as its
  options, and 24 hours ran past the bottom of a short window. Lists are now capped at the window's height and scroll,
  the way the session row's menu already did, and a list opens with the chosen option scrolled into view. That only
  shows when a list is taller than the window. The first frame of such a list shows its top for one frame before it
  scrolls to the chosen option.
- **The desktop app's time zone code moved into the helm.** The app's clock times and quiet hours read the local time
  through the same two C library calls, so they now live once, in the helm, and the app uses them from there. Nothing
  about clock times should look different.
- **Send test's reply changed shape.** The helm now answers it with what it did (200 with the priority and whether quiet
  hours applied) instead of an empty 204. An app from before this change ignores the answer and shows its usual line,
  and the new app shows the plain "Test alert sent" if an older helm answers.
- **Three Settings screenshots the GUI tests compare against were re-rendered** (each dark and light): the Notifications
  form with an unsaved edit, the same with a refused save, and the close question over Notifications. The form is longer
  now, so at those scenes' fixed scroll they show the quiet hours controls where they used to show Send test.
- **The Immediate bucket of TODO.md is empty now**; this was its only entry. I left the heading in place.
- **No real ntfy server was used.** The helm's tests send to a stand-in server and check the priority each request
  carried.

## Open questions and possible follow-ups

- If the server-zone case matters to you, the form could show the helm's current local time or zone beside the hour
  pickers, so the hours are easy to set for a helm that is not where you are.
- More than one range, or different hours per weekday, were left out on purpose: the plan's settled first shape was one
  daily range.
- Not from this plan, but found while checking it: a helm function for saving the ntfy settings is now used only by
  tests, so `cargo clippy` with warnings denied fails on main for any build of the helm without its tests. Marking that
  function test-only would clear it. I did not change it here.
- Before a release, you may want one manual Send test and one real alert against a real ntfy server inside quiet hours.

## PRs

- #5179 feat: apply ntfy quiet hours in the helm (the stored setting and its save rules, quiet hours applied when an
  alert is sent, Send test's answer, the shared local time code, the implementation notes, the changelog fragment).
- #5180 feat: add quiet hours to the ntfy alerts settings (the Settings controls and Send test line, the select list's
  height cap and scrolling, Settings search, the specification and docs pages, the screenshots, the TODO removal).

## Checks

Run during this plan, through the test recorder:

- Run `90b08096`,
  `cargo nextest run -p farhelm-proto -p farhelm-helm --lib -E 'test(/^ntfy::/) | test(/^local_time::/)'`: 62 passed.
  These are the ntfy settings and sender tests, among them the new ones: the half-open range and its wrap past midnight,
  the save rules, quiet hours through the settings route, the priority an alert goes out at inside and outside quiet
  hours, an alert held through the grace period and sent after quiet hours began going out at their priority, Off
  dropping five alerts and the next five after quiet hours all arriving (so Off took no rate-limit slot), and Send
  test's three answers and priorities. It ran on the final state of #5179.
- Run `90b039d3`, `cargo nextest run -p farhelm-gui --lib`: 3648 passed, the whole GUI library suite, on the final state
  of #5180. It includes the new select test (24 hours in a short window: the list stays inside the window, opens with
  the chosen 23:00 shown, and the keyboard brings 00:00 into view), the Settings test that saves quiet hours with Off
  and reads all three Send test lines, the test that a start equal to the end is left to the helm's refusal, and the
  search test's new quiet hours queries.
- Run `6dab774a`, `python3 scripts/check-gui-goldens.py`: every screenshot compared, passed, on the final state of
  #5180, after the six re-rendered pictures above were published. Before publishing, a full update run showed exactly
  those six changed, and I looked at each.
- Run `dd7a6e47`, `cargo nextest run -p farhelm-desktop`: 182 passed, on the final state of #5180, for the desktop app's
  switch to the helm's time zone code.
- `cargo clippy --all-targets` on the proto, helm and desktop crates (clean but for the existing warning above) and with
  warnings denied on the GUI crate, `cargo fmt`, the test sleep check, `dprint check` on the changed Markdown, and the
  changelog format check: all clean, on the final state of #5179 and #5180.

Reused: none. Every check above ran on the code as it lands; main has not moved since.

Earlier runs, superseded or same-session failures: `419f62de` and `d31038ec` failed on two existing tests that still
expected Send test's old empty answer, which I then updated; `fbd5c73b` and `edf2fcd6` failed on two existing Settings
tests that expected the save without quiet hours, and on the first version of the select's scroll to the chosen option,
which did nothing; `e7894d35` (357 Settings, select and focus tests) and `848d1eab` (the whole GUI suite) passed on
earlier states of #5180.

Skipped: the workspace-wide test run, since only the ntfy settings and sender, the select, the Settings form and search,
and the desktop app's time zone changed, and each of those ran above. The docs site build, since the docs edits are
sentences inside existing paragraphs with no new links or headings.

## Review gate

Each PR had one fresh-context Opus 5.5 review at high effort.

- #5179: no blocker. Its should-fix was that no test showed quiet hours are decided when a held alert is sent rather
  than when it is queued, since the other tests send at once; the held-alert test above was added for it. Its six nits
  were applied: a simpler way for tests to set the hour the helm reads, a simpler Send test outcome, a premise check in
  the Off test, and three corrected comments.
- #5180: no blocker. Its two should-fix items were that the list's scroll to the chosen option left the option's edge
  under the list's border, now scrolled fully clear of the list's border, and that the select test's keyboard step
  proved nothing, now walking to an option scrolled out of view. Its five nits were applied: a note in the focus code's
  docs, a corrected comment, "urgency" dropped from the quiet hours search words (it belongs to Priority), the search
  queries pinned in a test, and the equal start and end test.

The fixes were not reviewed again. Runs `90b08096` and `90b039d3` ran after them.
