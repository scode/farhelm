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
