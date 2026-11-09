# Sandbox sessions

## Since the last review

The same three draft PRs are retained. The backend PR, #3264, now waits for the new sandbox's connection to settle
before its session task calls ordinary create. It ignores Connecting and the held setup state; a settled refusal keeps
the ordinary state-specific error and cleans up the fresh resource. Ordinary creates retain their immediate refusal
behavior. The landing monitor's fixes remain in all three branches.

The isolated scripted launch failed before this repair. Normal production attachment already waits for a connection; the
reproduced race used injected setup, which finishes before the real connection worker. The task now checks its own
connection precondition as well, including a connection lost after attachment. No new failure was reproduced against
real Tensorlake. This repair adds no timeout, timed create retries or recovery protocol.

## What this was about

A session can launch into its own named Tensorlake sandbox from a recipe, directly or through a template. Clone creates
another sandbox from that recipe. Individual Suspend, Resume, Stop and suspend, and deliberate Delete control it.
Ordinary Tensorlake sandboxes added as hosts remain supported.

The maintainer replaced the earlier membership-proof design with whole-sandbox Delete, preservation of existing work
when Replace fails, and agent refusal in the helm. The drafts implement that smaller contract; the obsolete membership,
handshake, supervisor-policy and notification machinery is removed.

## Things you should know

- **Delete destroys the entire sandbox:** every session and process, files, checkout, unpushed work and snapshots,
  including an unlisted session left by a failed Replace. It always confirms and works while suspended or disconnected,
  subject to the ordinary provider and credential requirements. Failed cleanup stays visible for an explicit retry.
- Replace stays in the existing sandbox and removes its source session only. A failed or ambiguous Replace preserves its
  sandbox and files, identifies it and reports what is known. Rare failures may require manual recovery. A failed first
  launch may instead discard only the new sandbox it created, including after a lost create reply.
- Plain Suspend preserves processes. The fixed 30-minute idle timeout starts after Farhelm's connection ends; a
  forgotten connected session can keep billing. Snapshot storage bills while suspended. Automatic resume is disabled.
- Session-owned sandboxes appear in Hosts and the ordinary update, problem and progress area. They have no permanent
  navigation-rail icon and are not destinations for unrelated launches. Agents in them cannot ask Farhelm to act; the
  owning helm enforces that restriction without a notification or stored supervisor policy. Other agents cannot create
  or destroy billed sandboxes through these sessions or recipe templates.
- This is experimental. Local SSH-key presence does not prove account registration; a refused key shows the registration
  remedy and attempts cleanup. Claude subscription logins may encounter first-run setup in each new sandbox. The
  Tensorlake sandboxes documentation describes launch, billing and deletion.

Prior real verification used **four sandboxes out of the approved whole-plan limit of 100**: a builder, a preparer, the
original session and its Clone. All four were terminated, no attributable snapshots remained, and the test SSH
registration and isolated helm state were removed. This repair created no cloud resources and did not use the live
installation. The earlier cleanup evidence was inspected again.

## Open questions and possible follow-ups

There is no blocking product question. The approved deferrals remain recipe recent setups, Save as template from a
sandbox launch, and bulk Suspend, Resume and Stop and suspend. Recipe templates and individual controls work now.
TODO.md tracks these, per-repository warm snapshots with incremental fetch, session resize using future shared host
support, and possible shared scratch volumes and warm forks. Existing agent-trust and automatic-suspend follow-ups
remain. No new management or recovery framework was introduced.

## The PRs

- [#3263](https://github.com/scode/farhelm/pull/3263/changes): the amended product and implementation contracts.
- [#3264](https://github.com/scode/farhelm/pull/3264/changes): ownership, recipe launch, connection readiness, cleanup
  and helm-side agent refusal.
- [#3265](https://github.com/scode/farhelm/pull/3265/changes): launcher and lifecycle UI, templates, browser coverage,
  documentation, the experimental changelog entry and final TODO follow-ups.

## Checks run, reused and skipped

All repair runtime checks used the recorder. Rust used pinned nextest 0.9.148 and required tmux validation; browsers
used controlled Playwright mode. Both runners used zero retries. Evidence discovery is complete for 44 recorded
commands: 43 passed and the unchanged baseline failed. All output scans completed without runtime skip markers. Case
counts below are executions, not distinct tests across overlapping selections.

**Run in this repair:**

- The unchanged isolated scripted launch failed in `59c79457-0fdb-4631-a795-2690ee12c3b7`, before the fix. Its trace
  showed setup finishing before the connection worker reached Connected. The failure remains retained.
- `cargo nextest run -p farhelm-helm --lib -E 'test(recipe_session_task_)'`: `cc462578-b5b2-428a-a3a4-4727e1e3fe6e`, **2
  passed**. These exercise successful creation, connected-create refusal, and settled unreachable/version-skew errors
  with fresh-resource cleanup.
- The required exact scripted launch test ran alone twenty times in batch `23ee33c3-d0b3-48c7-b50c-69c33bec0602`:
  **20/20 passed**. A continuous CPU sample covered the batch after other heavy jobs ended; no other build or test was
  launched alongside it. Source was `0cafcd803add6a6ed67270de94b8489db4d02a7d` plus the final two-file working diff,
  fingerprint prefix `a92bca5f1d17…`. A previous batch, `952febc0-d7ce-452f-9655-759dee7dc721`, also passed 20/20 under
  neighboring load and is supplementary evidence, rather than the required quiet run.
- Browser selection `tensorlake-(sandbox-sessions|recipes)\.spec\.ts`, run `f6b54009-7860-49b2-b9ac-19bffebe870a`: **40
  passed**, 20 each on Chromium and WebKit, three workers, zero retries and no skipped cases. It uses the final source
  fingerprint above, with complete reports and output.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed.
- Current shipping CLI and test fixtures built successfully. The pinned Dioxus 0.7.10 web build passed. Rust formatting
  and the test-delay check passed; the latter inspected 429 delays with none missing a rationale.

**Reused with the stated limits:**

- Full `cargo nextest run -p farhelm-helm`, run `c4de7f98-c54d-4a4b-88bd-34d4d244b100`: **1,673 passed**; two explicitly
  ignored remote macOS checks were not run. Local and SSH provisioning cases ran. Source fingerprint was `12631c9e…`;
  the only subsequent edits were comments, docstrings and a test failure diagnostic. Successful behavior and assertions
  are unchanged, so that full-crate coverage still applies. The focused two-case run has the same reuse boundary. The
  quiet repetitions and browser selection use the final source.
- Earlier real-sandbox checks used source `05488a7fcbea087b61cc8893cc390c42d6ded44a`, fingerprint prefix
  `dfe041b1b8d1…`, through `record-test-run.py … -- deno run -A <private verification helper>`. Relevant backend launch,
  manager, transport, build, session and lifecycle inputs were compared directly: they are unchanged except for this
  state wait and the monitor's correction preserving the owned-row retry timeout. UI lifecycle handlers are unchanged;
  intervening edits add hover and accessibility text. The new wait does not alter confirmed lifecycle operations.

  Retained observations include managed checkout and a command (`fd56a3e8-6f9b-4463-a57d-e85da83f981c`), the same
  process identity/PID and fresh input/output after Suspend/Resume (`a0a028a8-4d24-4484-a86f-d4ab9085f366`), timeout
  1800 seconds with automatic resume off (`b1fb86c2-d02a-47f5-be01-1c0ea17b5077`), a GUI-created Clone on a distinct
  sandbox with the same launch/repository and no original-only file (`e25de53d-9c37-4630-8d1b-f1b04a833008`), confirmed
  GUI Delete while suspended (`196008a5-42aa-4c08-bcbc-bf93acaadeb9`), and complete provider/SSH/test-helm cleanup
  (`dc2875dd-b1ac-4021-ba75-e7a3fdef3c00`).

  Stop and suspend was observed previously. Direct proof that the stopped process was absent is retained in
  `c0e7fbeb-8908-4429-9e89-cd6b91dd8b52`, a helper command that subsequently failed on navigation; it is not counted as
  a passing command. Earlier failed private observer attempts remain failed. No running-progress-step or full Update
  verification is claimed. These historical lifecycle observations do not establish that the newly repaired race passed.

**Skipped:** another cloud run, full workspace/browser/desktop batteries, CI dispatch, installer/release checks and
live-install testing. The required local tests and both-engine specs address the connection repair; the unchanged
lifecycle evidence avoids creating more billed resources. No UI, documentation or instruction source changed in this
repair. No draft was marked ready or merged by this executor.

## Review gate outcome

The required single fresh-context Opus 5.5 reviewer at high effort passed the full backend PR with the repair. Four
comment and test-diagnostic findings were corrected. Its initial result covered only the narrow repair; the same
reviewer then read the full backend diff and relevant specifications and completed the gate. The original spec and UI
gates remain applicable to the retained stack. No removed membership or supervisor-policy guarantee was restored.

### Landing

Landed alone, as #3263, #3264 and #3265, on the second landing attempt. Nothing else reached main while the plan was
being landed, apart from planning bookkeeping, so nothing could interact with it.

#### Why it took two attempts

The first attempt (2026-11-09) stopped before merging anything. One of the helm's tests drives a whole recipe launch
against a scripted sandbox. It passed once in a full run on a busy machine, failed in the next full run, and failed
every time when run alone on a quiet one. It exposed a real race. After setting Farhelm up in the new sandbox, the
launch created the session at once, while the helm's own connection to the sandbox was still being made. The helm
refused the create with "host is connecting", and the launch then deleted the sandbox it had just paid for. Launches
against real sandboxes had passed only because something slowed them down enough for the connection to win.

The landing monitor sent the plan back with an instruction to fix the race. The launch had to wait for the helm's
connection state to settle, not retry on a timer, and it must not mistake the "setting up" state that setup leaves
behind for a failure. The executor did that: the launch now waits until the new sandbox's connection has settled before
creating the session. If it settles anywhere other than connected, the launch fails with that state's message and cleans
up the sandbox as before. That scripted test now passes 20 times out of 20 when run alone.

Sending the plan back was a plain instruction to fix a failing test, so it has no entry to revisit. The plan's earlier
block, when the Tensorlake API key was lost, was answered by the maintainer.

#### Fixes the landing made

Made during the first attempt, pushed to the plan's branches, and kept by the executor:

- Hover text on every new sandbox control and mark, which SPEC.md's hover rule requires and the plan had left out: the
  session's Suspend and Resume button, the Stop and suspend actions (in the session row, the session header and the
  suspend question), the suspend question's buttons, the billing question's Cancel, Create and launch and pricing link,
  the Copy registration command button, the sandbox mark and the Suspended badges. The template editor's Host field also
  lost its hover text in this plan; it is back.
- When setup was retried from Hosts on the entry a session's sandbox had left behind, the sandbox that retry created had
  no 30-minute idle timeout, which session sandboxes otherwise get. The timeout now follows whether the entry belongs to
  a session, and a helm test covers that retry.
- The changelog fragment now also claims #3265. It was added in #3264, so the top PR, a feature, had no fragment of its
  own.
- The Tensorlake sandboxes docs page said an ordinary host sandbox suspends after 24 hours without a connection. That
  applies only on Tensorlake's Usage Credits plan, and the page now says so.
- A database-upgrade test broke because of the plan's new record of which sandbox belongs to a session. It now removes
  that record first when it rewinds a database to the older schema.
- The recipe-preparation browser tests still expected the old status wording. The plan changed it to "the first new host
  or session prepares it", and the tests now expect that.

#### Reviews

The first attempt's fresh-context review found the hover-text gap and the changelog gap, both fixed above. It found no
problem with the data the plan adds, its handling of deletion, or how an agent's request to launch into a sandbox is
refused or billed.

The second attempt's fresh-context review read the executor's race fix and found nothing that must change. Its points
worth knowing are listed below.

#### Things you should know

From the second review, left as they are:

- A failed Farhelm install inside the sandbox still lets the launch carry on to creating the session; this bug predates
  the plan. It used to fail at once. With the new wait, it now fails after about 3 minutes and says "unreachable"
  instead of showing the install error. A fresh session launch takes this install path only when its usual way of
  attaching to a prepared sandbox does not apply, so it should be rare, but it is a worse experience when it happens.
  Not recorded anywhere yet; worth a follow-up.
- If the connection drops between setup finishing and the session being created, the launch now waits through one round
  of reconnection attempts, up to about 3 minutes, before failing. Meanwhile it keeps one of the helm's four setup
  slots, which are shared by every host setup, update and sandbox launch, so those may queue behind it.
- The executor's new tests check the outcome but would not reliably fail without the fix. The older scripted test above
  fails every time without it, so the race stays covered.
- Part of the wait, the exclusion of the leftover "setting up" state, never comes into play, because the helm already
  shows "connecting" by then. It is harmless.
- The wait could in theory hang forever, but only if the helm's connection handler for the new host crashed twice at
  specific moments.

From the first review, left as they are:

- Removing an ordinary host now also closes the local ports Farhelm had opened to forward to it. That seems right, but
  SPEC.md does not say so.
- Picking a host in the launcher now saves the launcher's draft again even when nothing changed. The review found no
  visible effect.
- An agent can tell from the refusal it gets whether a recipe template of a given name exists. It is a minor leak to the
  agent and was not worth holding the landing for.
- The page-wide hover-text check (the tooltip-coverage browser spec) never reaches the sandbox states, so no test covers
  the hover text added above.

Two groups of browser failures in the first attempt's full run came from earlier landings on main, not from this plan:

- the host-settings footer text, from the per-host checkout folder change;
- a feed test's reader count, from the trash bell.

Those landings ran only a narrowed browser selection, which is how the breakage got through. Draft PR #3275 fixes both
tests, and its two specs passed on both engines. It waits for the maintainer, because landing a fix outside a plan is
not the landing monitor's to do.

The four single-engine failures counted as flaky below were each seen in earlier landing rounds, recorded in the landing
monitor's log. This landing did not check whether FLAKES.md tracks them.

#### Checks

As the maintainer asked for the landings of 2026-11-09, the browser specs ran on Tensorlake sandboxes (cloud test
machines), and everything else ran on this development machine.

Run now, on the tree that merged (main equals it outside `plans/`):

- Here: `cargo fmt --all -- --check`, the test-sleep checker and
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed.
- Here, recorded `cargo nextest run -p farhelm-helm -p farhelm-proto`: 1917 passed, 3 skipped by design (run
  `d9b84462`). This includes the database-upgrade test the landing fixed and the new idle-timeout retry test.
- Here, the scripted sandbox launch test alone, 20 times: 20 of 20 passed (runs `06e07295` through `828d8de5`). Before
  the fix it failed every time this way (run `e9b3c1be`), and once in a full run (run `210c9ac6`).
- Tensorlake, browser, using the release web build, on Chromium and WebKit: the plan's `tensorlake-sandbox-sessions`
  spec and `tensorlake-recipes`. 40 passed, none failed, skipped or retried (runs `772d796b`, `b4db20c7`, `e2b19319`,
  `2b71cbcc`).

Reused from the first attempt. Those runs covered the plan's stack at its top commit `a10ff502`, which already held
every landing fix except the database-upgrade test and the recipe spec. Since then, the only product code that changed
is the helm's sandbox launch, which the checks above rerun. The two later test fixes are covered above.

- Here: `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `scripts/check-desktop-assets.sh`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop` and the website build
  passed.
- Here, the UI's JavaScript unit tests (`node --test`) passed. They ran directly rather than through the recorder, so
  they have no run id.
- Here, recorded `cargo nextest run -p farhelm-ui --lib`: 937 passed, none skipped (run `fbbe7feb`).
- Here, recorded, the agent request paths in `cargo nextest run -p farhelm` (the agent and spawn command tests, agent
  relay, agent listing against a real stack, prompting a running agent): 84 passed (run `7469c2a4`).
- Tensorlake, the whole browser suite on ten sandboxes: 3126 passed, 14 skipped by design, 24 failed (runs `f026780a`,
  `6da9a8f8`, `8d6dac39`, `a46d65aa`, `70937abd`, `66ea5b44`, `11a04617`, `d80bac06`, `6f073f09`, `140d760f`). The
  failures:
  - 12 were the recipe-preparation wording, fixed by the landing and passing above.
  - 8 were already broken on main: the host-settings footer and the feed reader count (draft PR #3275).
  - 4 were single-engine failures already seen as flaky in earlier rounds: WebKit's popover number popup, WebKit's
    multi-host launcher retarget, Chromium's provisioning incarnation churn, and Chromium's window-chrome version
    double-click.
- Tensorlake, the browser suite's tmux-terminal set on six sandboxes: 107 passed, 1 skipped by design (runs `7d056a13`,
  `78b40b17`, `f9fbad9f`, `ab8f8145`, `89c306ba`, `000aa7b3`).
- The browser runs went through the recorder's general command mode, because its Playwright mode always runs both
  engines and these runs split the engines across sandboxes. The sandbox image runs Node 24, a slightly older release
  than this machine's. Every sandbox reported that the services the tests rely on (a per-user systemd and SSH to itself)
  worked.

Reused from the executor: nothing.

Skipped: the workspace's other Rust tests (the plan changes the helm, protocol, UI and the agent request paths, all run
above), the desktop smoke (no desktop-only code beyond what `cargo check --features desktop` compiles), and hosted CI
(manual-only, not dispatched).
