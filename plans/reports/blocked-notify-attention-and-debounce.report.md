### What this was about

When an agent can't finish what you asked, it runs `farhelm agent notify "<reason>"`. Until now that only put a notice
in the notifications bell on the navigation rail, plus an ntfy alert if you had that switched on. The session itself
didn't turn red, didn't rise in the list and made no sound, unlike an agent that asks a question on screen. Separately,
an ntfy alert went out the moment a session started waiting, even when you were looking right at that session and about
to answer.

You decided:

- A blocked agent's session should look and sound exactly like a waiting one, with no new banner or hover text. It stays
  that way until you switch to it or type into it, or the session ends, stops, restarts, is replaced or is deleted. A
  question the agent shows on screen still clears only when the question goes away.
- The in-app alert happens whether or not ntfy is on. "Ask agents to notify when blocked" moves out of the ntfy settings
  into its own place.
- ntfy alerts, for both blocked and waiting sessions, wait out a grace period, 10 seconds by default and off at 0. Only
  direct evidence that you are attending to that particular session cancels one: switching to it, or typing, clicking or
  scrolling in it while it is open in the active window. Activity anywhere else, or just having it open, does not.

What was built: all of that, in three PRs.

- The bell notice is gone. The helm now keeps a "blocked" mark on the session, which survives a helm restart. The
  session list treats a marked session as waiting everywhere a question-asking session shows: the red indicator, the top
  of the activity order (even through a mute), the Waiting filter, a collapsed group's mark, and the waiting sound under
  your existing sound settings. Delete, Restart, Replace, Suspend and sending a prompt still judge the session by its
  real status.
- Settings → Notifications now has a "Blocked agents" card (the ask switch and the criteria text) beside the "ntfy
  alerts" card.
- The ntfy alerts card has a new Grace period choice. A held alert is cancelled when you attend to that session, and
  also when the session stops waiting, its blocked mark clears or its host is removed.

SPEC.md, SPEC_impl.md, two docs pages (the session list and Settings) and the changelog entry are updated. Both TODO
entries the plan covered are removed: "Blocked notify should also go red-active and ring the bell" and "Tweak
blocked-agent alerts".

### Things you should know

- **The three PRs must land together.** The first one alone removes the bell notice before the second adds the red
  indicator and sound, so a blocked agent would show nothing in the app in between (the ntfy alert would still go out).
- **Waiting alerts now arrive later by default.** If you use ntfy, a waiting session's alert used to go out about 3
  seconds after the session started waiting. With the default grace period it now goes out about 13 seconds after,
  unless you attend to the session first. This applies without any action on your part, including to settings saved
  before the change. Choose Off for the old timing.
- **Old bell notices for blocked agents are deleted when the helm upgrades.** They are not turned into blocked marks,
  because an old notice says nothing about whether that agent is still blocked, and marking them all would raise alerts
  for questions answered long ago.
- **Existing settings carry over.** Until you next save the ntfy settings, "Ask agents to notify when blocked" reads as
  on exactly when it was effectively on before: ntfy alerts on and "Agent is blocked" on. From the first save on, the
  two are independent.
- **One label differs from the plan's wording.** The ntfy switch for blocked agents keeps its existing label, "Agent is
  blocked", rather than "Send ntfy when agent is blocked". I read the plan's phrase as describing that switch, not
  renaming it. Its help text now points to the new card. This is a judgement call; say if you want the plan's wording.
- **The grace period is a choice of presets: Off, 5 s, 10 s, 30 s, 1 min and 2 min.** A free number field was the
  alternative; the presets read better in the card. The helm itself accepts any value up to 10 minutes (600 s), and a
  stored value outside the presets shows as an extra choice.
- **What counts as attending to a session, by effect.** There are three kinds of evidence, and they do different things:
  - **Switching to the session** (your own pick in the list or the quick switcher, in the active window) clears its
    blocked mark and cancels its held alert. A session the app opens by itself, such as one you just launched, does not
    count. The app reports a switch only for a session that reads as waiting, since only such a session can have an
    alert held.
  - **Typing into its terminal** (keys and pastes, and clicks or drags when the program there takes mouse input),
    including in its shell tabs, also clears the mark and cancels the held alert.
  - **Other activity in the open session** (a key, click or scroll while it is open in the active window and nothing
    such as a dialog or menu covers it) only cancels the held alert; the red mark stays until you switch to the session
    or type into it. The app reports this at most once every 2 seconds per session. A report held back by that limit is
    sent late with its age, so the helm dates it to when you acted.
  - **Nothing else counts.** Focus changes, moving the pointer without a button, and the scroll wheel are not counted
    from the terminal's own input. The app sends some of these by itself, and the helm can't tell which window they came
    from. Scrolling counts only through the app's report above, which checks the window is active.
- **A brief connection drop can lose a waiting alert, but not a blocked one.** When a host's connection drops and comes
  back, the helm treats a session that was already waiting as already alerted, so a waiting alert it was still holding
  is lost, not delayed. This was already true within the 3-second settle before; the risk now lasts for the whole grace
  period, so at the 2-minute preset a short blip during those 2 minutes drops the alert. A blocked mark survives a
  reconnect, and so does its held alert.
- **A narrow case where a blocked mark can stay after you act.** If an agent reports blocked again within a fraction of
  a second of your keystroke, that keystroke may not clear the new mark; the next key, paste or click does.
- **Switching to a retired container session clears its blocked mark.** Such a session otherwise allows only Delete, but
  a mark that could never be cleared would be worse.

### Open questions and possible follow-ups

- Possible follow-up: say if you want the ntfy switch relabelled "Send ntfy when agent is blocked" (above).
- Possible follow-up: a waiting alert held when a connection drops is lost (above). Making a reconnect keep it would
  mean the helm remembering, across the drop, which sessions had an alert still held.
- Possible follow-up: clippy with warnings as errors still fails for anything that builds the helm, because of the ntfy
  settings-saving function that only the helm's tests call (it was already so on main before this plan; the hosts
  Tailscale column plan's report also mentions it). This plan doesn't use or change it.

### The PRs

- #5172 `feat: mark a blocked agent's session instead of a bell notice`: the helm's blocked mark, how it clears, the
  attention signal, removing the bell notice and its old rows, specs, docs pages, the changelog entry.
- #5174 `feat: show a blocked agent's session as waiting in the app`: the app's waiting indicator, order, filter, group
  mark and sound for a blocked session, the switch signal, the Blocked agents card, specs, docs, screenshots.
- #5178 `feat: hold ntfy alerts for a grace period the user can cut short`: the grace period, its setting, cancelling
  held alerts, the app's interaction signal, specs, docs, screenshots, and both TODO entries' removal.

### Checks

Run on the stack rebased onto the latest main. Since the work started, main gained the terminal tab shortcuts (Cmd+T and
Cmd+W), always asking before deleting a session with open tabs, the command-line parsing fixes, the hosts Tailscale
column, the file hover size hint and several drift tests. The only conflict was a TODO.md line, resolved by keeping both
removals. The shortcuts touch the same session pane as the new interaction signal: a key pressed in an open waiting
session counts as attending to it, which is the intended rule. None of the rest touches what this plan changes.

Just before delivery main also gained the terminal tab overflow controls (a pinned main tab, scroll arrows, edge fades
and an all-tabs menu), and the stack was rebased onto that too, again with only a TODO.md line to resolve. Those
controls sit in the same session pane as the app's activity report, which only listens and never blocks a click or
scroll, so they keep working and a click on them counts as activity in that session. After this second rebase:

- the app's session pane, terminal tab, session list and Settings tests: run 796a8a62-50aa-4399-83b7-b9a020005fe3, 1482
  passed;
- every reference screenshot rendered again and compared: run 97c0462c-93f4-4107-8a5a-d21608d750a0, all matched.

The second rebase changed no helm, protocol or command-line code, so the runs below still apply there.

- After the first rebase, through the test-run recorder, run e0c5243f-9d25-4e15-a769-2cce70705770 (1208 passed):

  ```
  cargo nextest run -p farhelm-gui -p farhelm-helm -p farhelm --lib --test agent_cli --test e2e -E 'test(/^session_pane::/) | test(/^session_list::/) | test(/^settings::/) | (package(farhelm-helm) & (test(/^attention::/) | test(/ntfy/) | test(/notify/))) | binary(agent_cli) | test(/^agent_listing_real_stack::/)'
  ```

  That is the app's session pane, session list and Settings tests, the helm's attention, ntfy and notify tests, every
  agent command-line test, and the end-to-end test that runs `farhelm agent notify` against a real helm and supervisor.
- On the final code before either rebase:
  - every helm and protocol unit test: run 2a353c7b-7dda-4dcd-8303-e8dfdd1e0d20, 2327 passed;
  - the app's session list, session pane, sounds, Settings and notifications-bell tests: run
    916e4379-6047-4861-bfc1-f948140e5e30, 1134 passed;
  - the throttle test after its last change: run 441290b1-9257-4630-97d0-c07adae3d675, 4 passed.

  Nothing in those areas changed in either rebase apart from TODO.md lines, so they still apply.
- The two skipped tests in the helm run are the macOS-only real-machine tests, which never run on Linux.
- The app's reference screenshots: the Settings → Notifications pictures changed (the new card in #5174 and the Grace
  period row in #5178, light and dark). Each changed picture was inspected and published with its PR, before the
  rebases. Main changed two other pictures meanwhile (the keyboard shortcuts dialog and the file hover card) and no
  Settings code. After the first rebase, every Settings scene was rendered again and matched its published picture (run
  c0a09abd-fb59-449c-92e9-7c25fc9f70c5).
- After the first rebase: `cargo clippy` with warnings as errors on the app, protocol and command-line crates: clean.
  Before either rebase, on the final code: the helm's test target clean too; the helm library fails only on the
  pre-existing warning above.
- On the final code before either rebase: `cargo fmt`, the test-sleep check (no unannotated delays), `dprint check` on
  the changed Markdown, the changelog format check and the docs site build (all internal links valid): all clean. The
  rebases changed only a TODO.md line of this plan's own diff.

Earlier failed runs, all fixed and none a flaky test: a helm run where a schema fixture lacked the new table
(752b4fa7-617d-47d9-a581-cbba7a60c48e), a helm run where a new test's fixture lacked a table
(b2b256fc-5138-4aeb-8a7c-043c818e7bd2), an app run where new Settings tests clicked switches the new card had pushed out
of view (5984fe8c-2e5a-42c4-80d1-3c21d413361e), a helm run that caught a missing withdrawal on host removal
(38a2864b-a4cc-4914-a26a-342dbb7ad381), and three runs of a new quick-switcher test while it was being written. Earlier
passing runs are superseded by the ones above.

Skipped: the workspace-wide run and the remaining end-to-end suites. The change is confined to the helm's session
listing, its alerts and the app's session list and Settings, all covered above.

### Review gate

Each PR was reviewed by a fresh-context Opus 5.5 agent at high effort, every finding was fixed, and the same reviewer
verified the fixes (twice for the first and third PRs).

- First PR: no blocking bugs. Main fixes: marks could have become impossible to clear if reading them at startup failed;
  a keystroke could wait on the database; the docs still mentioned the bell; the pointer moving or the wheel counted as
  attending; one place now handles every mark that is cleared.
- Second PR: one real bug: picking a blocked session in the quick switcher, when the session list's filter hid it, did
  not clear its mark. Also wording fixes, including the new card's help text.
- Third PR: one real bug: a dropped connection threw away the held alert of an agent that was still blocked. Also tests
  that could pass for the wrong reason, a small timing gap at notify, and the late attention report being dated on
  arrival rather than when you acted.

### Landing

#### What landed with it

Nothing reached main between the executor's last rebase of this plan and its landing, apart from the queue's own
bookkeeping. It was the only plan in this round, and its three PRs landed one after another within a minute, so nothing
was built or released from main with the first one's removal of the bell notice but without the second one's red
indicator.

An independent reviewer and I read the change against what was already on main, mainly the terminal tab strip's overflow
controls (the terminal-tab-overflow-ux plan, landed just before this one) and the shell tab shortcuts (the
terminal-tab-shortcuts plan), which sit in the same session pane as this plan's activity report. The activity report
only listens: it does not take clicks, scrolls, keys or focus from the tab strip or its controls, and a click on the
strip's own controls counts as activity in that session, which fits your decision that activity inside the open session
counts. We also checked:

- **The helm's database step.** It adds the table that holds blocked marks and deletes only the old blocked-agent bell
  notices; every other notice stays. Main had added no database step of its own since the plan was based, so nothing
  needed renumbering.
- **The protocol.** No protocol version change is needed: everything the plan changes between programs passes only
  between the helm and the desktop app, which always come from the same build. What the helm and its supervisors say to
  each other changed only in comments.
- **Saved settings.** Saved ntfy settings from before the change read as the report says, and a saved configuration
  without a grace period reads as the 10 second default.
- **Clearing the mark and cancelling held alerts.** Stop, Restart, Delete, Replace, a host's removal and a dropped
  connection each clear the mark or cancel the held alert as the report describes.
- **The session list's other readers.** The session list's new "blocked" field breaks none of them.

We found nothing that had to be fixed before landing.

Worth knowing, from the review:

- **Quitting Farhelm during the grace period drops the held alert.** Held alerts live only in the running helm, which
  runs inside the app. If an agent reports blocked (or a session starts waiting) and you quit Farhelm, or it restarts
  for an update, before the grace period ends, the ntfy alert is never sent. Before this plan it went out within about 3
  seconds, or at once for a blocked report. The report does not mention this. You could accept it as a consequence of
  the grace period, or ask for held alerts to be sent at once when the app quits.
- **Clicks and keys inside the all-tabs list do not count as activity.** The list is drawn above the session pane rather
  than in it. The click on `▾` that opens it does count, so in practice you have already been counted.
- **Lowering the grace period does not shorten alerts already held.** Choosing a shorter grace period, Off included,
  does not release alerts already held: each still waits until the time set when it was held. Turning ntfy alerts, or
  that kind of alert, off does drop them.
- **A narrow window after Restart.** The mark is cleared after the restart succeeds, so an agent in the new run that
  reports blocked before that clear would lose its new mark, and its ntfy alert with it. Agents take seconds to start,
  so this is unlikely.
- **Two new tests claim slightly more than they check.** One says a held blocked alert survives a dropped connection,
  but it stops looking as soon as the host reads connected again, before that first refresh's own clean-up has run, so
  it does not show that refresh keeps it too. The other says a second, cut-short listing leaves the vanished session's
  mark alone, but it moves on as soon as another mark from that listing clears, possibly before the listing has judged
  that session, so it would not notice the mark being wrongly cleared there. In both cases the code does what the test
  describes; only the tests check less than they say.

Of these, I would take up the quit question if phone alerts matter to you when you leave the desk, and leave the rest
unless they bother you in use.

#### Fixes made while landing

None.

#### What the landing made untrue in the report

The report's Checks section says that "a key pressed in an open waiting session counts as attending to it". That holds
for keys that reach the terminal, but not for the app's own shortcuts: reading the code, a key that a shortcut handles
(the new-tab and close-tab shortcuts, the quick switcher, Rename, quick launch and the others) never reaches the
activity report, so pressing one in a waiting session does not cancel its held alert. The error runs the safe way: at
worst an alert goes out that was not needed. Its statement that a click on the tab strip's controls counts holds for the
strip itself, but not inside the all-tabs list, as above.

#### Checks

Run once, on the plan's three PRs as they landed (main had gained only queue bookkeeping). The plan changes the helm's
session listing, database and alerts, the channel between the helm and the desktop app, and the app's session list,
session pane and Settings, so the whole Rust workspace's tests ran, on three boats, with the desktop app's end-to-end
smoke on a fourth. The rest ran locally:

- `cargo clippy --all-targets`, `cargo clippy -p farhelm --bins` and `cargo clippy -p farhelm-desktop --all-targets`,
  without `-D warnings`, because one warning already on main since #5120 (an unused method in the helm's ntfy settings
  code, which the report also mentions) would fail that flag. That was the only warning. With `-D warnings`,
  `cargo clippy -p farhelm-gui -p farhelm-proto --all-targets`: clean.
- The whole workspace (`cargo nextest run --workspace --exclude farhelm-desktop`, from one archive split into three
  parts), recorded runs ca887ae2-f7da-4f9e-bfe1-dc33579f1bd2, 8ba8de46-0cc8-49d6-8528-d239d909b19f and
  86ecfa46-1178-4ab9-9b05-b289df061204: 9149 tests, 9148 passed, and no test reported that it skipped for a missing
  system service. The one failure is the test-trace macro's compile contract, which always fails on a boat; run locally
  (`cargo nextest run -p farhelm-testtrace-macros --test compile_contract`), recorded run
  18007705-c25e-4ad8-9524-6773b51c14f4, it passed.
- The desktop end-to-end smoke (`scripts/gui-e2e-smoke.py`, headless on Linux), recorded run
  b6983d0c-89dc-42ac-a55c-47cac5fd7077: 49 ran and passed, 1 skipped (the paste test, which cannot own the clipboard in
  a headless session).
- The screenshot goldens (`python3 scripts/check-gui-goldens.py`), recorded run bc5e56df-8fb4-46cd-bc8b-b6a8c9525ad3: no
  differences from the published references, the plan's changed Settings pictures included.
- `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`), `dprint check` on the changed
  Markdown, docs pages and session-list contract file, `python3 releasing/check-changelog.py format` and the docs site
  build (`bun run build` in `website/`): passed.

Reused: nothing. Skipped: the opt-in tests (real agents, the real Podman and SSH variants), since the plan touches none
of what they cover; and any run on a Mac, since none was available to the landing.
