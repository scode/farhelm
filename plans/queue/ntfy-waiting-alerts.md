# The helm sends an ntfy alert when a session starts waiting

Written against main at 66af18cd on 2026-10-13. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

Today the only sign that a session needs the user is a sound in whichever Farhelm window happens to be open, so nothing
reaches a user who is away from the screen. Let the user configure an [ntfy](https://ntfy.sh/) topic, and have the helm
post a short alert to it whenever a session starts waiting on the user.

Facts from planning that shape the design:

- Only the helm sees status changes when no window is open, and it sees every session's status change in one place: the
  host actor's refresh in `crates/farhelm-helm/src/manager.rs` (`refresh_once`), whose identity-backed path goes through
  `HelmStore::replace_host_sessions` (`store.rs`); hosts without an identity take an in-memory path
  (`retain_unknown_list_status`). The cache's previous rows survive a helm restart, so they are not the place to take
  transitions from (see the outline).
- An agent's own approval prompt shows up as status `Waiting`. Farhelm's own approval cards are different: the helm
  refuses them when no window is open (`approvals::ask`, `NO_GUI_REFUSAL`), so they are not part of this plan.
- Waiting detection is heuristic (SPEC.md, Status): Claude and Codex have dedicated screen readers; OMP and Grok never
  report waiting. A reading can flicker.
- The desktop app's embedded helm stops when its window closes, so for desktop-only users alerts arrive only while the
  window is open but not in front. A standalone helm keeps sending with no window. Say so in the docs.
- "Notifications" already names the per-session bell in Farhelm
  (`crates/farhelm-supervisor/src/service/notifications.rs`, SPEC.md Status). Give this feature a distinct user-facing
  name (for example "phone alerts" or "ntfy alerts") and use it consistently; log the DECISION.

Acceptance criteria:

- Settings dialog: a section for these alerts with an on/off switch (off by default), a server URL (default
  `https://ntfy.sh`), a topic, an optional access token, a priority (ntfy's min, low, default, high, urgent; default
  "default"), and a **Send test** button. The settings are helm-wide (shared by every client) but are helm
  configuration, NOT part of the shared preferences row: they get their own route (for example
  `GET`/`PUT /api/ntfy-alerts` and `POST /api/ntfy-alerts/test`), saved explicitly with refusals shown inline, following
  the precedent of the managed-checkout folder and post-clone command (`/api/checkout-root`, `CheckoutRootField` in
  `crates/farhelm-ui/src/settings.rs`). The preferences write path is fire-and-forget with replay after sign-in, and the
  preferences row is fetched on every load and echoed back as stored, so it fits neither a validated server URL nor a
  secret.
- The token is a secret: `GET` returns only whether one is set; the UI allows replacing or clearing it; no log line,
  error message, refusal or API response ever contains it. The topic is never logged either (anyone who knows an ntfy.sh
  topic can read it): reqwest errors are logged only after stripping their URL (`reqwest::Error::without_url()`), since
  the URL contains the topic, and ntfy's response body (which names the topic) is never logged or shown.
- **Send test** posts one alert directly with the saved settings (not the unsaved form, which never receives the stored
  token), bypassing the queue, policy and rate cap, and waits for that one post under the sender's short timeout so it
  can report success or the failure reason inline. This user-initiated request is the one place that waits on ntfy.
- When enabled and configured, the helm posts one alert when a session's status becomes `Waiting` from anything else,
  once per waiting spell, and only after the waiting status has lasted a short settle time (planner proposal: about 10
  seconds; choose within 5 to 30 and log it) so a flickering reading does not alert. A session that stops waiting before
  the settle time sends nothing. Leaving and re-entering waiting after the settle is a new spell. A session first seen
  already waiting after the baseline counts, as it does for the waiting sound.
- No alert on the helm's first observation of a host after start or reconnect (a session already waiting then is a
  baseline, not a transition), and none for `Unknown` readings (no new evidence).
- A fleet-wide rate cap applied in the single sender task (planner proposal: at most a handful per minute). Excess
  alerts are dropped with one log line per window, and the next alert that goes out may say "(and N more)". No delayed
  summary alert.
- The alert reads like "<session title> on <host> is waiting for you": the session's own title, sent as is after
  stripping control characters and capping the length (titles are agent-controllable text; a title Farhelm generated
  from a folder name is still just the title, not filtered), and the host's display name. Never screen content, prompts,
  working directories, command lines or paths beyond what the title itself says. The ntfy title is "Farhelm". The
  configured priority goes in ntfy's priority header.
- Background sending never blocks or slows the refresh path, terminal I/O or any request: alerts go through a bounded
  queue to one sender task with a short timeout and no redirects, and a failed or slow send is logged (without topic,
  token or title) and dropped, never retried in a loop. A full queue drops with a log line.
- SPEC.md: narrow the Non-goals (the list entry and the Status section's "Desktop or operating-system notifications …
  are not part of v1") so ntfy alerts are in and other notification kinds stay non-goals; describe the feature in
  Status, next to the sounds, and its settings where SPEC.md describes the settings dialog (Session list, the paragraph
  about the gear); and add the new outbound connection to the Security section's list. SPEC_impl.md gets the mechanism
  (where transitions are observed, settle, cap, sender, secret handling, the settings route).
- The docs website gets a short page or section on setting it up (`website/AGENTS.md` and `website/EDITORIAL_RULES.md`
  govern), including the desktop-window caveat and that anyone who knows an ntfy.sh topic can read it.
- Changelog fragment, `kind: added`. The TODO.md entry is removed.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: alerts for approval cards, turn-finished alerts, suppressing alerts while a window is focused (the
maintainer chose "always send"), per-session or per-host mute, other notification services, OS notifications, and deep
links back into Farhelm.

## Requirement sources

**The user's request (2026-10-13):** plan this Near term TODO entry. Its text:

> **Send ntfy.sh notifications when an agent is blocked.** Today the only alert that a session needs the user (it starts
> waiting, or an agent asks for approval) is a sound in whichever client happens to be open, so nothing reaches the user
> when no Farhelm window is in front of them. Support sending a notification through ntfy.sh (a topic the user
> configures) when an agent becomes blocked on them. SPEC.md currently lists desktop and operating-system notifications
> as a non-goal for v1, so this needs a SPEC change alongside it.

**The user's decisions (2026-10-13):**

- Triggers: a session starting to wait only. Approval cards and turn-finished are out.
- Settings: server URL (default ntfy.sh), topic, optional access token, on/off and Send test; and, added by the
  maintainer: "also allow configuring criticality/priority".
- Content: session title plus host. The maintainer noted the planner's example did not look like a real session title;
  use the session's actual title.
- Always send, with once-per-spell, a settle time and a fleet-wide rate cap; no focus-based suppression.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md Status (statuses, sounds, heuristic waiting detection), Session list (the settings dialog, shared versus
  per-device settings), Errors and diagnostics (failed operations surface), Security (outbound connections), Non-goals;
  SPEC_impl.md "Feedback forwarding" (the outbound-HTTP pattern to follow).
- `TRIAGE_OUTCOMES.md` records a finding that the plans flow leaked a private ntfy topic through curl's arguments to
  other local accounts; the helm posts in-process, but the lesson stands: the topic and token never go on a command
  line, in a log, or in an error.
- Root `AGENTS.md`: Conventional Commits; Finishing work; Releases and the changelog; Docs website; Talking to the user;
  The live install is off-limits; Sharing the machine with other agents (tests bind port 0, never a fixed port).
- Global instructions: tests never modify the test process's own environment variables; inject the HTTP client, server
  URL and clock.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the outline below, the settle time, the rate cap shape and storing the token helm-side in its
own settings, never returned by the API (authenticated clients are already fully trusted with the fleet, so that is
enough; it is not encrypted at rest, like the rest of helm.db). The planning review moved the settings out of the
preferences row, observed transitions in the connection loop instead of the store, and dropped coalescing timers.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- Status pipeline: supervisor screen readers (`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`), sampled by
  `service/ticker.rs`, summarized in `service/status.rs`, hinted to the helm (`service/hints.rs`); the helm's host actor
  (`manager.rs` `serve` → `refresh_once` → `HelmStore::replace_host_sessions`, which reads the previous `session_cache`
  rows and computes `changed`).
- Sounds are client-only (`crates/farhelm-ui/src/sounds.rs`, `assets/sounds.js`, `js-tests/sounds.test.js`): useful as
  the reference for which transitions count as "became waiting".
- Helm configuration with its own route and explicit save: `/api/checkout-root` and the settings dialog's
  `CheckoutRootField` (`crates/farhelm-ui/src/settings.rs` calls these "helm configuration, not preferences"). The
  shared preferences (`store.rs` `Preferences`, `GET`/`PUT /api/preferences`, `api::store_preference`) are NOT the
  model: their writes are fire-and-forget and the row is echoed back as stored.
- Outbound HTTP: `crates/farhelm-helm/src/feedback.rs` `FeedbackForwarder` (`production()` / `offline()` /
  `with_client()`, held in `AppState`, 15 s timeout, no redirects) and its tests against a stand-in server on
  `127.0.0.1:0`. Reuse its reqwest setup, but not its error logging as is: it logs reqwest's error text, which includes
  the URL, harmless only because the feedback URL is constant.
- The helm refreshes each connected host every few seconds (`REFRESH_INTERVAL`) and at once on a supervisor's change
  hint, so a settle check on each refresh needs no timers. `farhelm-testtrace` is the repository's way to capture logs
  in a test.

Here's the work, as a proposal:

1. **Helm side.** The settings route and storage (validation: http/https server URL, a topic grammar ntfy accepts,
   length caps, priority enum; the token never returned), an `AppState`-held sender with an offline default for test
   helms, the Send test endpoint, and the alert policy: a small, pure value fed `(session id, title, status)` and `now`.
   Keep one policy value per host connection as a local in the host actor's connection loop (`serve` in `manager.rs`),
   and feed it after each successful, non-discarded refresh: for hosts with an identity, take those fields from the list
   before it moves into `replace_host_sessions`; for hosts without one, after `retain_unknown_list_status`. A new
   connection starts a new policy value, which is the baseline after start or reconnect; removed sessions drop out
   naturally. Do not derive transitions from the durable session cache, which survives a helm restart and would turn the
   baseline into transitions. SPEC_impl.md in this PR. Nothing user-visible yet, so no changelog fragment here.
2. **The settings section, SPEC.md, docs page, changelog fragment and TODO removal**, so the switch ships together with
   the code that honours it. One PR for everything is also fine.

### Tests

- The alert policy as a pure unit: baseline on first observation and after reconnect; settle (waiting shorter than the
  settle sends nothing; longer sends once); once per spell; re-entry is a new spell; `Unknown` ignored; exited or
  removed sessions drop pending spells; a session first seen already waiting after the baseline; rate cap at the
  boundary and the "(and N more)" count.
- The sender against a stand-in server on port 0: request shape (method, path from server URL and topic, title, priority
  header, auth header only when a token is set, body text), timeout, refused connection, non-2xx (with a body naming the
  topic), redirect not followed, slow server not blocking the queue, full queue dropping; and that no log line, error
  string or API response contains the topic, token or title (capture logs with `farhelm-testtrace`).
- Send test: success, each failure reason shown, uses saved settings, unaffected by the rate cap.
- The settings route: validation refusals that never echo the token, token never returned by `GET`, clearing and
  replacing, explicit-save errors surfaced.
- A `manager.rs`-level test with a scripted transport feeding a Running → Waiting list that produces exactly one post at
  the stand-in, and a flickering one that produces none.
- The settings UI section's behavior where the UI crate has seams for it.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- through the recorder, the farhelm-helm tests for preferences, the sender and the manager (by filter while iterating,
  then the farhelm-helm package once), per `.agents/narrow-tests.md`; farhelm-ui tests for the settings section;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` (the settle and timeout tests must use an
  injected clock, not sleeps);
- the website build (`cd website && bun install --frozen-lockfile && bun run build`) for the docs page;
- Playwright on Chromium and WebKit for the settings dialog only if the dialog spec covers the section you change;
- `dprint check` on changed Markdown and `python3 releasing/check-changelog.py format`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-ntfy-waiting-alerts-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/ntfy-waiting-alerts/<nn>-<short-name>` (for example `plan/ntfy-waiting-alerts/01-alert-settings`).
- Expected shape: the outline's two PRs (or one), each leaving main correct on its own; the user-visible switch never
  ships before the code that honours it. Avoid churn: code added in one PR and deleted in a later one means the stack
  should have been shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected
  on top, and that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits; the user-facing PR is `feat`. Each `feat` PR adds its
  changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, the outline and the tests list.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check specifically that the topic and token cannot reach a log, error or API response, that sending
cannot block the refresh path, and that the policy cannot alert on a baseline or a flicker.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example client-side sending, focus reporting,
  a retry/backoff subsystem, or encrypting the token at rest (examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular:

- the feature's user-facing name;
- settle time and rate cap values;
- the settings route's shape;
- validation rules for server URL and topic;
- the stack's shape;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: alerting on approval cards or finished turns,
including more session detail in the message, sending from clients, or dropping the token or priority settings.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
