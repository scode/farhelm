# Open a remote session's localhost link through a tunnel

Written against main at 78394ca6 on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

In the Mac app, let the user open a localhost link printed in a remote session's terminal (a dev server the agent just
started, say) through a tunnel: a port on the Mac relayed to that port on the session's host, opened in the browser.
Today such a link opens localhost on the Mac, which is the wrong machine.

TODO.md entry this plan covers (Near term): "**Tunnel and open a localhost link.** When a session's terminal shows a
link to localhost (a dev server the agent just started, say), offer "tunnel and open in browser": forward a port from
the user's machine to that port on the session's host, then open the forwarded address in the browser. Today a localhost
link printed on a remote host opens localhost on the user's own machine, which is the wrong machine, and SPEC.md notes
the same gap for login redirects to a localhost callback server on the remote host. Where the offer appears (the link's
hover, a click modifier, the link permission card), which addresses count as local, how long a tunnel lives, and what it
means for a session on the local host are to be decided when the entry is planned."

### Acceptance criteria

- **Mac app only.** Browsers (the standalone helm's web UI) get none of this, and a standalone helm refuses tunnel
  requests.
- **Offer:** in a session on a remote host, the hover on a localhost link says it points at that host and that ⌘-click
  opens it through a tunnel; a plain click opens the link as today. The agent's open-link approval card gets a "Tunnel
  and open" answer, shown first, for localhost links from a remote host; "Open" keeps today's meaning. Sessions on the
  local host get no tunnel and no offer. Mockup (hover, card, and list option A):
  https://snippets.scode.org/s/farhelm-localhost-tunnel-ui/
- **Local addresses:** loopback only: `localhost` and its subdomains, `127.0.0.0/8`, `::1`, and `0.0.0.0` / `::`. Not
  LAN addresses or single-label names.
- **Relay:** bytes travel over Farhelm's own helm-to-supervisor connection, not ssh port forwarding. The supervisor
  connects only to its own loopback: for `localhost`, `0.0.0.0` and `::` it tries 127.0.0.1 and then ::1 (dev servers
  often listen only on ::1); an IP literal is used as given. Websockets work (raw TCP, no HTTP awareness).
- **Local side:** the tunnel listens on 127.0.0.1 on the Mac, on the same port number when it is free and otherwise any
  free port, and the opened URL names 127.0.0.1, never `localhost` (the Mac's own server on `[::1]` at that port could
  capture it).
- **Lifetime:** kept in the helm's memory only. A tunnel lives until its session is deleted, the app quits, or the user
  closes it; ⌘-clicking the same link again reuses it. A host disconnect makes new connections fail until the host is
  back.
- **Open tunnels:** a strip under the session header, shown while the session has tunnels, one chip per tunnel (local
  port → host port) with a close button (mockup option A).
- **Consent:** before the first tunnel opens, a prominent warning explains the security consequences below and must be
  accepted; until then nothing is tunneled. A Settings switch shows the choice (off until accepted) and can turn tunnels
  off again; after that, the next ⌘-click or card button shows the warning again. The choice is a shared helm
  preference.
- **Hardening:** the helm refuses requests with no Origin whose `Sec-Fetch-Site` is `same-site`, in both modes, because
  a tunneled page on 127.0.0.1 is same-site with the helm.
- **Remote hosts need updating:** the protocol change means remote hosts show "needs update" until updated; the
  changelog says so.
- SPEC.md: amend the "never forward ... ports" rule, the "exactly two steady-state network edges" statement, the
  localhost-callback passages, and the rationale for never opening HTML files ("an agent's page run scripts ...") so
  they agree with this feature; document the security consequences and the tunnel's unauthenticated listener; describe
  the feature where terminal links and the open-link card are described. SPEC_impl.md records the protocol bump's reason
  and the relay. Docs and changelog fragments.

## Requirement sources

**The user's decisions (2026-10-17), in their words where given:**

- Mac app only: "i dont know what it even means to set up a tunnel in a browser. we should just not support it"; browser
  on another machine: "unsupported, desktop only". Confirmed after hearing that a browser on the helm's own machine
  would have worked.
- Over Farhelm's own connection to the host's supervisor, accepting the protocol bump.
- Offer: ⌘-click (a modifier-click) with the hover saying so, plain click unchanged, plus a button on the agent's link
  card.
- Same port if free, else any free one; 127.0.0.1.
- Lifetime: until the session is deleted, the helm stops, or the user closes it in a list in the session view; reuse on
  the next click. List: option A, the strip under the session header. Hover and card as drawn.
- Local-host sessions: no tunnel. Loopback addresses only. Login callbacks out of scope ("out of scope though i dont
  even know what you mean by login").
- Harden the helm against tunneled pages: yes.
- Security: "Accept all of it and write it into the spec | also put a prominent warning about this in front of the user.
  before the first auto-tunnel occurs it must be accepted (and can be unaccepted in settings)". The consent is stored
  with the other shared settings in the helm's database, which the user accepted moves it to a new schema version with
  no downgrade path.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

The security consequences the user accepted, to state in the warning and the spec: a tunneled page is served from
127.0.0.1 on the Mac, and browsers do not separate cookies by port, so it can read cookies set by any other service on
127.0.0.1 on the Mac; browsers treat it as a local page, so it skips their private-network checks; like ssh port
forwarding, the tunnel's port accepts connections from any program on the Mac; and after the hardening, an ordinary link
from another 127.0.0.1 page to a standalone helm on the same Mac stops working.

**Binding repository constraints:** SPEC.md's protocol rules (no compatibility across protocol versions; a clear
refusal; record the bump's reason), "Upgrade compatibility and client scale" (the update path must not break; tell the
maintainer about data loss, of which in-memory tunnels have none; the new preference's no-downgrade consequence was
accepted above), the helm's security model (full-authority helm-only operations, loopback binding, origin checks), the
terminal links section and its click semantics. Root `AGENTS.md` (Tests and validation, Changelog fragments).

**Planner proposals (checked by a fresh-context planning review):**

- Relay modelled on the helm-only, credit-paced download channel (`crates/farhelm-proto/src/lib.rs`, StatFile and
  BeginDownload; `crates/farhelm-helm/src/client.rs` and `client/downloads.rs`; supervisor handlers in
  `crates/farhelm-supervisor/src/service/`): a control request that opens a channel already fixes its meaning, so no new
  frame type; the open request carries a port and, when the URL has one, a loopback IP literal; reuse the existing bulk
  queue, upload window and credit constants; arm any stall timer only while bytes are unacknowledged, or idle websockets
  get killed; no half-close. Only the helm may open a relay: the new requests stay off the supervisor's allowlist for
  session-authenticated connections.
- The helm side gates on the desktop link opener being present (`state.link_opener`) and refuses tunnel routes
  otherwise; routes sit beside the existing desktop file routes in `crates/farhelm-helm/src/lib.rs`, with a loopback
  classifier beside `crates/farhelm-helm/src/browser.rs` (whose existing "local network" classification is broader than
  this feature's).
- The terminal side follows the desktop-only ⌘-click file open: a `native` flag in the terminal code, covered by JS unit
  tests (`crates/farhelm-ui/assets/terminal.js`, `terminal-links.js`, `crates/farhelm-ui/js-tests/`). The browser e2e
  suite cannot see Mac-app-only UI, as is already true for desktop file open; accept that rather than adding a test
  capability.
- The card's "Tunnel and open" is a new answer in the helm-to-GUI contract (an Allow plus a tunnel would open the raw
  URL on the wrong machine); the helm decides whether the button appears (`crates/farhelm-ui/src/approvals.rs`, the
  helm's approval routes).
- The list needs no live push beyond what the session view already refreshes; tunnels close in the session-teardown
  path.

## Outline

1. `fix`: the same-site no-Origin rejection in `crates/farhelm-helm/src/middleware.rs`, with its spec sentence. Tests:
   middleware unit tests.
2. `feat`: the protocol relay and supervisor side; protocol version bump with its reason in SPEC_impl.md; the
   protocol-version test renamed. Tests: one supervisor-level test shaped like
   `crates/farhelm/tests/e2e/file_downloads.rs` relaying a real HTTP server and a websocket, including a ::1-only
   listener.
3. `feat`: the helm tunnel manager (in-memory, same-port-or-free, reuse, close on session delete and app exit, refusal
   in a standalone helm) and its routes; the consent preference (shared helm preference, schema bump, unset reads as not
   accepted). Tests: in-process helm tests for the classifier, manager lifecycle, reuse, port fallback, refusal without
   the desktop opener and without consent.
4. `feat`: the hover, ⌘-click, the card's "Tunnel and open", the consent warning and its Settings switch, the tunnels
   strip; spec and docs. Tests: JS unit tests for link classification and the ⌘-click decision; component tests where
   the UI crate has them. Removes the TODO entry.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the touched crates' unit and integration tests through the recorder
(including the relay test and the protocol-version test), the JS unit tests, `python -B scripts/check-test-sleeps.py`
per `docs/test-sleep-check.md`, `python3 releasing/check-changelog.py format`, and a selected browser subset on Chromium
and WebKit through the recorder (`terminal-links.spec.ts`, `terminal-link-clicks.spec.ts`, `agent-relay.spec.ts`,
`settings.spec.ts`) to show browsers are unchanged. Not the full suite. The Mac-app UI cannot be exercised from Linux:
say in the report's "things you should know" that the hover, ⌘-click, card button, warning and strip need a check in the
Mac app (the maintainer can use the devbuild flow) with a remote host.

## Fallbacks

- If a websocket or HTTP dev server misbehaves over the relay for a reason outside the relay (an HTTP Host-header check
  in the dev server, say), document it in the report rather than adding HTTP awareness.
- If the consent preference and another plan's preference land in the same release, that is fine; each takes the next
  free schema number when it runs.

Examples of departures that need the scope reassessment: ssh `-L` forwarding, an HTTP proxy under the helm's origin,
browser support, persistence of tunnels across restarts, idle timeouts, login-callback forwarding, tunnels for
local-host sessions.

Decisions to log: the relay message shapes, the stall-timer rule, how the card's new answer travels, the consent
warning's wording and placement.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-tunnel-localhost-links-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
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

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/tunnel-localhost-links/<nn>-<short-name>` (for example `plan/tunnel-localhost-links/01-same-site-origin`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
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
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
