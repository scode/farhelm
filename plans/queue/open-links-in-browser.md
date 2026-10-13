# Let agents open links in the user's browser

Written against main at 5cd50442 on 2026-10-12. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Let agents open URLs in the user's browser": an agent in a session that tries to open a
link fails, because the session's host has no browser ("Failed to open browser for https://…: No valid browsers
detected. You can specify one in BROWSER environment variable"). Proxy `BROWSER` so that such a request is forwarded to
the user's Farhelm client and opened in the browser on the machine the user is sitting at, with safety precautions.

Acceptance criteria:

- **`BROWSER` in remote sessions.** For sessions on remote hosts only, Farhelm sets `BROWSER` for the agent process and
  for the session's terminal tabs. Sessions on the helm's own machine ("this machine") get nothing new and keep opening
  links as they do today. The value is the session's own `farhelm` executable by absolute path followed by the verb,
  `--` and `%s` (for example `/abs/farhelm agent open-url -- %s`), because:
  - the common readers (the Rust `webbrowser` crate, Python's `webbrowser`, `xdg-open`) all split the value on `:`, and
    the Rust crate splits on whitespace with no quoting, so when that path contains whitespace, `:` or `%`, Farhelm sets
    no `BROWSER` at all and logs why;
  - terminal tabs get no `PATH` change, so a bare `farhelm` could resolve to another install;
  - `--` stops a URL that starts with `-` from being read as a flag. A `BROWSER` the session would otherwise have wins:
    for the agent, decided where the agent's environment is assembled from what it inherits; for tabs, tmux's `-e`
    followed by the user's shell startup files already behaves that way. Git and the post-clone command do not get it.
- **A new agent verb** (for example `farhelm agent open-url <url>`), relayed like the other `farhelm agent` verbs over
  the session's credential and the supervisor's connection to the helm. It is not a mutating verb (it takes no
  per-session change lock, so a waiting link card does not block the session's other requests, and the reverse) but it
  may wait for the user.
- **URL rule, enforced at the helm.** Only absolute `http` and `https` URLs with a host are accepted, the same rule as
  terminal links (`isPlainWebUrl` in `crates/farhelm-ui/assets/terminal-links.js`), ported step by step (prefix check,
  raw whitespace and control check, parse without a base, scheme and non-empty host; keep the original string) rather
  than by parsing alone, with a length cap of a few KiB. The CLI may check too, for friendlier errors, but the helm's
  check is the authority. The UI checks again with `isPlainWebUrl` immediately before opening, because the desktop
  opener navigates the app's own page.
- **An approval card**, like the one for an agent starting a session: it names the session and host that asked and shows
  the full URL as labelled data, escaped as the other cards display agent text, with allow, always allow from this host,
  and deny, and the cards' existing short delay before the buttons take a click. With no Farhelm window connected, the
  request is refused at once, as other agent requests are.
  - In the web UI, Allow opens the link synchronously inside the click handler (so the browser does not block it as a
    pop-up), after the card's double-answer guard and only on an armed button, and before the answer reaches the helm.
    It is accepted that if the helm then refuses the answer (expired, answered elsewhere, or a host whose identity
    changed), the link is already open.
  - On a helm embedded in the desktop app, Allow is answered to the helm and the helm opens the link in the default
    browser through a native link opener the desktop shell registers (below); the desktop page opens nothing itself.
- **Two per-host settings, both off by default**, in the Permissions section of host settings beside "Start YOLO
  sessions without asking" and "Run farhelm commands without asking", each with a short explanation:
  - "Open links without asking" covers public addresses;
  - "Open local-network links without asking" covers links to the user's own machine or private network: loopback
    addresses and `localhost` names, private and link-local IPv4 and IPv6 ranges, `.local` names, and similar (decide
    the exact list and log it as a DECISION). A link to such an address always shows the full card unless this setting
    is on, whatever the first setting says, and that card says why it is asking.
  - "Always allow from this host" on a card turns on only the setting that card's link falls under. Neither setting
    covers commands, and the commands setting covers neither. Like the existing two, both are reset to asking when the
    host adopts a new identity, and "always allow" writes only if the host still holds the identity that asked.
- **With the matching setting on**: a helm embedded in the desktop app opens the link in the default browser right away,
  with no card. A standalone helm (web clients only) shows a small card with the URL, an "open" button and a dismiss
  control, where one click opens it in that browser (synchronously, as above); dismissing tells the agent the link was
  not opened. The card carries whether it is this pre-approved variant, so the UI needs no second request.
- **The command's result**: it waits for the answer (up to the existing approval wait). It exits successfully once the
  user allowed or opened the link (web) or the native open returned (desktop), and exits with an error and a short
  message on denial, dismissal, refusal or timeout. Callers that wait for it (Python's `webbrowser`, `xdg-open`) see
  that result. Callers that run `BROWSER` in the background, as the Rust `webbrowser` crate does (Codex uses it, and
  Node's `open` defaults to it), report success at once and cannot fall back to printing the URL; do not chase that.
  Check what Codex and Claude Code actually do, and say so in the docs and the report.
- **Docs**:
  - SPEC.md: the agent-spawned sessions section (the injected environment, deciding whether `BROWSER` joins the
    guaranteed list; the new verb; the acting-verb paragraph that names only the commands setting; the card paragraph);
    Topology (the host settings list and its reset-on-adoption sentence); and Security, which names the commands setting
    and lists bounded effects across the host boundary beside OSC 52 clipboard writes and file downloads. State there
    that with "Open local-network links without asking" on, a remote host can make the user's browser load addresses on
    the user's own machine or network without a click.
  - SPEC_impl.md, and the user-facing pages that cover agent commands, approvals and host settings. Say plainly that a
    login whose redirect goes to `localhost` on the remote host still cannot complete from a local browser, and that a
    running session gets `BROWSER` at its next launch while new terminal tabs get it at once.
- Tests as listed under Outline.
- A linear stack of draft PRs, each passing its review gate, with a changelog fragment, the last one removing the
  TODO.md entry. No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-12):** plan this TODO entry.

**The user's decisions (2026-10-12):**

- In the maintainer's words: "a card with permission like when agents try to spawn sessions with an 'always allow from
  this host' option. surfaced in per-host settings so you can turn off later."
- Its own per-host setting, not the existing "Run farhelm commands without asking" (offered and not chosen: links as one
  more farhelm command under that setting).
- With the setting on, the desktop app opens links directly and web-only setups get a one-click "open" card (offered and
  not chosen: without asking only through the desktop app; always a card).
- `BROWSER` is set for both the agent process and the session's terminal tabs (offered and not chosen: the agent only),
  and only on remote hosts (offered and not chosen: every host, including this machine, which would turn links that open
  directly today into cards).
- Links to the user's own machine or private network get a setting of their own, in the maintainer's words: "separate
  setting. so you can allow always if you really want, but separate setting from any generic url" (offered and not
  chosen: always asking for them, treating them like any link, refusing them).
- Stated by the planner without objection: `http` and `https` only; a `BROWSER` the session would otherwise have wins;
  the command waits for the answer and fails on denial; the card shows the full URL with the usual delay; `localhost`
  login redirects stay out of scope.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md agent-spawned sessions ("Every acting verb of `farhelm spawn` and `farhelm agent` asks the user first…"; the
  card rules; the no-GUI refusal; the nine-minute wait), Terminal experience (the link rules), Topology (host settings
  reset on adoption), Client hardening, and Security, "Local authority and trust between hosts": agent-controlled output
  is untrusted, and no host may cause an effect on another machine that does not go through the user's approval or that
  host's setting. This plan adds one such bounded effect; record it there.
- `crates/farhelm-proto/src/lib.rs` `PROTOCOL_VERSION` and its doc comment: the new `AgentVerb` variant is a tagged
  variant and needs a bump. Approval and host-view changes travel helm to UI under the build stamp and need none.
  SPEC.md "Upgrade compatibility and client scale" for the store migration of the new host settings.
- `BROWSER` is the same for every harness, so it does not go into the per-harness `spec.env` or `agent_kind`.
- The user's standing rule: tests never modify the environment of the process running them. Tests reach any inherited
  `BROWSER` through a seam (`SupervisorSeams::launch_env`, or a parameter where the agent's environment is assembled),
  never through `std::env`. Once this ships, agents running the suite inside Farhelm sessions will have `BROWSER` set.
- Root `AGENTS.md`: Conventional Commits; Finishing work; Releases and the changelog; Agent scratch space; Sharing the
  machine with other agents; Docs website (read `website/AGENTS.md` and `website/EDITORIAL_RULES.md` before touching a
  page).
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanism, stack and test shapes in the outline below, checked against main and corrected
by a fresh-context planning review.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- Session environment: `crates/farhelm-supervisor/src/launch.rs` `launch_child_command` (shared with git and the
  post-clone command) and `agent_command` (adds the credential and the per-harness environment); `service/core.rs`
  `tab_environment` with `launch.rs` `tab_window_command` (tabs: the same credential, passed with tmux `-e`, no `PATH`
  change). Nothing sets `BROWSER` today.
- The agent channel: `crates/farhelm/src/main.rs` `AgentCmd`, the wire enum `farhelm_proto::AgentVerb` (with exhaustive
  `is_mutating` and `may_wait_for_user`), `crates/farhelm/src/agent_client.rs` `relay_request`,
  `crates/farhelm-supervisor/src/service/agent_relay.rs`, and the helm's `crates/farhelm-helm/src/agent_requests.rs`,
  which decides by the connection a request arrived on and takes `agent_request_locks` for mutating verbs.
- Approvals: `crates/farhelm-helm/src/approvals.rs` `ask` (the host's `commands_without_asking` short-circuits; no GUI
  refuses; otherwise waits up to `APPROVAL_WAIT`), `farhelm_proto::approvals::{ApprovalAction, ApprovalAnswer}`,
  AlwaysAllow through `allow_commands_for_identity` (409 on an identity mismatch), `GET/POST /api/approvals`, and the
  UI's `crates/farhelm-ui/src/approvals.rs` `ApprovalCards` with `assets/approval-layout.js`. Each host connection has
  four answer slots (`AGENT_ANSWER_SLOTS`), so cards that nobody answers can starve a host's other requests; every card
  needs a way to end.
- Host settings: the "Run farhelm commands without asking" row in `crates/farhelm-ui/src/hosts/settings_dialog.rs`, its
  helm route `/api/hosts/{id}/commands-without-asking`, its store column, and `store.rs` `adopt_identity`, which resets
  the asking settings; mirror all of these for the two new settings, with tests like
  `adoption_resets_the_host_to_asking_before_farhelm_commands`.
- Opening links: the desktop page's `open_external_page` in `crates/farhelm-ui/src/app_bar.rs` (navigation of the
  `dioxus:` page that dioxus-desktop hands to `webbrowser::open`); the web UI's
  `window.open(url, '_blank', 'noopener')`, which returns `null` and cannot tell whether a pop-up was blocked.
- The helm's `crates/farhelm-helm/src/clipboard.rs` `ClipboardSink` is passed only through `run_embedded` by the desktop
  shell, and an embedded helm serves no web UI and ends when the window closes. So "the desktop app is connected" is
  simply "this helm has a link opener", decided when the helm is built; no client tracking is needed.

Here's the work, as a proposal; shape the stack as you see fit, each PR leaving main correct and `BROWSER` unset until
the card, the settings and the opener exist:

1. **Verb and URL rule**: the new `AgentVerb` (not mutating, may wait) and CLI subcommand, the helm-side URL rule with
   its length cap and the local-address classification, and the command's exit behavior. Protocol bump.
2. **Approval and settings**: a new `ApprovalAction` for opening a link (carrying the URL, whether it is a local
   address, and whether it is the pre-approved "open" variant); the two host settings with store columns, migration,
   reset on adoption, helm routes and identity-checked AlwaysAllow; `ask` consults the setting the link falls under.
3. **Opening**: a `LinkOpener` callback beside `ClipboardSink`, given only by `run_embedded` and opening through
   `webbrowser::open` (already in the dependency tree through dioxus-desktop), with no HTTP route, so it can only open a
   link the helm admitted. On an embedded helm it serves both Allow and the setting-on case. On a standalone helm the
   web page opens in the click, as above.
4. **`BROWSER` injection** for remote-host sessions' agents and tabs, as specified.
5. **UI**: the link card (with the local-address explanation when it applies), the pre-approved "open" card with its
   dismiss control, and the two host settings rows.
6. **Docs** as in the acceptance criteria.
7. **Tests**:
   - unit tests for the URL rule (accepts `http`/`https` with a host; refuses other schemes, relative, malformed,
     control-character and over-long URLs, with parity against `isPlainWebUrl`'s cases) and for the local-address
     classification;
   - unit tests for `ask` with the new action: each new setting skips only its own kind of link, the commands setting
     skips neither, neither new setting skips commands; AlwaysAllow sets only the matching setting and refuses on an
     identity mismatch; adoption resets both;
   - Rust e2e in the style of `crates/farhelm/tests/e2e/agent_relay.rs`: the command waits for the answer, succeeds on
     allow, fails on deny, dismissal and no GUI, and a waiting link card does not block the session's other requests;
   - supervisor tests that a remote session's agent and a new tab get `BROWSER`, that a local session gets none, that an
     inherited value is kept, and that a path with whitespace or `:` gets none, all through seams;
   - Playwright, in the style of `e2e/tests/approval-layout.spec.ts` and `spawn.spec.ts` with the helpers in
     `e2e/tests/helpers/approvals.ts`, on Chromium and WebKit: the link card shows the full URL as data; Allow calls the
     opener inside the click (a stubbed `window.open` that records `navigator.userActivation.isActive` at call time,
     asserted true; never a real external site); deny opens nothing; a local-address link shows the full card even with
     the public setting on; the host settings toggles; with a setting on, the "open" card opens on click and its dismiss
     opens nothing;
   - the embedded helm's immediate open, with a stand-in `LinkOpener`; nothing opens a real browser in a test.
8. **Bookkeeping**: changelog fragment, `kind: added`. Remove the TODO.md entry.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop` and
  `cargo check -p farhelm-desktop`;
- through the recorder: the touched crates' unit tests and the `farhelm` e2e modules for the agent relay and session
  environment, then whatever the diff shows the change can reach;
- the changed Playwright specs on Chromium and WebKit through the recorder;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and the website build for changed
  pages.

Do not run the full battery without a specific risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-open-links-in-browser-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/open-links-in-browser/<nn>-<short-name>` (for example `plan/open-links-in-browser/01-open-url-verb`).
- Bite-sized PRs, each leaving main correct on its own; the verb must not be reachable from `BROWSER` before the card
  and the setting exist. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`feat` for the
  user-visible PRs). Such a PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check:

- that no URL can open on the user's machine without an Allow click or the matching host setting being on, and that a
  local-address link never rides on the public setting;
- that only `http`/`https` URLs with a host pass, and that agent-controlled text is displayed as data;
- that the two new settings and the commands setting are independent in every direction, reset on adoption, and that
  AlwaysAllow is identity-checked;
- that the link opener can only open a link the helm admitted, exists only on an embedded helm, and that the UI
  re-checks the URL before any open;
- that `BROWSER` is set only for remote sessions' agents and tabs, never overrides a value the session would otherwise
  have, is omitted for an unsafe path, and that tests read the environment only through seams;
- that the protocol bump and the store migration follow the repository's rules.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a per-client channel in the event
  feed, a new notification system, accepting schemes beyond http and https, or forwarding localhost login callbacks
  (examples, not a blacklist);
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

- the verb's name and the exact `BROWSER` value;
- the local-address list;
- what Codex and Claude Code do with `BROWSER`;
- how a web click opens the link without being blocked;
- how the desktop sink is registered and invoked;
- what the agent is told in each outcome;
- the settings' labels and explanations;
- the stack's shape;
- the SPEC.md, SPEC_impl.md and website wording;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: reusing the commands setting, merging the two
link settings, making a setting cover other actions, opening links without a click or the matching setting, accepting
other schemes, setting `BROWSER` only for the agent, or setting it on this machine.

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
