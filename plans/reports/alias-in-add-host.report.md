## What this was about

The TODO entry "Allow aliasing hosts in add host flow". A host's alias, the name Farhelm shows instead of its ssh
destination, could only be set after the host existed, from its settings dialog. While planning this you also decided
that adding a destination that is already a host must be an error ("this surfaces ridiculous behavior. if someone tries
to add a host that is already added they should get an error"). Until now, such an add quietly set up the existing entry
again and rewrote where it records that host's Farhelm install.

Now the add host dialog has an optional "alias (optional)" field, and the host is added under that name. That includes
the case where Farhelm first has to be installed on the host and the setup question is shown. The alias follows the same
rules as in the settings dialog: the dialog checks length and characters itself, and the helm refuses an alias that is
already some host's displayed name. Adding a destination that is already a host is refused with "this host is already
added as <name>; set it up again from that host's own row", where the name is the alias if it has one. The destination's
help now says "user@host, or a Host name from your ssh config", so the dialog no longer uses "alias" in two meanings.

## Things you should know

- Only the add host dialog's request now tells the helm "this is a new host". A host row's "re-run", a confirmed setup's
  own registration after the install, Update, the startup list of hosts and the hosts API all behave as before. "Re-run"
  is still how an existing host is set up again.
- The duplicate and alias checks happen at three points. The first is before Farhelm contacts the destination over ssh,
  so the person hears at once. The second is when the setup question is confirmed, in case the same destination was
  added from somewhere else meanwhile. The last is inside the write that adds the host, which also stores the alias; on
  the setup path that write happens right after confirming, before anything is installed, and the registration after the
  install only updates where that same host's install lives. Only that last check cannot race a concurrent add or
  rename, and doing it in the same write means a host is never added without its alias.
- "Already added" means the destination exactly as typed, as you decided. Two spellings of one machine are not
  normalized: adding a second spelling of a machine that is already a host adds a second entry, and once it connects it
  says which host already holds that machine and asks you to remove one of them, as before. Nothing ever connects twice.
- A browser tab that was open before the update keeps the old dialog until it is reloaded. Re-adding an existing host
  from such a tab still sets that host up again, because the old dialog sends the same request as "re-run" and the helm
  cannot tell the two apart. The changelog entry says so.
- SPEC.md's host registry paragraph now describes the alias field and the duplicate refusal. SPEC_impl.md's alias
  section says where the refusals happen and which requests carry the new-entry marker.
- The Add a remote host docs page gained a sentence about the alias field and a troubleshooting line about the refusal.

## Open questions and possible follow-ups

- The Add a remote host page's add-dialog screenshot predates the alias field. The next docs screenshot refresh will
  show it; I did not recapture it here.
- The planned follow-up, `agent-add-host` (an agent's "add host X with alias Y" prefilling this dialog), waits on this
  plan and can start once it lands.

## The PRs

- #2666 feat: refuse adding a host twice, and let the add carry an alias (the helm side, SPEC.md and SPEC_impl.md, the
  changelog entry)
- #2667 feat: take an alias when adding a host, and refuse adding one twice (the dialog, browser tests, docs page, TODO
  entry removed)

## Checks

Run in this plan:

- `cargo nextest run -p farhelm-helm --lib -E 'test(/^provisioning::/) | test(/^store::/) | test(/^hosts::/)'`: run
  58b5c05f, 451/451 passed. This includes seven new tests: a duplicate refused before ssh and in the write, a taken or
  malformed alias refused before ssh, the alias kept through a setup confirmation, and a duplicate that appears before
  confirmation refused at confirmation with nothing installed.
- `cargo nextest run -p farhelm-ui --lib -E 'test(/^hosts::/) | test(/^provisioning::/)'`: run a8f2db8d, 102/102 passed.
- Browser tests on Chromium and WebKit, `cd e2e && npx playwright test <five spec files> -g <the tests below>`, run
  3a1ae96f, 40/40 passed. Counts are per test and engine. The selection:
  - every add-dialog, discovery, setup and re-run test in the provisioning spec, three of them new (adding with an alias
    and then the same destination again; a taken and a malformed alias; the alias surviving the setup question);
  - the sidebar's two aliasing tests;
  - the tooltip and button-style sweeps that cover the add dialog;
  - the ssh-help test that opens from the add dialog.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check` and
  `cargo check -p farhelm-ui --features desktop` passed at the tip. The fmt check needed one reformat, now applied.
- `python -B scripts/check-test-sleeps.py` reported 0 delays without a rationale.
- `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown passed.

Reused: none. Main moved only in the plan queue while this ran, so no rebase was needed.

Skipped:

- The full Rust and browser batteries. The change is confined to adding a host, and the suites above cover that and
  every other caller of the changed requests.
- The docs website build. The page edit adds two plain sentences and no links.

## Review gate

One fresh gpt-6.1-sol review at high effort per PR, as the plan requires, with the test-authoring checklist. #2666 had
one finding, a changelog sentence written as an instruction, which was reworded; there were no correctness or design
findings. #2667 had no findings.

### Landing

Landed as #2666 and #2667, merged bottom-up, with the content the report describes and two small wording fixes described
below. It was the only plan in this landing round.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained session-view-fixes-oct22 (session menus, terminal focus, the YOLO launch question
and the outage notice in the GUI), which touches neither hosts nor the add host dialog. A fresh-context reviewer and I
also checked every other way a host gets added or set up again, since #2666 now refuses adding a destination that is
already a host:

- Re-running a failed setup, updating a host, the helm's startup list of hosts it must have (`--ensure-hosts`), adding a
  host through the helm's API, and the test harnesses that register hosts do not go through the new check, so none of
  them is newly refused. The browser tests that add a host again remove it first.
- The duplicate check compares destinations exactly as written, the same way the rest of the helm does; two spellings of
  one machine are still left to the existing same-machine check, as you decided while planning.
- agent-add-host, the plan waiting on this one, is consistent with it.

#### Fixes made during the landing

- The "Add a remote host" docs page told a user whose destination is already a host to use **re-run** in that host's
  menu. A host whose setup succeeded has no re-run there; it has **update**. The landing changed the sentence to "use
  **update** in its `⋯` menu, or **re-run** if its setup failed", and changed "the aliases in `~/.ssh/config`" on the
  same page to "the Host names in `~/.ssh/config`", so that "alias" there no longer means two things; the dialog's own
  help now says "a Host name from your ssh config". Both in #2667.
- The warning the helm logs at startup for a host in its `--ensure-hosts` list with a path starting with `~` told the
  operator to repair it by adding the same destination again in the hosts panel, which #2666 now refuses. The landing
  removed that remedy in #2666; the warning now only says to repair the host "with its set up action" (see the
  follow-ups).

#### Possible follow-ups

- Found at landing: that startup warning's remaining remedy, the host's "set up action", matches no item in a remote
  host's menu, which offers **update**, and **re-run** after a failed setup. It could name the right item; I did not
  change it further because I have not checked which of them repairs a stored `~` path.
- Found at landing, for release curation: the changelog entry's second paragraph is what tells users that adding a host
  that is already listed is now refused; keep it when the entry is shortened.
- Found at landing, for the next docs screenshot refresh: a callout on the "Add a remote host" screenshot still says
  "host alias" for what is now the destination's ssh Host name, next to the new alias field.

#### Checks

Adding a host is a request many Rust end-to-end tests and browser tests make while setting up, and any of them that
added a destination twice would now be refused, which the executor's selections did not cover. So this round ran the
whole Rust workspace and the full browser suite on the plan stacked on this round's main. The first four items below ran
before the landing's wording fixes (a docs page and a log message); the fifth ran after them:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (one warning, about a citation main already had), the docs website build, the
  workspace debug build, the UI's release web build and `scripts/check-desktop-assets.sh`: all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  slots, no retries, run `f21932ee-306b-41fd-b228-57ac27387853`: 4319 of 4320 passed, 40 skipped by design (helper
  entries that only run as child processes of other tests, and tests that need a real signed-in Claude or Codex). The
  one failure, `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, is a known open flake in terminal
  output flow control, recorded in FLAKES.md, which this plan does not touch.
  `cargo nextest run -p farhelm-ui --features desktop`, same settings without tmux, run
  `4776902a-8209-47d8-b8c1-6942b74e26de`: 917 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `0f56085c-045c-42df-9d2d-93583fd67eb9`: 2495 passed, 14 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), 1 failed: Chromium's "the
  sidebar app bar explains the helm and window builds", which failed inside the test's own network interception ("Route
  is already handled!") rather than on an assertion.
- That test, ten Chromium repeats on this plan (run `8000f125-1f43-45c9-9493-63f3d6020c31`): 4 failed the same way; and
  ten on main without the plan, with main's own binaries and web build (run `5888f7a6-44e2-4c33-87fc-6b1afec0fa57`): 3
  failed the same way. Main already fails this test about a third of the time on Chromium, in a part of the app this
  plan does not change. The same error in this test was recorded in FLAKES.md as fixed on 2026-09-05, so this is a
  recurrence; I am recording it as a new dated entry, with a Deflake entry in TODO.md, in a separate draft PR.
- After the wording fixes, run by hand outside the round's check script: `cargo fmt --all -- --check`,
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings` and the docs website build passed again.
- Reused: none. Skipped: the doctests (no documentation example changed), the desktop smoke test (it registers its host
  through the helm's API, which the new check does not cover), the native Mac app (not runnable here) and the installer
  (unchanged).

The report's checks ran the helm's provisioning, store and host tests, the GUI's host tests and the add-dialog browser
tests; this round ran the whole Rust workspace and the full browser suite on the plan before the landing's wording
fixes, then the checks that read those fixes. One sentence in the report was already inaccurate: "re-run" sets a host up
again only when its last setup failed; a host that was set up successfully is set up again with **update**, as the docs
page now says. Nothing else in the executor's report became untrue at landing.
