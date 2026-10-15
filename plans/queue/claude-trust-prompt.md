# Opt-in: answer Claude's "trust this folder?" for managed checkouts

Written against main at 5de6290c on 2026-10-15. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

When Claude starts in a folder it has not been told to trust, it asks "Quick safety check: Is this a project you created
or one you trust?" (default answer "No, exit"). Managed checkouts are a fresh clone for every session, so a Farhelm user
meets this question constantly. The TODO.md entry (Near term, "Investigate suppressing Claude Code's Project Trust
prompt") asked whether a supported switch exists and whether Farhelm could update the user's configuration instead.
Planning established that no switch exists (below) and the maintainer decided to build the configuration route, opt-in,
with a discoverable hint.

### Acceptance criteria

**The setting**

- An off-by-default switch in the Settings dialog's managed-checkouts area (beside the checkout folder and the
  post-clone command), labelled along the lines of "Answer Claude's "trust this folder?" for new checkouts". It is helm
  configuration shared by every client and applies on every host. "This has to be opt-in in a setting, and the user has
  to be told clearly about the caveats to the safety here and that this is very much a best effort" (the maintainer).
- Its help leads with the mechanism, which is the surprising part; the trust point that every Claude user already knows
  gets one line. The maintainer: "what is not obvious to them are the risks associated with the mechanism Farhelm uses
  to change the config. That's the big thing we need to highlight, not so much the trust risk, in that that applies to
  Claude Code in general." The agreed draft, which you may tighten without changing its meaning:
  - Plain line: for folders Farhelm clones, Farhelm marks the folder as trusted before the agent starts, so Claude
    doesn't ask. Folders you point a session at yourself still get Claude's question. (Say also that checkouts an agent
    requests are covered, and that on a remote host the edit is made to that host's own Claude settings.)
  - A highlighted caution box (amber, like the UI's other caution surfaces), headed "How this works, and why it's off by
    default": Claude has no supported way to do this, so Farhelm edits Claude's own settings file directly
    (`~/.claude.json` on the session's host), copying how Claude writes it. That file also holds your Claude sign-in,
    settings and per-folder state. Farhelm changes only the one trust entry and replaces the file in a single step, so
    it's never left half-written. But a Claude session exiting at the same instant can, rarely, undo Farhelm's edit or
    lose one of its own updates; and any Claude update can change how that file is written, without notice, after which
    the edit may stop working (you just see Claude's question again) or collide with Claude's own writes, and Farhelm
    can't detect every such change. This is best effort, and Farhelm can't promise it stays safe across Claude versions.
  - One line: as when you answer Yes yourself, the repository's own Claude hooks and commands then run without Claude's
    warning. When Farhelm deletes the checkout, it removes that folder's entry again.

**The hint band**

- While Claude's trust question is on screen in a session whose folder is a managed checkout, the setting is off, and
  the band has not been dismissed, a band appears **above the terminal** (between the session's tabs and the terminal),
  full width, in the UI's informational accent. In the maintainer's words, it "doesn't block the user. It's not a modal.
  It doesn't take keyboard focus or anything, but it should be obvious and not easy to miss. It shouldn't block the
  actual question Claude is asking." The terminal gives up the rows the band needs while it is shown.
- Agreed text (tighten without changing meaning): "**Claude asks this for every new checkout.** Farhelm can answer it
  for you, but it's off by default: there's no supported way to do it, so Farhelm would have to edit Claude's own
  settings file, which comes with caveats." The maintainer asked that the band hint "that there are caveats beyond just
  the trust issue: that there are also limitations to the mechanism Farhelm has to use", in a few words.
- Buttons: **Settings and caveats** opens the Settings dialog scrolled to the switch; **Don't show again** sets a shared
  preference that hides the band on every client, with a way to bring it back in Settings (next to the switch); **×**
  hides the band for this occurrence of the question only. The band disappears when the question leaves the screen.
- The band is announced to screen readers without moving focus.

**The mechanism**

- Applies only to managed checkouts (fresh clones Farhelm makes, including agent-requested ones), never to a folder the
  user points a session at. The maintainer chose every new managed checkout while the setting is on, whatever agent the
  session starts with, because a checkout created with another agent can later run Claude (Replace with, a second
  session) and the band would otherwise advertise a setting that cannot help.
- Runs in the launch step inside the session's terminal (`run_checkout_preparation` in
  `crates/farhelm-supervisor/src/launch.rs`), after the clone and post-clone command have completed and setup is
  recorded ready, immediately before the agent starts. That process has the login-shell environment Claude itself gets,
  so the config location resolves the same way for both.
- Steps, all best effort and never fatal to the launch:
  1. Resolve Claude's config file as Claude does: `$CLAUDE_CONFIG_DIR/.claude.json` when that variable is set, otherwise
     `~/.claude.json`; a non-empty legacy `.config.json` in the config directory takes precedence. Confirm this against
     the installed Claude before relying on it. If the file does not exist, do nothing.
  2. Take Claude's own lock: create the directory `<config file>.lock` beside the real file (the symlink target if the
     file is a symlink) with mkdir. Retry for about 12 seconds (Claude treats its lock as stale after 10 s). Never
     remove a lock Farhelm did not create. Hold it for well under 5 seconds. On giving up, print one line in the
     terminal saying Farhelm could not mark the folder trusted and Claude may ask.
  3. Re-read the file under the lock. If it is not a JSON object, change nothing. Otherwise set only
     `projects["<path>"].hasTrustDialogAccepted = true`, where the path is the clone root resolved through symlinks by
     the launch step itself (macOS spells temp and home paths through `/private`); leave every other key as it is. Key
     order need not be preserved (Claude rewrites the object itself and does not depend on it); do not enable a
     workspace-wide JSON feature such as serde_json's `preserve_order` to get it, because feature unification would
     change JSON ordering everywhere.
  4. Write a temporary file beside the real one with the same mode, fsync it, rename it over the real file, fsync the
     directory, then remove the lock.
  5. Record in the checkout's preparation record which config file and key were written, for the remover.
- **Removal:** when the supervisor archives the checkout (the last-reference Delete moves it into the archive; a
  vanished folder retired as missing counts too), remove that one key using the recorded file, under the same lock and
  write rules, off the reply path so it never delays Delete (SPEC: archiving never blocks deleting a session), logged on
  failure. Removal happens at the archive move because that is when the path becomes free: the lowest-free allocation
  can reuse the same path for a new clone, which a stale entry would silently trust even with the setting off. A
  checkout made before this feature has no record and needs nothing. If the archive move fails, the folder stays and so
  does its entry.
- **Protocol:** the setting travels with each create the way the post-clone command does: a field beside `post_clone` in
  `ResolvedGithubCheckout` and in the launch spec's `CheckoutPreparation` (`#[serde(default)]` so specs written before
  the upgrade still parse), so retries replay it like the post-clone command. This changes what a supervisor does, so it
  bumps `PROTOCOL_VERSION`; a supervisor on another version is refused at the hello until upgraded, as for every bump.
  No per-version gating code.
- **Detection for the band:** a Claude-specific refinement in `ClaudeReader` (`agent_kind/screen_reader.rs`) recognizes
  the trust question (the existing fixtures `claude/2.1.285` and `claude/2.1.296` `waiting-trust.txt` show it) and the
  supervisor reports it as an optional `SessionInfo` bool, omitted when false, filled where the session info is built
  for listings and renames, with a sessions-changed hint when it flips. The client already knows whether a session is in
  a managed checkout (`SessionInfo.working_copy`, `github_repo`); add no plumbing for that.
- **Helm storage:** the setting (a checkout-configuration column) and the "don't show again" preference (a preferences
  column, like `welcome_seen`) in one schema migration. Upgrade note for the report: an older release's helm then
  refuses this database (no data loss, no downgrade). Decide whether toggling the setting bumps the checkout
  configuration revision; it does not change any path, so prefer not invalidating an open launch preview, and log it.

**Spec, docs, tests**

- SPEC.md: replace "Each setting applies to one launch and never writes vendor trust state" / "Claude ... no supported
  interactive workspace-trust switch" and the capture section's no-agent-configuration rule with a narrowly scoped
  exception for this one key, opt-in, managed checkouts only, removed at archive, best effort; describe the band and the
  setting. SPEC_impl.md: the mechanism, the lock protocol, the record, removal timing, and the limits (the exit-save
  window, Claude version changes). Docs pages: `website/src/content/docs/docs/agents/claude.md` (which says the launcher
  cannot answer the question), `agents/index.mdx` (the trust column and tip), and the managed-checkouts page, following
  `website/AGENTS.md` and `website/EDITORIAL_RULES.md`.
- Tests: the writer and remover against fixture config files in a temp directory (sets only the key and keeps every
  other key and value, keeps the file mode, writes through a symlink, refuses a non-object file, waits on a held lock
  and gives up without removing it, leaves no temp file behind, the remover is a no-op without a record); the detection
  over the existing trust fixtures and a non-trust Claude dialog; the proto field's defaults; the helm setting,
  preference and create field; the band's visibility rules as a pure function with unit tests; one browser test on
  Chromium and WebKit for the switch and the band rendered from a seeded session flag (no real Claude). No test sets
  environment variables of its own process: inject the config directory and the environment the writer reads.
- Changelog fragment, `kind: added`, saying it is opt-in and best effort.
- The last PR removes the TODO.md entry.

## Requirement sources

**The user's request and decisions (2026-10-15):** the TODO entry. Build the mechanism; only managed checkouts; off
until turned on; remove the entry when the checkout is deleted; the discoverability band with the placement, dismissal
and wording above, agreed from two rounds of mockups (the mockups live outside the repository, so the text above is the
design); the setting's emphasis on the mechanism's risks. Review gate: one fresh-context gpt-6.1-sol reviewer at high
effort, no swarm. No-workhorse mode, which `plans/AGENTS.md` requires.

**Research during planning** (Claude Code 2.1.300, observed with `scripts/agent-tui.py` in throwaway folders, or read
from the code embedded in the binary):

- No command-line flag or `--settings` key skips the interactive question (observed with `--permission-mode auto`,
  `--dangerously-skip-permissions` and `--permission-mode bypassPermissions`); only non-interactive `-p` mode does. The
  undocumented `CLAUDE_CODE_SANDBOXED=1` skips it but treats every folder as fully trusted and likely leaks into the
  agent's commands: rejected.
- Trust is `projects["<abs path>"].hasTrustDialogAccepted = true` in the config file above, keyed by the git root
  resolved through symlinks. An entry holding only that key is valid. A trusted parent covers plain subfolders but not a
  git clone below it, so trusting the checkout root once does not work.
- Claude writes the file under a mkdir lock (`<file>.lock`, stale after 10 s, refreshed every 5 s), re-reading from disk
  and applying only its own change, then temp file, fsync, rename with the same mode; it writes through a symlink, keeps
  backups in `~/.claude/backups/`, and running instances pick up outside edits within about a second. Its save on exit
  re-reads but takes no lock, which leaves a window of milliseconds in which one writer's update can be lost.
- Confirm these against the Claude installed when you run, in throwaway folders per `.agents/real-agent-tui.md`, before
  building on them; a changed format is a reason to adapt or block, not to guess.

**Binding repository constraints:** SPEC.md's launch-trust and no-agent-configuration rules (changed here as agreed),
managed checkouts (archive, lowest-free path reuse, archiving never blocks delete), "Upgrade compatibility and client
scale"; `PROTOCOL_VERSION`'s rules; root `AGENTS.md` "Harness-specific code" (Claude-specific behavior lives where
`agent_kind/mod.rs` maps it); the user's rule that tests never modify their own process environment.

**Planner proposals** (reviewed by a fresh-context planning review): running the write in the launch step, the record
for the remover, removal at archive time, the 12-second lock wait, the field beside `post_clone`, the detection bool.

## Outline

1. **Writer and remover** as pure, injected-environment functions with their tests (supervisor crate, placed per the
   agent_kind map as Claude-specific).
2. **Protocol and launch:** the create field, `CheckoutPreparation`, the call in the launch step, the record, the
   protocol bump; removal at archive and retire-missing.
3. **Detection:** the trust refinement in `ClaudeReader` and the `SessionInfo` bool with its hint.
4. **Helm:** the setting and the preference in one migration, the create field filled from the setting.
5. **GUI, spec and docs:** the switch and help, the band, SPEC, SPEC_impl, docs pages, fragment, TODO removal.

PRs roughly follow these steps; merge adjacent ones if they come out small. Avoid churn.

### Validation

Per root `AGENTS.md` "Finishing work", choose the smallest set that covers the change: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the affected supervisor,
proto, helm and UI tests through the recorder (including migration and HTTP contract tests), the managed-checkout
supervisor and e2e tests that cover creation, retries and archive, the selected browser specs on Chromium and WebKit,
the test-sleep checker, `python3 releasing/check-changelog.py format`, `dprint check`, and the docs website build. One
manual check with the real Claude in a throwaway clone (setting on: no question; entry present; archive: entry gone) is
worth doing; record it, and never point it at the maintainer's real checkouts.

## Fallbacks

- If the installed Claude stores or locks trust differently from the research above, adapt the writer to what you
  observe and say so in the report. If no safe write is possible (for example Claude stops using a lock you can share),
  build the band and the setting's text as far as they stay true, leave the switch out, and block with the findings.
- Examples of departures that need the scope reassessment: trusting folders other than managed checkouts, writing any
  key other than `hasTrustDialogAccepted`, using `CLAUDE_CODE_SANDBOXED` or a private `CLAUDE_CONFIG_DIR`, durable state
  for the trust step beyond the removal record, per-version protocol gating, a workspace-wide JSON feature change, or
  auto-answering the prompt by sending keys.

Decisions to log: which checkouts get trust, the config resolution rules verified, the exact wording shipped, whether
the setting bumps the checkout configuration revision, and the PR split.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-claude-trust-prompt-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/claude-trust-prompt/<nn>-<short-name>` (for example `plan/claude-trust-prompt/01-trust-writer`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `feat`. Add a changelog fragment (`kind: added`) per root
  `AGENTS.md`.
- The last PR removes the TODO.md entry this plan covers.
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
- the acceptance criteria: this file's goal, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists,
its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.
