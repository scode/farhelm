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

### Landing

#### What landed with it

Nothing reached main between the executor's last rebase of this plan and its landing, apart from the queue's own
bookkeeping. It was the only plan in this round.

An independent reviewer and I read the change against what was already on main, mainly the held ntfy alerts and their
grace period (the blocked-notify-attention-and-debounce plan, landed in the round before this one), which this plan
builds on, and the menu rows shared by the app's menus and select lists, which the terminal-tab-overflow-ux plan
changed. We checked:

- **Held alerts.** Quiet hours are decided when an alert actually goes out, both for one sent at once and for one sent
  when its grace period ends, so an alert held across the start of quiet hours goes out quietly, as the report says.
- **The five-a-minute limit.** An alert that quiet hours silence is dropped before the limit counts it, so it takes no
  place from the alerts sent once quiet hours end and adds nothing to the "and N more" count.
- **Saved settings.** Quiet hours are stored with the other ntfy settings. Settings saved before this read as quiet
  hours off.
- **The protocol.** No protocol version change is needed: the changed settings and the Send test answer pass only
  between the desktop app and its own helm, which always come from the same build.
- **Every select list in the app.** Lists taller than the window now stop at the window and scroll, the way the session
  row's menu and the all-tabs list already do. Scrolling to the chosen option scrolls only the list, never the Settings
  page behind it. Shorter lists look the same: of the screenshots the GUI tests compare against, only the Notifications
  ones changed, because the form is longer.
- **The time zone code the desktop app's clock times use.** It moved into the helm unchanged.

We found nothing that had to be fixed before landing.

One test outside this plan failed once in the workspace run: the check that Stop leaves alone the jobs of a tmux server
that a session started itself. It failed before Stop ran, at its own starting check that the tmux window's job had moved
out of the systemd scope Farhelm runs the session's agent in. That move is tmux's own (a tmux built with systemd asks it
to give each new window a scope of its own), and this plan touches nothing of tmux, sessions or Stop. Run again alone it
passed 26 times out of 26, so I took it as a timing problem in the test's setup, most likely tmux's move not finishing
on a busy test machine, and kept the failed run. It is not recorded as a known flaky test after one failure; if it fails
again it should be.

Worth knowing, from the review:

- **The form's note says quiet hours follow "the clock of the machine this helm runs on".** The helm always runs inside
  the desktop app, so that is always your own computer's clock. Someone whose sessions run on hosts in another time zone
  could still wonder whether 22:00 means their time or the host's. The SPEC.md sentence and the changelog fragment use
  the same wording. "Your computer's clock" would say it plainly.
- **The hour pickers and the quiet hours priority stay active while the Quiet hours switch is off.** Choosing an hour
  then changes nothing until the switch is on, which the form does not say.
- **A Settings search for an hour such as "23:00", or for "silent", opens Notifications at its top.** The quiet hours
  controls are below the fold there, so you have to scroll to them. The search results for the existing priority and
  grace period choices already behave this way.
- **When a list is opened, the first frame shows its top before it scrolls to the chosen option.** The report mentions
  this too.

Of these, I would take up the note's wording as a small follow-up, and leave the rest unless they bother you in use.

#### Fixes made while landing

None.

#### What the landing made untrue in the report

The report's "Things you should know" says a helm running on a server uses the server's time zone, which may not be
yours, and its follow-ups suggest showing the helm's time or zone beside the hour pickers for that case. A helm runs
only inside the desktop app (the standalone helm is gone), so quiet hours always follow the clock of the computer the
app runs on, a Mac or a Linux machine, and that follow-up has no case to serve. Likewise, the report's notes on an app
from before this change talking to a newer helm, or the other way round, describe what the code does, but app and helm
always ship together, so you will not meet either case.

#### Checks

Run once, on the plan's two PRs as they landed (main had gained only queue bookkeeping). The plan changes the helm's
ntfy alerts and settings, the local time code shared with the desktop app, the app's Settings and search, and the select
component every select list in the app uses, so the whole Rust workspace's tests ran, on three boats, with the desktop
app's end-to-end smoke on a fourth. The rest ran locally:

- `cargo clippy --all-targets`, `cargo clippy -p farhelm --bins` and `cargo clippy -p farhelm-desktop --all-targets`,
  without `-D warnings`, because one warning already on main since #5120 (an unused method in the helm's ntfy settings
  code, which the report also mentions) would fail that flag. That was the only warning. With `-D warnings`,
  `cargo clippy -p farhelm-gui -p farhelm-proto --all-targets`: clean.
- The whole workspace (`cargo nextest run --workspace --exclude farhelm-desktop`, from one archive split into three
  parts), recorded runs 3305f8d0-be2f-47f7-94b9-aaaf1cfbe0f0, c83792cc-a517-466b-9287-5398b3a33bff and
  cbc544df-ae9a-46c0-8c90-e13aead5358e: 9167 tests, 9165 passed, and no test reported that it skipped for a missing
  system service. One failure is the test-trace macro's compile contract, which always fails on a boat; run locally
  (`cargo nextest run -p farhelm-testtrace-macros --test compile_contract`), recorded run
  9b493e37-8426-47e0-b563-98ce68d4defa, it passed. The other is
  `session_lifecycle::a_scoped_stop_leaves_jobs_in_a_session_started_stock_tmux_server`, in run
  cbc544df-ae9a-46c0-8c90-e13aead5358e, described under "What landed with it": run again alone, it passed 20 times
  locally, with the machine's tmux built with systemd, so none skipped (recorded runs
  a18c18da-fd57-4414-9e8a-3bf92eab32be, 4586cceb-5927-426f-a932-c58bcbf1e46c, f175e68a-89f4-47f4-9149-3462e16bbd7f,
  8d6f004f-483d-4a7c-9cd0-f67cbed46905, 7d839e93-254c-436a-9bb4-5a67e5c4fa51, f8dc3063-06be-448b-b20e-477e5fe34465,
  47e660b8-352b-4694-b1af-e67e691ec077, f937d5a2-f168-4a83-965b-082d0ad36846, 617d9eb8-b38c-4a94-9b57-33e1d194e3cd,
  52cc2dd9-d1c4-4763-8dbc-93faafcf9f35, 43628394-1cbd-4320-a6cf-70607b84d4a0, f690f42d-32ef-421e-a75c-7a503bcec5cf,
  c5e370d5-ac6e-44b7-a12c-92f1cc238a3c, 4168d888-3f6c-4d88-8267-f8fa491d2187, b7bc7f6d-d0dc-4981-a7d5-cf0238a3c07c,
  dc52bcd3-33ad-4ef7-9186-cff94ba7557d, a96b5c7b-a5c2-46b0-984f-77be5af44d0a, 3db05e42-0147-4800-82a6-fe75b1fcac92,
  fbf25317-aceb-4a80-bdf1-e53ba24dcf73 and 45ee1697-a7e7-473a-8179-58a63806d2fb), and 6 times on the boats, again with
  none skipped (recorded runs 9a8b5827-49fd-4b22-b7f4-02e6caaecee4, c048735e-1d22-4a3e-8621-306871cd05ef,
  cc2b3a32-d398-4bd6-a52b-bd3433457a6e, 73b2df05-a7ca-40d8-a71f-66670cb7d6ae, 8bf8367e-7a19-45b7-8898-9420b0dbd826 and
  81cb32ab-35e2-4583-b9f4-474cf2ff14c2).
- The desktop app's own tests (`cargo nextest run -p farhelm-desktop`, for the moved time zone code), recorded run
  31d005d2-cf96-4ecb-b5d6-0398866d12ac: 182 passed.
- The desktop end-to-end smoke (`scripts/gui-e2e-smoke.py`, headless on Linux), recorded run
  46b7109d-87cd-4bf3-8de8-25376cce4330: 49 ran and passed, 1 skipped (the paste test, which cannot own the clipboard in
  a headless session).
- The screenshot goldens (`python3 scripts/check-gui-goldens.py`), recorded run 4c47adae-6d84-44e5-8cc9-42a96a3b7c50: no
  differences from the published references, the plan's six re-rendered Notifications pictures included.
- `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`), `dprint check` on the changed
  Markdown and docs pages, `python3 releasing/check-changelog.py format` and the docs site build (`bun run build` in
  `website/`): passed.

Reused: nothing. Skipped: the opt-in tests (real agents, the real Podman and SSH variants), since the plan touches none
of what they cover; any send to a real ntfy server, since the helm's tests send to a stand-in server and check each
request's priority; and any run on a Mac, since none was available to the landing.
