# SSH sign-in prompts (2FA through askpass)

Written against main at d4ed41f5 on 2026-11-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file states a change to them that the maintainer settled.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **SSH askpass so 2FA works through Farhelm.** Support ssh's askpass mechanism so 2FA (a keyboard-interactive prompt
> after public-key auth succeeds) can just work through farhelm. It will still mean it's super annoying and will prompt
> people all the time on reconnects, but it needs to just work; guiding them on what to do to avoid that being
> annoying/repeated is separate. General shape: only the command that starts the ssh master connection becomes
> interactive, run with `SSH_ASKPASS` pointing at a Farhelm helper, `SSH_ASKPASS_REQUIRE=force` and
> `NumberOfPasswordPrompts=1`; the helper relays the prompt text to the helm, the GUI shows it as a text field, and the
> answer goes back. Every other ssh client keeps `BatchMode=yes`. Cancel in the UI should kill the ssh process rather
> than rely on the helper's exit code, because ssh turns a failed askpass into an empty answer, which counts as a failed
> 2FA attempt. `SSH_ASKPASS_REQUIRE` needs OpenSSH 8.4 or later.

The maintainer's words when starting the planning, verbatim:

> Please keep in mind that people are on Wi-Fi, things get disconnected back and forth. So the user will need to see the
> popup both on startup or when adding a host, but also just randomly at any time while they're working; they're going
> to suddenly need to re-authenticate. And so we need to handle all of those cases.

In short: whenever ssh, while Farhelm signs in to a host, asks a question (a verification code, a Duo menu, a password,
a key passphrase, an unknown host key's confirmation), Farhelm shows the host's question in the desktop app and sends
the user's answer back, at startup, while adding a host, while setting up or updating one, and whenever a dropped
connection signs in again; a security-key "Confirm user presence" notice is shown while ssh waits for the touch. Nothing
pops up over the user's work.

### The design, as the maintainer chose it on full-size mockups

The mockups live outside the repository, so here it is in words. Wording in quotes is the planned wording; refine it
where the code's existing vocabulary demands, and log the change.

- **Attention row.** A host whose sign-in is asking a question gets a row in the session list's attention list (the
  bordered box above the session heading, `crates/gui/farhelm-gui/src/session_list/attention.rs`), for example
  "build-box · Waiting for sign-in · [Answer]". Today a connecting host gets no row there; a host waiting for sign-in,
  and one whose sign-in needs the user (below), do. The row's button opens a popover anchored beside the row.
- **The popover** holds: a title ("Sign in to build-box"), which connection is signing in, in product words ("for
  sessions" or "for setup and updates"), the host's question exactly as ssh gave it (multi-line questions such as Duo's
  menu or a host-key confirmation with its fingerprint included; control-escaped and capped in length like other
  host-supplied text, in the monospace font), a masked answer field with a Show toggle, and Send (primary) and Cancel.
  Enter sends. A security-key notice ("Confirm user presence for key …") shows the same way with no field and no Send;
  it disappears by itself when ssh stops the helper.
- **One sign-in field component** serves both the popover and the Add host dialog's inline section (below).
- **Focus.** Mid-work, nothing opens and nothing takes focus: the row appears and waits (SPEC.md, "Who may move focus":
  things the user did not start never take focus). When the user is waiting on it, the popover opens by itself with the
  field focused: the app just opened, or the user just pressed Add, Set up, Update or a Sign in button, and in each case
  has not pressed a key or clicked inside Farhelm since (the spec's existing "moved on" test). Once they have moved on,
  it does not open by itself. Several hosts waiting at once each have their own row; after a Send while the user is
  still in the field, the next waiting host's popover may open, under the same test.
- **Attention cue.** When a new attention row appears that needs the user to act (a sign-in question or "Sign-in
  needed", a failed setup or update, identity problems; not "Unreachable, retrying", which fixes itself and comes and
  goes with every network blip), a brief "comet" plays once: a small amber light with a glow travels from the middle of
  the window to the row, and the row flashes amber when it lands, about a second in all. It takes no input and no focus
  and needs no dismissal; afterwards the row keeps a steady amber left edge until it is dealt with. Where the platform's
  reduce-motion setting is available, it degrades to a brief amber flash of the row without movement. The maintainer:
  "we must not add a bunch of hacks across components here to get this to work generally. we need one reasonable clean
  mechanism to do this kind of thing that can be used for similar things in the future." See the Opus design consult
  under "How to run".
- **Rail avatar.** While a host waits for sign-in or needs it, its rail avatar carries a small amber key badge, in the
  family of the existing amber marks (the update arrow, the failed-setup dot), and its tooltip words say so.
- **Host words.** The Hosts panel and tooltips read "Waiting for sign-in" while a question is open and "Sign-in needed"
  once Farhelm has stopped asking (amber, the advisory tone), with a Sign in button on the host's row. Session rows on
  that host read "Waiting for sign-in" or "Sign-in needed" where they read "Waiting for supervisor" today.
- **Session view.** The "Session unavailable" card a session on that host shows today says the host is waiting for
  sign-in (or that sign-in is needed) and carries a Sign in button that opens the same popover (or, after "Sign-in
  needed", asks the host again).
- **Add host.** While the Add host dialog checks a destination and the host asks a question, the question shows inside
  the dialog, below the destination field, in the shared sign-in field component, labeled as the first of two sign-ins
  ("checking and setting up"), with a line saying that Farhelm signs in to a host twice when adding it: once to check
  and set it up, once for sessions. The field takes focus under the focus rule above. Cancel stops the check and leaves
  the draft intact, as any other check failure does. The second sign-in (sessions) comes after the dialog has closed,
  through the attention row and popover, which open by themselves if the user has not moved on.
- **Sound.** A new sign-in question plays a sound, under its own switch in Settings → Notifications, on by default, so a
  question that arrives while Farhelm is not the app in front is heard. No ntfy alert, no OS notification.

### Behavior

- **Which hosts:** every SSH host, automatically. ssh consults the helper only when a host asks something, so a host
  that never asks behaves as today. Tensorlake sandboxes (Farhelm's own keys) never use the helper.
- **Which questions:** whatever ssh asks: codes, passwords, key passphrases, an unknown host key's confirmation. A
  changed host key still fails as today (ssh refuses those without asking). A jump host's questions, when the user's SSH
  config reaches a host through one, come through the same way, since the jump ssh inherits the environment.
- **Ask once per drop.** A sign-in whose question was shown and then cancelled, left unanswered for 2 minutes, or
  answered wrongly (Farhelm allows ssh one try) ends; the host then shows "Sign-in needed" and does not retry by itself
  until the user presses Sign in. A connection that later drops after a successful sign-in asks again, once. A sign-in
  that fails before any question was shown (no network, host down) stays on the ordinary retry ladder and shows no
  prompt. Opening the app starts each host fresh, so a host left at "Sign-in needed" asks once more at startup.
- **Expiry: 2 minutes**, on Farhelm's side, matching OpenSSH's default server-side limit (ssh blocked on the helper does
  not notice the server giving up). On expiry, kill the sign-in ssh and show "Sign-in needed".
- **Cancel kills** the waiting ssh (its process group), never sends an empty answer.
- **Answers are never stored** and never appear in logs, traces, host details, errors or the UI channel's diagnostics.
- **Dead connections are noticed within about a minute:** SSH keepalives on Farhelm's shared connections, so a
  connection whose network died (a Wi-Fi change, sleep) ends by itself and the next attempt signs in fresh.
- **Supported ssh:** no version check. The maintainer, verbatim: "spec: we assume ssh is equal to or newer than what is
  in Ubuntu 24.04.5 LTS or the previous generation macos. no complexity added to support older crap unless it's a super
  simpler "just give user a clean error instead of weirdly failing" kind of check".
- **"SSH needs 2FA?" goes away:** the Help menu item, its dialog, its Add host link ("Does this host ask for a second
  factor…"), and the `POST /api/ssh-connections` endpoint and its types. Connections a user started by hand keep
  working: Farhelm reuses a live shared connection and never prompts for it.

### Acceptance criteria

1. On a host that asks a question at login, the sessions connection's question reaches the app as an attention row and
   popover at startup, after a connection drop mid-work, and after Sign in; the setup connection's question reaches it
   while adding a host (inline in the dialog) and when a setup or update starts. A correct answer connects (or lets the
   setup step proceed) with no further question while that shared connection lives.
2. Focus moves only under the rule above, verified for: a mid-work question while a terminal has focus (the terminal
   keeps every key), startup before any key or click (popover opens, field focused), and after a key press (no
   auto-open).
3. Cancel, a 2-minute expiry and a wrong answer each end the sign-in by killing the ssh, and the host shows "Sign-in
   needed" with Sign in, with no automatic retry until pressed; a failure before any question stays on the ordinary
   ladder.
4. Every other ssh client keeps `BatchMode=yes`; concurrent clients of one shared connection never produce two questions
   at once for it.
5. The attention cue is one mechanism that the attention list uses for the row kinds named above, plays once per new
   row, takes no input or focus, and has a reduced-motion fallback where the platform reports the setting.
6. Keepalives end a dead shared connection within about a minute, on both of a host's shared connections.
7. No answer reaches a log, a trace, a stored file, an error message or the UI channel's diagnostics (a test proves the
   paths an answer travels through do not log it).
8. The "SSH needs 2FA?" dialog, its link and its endpoint are gone, and SPEC.md, SPEC_impl.md and the docs pages say
   what Farhelm now does (see Outline).
9. GUI goldens cover the new surfaces: the attention row in both states, the popover (code question, multi-line
   Duo-style question, host-key confirmation, security-key notice), the Add host inline section, the session card with
   Sign in, and the rail badge, in light and dark.

## Decisions already made

Settled with the maintainer on the 2026-11-19 planning page, over three rounds:

- **Mid-work presentation:** the attention row plus popover (chosen over a top-of-pane card like agent approvals, and
  over a question inside the session view). "We definitely don't want more things that are just in your face as the
  user. It can be incredibly infuriating."
- **Attention cue:** the comet, for rows that need the user to act (not "Unreachable, retrying"), built as one reusable
  mechanism, with a required Opus design consult (below).
- **Focus:** only when the user is waiting on it, by the spec's "moved on" test; the popover opens by itself then; the
  session card gets a Sign in button.
- **Add host:** the question shows inside the dialog.
- **Retry:** ask once per drop; after cancel, expiry or a wrong answer, "Sign-in needed" until Sign in.
- **Expiry:** 2 minutes, on Farhelm's side.
- **Background attention:** a sound with its own switch, on by default. No ntfy alert.
- **Field:** masked by default with a Show toggle.
- **Scope:** every SSH host; whatever ssh asks; unknown host-key confirmations relayed (the maintainer: allow it if ssh
  routes it through the same mechanism and all Farhelm has to do is not block it; if keeping today's failure turned out
  to be the simpler option, keep failing). The security-key touch notice shown (display only; it cannot work for a key
  held in ssh-agent, and docs must not promise that).
- **Rail key badge:** kept.
- **Dead connections:** SSH keepalives, about 45 to 60 seconds, on both shared connections of every SSH host.
- **Old ssh:** no version check; SPEC.md gains the supported-version statement above.
- **"SSH needs 2FA?":** removed, with its Add host link and endpoint.
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **No-workhorse mode**, stated on the planning page as an explicit choice.
- **Release note:** the maintainer's own entry under `## Release note` below.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the quoted words, and the decisions above.

**Binding repository constraints:**

- SPEC.md, "Who may move focus" (things the user did not start never take focus or open over the user's work; the "moved
  on" test); SPEC.md's Feedback description of **SSH needs 2FA?** and the "Confirmed 2026-10-05" paragraph under
  "Supported user environments" (2FA hosts best-effort, restarting connections is the user's job), both of which this
  plan replaces; SPEC.md's statement that Farhelm's own ssh connections disable agent, X11 and port forwarding and
  override `RemoteCommand`, a forced terminal and `LocalCommand` (still true of the sign-in ssh, except that the sign-in
  may run a trivial remote command of its own, see below); SPEC.md "Errors and diagnostics" (connection state always
  visible on the rail avatar; bounded retries then periodic re-probing).
- SPEC_impl.md, "Helm ↔ supervisor transport: system ssh + stdio protocol" (two shared connections per host, `%C` and
  `p%C`, their sharing-off fallback when a socket path cannot fit or contains `${`, stderr relayed as bounded tracing,
  live Connecting detail carrying only the stage vocabulary and never stderr) and "Helm internals" (the connection
  actor, its ladder and deadlines).
- Root `AGENTS.md` "Code rules": no `matches!` or `kind == X` over SSH access kinds or providers at a call site; the
  Tensorlake skip is a method on the access kind (`SshAccessKind` in `crates/farhelm-helm/src/ssh.rs`).
- Clippy runs with `-D warnings` and `farhelm-gui` has no dead-code allowance, so no PR may land code nothing uses yet.
- `.agents/validation.md` for GUI changes and goldens; root `AGENTS.md` "Conventional Commits", "Changelog fragments",
  "Tests and validation"; `.agents/test-authoring.md`; `website/AGENTS.md` before editing docs pages (an incidental docs
  edit starts no preview server; name the changed pages in the report); "Sharing the machine with other agents" for any
  real sshd a test starts (ephemeral port, the run's own random directory).

**Planner proposals** (verified against the code and checked by an independent planning reviewer; take them as the
outline, not as orders):

- **An explicit sign-in step per shared connection, in two places only:** the host's connect attempt (sessions socket,
  `%C`) and the start of a provisioning operation, the Add host probe included (setup socket, `p%C`). It runs only when
  no live shared connection answers (`ssh -O check`). Every other client is unchanged (`BatchMode=yes`,
  `ControlMaster=auto`) and rides the signed-in connection. Making the first ordinary client interactive instead would
  mean suspending the 20 s connect deadline, the 15 s probe timeout and the 30 s command timeout while a person types;
  the separate step keeps one rule: the attempt's deadline runs until a question appears, then the 2-minute expiry
  governs.
- **The sign-in ssh's shape:** do not use `ControlMaster=yes` with `-N`. With `yes`, ssh claims the socket only after
  authenticating; if something else claimed it first (a racing client, the user's own master), ssh disables sharing and
  `-N` stays in the foreground forever. The reviewer's proposal: a throwaway client with `ControlMaster=auto`,
  `ControlPersist` as today (60 s; the port-forward reasoning in `CONNECTION_OVERRIDES`' docs depends on it),
  `BatchMode=no`, `NumberOfPasswordPrompts=1`, the connection overrides, and a trivial remote command, whose exit 0
  means the shared connection is up. Environment: `SSH_ASKPASS` = the farhelm binary (the desktop app already locates
  it, `crates/farhelm-desktop/src/bundle.rs`), `SSH_ASKPASS_REQUIRE=force`, the helper's socket path and an attempt id.
  Run it in its own session (`setsid`, not just its own process group) with stdin empty, so ssh can never open a
  terminal. Verify against a real sshd that this shape behaves as described (the backgrounded master, the exit status);
  if `-N` turns out cleaner, success is "`-O check` passes" and the leftover process is killed. Log the choice.
- **Concurrency on one socket:** container hosts on an SSH place reconnect over the place's sessions socket at the same
  moment as the place. Either only the place's own connection signs in and the containers simply retry, or a small lock
  keyed by socket path makes them wait. A close call that is yours; log it. Acceptance criterion 4 must hold either way.
- **The helper** is a hidden `farhelm internal askpass` subcommand: it reads the question (argv) and
  `SSH_ASKPASS_PROMPT` (`none` for a notice, which it holds open until ssh kills it), connects to a helm-private unix
  socket named in its environment, relays the question with the attempt id, prints the answer on stdout and exits 0, or
  exits non-zero when told the sign-in ended. No existing channel fits: the helm's HTTP API issues credentials only
  inside its own process (`crates/farhelm-helm/src/auth.rs`), `farhelm agent` commands talk to the supervisor, the
  desktop socket is one-way, and ssh closes extra descriptors. Follow `second_launch.rs`'s pattern (a 0600 socket in the
  private state directory, bounded reads). The attempt id identifies the attempt; it is not a secret and protects
  nothing against same-user processes, so do not describe it as one.
- **A pending sign-in list, separate from approvals:** a new UI channel collection with the open questions (attempt id,
  host or Add host draft, which connection, the question text, whether it is a notice, its expiry) and an answer/cancel
  POST. Approvals are tied to the requesting host's connection and grants and refuse when no window holds the UI seat; a
  sign-in carries a secret answer, cancels by killing a process, expires, and has no host id during Add host. Reuse the
  generic pieces (the collection push, the POST shape) and keep the contracts apart. The question text lives only in the
  collection; the host state carries only a stage word.
- **Host states:** a Connecting stage for "waiting for sign-in" (display words only) and a new frozen host state for
  "Sign-in needed", like the identity-mismatch and duplicate states (Unreachable cannot be reused: it promises retrying
  forever). Every Sign in button uses the existing `POST /api/hosts/{id}/retry`; no new endpoint.
- **Add host's 20 s reply bound:** `POST /api/hosts/probe` answers 504 after `HOST_REPLY_BOUND` (20 s), and a code takes
  longer to type. When the setup socket has no live connection, the probe starts the sign-in and replies at once with a
  "sign-in pending" result; the dialog shows the question from the pending list and probes again once the sign-in
  succeeds. The 20 s bound, the 15 s probe timeout and the 30 s command timeout stay unchanged.
- **A setup confirmation left open over a minute asks again:** the setup connection closes 60 s after it goes idle, as
  today, so adding a host can ask up to three times (setup, setup again after a long Set up confirmation, sessions). The
  maintainer was told and accepted that; do not add machinery to avoid it.
- **Keepalives:** `ServerAliveInterval=15` and `ServerAliveCountMax=3` in `ssh_base_args`, for both shared connections
  (Tensorlake already uses 30 and 3; the maintainer chose about 45 to 60 seconds). They override the user's own
  ServerAlive settings for Farhelm's connections only; document that beside the other overrides. Without them a master
  whose network died still answers `-O check`, so no sign-in would ever start.
- **Hosts with no shared socket** (path too long, or containing `${`) skip the sign-in and fail as today; keep the
  explanation `manual_master_command_from` produces for that case (in `crates/farhelm-helm/src/ssh.rs`) for the host's
  error text rather than deleting it with the help dialog.
- **Tests and diagnostics reuse:** add the sign-in as a method on the existing `HostTransport` trait so the manager
  tests' fakes cover it; extend `crates/farhelm-helm/src/provisioning/ssh_stand_in.rs` rather than writing a second ssh
  fake; classify sign-in failures through `ssh_failure_detail`. For real OpenSSH, `scripts/gui-agent.py`'s private sshd
  (`.agents/validation.md`) shows how this repository starts a user-level sshd on an ephemeral port; a
  passphrase-protected key with `IdentityAgent=none` and an unknown host key are two questions a user-level sshd can
  produce without root. Use them where cheap, and say in the report what was verified against real OpenSSH and what only
  against the stand-in.

## Outline

A linear stack, split by behavior so each PR is used when it lands. Adjust the split if the code argues for it (log it);
do not add PRs for unrequested work.

1. `plan/ssh-askpass-2fa/01-keepalives` (`fix:`): keepalives on both shared connections, the SPEC_impl.md override list,
   a fragment (`kind: fixed`: reconnecting after a Wi-Fi change or sleep no longer takes many minutes).
2. `plan/ssh-askpass-2fa/02-sessions-sign-in` (`feat:`): the helper, its private socket, the pending sign-in list and
   its answer/cancel POST, the sign-in step on the sessions connection, expiry, cancel by kill, the "waiting for
   sign-in" stage and the "Sign-in needed" state with Sign in through retry, the host words for the new state in the GUI
   (the exhaustive matches require them), and SPEC.md/SPEC_impl.md for all of it, including the supported ssh versions
   statement and the replacement of the "Confirmed 2026-10-05" paragraph. If this PR gets too large, split the helper
   and list from the connection actor's use of them, keeping each piece used when it lands.
3. `plan/ssh-askpass-2fa/03-sign-in-ui` (`feat:`): the shared sign-in field component, the attention row in both states,
   the popover (question, notice, Show toggle), auto-open under the focus rule, the session card's Sign in, the rail key
   badge, the Hosts panel row button, the sound and its switch, goldens.
4. `plan/ssh-askpass-2fa/04-attention-cue` (`feat:`): the comet as one reusable mechanism, wired to the attention row
   kinds named above, with the reduced-motion fallback; goldens where a still frame is meaningful.
5. `plan/ssh-askpass-2fa/05-setup-sign-in` (`feat:`): the sign-in step on the setup connection (probe, setup, update,
   uninstall), the probe's "sign-in pending" reply, the Add host inline section, goldens, SPEC.md's Add host text.
6. `plan/ssh-askpass-2fa/06-remove-2fa-help` (`feat:`, with a `kind: removed` fragment, since a Help menu item goes
   away): remove "SSH needs 2FA?", its Add host link, `POST /api/ssh-connections` and their types; update the docs pages
   that mention 2FA (`website/src/content/docs/docs/get-started/add-a-remote-host.mdx`,
   `website/src/content/docs/docs/using/send-feedback.md`, and any other page a search turns up) and
   `docs/gpui/parity/settings-and-hosts.md` if it describes the dialog; remove this plan's TODO.md entry.

Each PR leaves the changelog fragment its type calls for. Fragments of PRs 2 to 5 describe the one feature from
different angles; a later PR in this stack amends the earlier PR's fragment rather than adding a second one where they
describe the same user-visible change.

## Fallbacks

- If publishing updated GUI goldens is blocked because the executing machine lacks push access to the private golden
  storage repository (`scripts/gui-goldens.py` says where), keep the updated captures verified locally and block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), rather than landing failing goldens.
- If real OpenSSH shows that an unknown host key's confirmation does not go through the helper under
  `SSH_ASKPASS_REQUIRE=force`, keep today's failure for it (the maintainer's stated fallback) and log it.
- If GPUI cannot draw the comet across the window without per-component code, the Opus consult decides the closest clean
  alternative (for example a cue drawn only within the attention list) and you log it; never ship per-component hacks to
  get the full comet.
- If the platform's reduce-motion setting is not available through the toolkit, play the comet always, log it, and list
  it in the report's follow-ups.
- If the expiry cannot kill the sign-in ssh cleanly on one platform, stop and block rather than leave a stale question
  live: a stale question the user answers is the failure the expiry exists to prevent.

Decisions to log: the sign-in ssh's exact shape and how it was verified, the socket concurrency call, the helper
protocol, the collection's shape, the sound used, the comet's mechanism (with the consult's findings), every wording
change from this file, and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-ssh-askpass-2fa-log.md` in the parent directory of the checkout you run in, derived as that section says,
and the watchdog's status file is `farhelm-plan-ssh-askpass-2fa-watchdog.json` beside it. Resolve both to absolute paths
before you start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### The live install

Root `AGENTS.md`, "The live install is off-limits", holds for this whole plan: test only against helms, supervisors and
sshds your own runs start. Never touch the maintainer's own ssh shared connections (sockets in any Farhelm state
directory you did not create), their ssh config or their known_hosts.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (the helm side and the GUI side of PR 3 onward are fairly independent, for instance). Concurrent
agents that might otherwise conflict can work in separate jj workspaces whose results you merge back; you still own the
stack and every PR. Launch a sub-agent natively, with model and effort set explicitly where the mechanism allows; when
the model you need is not available as a native sub-agent type (the reviewer below, for instance),
`$scode-harness-shellout` supplies the launch mechanics.

The review gate and the Opus design consult keep the models and efforts this file demands; the scope reassessment runs
at your own model and effort. The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) are exempt from the rules above and run
as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/ssh-askpass-2fa/<nn>-<short-name>`, as
  the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what that PR
  built, with the kind that fits it. Where what got built departs from the release note entry (a part left out, a
  limitation it does not mention), the fragment's context paragraphs say how (`plans/.agents/executing.md`, "Executing
  one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- GUI drawing changes update the screenshot goldens in the same PR, as `.agents/validation.md` describes:
  `scripts/check-gui-goldens.py --update`, look at every changed or added picture its report names before committing,
  then `python3 scripts/gui-goldens.py publish` and commit the rewritten manifest lines. A golden change you did not
  expect is a finding to explain or fix, not a picture to accept.
- The last PR of the stack removes this plan's TODO.md entry.

### Opus design consult for the attention cue

The maintainer requires it: once the sign-in UI (PR 3) is settled and before you implement the cue (PR 4), consult a
single fresh-context Opus 5.5 agent at high effort on the cue's overall design. Give it this file's attention-cue
description and the maintainer's words quoted there, the relevant code (the attention list, `layers.rs` and the layer
rules, the GUI's existing animation uses, the focus rules), and your proposed design. Its charter: propose or approve
one reasonable, clean mechanism through which any surface can ask for an attention cue on an element, which future
attention-worthy items can reuse, with no per-component special cases; say where it lives, how a caller requests a cue,
how it finds the target's position, how it stays out of input and focus, how it handles reduced motion, and how it is
tested; reject designs that spread cue logic across components. Findings go to a file in your scratch directory; it
edits nothing and touches no VCS state. Follow its design unless you find a concrete defect in it, which you log with
your alternative. This consult is not the review gate; PR 4 still passes the review gate below.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, design, behavior, acceptance
criteria, decisions and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; a second way to reach the helm, a widened approvals table, a version check, or new machinery to avoid the
  repeated Add host sign-ins would also be departures; they are examples, not a blacklist), including a new mechanism
  that does the job of an existing one or a generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, in the order the outline gives (adjusted only as logged),
together meeting the acceptance criteria above, each having passed its review gate, the attention cue having had its
Opus design consult, and the last PR removing this plan's TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.

## Release note

Farhelm now better supports remote hosts that have interactive password/2FA/security key flows so long as it goes
through ssh's usual SSH_ASKPASS mechanism. You will be prompted/alerted when needed.
